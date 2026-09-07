//! `#[async_boxed]` 的解析与展开逻辑。
//!
//! 属性宏入口定义在 crate 根(`lib.rs`):proc-macro crate 只允许在根级导出宏。

use proc_macro2::Span;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    FnArg, GenericParam, Ident, ItemFn, Lifetime, LifetimeParam, PatType, ReturnType, Token, Type,
    parse_quote,
};

/// `#[async_boxed(...)]` 的属性参数,目前仅支持可选的 `send` 标记。
#[derive(Default)]
pub(crate) struct AsyncBoxedArgs {
    pub(crate) send: bool,
}

impl Parse for AsyncBoxedArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut send = false;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "send" if !send => send = true,
                "send" => return Err(syn::Error::new(key.span(), "`send` 参数只能指定一次")),
                _ => return Err(syn::Error::new(key.span(), "仅支持 `send` 参数")),
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self { send })
    }
}

/// 异步函数转 boxed future 的核心展开逻辑,`register_async` 内部复用。
///
/// 转换后的函数保留原名与可见性,仅改写签名;泛型函数、方法、带显式
/// 生命周期的引用参数、嵌套引用及 `impl Trait`/引用/trait object 返回值
/// 均不支持并返回编译错误。
pub(crate) fn expand_async_boxed(
    args: AsyncBoxedArgs,
    mut function: ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    if function.sig.asyncness.is_none() {
        return Err(syn::Error::new(
            function.sig.fn_token.span(),
            "仅支持异步函数",
        ));
    }

    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new(
            function.sig.generics.span(),
            "不支持泛型函数",
        ));
    }

    if function.sig.receiver().is_some() {
        return Err(syn::Error::new(
            function.sig.ident.span(),
            "仅支持自由函数,不支持方法",
        ));
    }

    for input in &mut function.sig.inputs {
        let FnArg::Typed(PatType { ty, .. }) = input else {
            return Err(syn::Error::new(input.span(), "仅支持自由函数,不支持方法"));
        };

        // 值类型参数不携带生命周期,原样保留,由 async move 按值捕获。
        let Type::Reference(reference) = ty.as_mut() else {
            continue;
        };

        if let Type::Reference(_) = reference.elem.as_ref() {
            return Err(syn::Error::new(
                reference.span(),
                "不支持嵌套引用参数(如 `&&T`)",
            ));
        }

        match &reference.lifetime {
            // 省略生命周期的引用参数注入 `'a`。
            None => {
                reference.lifetime = Some(Lifetime::new("'a", Span::call_site()));
            }
            // 已经是 `'a` 的引用参数直接放行。
            Some(lifetime) if lifetime.ident == "a" => {}
            Some(lifetime) => {
                return Err(syn::Error::new(
                    lifetime.span(),
                    "要求引用参数省略生命周期(如 `&T` / `&mut T`)",
                ));
            }
        }
    }

    // 返回值必须是裸类型(拒绝 `impl Trait`、引用与 trait object 输出)。
    let output = match &function.sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::ImplTrait(_) => {
                return Err(syn::Error::new(
                    ty.span(),
                    "不支持 `-> impl Trait` 返回类型",
                ));
            }
            Type::Reference(_) => {
                return Err(syn::Error::new(ty.span(), "不支持返回引用(如 `-> &T`)"));
            }
            Type::TraitObject(_) => {
                return Err(syn::Error::new(ty.span(), "不支持返回 trait object"));
            }
            _ => quote!(#ty),
        },
    };

    // 注入生命周期参数、移除 async、改写返回类型与函数体。
    function.sig.asyncness = None;
    function
        .sig
        .generics
        .params
        .push(GenericParam::Lifetime(LifetimeParam::new(Lifetime::new(
            "'a",
            Span::call_site(),
        ))));

    let send = args.send.then(|| quote!(+ ::std::marker::Send));
    function.sig.output = parse_quote! {
        -> ::std::pin::Pin<
            ::std::boxed::Box<
                dyn ::std::future::Future<Output = #output> #send + 'a,
            >,
        >
    };

    let block = function.block;
    function.block = Box::new(syn::parse_quote!({
        ::std::boxed::Box::pin(async move #block)
    }));

    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn boxes_function_with_reference_parameters() {
        let function: ItemFn = syn::parse2(quote! {
            pub async fn init(
                config: &AppConfig,
                register: &mut TypeMap,
                _router: &mut AppRouter,
            ) -> Result<()> {
                info!("初始化");
                Ok(())
            }
        })
        .unwrap();

        let expanded = expand_async_boxed(AsyncBoxedArgs::default(), function)
            .unwrap()
            .to_string();

        assert!(expanded.contains("pub fn init < 'a >"));
        assert!(expanded.contains("config : & 'a AppConfig"));
        assert!(expanded.contains("register : & 'a mut TypeMap"));
        assert!(expanded.contains("_router : & 'a mut AppRouter"));
        assert!(expanded.contains("dyn :: std :: future :: Future < Output = Result < () > >"));
        assert!(expanded.contains("Box :: pin (async move"));
    }

    #[test]
    fn supports_send_flag() {
        let function: ItemFn = syn::parse2(quote! {
            async fn foo(ctx: &Context) -> Result<u32> { Ok(1) }
        })
        .unwrap();

        let expanded = expand_async_boxed(AsyncBoxedArgs { send: true }, function)
            .unwrap()
            .to_string();

        assert!(expanded.contains("+ :: std :: marker :: Send"));
    }

    #[test]
    fn keeps_value_parameters_untouched() {
        let function: ItemFn = syn::parse2(quote! {
            async fn compute(ctx: &Context, count: u32) -> Result<u32> { Ok(count) }
        })
        .unwrap();

        let expanded = expand_async_boxed(AsyncBoxedArgs::default(), function)
            .unwrap()
            .to_string();

        assert!(expanded.contains("ctx : & 'a Context"));
        assert!(expanded.contains("count : u32"));
    }

    #[test]
    fn uses_unit_output_for_functions_without_return_type() {
        let function: ItemFn = syn::parse2(quote! {
            async fn init() {}
        })
        .unwrap();

        let expanded = expand_async_boxed(AsyncBoxedArgs::default(), function)
            .unwrap()
            .to_string();

        assert!(expanded.contains("Output = ()"));
    }

    #[test]
    fn parses_send_argument() {
        assert!(syn::parse2::<AsyncBoxedArgs>(quote!(send)).unwrap().send);
        assert!(!syn::parse2::<AsyncBoxedArgs>(quote!()).unwrap().send);
        assert!(syn::parse2::<AsyncBoxedArgs>(quote!(send, send)).is_err());
        assert!(syn::parse2::<AsyncBoxedArgs>(quote!(sync)).is_err());
    }

    #[test]
    fn rejects_invalid_functions() {
        let sync_function: ItemFn = syn::parse2(quote! {
            fn load(ctx: &Context) -> Result<()> { Ok(()) }
        })
        .unwrap();
        let generic_function: ItemFn = syn::parse2(quote! {
            async fn load<T>(ctx: &Context) -> Result<()> { Ok(()) }
        })
        .unwrap();
        let method: ItemFn = syn::parse2(quote! {
            async fn load(&self) -> Result<()> { Ok(()) }
        })
        .unwrap();
        let explicit_lifetime: ItemFn = syn::parse2(quote! {
            async fn load(ctx: &'b Context) -> Result<()> { Ok(()) }
        })
        .unwrap();
        let nested_reference: ItemFn = syn::parse2(quote! {
            async fn load(ctx: &&Context) -> Result<()> { Ok(()) }
        })
        .unwrap();
        let impl_trait_return: ItemFn = syn::parse2(quote! {
            async fn load(ctx: &Context) -> impl Trait {}
        })
        .unwrap();

        assert!(expand_async_boxed(AsyncBoxedArgs::default(), sync_function).is_err());
        assert!(expand_async_boxed(AsyncBoxedArgs::default(), generic_function).is_err());
        assert!(expand_async_boxed(AsyncBoxedArgs::default(), method).is_err());
        assert!(expand_async_boxed(AsyncBoxedArgs::default(), explicit_lifetime).is_err());
        assert!(expand_async_boxed(AsyncBoxedArgs::default(), nested_reference).is_err());
        assert!(expand_async_boxed(AsyncBoxedArgs::default(), impl_trait_return).is_err());
    }
}
