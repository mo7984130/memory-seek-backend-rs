//! `common` 基础设施使用的过程宏。

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, ItemFn, LitStr, Path, ReturnType, Token, Type, parse_macro_input};

/// 将单参数异步函数注册到 `linkme` 分布式切片。
///
/// 函数会保留原样，宏额外生成一个唯一的静态注册项，并将异步函数适配为
/// 调用方提供的函数指针类型。该类型通常是返回 boxed future 的高阶函数指针。
/// 调用此宏的 crate 必须直接依赖 `linkme`。
///
/// ```ignore
/// #[common::register_async(
///     slice = crate::db_init::AFTER_SCHEMA_TASKS,
///     ty = crate::db_init::DbInitFn,
/// )]
/// async fn create_indexes(db: &DatabaseConnection) -> ContextualResult<()> {
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn register_async(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RegisterAsyncArgs);
    let function = parse_macro_input!(item as ItemFn);

    match expand_register_async(args, function) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

struct RegisterAsyncArgs {
    slice: Path,
    callback_type: Type,
}

impl Parse for RegisterAsyncArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut slice = None;
        let mut callback_type = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "slice" if slice.is_none() => slice = Some(input.parse()?),
                "ty" if callback_type.is_none() => callback_type = Some(input.parse()?),
                "slice" | "ty" => {
                    return Err(syn::Error::new(key.span(), "参数只能指定一次"));
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "仅支持 `slice = ...` 和 `ty = ...` 参数",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self {
            slice: slice.ok_or_else(|| input.error("缺少 `slice = ...` 参数"))?,
            callback_type: callback_type.ok_or_else(|| input.error("缺少 `ty = ...` 参数"))?,
        })
    }
}

fn expand_register_async(
    args: RegisterAsyncArgs,
    function: ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    if function.sig.asyncness.is_none() {
        return Err(syn::Error::new(
            function.sig.fn_token.span(),
            "`#[common::register_async]` 仅支持异步函数",
        ));
    }

    if function.sig.inputs.len() != 1 {
        return Err(syn::Error::new(
            function.sig.inputs.span(),
            "`#[common::register_async]` 要求函数恰好接收一个上下文参数",
        ));
    }

    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new(
            function.sig.generics.span(),
            "`#[common::register_async]` 不支持泛型函数",
        ));
    }

    let RegisterAsyncArgs {
        slice,
        callback_type,
    } = args;
    let function_name = &function.sig.ident;
    let registration_name = format_ident!(
        "__REGISTER_ASYNC_{}",
        function_name.to_string().to_uppercase()
    );

    Ok(quote! {
        #function

        #[::linkme::distributed_slice(#slice)]
        static #registration_name: #callback_type = |__register_async_context| {
            ::std::boxed::Box::pin(#function_name(__register_async_context))
        };
    })
}

/// 为返回 `Result` 的异步函数记录调用次数、耗时和成功次数。
///
/// 默认使用函数名作为指标操作名，也可以通过 `name = "..."` 显式指定。
#[proc_macro_attribute]
pub fn metered(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MeteredArgs);
    let function = parse_macro_input!(item as ItemFn);

    match expand_metered(args, function) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[derive(Default)]
struct MeteredArgs {
    name: Option<LitStr>,
}

impl Parse for MeteredArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self::default());
        }

        let key: Ident = input.parse()?;
        if key != "name" {
            return Err(syn::Error::new(key.span(), "仅支持 `name = \"...\"` 参数"));
        }
        input.parse::<Token![=]>()?;
        let name: LitStr = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("`name` 参数后存在多余内容"));
        }

        Ok(Self { name: Some(name) })
    }
}

