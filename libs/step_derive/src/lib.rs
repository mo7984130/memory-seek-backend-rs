//! `declare_transaction_step` — 事务内 `common::pipeline::Step` 声明宏(attribute 宏)
//!
//! 作用于一个 `impl` 块,从其中提取目标类型并声明一个清理/变更步骤,同时
//! **定义即注册**:通过 `linkme` 分布式切片将步骤注册进调用方声明的步骤集合。
//!
//! ```ignore
//! #[step_derive::declare_transaction_step(
//!     ctx = crate::services::photo_service::PhotoDeleteContext,
//!     slice = crate::services::photo_service::PHOTO_DELETE_STEPS,
//!     name = "foo_cleanup",
//!     owns = ["FooMapper", "BarMapper"],
//!     is_final = true,          // 可选:最后执行的步骤(受外键约束时置位)
//! )]
//! impl FooService {
//!     async fn on_photo_delete(
//!         &self,
//!         txn: &sea_orm::DatabaseTransaction,
//!         ctx: &mut crate::services::photo_service::PhotoDeleteContext,
//!     ) -> common::Result<()> {
//!         // 具体清理逻辑
//!         Ok(())
//!     }
//! }
//! ```
//!
//! 宏展开为「原 impl 块 + `impl Step<Ctx> for FooService` + 一个 linkme 分布式切片元素」,
//! 生成的 `execute` 委托调用块内的 `on_photo_delete(txn, ctx)` 方法:
//! - 步骤方法名统一为 `on_photo_delete`;
//! - 生成的步骤结构即为 service 本身(unit struct),无需额外定义 Step 结构体;
//! - 宏不绑定任何业务类型:`ctx` / `slice` / `name` / `owns` / `is_final` 全部参数化。
//!
//! 要求调用 crate 依赖 `common`、`sea-orm` 与 `linkme`(宏生成的路径)。
//! `on_photo_delete` 的参数名任意(按位置传递),但参数类型必须与生成签名一致。

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{bracketed, parse_macro_input, Ident, ImplItem, ItemImpl, LitBool, LitStr, Token, Type};

/// `#[declare_transaction_step(...)]` 属性宏入口
#[proc_macro_attribute]
pub fn declare_transaction_step(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as Args);
    let item_impl = parse_macro_input!(item as ItemImpl);
    match expand(args, item_impl) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// `declare_async_event!(<状态类型>, <事件类型>, <切片名>, <发布函数名>, <事件名>)` — 声明提交后的异步事件。
///
/// 展开为一个 `linkme` 分布式切片和调用 `common::tokio::event::dispatch_async_event` 的发布函数。
#[proc_macro]
pub fn declare_async_event(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as EventArgs);
    let EventArgs {
        state,
        event,
        slice,
        dispatch,
        name,
    } = args;
    quote! {
        #[::linkme::distributed_slice]
        pub(crate) static #slice: [&'static dyn ::common::tokio::event::EventConsumer<#state, #event>] = [..];

        pub(crate) fn #dispatch(
            state: ::std::sync::Arc<#state>,
            event: #event,
        ) {
            ::common::tokio::event::dispatch_async_event(#name, state, event, &#slice);
        }
    }
    .into()
}

/// `#[declare_event_consumer(...)]` — 声明并注册一个提交后异步事件消费者。
///
/// 标记的 impl 必须且只能包含一个异步回调方法；宏将其作为事件消费者。
#[proc_macro_attribute]
pub fn declare_event_consumer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as EventConsumerArgs);
    let item_impl = parse_macro_input!(item as ItemImpl);
    match expand_event_consumer(args, item_impl) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// `declare_pipeline!(<ctx 类型>, <切片名>, <管道名>)` — 声明一个步骤管道
///
/// 展开为「一个 `linkme` 分布式切片 + 一个惰性 `StepPipeline`」,供 `#[declare_transaction_step]`
/// 注册步骤(定义即注册)与直接执行(`<管道名>.run(...)`)使用:
///
/// ```ignore
/// step_derive::declare_pipeline!(PhotoDeleteContext, PHOTO_DELETE_STEPS, PIPELINE);
/// // 展开:
/// #[linkme::distributed_slice]
/// pub(crate) static PHOTO_DELETE_STEPS: [&'static dyn Step<PhotoDeleteContext>] = [..];
/// static PIPELINE: LazyLock<StepPipeline<PhotoDeleteContext>> =
///     LazyLock::new(|| StepPipeline::from_slice_stable(PHOTO_DELETE_STEPS.to_vec()));
/// ```
///
/// 要求调用 crate 依赖 `common` 与 `linkme`。
#[proc_macro]
pub fn declare_pipeline(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as PipelineArgs);
    let PipelineArgs {
        ctx,
        slice,
        pipeline,
    } = args;
    let expanded = quote! {
        #[linkme::distributed_slice]
        pub(crate) static #slice: [&'static dyn ::common::pipeline::Step<#ctx>] = [..];

        static #pipeline: ::std::sync::LazyLock<::common::pipeline::StepPipeline<#ctx>> =
            ::std::sync::LazyLock::new(|| {
                ::common::pipeline::StepPipeline::from_slice_stable(#slice.to_vec())
            });
    };
    expanded.into()
}

