use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const EPOCH_MILLIS: i64 = 1_704_067_200_000; // 2024-01-01T00:00:00Z
const WORKER_ID_BITS: u8 = 10;
const SEQUENCE_BITS: u8 = 12;
const WORKER_ID_MASK: i64 = (1 << WORKER_ID_BITS) - 1;
const SEQUENCE_MASK: i64 = (1 << SEQUENCE_BITS) - 1;
const WORKER_ID_SHIFT: u8 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u8 = WORKER_ID_BITS + SEQUENCE_BITS;

#[derive(Default)]
struct State {
    last_millis: i64,
    sequence: i64,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn worker_id() -> i64 {
    std::env::var("MEMORY_SEEK_AUDIT_WORKER_ID")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| (0..=WORKER_ID_MASK).contains(value))
        .unwrap_or(0)
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

/// 生成 64 位 Snowflake ID。
pub fn next_id() -> i64 {
    let state = STATE.get_or_init(|| Mutex::new(State::default()));
    let worker_id = worker_id();
    let mut state = state.lock().expect("snowflake state mutex poisoned");

    loop {
        let millis = current_millis();
        let timestamp = millis.max(state.last_millis);

        if timestamp == state.last_millis {
            state.sequence = (state.sequence + 1) & SEQUENCE_MASK;
            if state.sequence == 0 {
                thread::yield_now();
                continue;
            }
        } else {
            state.sequence = 0;
        }

        state.last_millis = timestamp;
        return ((timestamp - EPOCH_MILLIS) << TIMESTAMP_SHIFT)
            | ((worker_id & WORKER_ID_MASK) << WORKER_ID_SHIFT)
            | state.sequence;
    }
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