/// 为异步 Result 函数注入指标计数逻辑.
fn expand_metered(
    args: MeteredArgs,
    mut function: ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    if function.sig.asyncness.is_none() {
        return Err(syn::Error::new(
            function.sig.fn_token.span(),
            "`#[common::metered]` 仅支持异步函数",
        ));
    }

    if !returns_result(&function.sig.output) {
        return Err(syn::Error::new(
            function.sig.output.span(),
            "`#[common::metered]` 要求函数返回 `Result`",
        ));
    }

    let metric_name = args
        .name
        .unwrap_or_else(|| LitStr::new(&function.sig.ident.to_string(), function.sig.ident.span()));
    let original_block = function.block;

    function.block = Box::new(syn::parse_quote!({
        ::common::metrics_group!(#metric_name);

        let __metered_result = (async #original_block).await;
        if __metered_result.is_ok() {
            ::common::metrics_success!(#metric_name);
        }

        __metered_result
    }));

    Ok(quote!(#function))
}

/// 判断函数返回类型的末段是否为 Result.
fn returns_result(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    let Type::Path(type_path) = ty.as_ref() else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn expands_async_result_function_with_default_name() {
        let function: ItemFn = syn::parse2(quote! {
            async fn load() -> Result<u32, Error> { Ok(1) }
        })
        .unwrap();

        let expanded = expand_metered(MeteredArgs::default(), function)
            .unwrap()
            .to_string();

        assert!(expanded.contains("metrics_group ! (\"load\")"));
        assert!(expanded.contains("__metered_result . is_ok"));
        assert!(expanded.contains("metrics_success ! (\"load\")"));
    }

    #[test]
    fn uses_explicit_metric_name() {
        let function: ItemFn = syn::parse2(quote! {
            async fn execute() -> anyhow::Result<()> { Ok(()) }
        })
        .unwrap();

        let expanded = expand_metered(
            MeteredArgs {
                name: Some(LitStr::new("scheduled", proc_macro2::Span::call_site())),
            },
            function,
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("metrics_group ! (\"scheduled\")"));
    }

    #[test]
    fn rejects_sync_or_non_result_functions() {
        let sync_function: ItemFn = syn::parse2(quote! {
            fn load() -> Result<(), Error> { Ok(()) }
        })
        .unwrap();
        let non_result_function: ItemFn = syn::parse2(quote! {
            async fn load() -> u32 { 1 }
        })
        .unwrap();

        assert!(expand_metered(MeteredArgs::default(), sync_function).is_err());
        assert!(expand_metered(MeteredArgs::default(), non_result_function).is_err());
    }

    #[test]
    fn registers_async_function_with_generated_static_item() {
        let function: ItemFn = syn::parse2(quote! {
            async fn initialize(db: &DatabaseConnection) -> ContextualResult<()> {
                Ok(())
            }
        })
        .unwrap();
        let args: RegisterAsyncArgs = syn::parse2(quote! {
            slice = crate::db_init::AFTER_SCHEMA_TASKS,
            ty = crate::db_init::DbInitFn,
        })
        .unwrap();

        let expanded = expand_register_async(args, function).unwrap().to_string();

        assert!(expanded.contains("async fn initialize"));
        assert!(expanded.contains("__REGISTER_ASYNC_INITIALIZE"));
        assert!(expanded.contains("linkme :: distributed_slice"));
        assert!(expanded.contains("crate :: db_init :: AFTER_SCHEMA_TASKS"));
        assert!(expanded.contains("crate :: db_init :: DbInitFn"));
        assert!(expanded.contains("Box :: pin (initialize"));
    }

    #[test]
    fn rejects_invalid_async_registration_functions() {
        let sync_function: ItemFn = syn::parse2(quote! {
            fn initialize(db: &DatabaseConnection) -> ContextualResult<()> { Ok(()) }
        })
        .unwrap();
        let no_argument_function: ItemFn = syn::parse2(quote! {
            async fn initialize() -> ContextualResult<()> { Ok(()) }
        })
        .unwrap();
        let generic_function: ItemFn = syn::parse2(quote! {
            async fn initialize<T>(db: T) -> ContextualResult<()> { Ok(()) }
        })
        .unwrap();

        let args = || {
            syn::parse2(quote! {
                slice = crate::db_init::AFTER_SCHEMA_TASKS,
                ty = crate::db_init::DbInitFn,
            })
            .unwrap()
        };

        assert!(expand_register_async(args(), sync_function).is_err());
        assert!(expand_register_async(args(), no_argument_function).is_err());
        assert!(expand_register_async(args(), generic_function).is_err());
    }
}
