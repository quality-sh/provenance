//! Wall-time rows for the served side of one case.
//!
//! The rows are a report, not a gate: the ignored `timing_comparison_rows` test
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
    pub served_ms: f64,
}

/// One row per case, one summary row per operation (the medians over its
/// cases), then the scan, rebuild, and steady-state catch-up times.
pub fn print_rows(store: &str, rows: &[Row], scan_ms: f64, rebuild_ms: f64, catch_up_ms: f64) {
    println!("Timing over {store}: operation request served_ms");
    for row in rows {
        println!("{} {} {:.1}", row.operation, row.request, row.served_ms);
    }
    let mut operations: Vec<&'static str> = Vec::new();
    for row in rows {
        if !operations.contains(&row.operation) {
            operations.push(row.operation);
        }
    }
    for operation in operations {
        let mut served: Vec<f64> = rows
            .iter()
            .filter(|row| row.operation == operation)
            .map(|row| row.served_ms)
            .collect();
        println!("{operation} summary {:.1}", median(&mut served));
    }
    println!("scan_ms {scan_ms:.1}");
    println!("rebuild_ms {rebuild_ms:.1}");
    println!("catch_up_ms {catch_up_ms:.1}");
}
