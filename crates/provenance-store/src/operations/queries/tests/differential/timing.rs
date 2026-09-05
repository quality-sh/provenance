//! Wall-time rows for the oracle and the served side of one case.
//!
//! The rows are a report, not a gate: the ignored `ab_timing_rows` test
//! prints them when someone asks for the numbers, and nothing asserts on
//! them.

use std::time::Instant;

/// Timed runs per case after one warm-up run.
pub const RUNS: usize = 5;

pub fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

pub fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

pub struct Row {
    pub operation: &'static str,
    pub request: String,
    pub oracle_ms: f64,
    pub served_ms: f64,
}

fn ratio(oracle_ms: f64, served_ms: f64) -> f64 {
    if oracle_ms > 0.0 {
        served_ms / oracle_ms
    } else {
        f64::INFINITY
    }
}

/// One row per case, one summary row per operation (the medians over its
/// cases), then the scan, rebuild, and steady-state catch-up times.
pub fn print_rows(corpus: &str, rows: &[Row], scan_ms: f64, rebuild_ms: f64, catch_up_ms: f64) {
    println!("A/B timings over {corpus}: operation request oracle_ms served_ms ratio");
    for row in rows {
        println!(
            "{} {} {:.1} {:.1} {:.2}",
            row.operation,
            row.request,
            row.oracle_ms,
            row.served_ms,
            ratio(row.oracle_ms, row.served_ms)
        );
    }
    let mut operations: Vec<&'static str> = Vec::new();
    for row in rows {
        if !operations.contains(&row.operation) {
            operations.push(row.operation);
        }
    }
    for operation in operations {
        let mut oracle: Vec<f64> = rows
            .iter()
            .filter(|row| row.operation == operation)
            .map(|row| row.oracle_ms)
            .collect();
        let mut served: Vec<f64> = rows
            .iter()
            .filter(|row| row.operation == operation)
            .map(|row| row.served_ms)
            .collect();
        let (oracle, served) = (median(&mut oracle), median(&mut served));
        println!(
            "{operation} summary {oracle:.1} {served:.1} {:.2}",
            ratio(oracle, served)
        );
    }
    println!("scan_ms {scan_ms:.1}");
    println!("rebuild_ms {rebuild_ms:.1}");
    println!("catch_up_ms {catch_up_ms:.1}");
}
