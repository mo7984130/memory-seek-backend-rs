use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use crate::time::Duration;

use snowflaked::sync::Generator;

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
