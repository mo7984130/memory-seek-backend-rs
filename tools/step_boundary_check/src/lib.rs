//! `step_boundary_check` — 静态检查 `common::pipeline::Step` 的表归属白名单约束
//!
//! 规则(作用于所有 `impl ... Step for Xxx` 以及 `#[step_derive::declare_step(...)]`
//! 标记的 impl 块的 `on_photo_delete` 方法体):
//!
//! 1. 调用路径倒数第二个 segment 以 `Mapper` 结尾时,类型名必须在 `owns()` 白名单内;
//! 2. 禁止直接使用 SeaORM 实体(`Entity::` / `Column::` / `ActiveModel` / `Model`),
//!    必须经由白名单中的 Mapper 访问数据库。
//!
//! 基于 `syn` 做语法级分析,不依赖 rustc 内部 API,适用于 stable toolchain。

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    Block, Expr, GenericArgument, ImplItem, Item, ItemImpl, Path, PathArguments, Type, UseTree,
};

/// 一次边界违规
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Violation {
    fn new(file: &str, span: Span, message: impl Into<String>) -> Self {
        let loc = span.start();
        Self {
            file: file.to_string(),
            line: loc.line,
            column: loc.column,
            message: message.into(),
        }
    }
}

/// 检查一份 Rust 源码中所有 `impl Step` 与 `#[declare_step]` 的边界约束
pub fn check_source(source: &str, file: &str) -> Vec<Violation> {
    let Ok(ast) = syn::parse_file(source) else {
        // 语法错误交由 rustc 报告,此处不重复
        return Vec::new();
    };

    let mut violations = Vec::new();
    collect_step_impls(&ast.items, &mut |item_impl| {
        let owns = parse_owns(item_impl);
        for impl_item in &item_impl.items {
            if let ImplItem::Fn(method) = impl_item {
                if method.sig.ident == "execute" {
                    check_body(&owns, &method.block, file, &mut violations);
                }
            }
        }
    });
    collect_declare_step_impls(&ast.items, file, &mut violations);
    check_error_boundaries(&ast, file, &mut violations);
    violations
}

fn check_error_boundaries(ast: &syn::File, file: &str, violations: &mut Vec<Violation>) {
    let normalized = file.replace('\\', "/");
    let is_domain_lower = normalized.contains("/src/mappers/")
        || normalized.contains("/src/mapper/")
        || normalized.contains("/src/repo/")
        || normalized.ends_with("/domains/backup/src/storage.rs")
        || normalized.ends_with("/domains/backup/src/exporter.rs")
        || normalized.ends_with("/domains/backup/src/hasher.rs")
        || normalized.ends_with("/domains/backup/src/state.rs");
    let enforce_caller_macro = normalized.contains("/domains/") || normalized.contains("/server/");

    let mut visitor = ErrorBoundaryVisitor {
        file,
        is_domain_lower,
        enforce_caller_macro,
        violations,
    };
    visitor.visit_file(ast);
}

struct ErrorBoundaryVisitor<'a> {
    file: &'a str,
    is_domain_lower: bool,
    enforce_caller_macro: bool,
    violations: &'a mut Vec<Violation>,
}

