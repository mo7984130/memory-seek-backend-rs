use std::hint::black_box;

use common::error::{AppError, ContextualError, contextual::Result};
use criterion::{Criterion, criterion_group, criterion_main};

fn successful_result(c: &mut Criterion) {
    c.bench_function("contextual_result_success", |b| {
        b.iter(|| {
            let result: Result<u64> = Ok(black_box(42));
            black_box(result)
        })
    });
}

fn error_construction_and_conversion(c: &mut Criterion) {
    c.bench_function("contextual_error_construct_and_convert", |b| {
        b.iter(|| {
            let error = ContextualError::error(
                "db_err",
                "数据库错误",
                black_box("connection refused"),
                AppError::InternalServerError,
            );
            black_box(AppError::from(error))
        })
    });
}

criterion_group!(
    benches,
    successful_result,
    error_construction_and_conversion
);
criterion_main!(benches);
