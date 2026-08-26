#![cfg(feature = "metrics")]

use metrics_util::CompositeKey;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};

#[common::metered(name = "metered_ok")]
async fn succeeds_with_early_return(early: bool) -> Result<u32, &'static str> {
    if early {
        return Ok(1);
    }
    Ok(2)
}

#[common::metered(name = "metered_err")]
async fn fails_with_question_mark() -> Result<(), &'static str> {
    Err("failed")?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn records_attempt_duration_and_each_success_once() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let _guard = metrics::set_default_local_recorder(&recorder);

    assert_eq!(succeeds_with_early_return(true).await, Ok(1));
    assert_eq!(succeeds_with_early_return(false).await, Ok(2));

    let snapshot = snapshotter.snapshot().into_vec();
    assert_metric(&snapshot, "common:metered_ok:attempts", |value| {
        matches!(value, DebugValue::Counter(2))
    });
    assert_metric(&snapshot, "common:metered_ok:success", |value| {
        matches!(value, DebugValue::Counter(2))
    });
    assert_metric(
        &snapshot,
        "common:metered_ok:duration_seconds",
        |value| matches!(value, DebugValue::Histogram(values) if values.len() == 2),
    );
}

#[tokio::test(flavor = "current_thread")]
async fn does_not_record_success_for_error() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let _guard = metrics::set_default_local_recorder(&recorder);

    assert_eq!(fails_with_question_mark().await, Err("failed"));

    let snapshot = snapshotter.snapshot().into_vec();
    assert_metric(&snapshot, "common:metered_err:attempts", |value| {
        matches!(value, DebugValue::Counter(1))
    });
    assert_metric(
        &snapshot,
        "common:metered_err:duration_seconds",
        |value| matches!(value, DebugValue::Histogram(values) if values.len() == 1),
    );
    assert!(
        snapshot
            .iter()
            .all(|(key, _, _, _)| key.key().name() != "common:metered_err:success")
    );
}

fn assert_metric(
    snapshot: &[(
        CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )],
    name: &str,
    predicate: impl FnOnce(&DebugValue) -> bool,
) {
    let value = snapshot
        .iter()
        .find_map(|(key, _, _, value)| (key.key().name() == name).then_some(value))
        .unwrap_or_else(|| panic!("未找到指标: {name}"));
    assert!(predicate(value), "指标值不符合预期: {name} = {value:?}");
}
