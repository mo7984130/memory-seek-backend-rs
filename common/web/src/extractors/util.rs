use validator::{ValidationErrors, ValidationErrorsKind};

use common_core::{ContextualError, error::AppError};

pub fn handle_validation_error(err: ValidationErrors) -> ContextualError {
    let msg = format_validation_errors(&err);
    ContextualError::warn_without_source(
        "validated_json_validate_error",
        format!("校验失败: {}", msg),
        AppError::bad_request(msg),
    )
}

fn format_validation_errors(errors: &ValidationErrors) -> String {
    errors
        .errors()
        .iter()
        .filter_map(|(field, kind)| match kind {
            ValidationErrorsKind::Field(errors) => Some(
                errors
                    .iter()
                    .map(|err| {
                        err.message
                            .as_deref()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("字段 '{field}' 校验失败"))
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("; ")
}