struct PipelineArgs {
    ctx: Type,
    slice: Ident,
    pipeline: Ident,
}

struct EventArgs {
    state: Type,
    event: Type,
    slice: Ident,
    dispatch: Ident,
    name: LitStr,
}

impl Parse for EventArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let state = input.parse()?;
        input.parse::<Token![,]>()?;
        let event = input.parse()?;
        input.parse::<Token![,]>()?;
        let slice = input.parse()?;
        input.parse::<Token![,]>()?;
        let dispatch = input.parse()?;
        input.parse::<Token![,]>()?;
        let name = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            state,
            event,
            slice,
            dispatch,
            name,
        })
    }
}

impl Parse for PipelineArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ctx: Type = input.parse()?;
        input.parse::<Token![,]>()?;
        let slice: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let pipeline: Ident = input.parse()?;
        Ok(PipelineArgs {
            ctx,
            slice,
            pipeline,
        })
    }
}

struct Args {
    name: LitStr,
    owns: Vec<LitStr>,
    is_final: Option<bool>,
    ctx: Option<Type>,
    slice: Option<Type>,
}

struct EventConsumerArgs {
    name: LitStr,
    state: Option<Type>,
    event: Option<Type>,
    slice: Option<Type>,
}

impl Parse for EventConsumerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut state = None;
        let mut event = None;
        let mut slice = None;

        while !input.is_empty() {
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                continue;
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if key == "name" {
                name = Some(input.parse()?);
            } else if key == "state" {
                state = Some(input.parse()?);
            } else if key == "event" {
                event = Some(input.parse()?);
            } else if key == "slice" {
                slice = Some(input.parse()?);
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    format!("期望 `name` / `state` / `event` / `slice`,发现 `{key}`"),
                ));
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            name: name.ok_or_else(|| input.error("缺少 `name = \"...\"` 参数"))?,
            state,
            event,
            slice,
        })
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut owns: Option<Vec<LitStr>> = None;
        let mut is_final: Option<bool> = None;
        let mut ctx: Option<Type> = None;
        let mut slice: Option<Type> = None;

        while !input.is_empty() {
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                continue;
            }
            let kw: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if kw == "name" {
                name = Some(input.parse()?);
                input.parse::<Token![,]>()?;
            } else if kw == "owns" {
                let content;
                bracketed!(content in input);
                let mut list = Vec::new();
                while !content.is_empty() {
                    list.push(content.parse()?);
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Token![,]>()?;
                }
                input.parse::<Token![,]>()?;
                owns = Some(list);
            } else if kw == "is_final" {
                let value: LitBool = input.parse()?;
                input.parse::<Token![,]>()?;
                is_final = Some(value.value);
            } else if kw == "ctx" {
                ctx = Some(input.parse()?);
                input.parse::<Token![,]>()?;
            } else if kw == "slice" {
                slice = Some(input.parse()?);
                input.parse::<Token![,]>()?;
            } else {
                return Err(syn::Error::new(
                    kw.span(),
                    format!("期望 `name` / `owns` / `is_final` / `ctx` / `slice`,发现 `{kw}`"),
                ));
            }
        }

        Ok(Args {
            name: name.ok_or_else(|| input.error("缺少 `name = \"...\"` 参数"))?,
            owns: owns.ok_or_else(|| input.error("缺少 `owns = [...]` 参数"))?,
            is_final,
            ctx,
            slice,
        })
    }
}

