use std::time::Instant;

#[derive(Debug)]
pub enum ErrorAllocating { Internal(String) }

#[derive(Debug)]
pub enum ErrorZeroCost { Internal(&'static str) }

#[test]
fn test_performance_comparison() {
    let iterations: i64 = 10_000_00000;

    let start_a = Instant::now();
    for _ in 0..iterations {
        let _err = ErrorAllocating::Internal("Redis query failed".to_string());
    }
    println!("String 分配模式耗时: {:?}", start_a.elapsed());

    let start_b = Instant::now();
    for _ in 0..iterations {
        let _err = ErrorZeroCost::Internal("Redis query failed");
    }
    println!("静态切片模式耗时: {:?}", start_b.elapsed());
}