impl<'ast> Visit<'ast> for ErrorBoundaryVisitor<'_> {
    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        if let Some(source) = forbidden_app_error_from_source(item) {
            self.violations.push(Violation::new(
                self.file,
                item.span(),
                format!(
                    "禁止实现 From<{source}> for AppError，基础设施错误必须先转换为 DeferredError"
                ),
            ));
        }
        syn::visit::visit_item_impl(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if self.is_domain_lower && imports_common_result(&item.tree, &[]) {
            self.violations.push(Violation::new(
                self.file,
                item.span(),
                "下层模块必须使用 common::error::deferred::Result，不能提前返回 common::Result/AppError",
            ));
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_path(&mut self, path: &'ast Path) {
        if self.is_domain_lower {
            let names = path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if names.windows(2).any(|pair| pair == ["common", "Result"])
                || names
                    .windows(3)
                    .any(|parts| parts == ["common", "error", "Result"])
            {
                self.violations.push(Violation::new(
                    self.file,
                    path.span(),
                    "下层模块必须使用 common::error::deferred::Result，不能提前返回 common::Result/AppError",
                ));
            }
            if names.last().is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "log_err" | "log_err_with_source" | "log_warn" | "log_warn_with_source"
                )
            }) {
                self.violations.push(Violation::new(
                    self.file,
                    path.span(),
                    "下层模块禁止直接记录错误，必须延迟到 service 边界",
                ));
            }
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.is_domain_lower {
            let method = call.method.to_string();
            if matches!(
                method.as_str(),
                "ok_or_warn"
                    | "ok_or_warn_bad_request"
                    | "ok_or_error"
                    | "true_or_warn"
                    | "false_or_warn"
            ) {
                self.violations.push(Violation::new(
                    self.file,
                    call.method.span(),
                    "下层模块禁止调用会记录日志的错误扩展",
                ));
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        if self.is_domain_lower
            && name
                .as_deref()
                .is_some_and(|name| matches!(name, "caller_warn" | "caller_error"))
        {
            self.violations.push(Violation::new(
                self.file,
                mac.path.span(),
                "下层模块禁止直接记录错误，必须把问题返回 service",
            ));
        }
        if self.enforce_caller_macro
            && name
                .as_deref()
                .is_some_and(|name| matches!(name, "warn" | "error"))
        {
            self.violations.push(Violation::new(
                self.file,
                mac.path.span(),
                "项目错误日志必须使用 caller_warn!/caller_error! 写入 caller 字段",
            ));
        }
        syn::visit::visit_macro(self, mac);
    }
}

fn forbidden_app_error_from_source(item: &ItemImpl) -> Option<String> {
    let Type::Path(self_type) = item.self_ty.as_ref() else {
        return None;
    };
    if self_type.path.segments.last()?.ident != "AppError" {
        return None;
    }

    let (_, trait_path, _) = item.trait_.as_ref()?;
    let from = trait_path.segments.last()?;
    if from.ident != "From" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &from.arguments else {
        return None;
    };
    let source = arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(Type::Path(source)) => Some(&source.path),
        _ => None,
    })?;
    let names = source
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let last = names.last()?.as_str();
    let forbidden = matches!(
        last,
        "DbErr" | "PoolError" | "RedisError" | "CacheError" | "AcquireError" | "JoinError"
    ) || (last == "Error" && names.iter().any(|name| name == "serde_json"));

    forbidden.then(|| names.join("::"))
}

fn imports_common_result(tree: &UseTree, prefix: &[String]) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            imports_common_result(&path.tree, &next)
        }
        UseTree::Name(name) => {
            matches!(prefix, [common] if common == "common") && name.ident == "Result"
                || matches!(prefix, [common, error] if common == "common" && error == "error")
                    && name.ident == "Result"
        }
        UseTree::Rename(rename) => {
            matches!(prefix, [common] if common == "common") && rename.ident == "Result"
                || matches!(prefix, [common, error] if common == "common" && error == "error")
                    && rename.ident == "Result"
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|item| imports_common_result(item, prefix)),
        UseTree::Glob(_) => false,
    }
}

/// 对一个 execute 方法体 / 宏 body 执行边界检查
fn check_body(owns: &[String], block: &Block, file: &str, violations: &mut Vec<Violation>) {
    let mut visitor = StepVisitor {
        owns,
        file,
        violations,
    };
    visitor.visit_block(block);
}

/// 递归收集所有 `impl Step` 块(含子模块)
fn collect_step_impls<'a>(items: &'a [Item], f: &mut impl FnMut(&'a ItemImpl)) {
    for item in items {
        match item {
            Item::Impl(item_impl) => {
                if is_step_impl(item_impl) {
                    f(item_impl);
                }
            }
            Item::Mod(item_mod) => {
                if let Some((_, items)) = &item_mod.content {
                    collect_step_impls(items, f);
                }
            }
            _ => {}
        }
    }
}

/// 递归收集所有带 `#[declare_step(...)]` 属性的 impl 块,检查其 `on_photo_delete` 方法体
fn collect_declare_step_impls(items: &[Item], file: &str, violations: &mut Vec<Violation>) {
    for item in items {
        match item {
            Item::Impl(item_impl) => {
                if let Some(owns) = parse_declare_step_attr(&item_impl.attrs) {
                    for impl_item in &item_impl.items {
                        if let ImplItem::Fn(method) = impl_item {
                            if method.sig.ident == "on_photo_delete" {
                                check_body(&owns, &method.block, file, violations);
                            }
                        }
                    }
                }
            }
            Item::Mod(item_mod) => {
                if let Some((_, items)) = &item_mod.content {
                    collect_declare_step_impls(items, file, violations);
                }
            }
            _ => {}
        }
    }
}

