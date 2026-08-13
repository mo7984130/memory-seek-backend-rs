//! `common` 基础设施使用的过程宏。

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, ItemFn, LitStr, ReturnType, Token, Type, parse_macro_input};

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
}
