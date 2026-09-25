//! 分布式有序 ID 生成(雪花算法)。
//!
//! 由 `id-gen` feature 启用(`id-gen = ["dep:snowflaked"]`),当前唯一使用方是
//! 审计记录器(给 `AuditEvent` 分配事件 ID)。
//!
//! 实例号取环境变量 `MEMORY_SEEK_AUDIT_WORKER_ID`(取值 0~1023,缺省 0);
//! 多实例部署时各实例需使用不同的实例号,否则可能产生重复 ID。

use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use snowflaked::sync::Generator;

use crate::time::Duration;

const EPOCH_MILLIS: u64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z
const WORKER_ID_MASK: i64 = (1 << 10) - 1;

static GENERATOR: OnceLock<Generator> = OnceLock::new();

fn worker_id() -> u16 {
    std::env::var("MEMORY_SEEK_AUDIT_WORKER_ID")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| (0..=WORKER_ID_MASK).contains(value))
        .unwrap_or(0) as u16
}

fn generator() -> &'static Generator {
    GENERATOR.get_or_init(|| {
        Generator::builder()
            .instance(worker_id())
            .epoch(UNIX_EPOCH + Duration::from_millis(EPOCH_MILLIS))
            .build()
    })
}

/// 生成 64 位 Snowflake ID。
pub fn next_id() -> i64 {
    generator().generate()
}

#[cfg(test)]
mod tests {
    use super::next_id;

    #[test]
    fn generates_increasing_ids() {
        let first = next_id();
        let second = next_id();
        assert!(second > first);
    }
}