/// 从 impl 块的属性中提取 `#[declare_step(...)]` 的 `owns` 白名单数组
fn parse_declare_step_attr(attrs: &[syn::Attribute]) -> Option<Vec<String>> {
    for attr in attrs {
        let is_declare_step = attr
            .path()
            .segments
            .last()
            .map(|seg| seg.ident == "declare_step")
            .unwrap_or(false);
        if !is_declare_step {
            continue;
        }
        if let syn::Meta::List(list) = &attr.meta {
            let mut iter = list.tokens.clone().into_iter().peekable();
            while let Some(tt) = iter.next() {
                if let TokenTree::Ident(ident) = &tt {
                    if ident == "owns" {
                        iter.next(); // `=`
                        if let Some(TokenTree::Group(group)) = iter.next() {
                            if group.delimiter() == Delimiter::Bracket {
                                return Some(extract_strings(group.stream()));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从 token 流中收集所有字符串字面量
fn extract_strings(tokens: TokenStream) -> Vec<String> {
    tokens
        .into_iter()
        .filter_map(|tt| {
            if let TokenTree::Literal(lit) = tt {
                literal_to_string(&lit)
            } else {
                None
            }
        })
        .collect()
}

fn literal_to_string(lit: &proc_macro2::Literal) -> Option<String> {
    let s = lit.to_string();
    syn::parse_str::<syn::LitStr>(&s).ok().map(|ls| ls.value())
}

/// 判断 impl 是否实现了名为 `Step` 的 trait
fn is_step_impl(item_impl: &ItemImpl) -> bool {
    let Some((_, trait_path, _)) = &item_impl.trait_ else {
        return false;
    };
    trait_path
        .segments
        .last()
        .map(|seg| seg.ident == "Step")
        .unwrap_or(false)
}

/// 解析 `owns()` 方法体中返回的字符串字面量数组
fn parse_owns(item_impl: &ItemImpl) -> Vec<String> {
    for impl_item in &item_impl.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        if method.sig.ident != "owns" {
            continue;
        }
        let mut collector = ArrayCollector::default();
        collector.visit_block(&method.block);
        return collector.names;
    }
    Vec::new()
}

#[derive(Default)]
struct ArrayCollector {
    names: Vec<String>,
}

impl<'ast> Visit<'ast> for ArrayCollector {
    fn visit_expr(&mut self, node: &'ast Expr) {
        if let Expr::Array(arr) = node {
            for elem in &arr.elems {
                if let Expr::Lit(lit) = elem {
                    if let syn::Lit::Str(s) = &lit.lit {
                        self.names.push(s.value());
                    }
                }
            }
        }
        syn::visit::visit_expr(self, node);
    }
}

struct StepVisitor<'a> {
    owns: &'a [String],
    file: &'a str,
    violations: &'a mut Vec<Violation>,
}

impl<'ast> Visit<'ast> for StepVisitor<'_> {
    fn visit_expr(&mut self, node: &'ast Expr) {
        // 结构体构造:ActiveModel { ... } / Model { ... }(单 segment path,visit_path 覆盖不到)
        if let Expr::Struct(s) = node {
            if let Some(seg) = s.path.segments.last() {
                let name = seg.ident.to_string();
                if matches!(name.as_str(), "ActiveModel" | "Model") {
                    self.violations
                        .push(Violation::new(self.file, s.path.span(), ENTITY_MSG));
                }
            }
        }
        syn::visit::visit_expr(self, node);
    }

    fn visit_path(&mut self, path: &'ast Path) {
        self.check_path(path, path.span());
        syn::visit::visit_path(self, path);
    }
}

impl StepVisitor<'_> {
    fn check_path(&mut self, path: &Path, span: Span) {
        let segments: Vec<_> = path.segments.iter().collect();
        if segments.len() < 2 {
            return;
        }
        let type_name = segments[segments.len() - 2].ident.to_string();

        if matches!(type_name.as_str(), "Entity" | "Column") {
            self.violations
                .push(Violation::new(self.file, span, ENTITY_MSG));
            return;
        }

        if type_name.ends_with("Mapper") && !self.owns.iter().any(|own| own == &type_name) {
            self.violations.push(Violation::new(
                self.file,
                span,
                format!(
                    "Step::execute 越权访问 Mapper `{type_name}`,它不在 owns() 白名单中;请将其加入 owns() 或拆分到对应步骤"
                ),
            ));
        }
    }
}

const ENTITY_MSG: &str = "Step::execute 禁止直接使用 SeaORM 实体(Entity/Column/ActiveModel/Model),请通过 owns() 白名单中的 Mapper 访问数据库";

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with(execute_body: &str, owns: &str) -> String {
        format!(
            r#"
use common::pipeline::Step;
struct MyStep;
impl Step<Ctx> for MyStep {{
    fn name(&self) -> &'static str {{ "my_step" }}
    fn owns(&self) -> &'static [&'static str] {{ {owns} }}
    async fn execute(&self, _txn: &DatabaseTransaction, _ctx: &mut Ctx) -> Result<()> {{
        {execute_body}
        Ok(())
    }}
}}
"#
        )
    }

    #[test]
    fn allows_owned_mapper() {
        let src = source_with(
            "CollectionMapper::update_count().await?;",
            r#"&["CollectionMapper"]"#,
        );
        assert!(check_source(&src, "t.rs").is_empty());
    }

    #[test]
    fn rejects_unowned_mapper() {
        let src = source_with(
            "CommentMapper::delete_all().await?;",
            r#"&["CollectionMapper"]"#,
        );
        let vs = check_source(&src, "t.rs");
        assert_eq!(vs.len(), 1);
        assert!(vs[0].message.contains("CommentMapper"));
        assert!(vs[0].line > 1);
    }

    #[test]
    fn rejects_logging_result_in_mapper() {
        let source = r#"
            use common::Result;
            async fn query() -> Result<()> {
                None::<()>.ok_or_warn("db", "query", AppError::InternalServerError)?;
                Ok(())
            }
        "#;
        let violations = check_source(source, "/repo/domains/photo/src/mappers/demo.rs");
        assert!(violations
            .iter()
            .any(|v| v.message.contains("common::error::deferred::Result")));
        assert!(violations.iter().any(|v| v.message.contains("错误扩展")));
    }

    #[test]
    fn allows_deferred_error_in_mapper() {
        let source = r#"
            use common::error::deferred::Result;
            async fn query() -> Result<()> { Ok(()) }
        "#;
        assert!(check_source(source, "/repo/domains/photo/src/mappers/demo.rs").is_empty());
    }

    #[test]
    fn rejects_app_error_result_imported_from_error_module() {
        let source = r#"
            use common::error::Result;
            async fn query() -> Result<()> { Ok(()) }
        "#;
        let violations = check_source(source, "/repo/domains/photo/src/mappers/demo.rs");
        assert!(violations
            .iter()
            .any(|v| v.message.contains("common::error::deferred::Result")));
    }

    #[test]
    fn rejects_infrastructure_from_for_app_error() {
        let source = r#"
            impl From<sea_orm::DbErr> for AppError {
                fn from(_: sea_orm::DbErr) -> Self { AppError::InternalServerError }
            }
        "#;
        let violations = check_source(source, "/repo/common/src/error/db_error.rs");
        assert!(violations.iter().any(|violation| {
            violation.message.contains("From<sea_orm::DbErr>")
                && violation.message.contains("DeferredError")
        }));
    }

    #[test]
    fn allows_domain_error_from_for_app_error() {
        let source = r#"
            impl From<BackupError> for AppError {
                fn from(_: BackupError) -> Self { AppError::InternalServerError }
            }
        "#;
        assert!(check_source(source, "/repo/domains/backup/src/error.rs").is_empty());
    }

    #[test]
    fn rejects_direct_error_macro_in_service() {
        let source = r#"
            fn service() { tracing::error!("failed"); }
        "#;
        let violations = check_source(source, "/repo/domains/photo/src/services/demo.rs");
        assert!(violations
            .iter()
            .any(|v| v.message.contains("caller_error")));
    }

    #[test]
    fn rejects_direct_entity_call() {
        let src = source_with("Entity::find().all(_txn).await?;", r#"&[]"#);
        assert_eq!(check_source(&src, "t.rs").len(), 1);
    }

    #[test]
    fn rejects_column_path() {
        let src = source_with("filter(Column::PhotoId.is_in(vec![1]));", r#"&[]"#);
        assert_eq!(check_source(&src, "t.rs").len(), 1);
    }

    #[test]
    fn rejects_active_model_struct() {
        let src = source_with(
            "let _ = ActiveModel { id: Set(1), ..Default::default() };",
            r#"&[]"#,
        );
        assert_eq!(check_source(&src, "t.rs").len(), 1);
    }

    #[test]
    fn ignores_context_method_call() {
        let src = source_with("let ids = _ctx.photo_ids();", r#"&["PhotoMapper"]"#);
        assert!(check_source(&src, "t.rs").is_empty());
    }

    #[test]
    fn ignores_std_type_paths() {
        let src = source_with(
            "let v: Vec<i64> = Vec::new(); let s = String::from(\"x\");",
            r#"&[]"#,
        );
        assert!(check_source(&src, "t.rs").is_empty());
    }

    #[test]
    fn ignores_non_step_impls() {
        let src = r#"
struct Other;
impl Other {
    fn execute(&self) {
        CommentMapper::x();
        Entity::find();
    }
}
"#;
        assert!(check_source(src, "t.rs").is_empty());
    }

    #[test]
    fn finds_step_in_nested_module() {
        let src = r#"
mod a {
    mod b {
        struct MyStep;
        impl common::pipeline::Step<Ctx> for MyStep {
            fn name(&self) -> &'static str { "x" }
            fn owns(&self) -> &'static [&'static str] { &["A"] }
            async fn execute(&self) { CommentMapper::x(); }
        }
    }
}
"#;
        let vs = check_source(src, "t.rs");
        assert_eq!(vs.len(), 1);
        assert!(vs[0].message.contains("CommentMapper"));
    }

    #[test]
    fn allows_owned_mapper_in_declare_step() {
        let src = r#"
#[step_derive::declare_step(
    ctx = crate::services::photo_service::PhotoDeleteContext,
    name = "foo",
    owns = ["CollectionPhotoMapper", "CollectionMapper"],
)]
impl FooService {
    async fn on_photo_delete(&self, txn: &sea_orm::DatabaseTransaction, ctx: &mut PhotoDeleteContext) -> common::Result<()> {
        let ids = ctx.photo_ids();
        CollectionPhotoMapper::delete_by_photo_ids(txn, &ids).await?;
        CollectionMapper::update_photo_count_delta_batch(txn, &HashMap::new()).await?;
        Ok(())
    }
}
"#;
        assert!(check_source(src, "t.rs").is_empty());
    }

    #[test]
    fn rejects_unowned_mapper_in_declare_step() {
        let src = r#"
#[step_derive::declare_step(
    ctx = PhotoDeleteContext,
    name = "foo",
    owns = ["CollectionMapper"],
)]
impl FooService {
    async fn on_photo_delete(&self, txn: &sea_orm::DatabaseTransaction, ctx: &mut PhotoDeleteContext) -> common::Result<()> {
        CommentMapper::delete_all(txn).await?;
        Ok(())
    }
}
"#;
        let vs = check_source(src, "t.rs");
        assert_eq!(vs.len(), 1);
        assert!(vs[0].message.contains("CommentMapper"));
    }

    #[test]
    fn rejects_direct_entity_in_declare_step() {
        let src = r#"
#[step_derive::declare_step(
    name = "foo",
    owns = [],
    ctx = PhotoDeleteContext,
)]
impl FooService {
    async fn on_photo_delete(&self, txn: &sea_orm::DatabaseTransaction, ctx: &mut PhotoDeleteContext) -> common::Result<()> {
        Entity::find().all(txn).await?;
        let _ = ActiveModel { ..Default::default() };
        Ok(())
    }
}
"#;
        let vs = check_source(src, "t.rs");
        assert_eq!(vs.len(), 2);
    }

    #[test]
    fn rejects_declare_step_in_nested_module() {
        let src = r#"
mod a {
    #[step_derive::declare_step(
        ctx = PhotoDeleteContext,
        name = "foo",
        owns = ["A"],
    )]
    impl FooService {
        async fn on_photo_delete(&self, txn: &sea_orm::DatabaseTransaction, ctx: &mut PhotoDeleteContext) -> common::Result<()> {
            BMapper::x(txn).await?;
            Ok(())
        }
    }
}
"#;
        let vs = check_source(src, "t.rs");
        assert_eq!(vs.len(), 1);
    }
}