/// 将步骤 impl 展开为 Step 实现和分布式注册项.
fn expand(args: Args, item_impl: ItemImpl) -> syn::Result<TokenStream2> {
    let Args {
        name,
        owns,
        is_final,
        ctx,
        slice,
    } = args;

    let ctx = ctx.ok_or_else(|| syn::Error::new(item_impl.span(), "缺少 `ctx = <Type>` 参数"))?;
    let slice =
        slice.ok_or_else(|| syn::Error::new(item_impl.span(), "缺少 `slice = <path>` 参数"))?;
    let self_ty = item_impl.self_ty.clone();

    if !item_impl
        .items
        .iter()
        .any(|item| matches!(item, ImplItem::Fn(method) if method.sig.ident == "on_photo_delete"))
    {
        return Err(syn::Error::new(
            item_impl.span(),
            "impl 块内缺少 `async fn on_photo_delete(...)` 方法",
        ));
    }

    let is_final_lit = if is_final == Some(true) {
        quote!(true)
    } else {
        quote!(false)
    };

    // 由 impl 目标类型生成唯一的注册元素名(同一模块多个事务步骤也不冲突)
    let self_ty_ident: Ident = match &*item_impl.self_ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|seg| seg.ident.clone())
            .ok_or_else(|| syn::Error::new(item_impl.span(), "无法从 impl 目标类型提取名称"))?,
        _ => {
            return Err(syn::Error::new(
                item_impl.span(),
                "`#[declare_transaction_step]` 仅支持路径类型的 impl 目标",
            ));
        }
    };
    let step_static = format_ident!("__step_{}", self_ty_ident);

    let generated = quote! {
        #[::async_trait::async_trait]
        impl ::common::pipeline::Step<#ctx> for #self_ty {
            fn name(&self) -> &'static str {
                #name
            }

            fn owns(&self) -> &'static [&'static str] {
                &[#(#owns),*]
            }

            fn is_final(&self) -> bool {
                #is_final_lit
            }

            async fn execute(
                &self,
                txn: &::sea_orm::DatabaseTransaction,
                ctx: &mut #ctx,
            ) -> ::common::error::contextual::Result<()> {
                self.on_photo_delete(txn, ctx).await
            }
        }

        // 定义即注册:将步骤注册进调用方声明的 linkme 分布式切片
        #[allow(non_upper_case_globals)]
        #[::linkme::distributed_slice(#slice)]
        static #step_static: &'static dyn ::common::pipeline::Step<#ctx> =
            &#self_ty as &dyn ::common::pipeline::Step<#ctx>;
    };

    Ok(quote! {
        #item_impl
        #generated
    })
}

/// 将事件消费者 impl 展开为 EventConsumer 实现和注册项.
fn expand_event_consumer(
    args: EventConsumerArgs,
    item_impl: ItemImpl,
) -> syn::Result<TokenStream2> {
    let EventConsumerArgs {
        name,
        state,
        event,
        slice,
    } = args;
    let state =
        state.ok_or_else(|| syn::Error::new(item_impl.span(), "缺少 `state = <Type>` 参数"))?;
    let event =
        event.ok_or_else(|| syn::Error::new(item_impl.span(), "缺少 `event = <Type>` 参数"))?;
    let slice =
        slice.ok_or_else(|| syn::Error::new(item_impl.span(), "缺少 `slice = <path>` 参数"))?;
    let self_ty = item_impl.self_ty.clone();

    let consumer_methods = item_impl
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if method.sig.asyncness.is_some() => {
                Some(method.sig.ident.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [consumer_method] = consumer_methods.as_slice() else {
        return Err(syn::Error::new(
            item_impl.span(),
            "带 `#[declare_event_consumer]` 的 impl 必须且只能包含一个 `async fn` 方法",
        ));
    };
    let consumer_method = consumer_method.clone();

    let self_ty_ident = type_last_ident(
        &self_ty,
        "`#[declare_event_consumer]` 仅支持路径类型的 impl 目标",
    )?;
    let event_ident = type_last_ident(&event, "`event` 必须是路径类型")?;
    let registration = format_ident!("__event_consumer_{}_{}", self_ty_ident, event_ident);

    Ok(quote! {
        #item_impl

        #[::async_trait::async_trait]
        impl ::common::tokio::event::EventConsumer<#state, #event> for #self_ty {
            fn name(&self) -> &'static str {
                #name
            }

            async fn consume(
                &self,
                state: ::std::sync::Arc<#state>,
                event: ::std::sync::Arc<#event>,
            ) -> ::common::Result<()> {
                <#self_ty>::#consumer_method(self, state, event).await
            }
        }

        #[allow(non_upper_case_globals)]
        #[::linkme::distributed_slice(#slice)]
        static #registration: &'static dyn ::common::tokio::event::EventConsumer<#state, #event> =
            &#self_ty as &dyn ::common::tokio::event::EventConsumer<#state, #event>;
    })
}

/// 从路径类型中提取最后一个标识符.
fn type_last_ident(ty: &Type, error_message: &str) -> syn::Result<Ident> {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.clone())
            .ok_or_else(|| syn::Error::new(ty.span(), error_message)),
        _ => Err(syn::Error::new(ty.span(), error_message)),
    }
}
