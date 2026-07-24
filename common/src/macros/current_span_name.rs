#[macro_export]
macro_rules! current_span_name {
    () => {
        tracing::Span::current()
            .metadata()
            .map(|m| m.name())
            .unwrap_or("unknown")
    };
}
