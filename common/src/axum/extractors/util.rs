use validator::{ValidationErrors, ValidationErrorsKind};

use crate::{ContextualError, error::AppError};

pub fn handle_validation_error(err: ValidationErrors) -> ContextualError {
    ContextualError::warn_without_source(
        "validated_json_validate_error",
        "校验失败",
        AppError::bad_request(format_validation_errors(&err)),
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
