//! Wall-time rows for the oracle and the served side of one case.
//!
//! The rows print on every run. The ceilings assert only under
//! `PROVENANCE_AB_GATE=1`, which the dedicated single-threaded CI job sets;
//! the shared test matrix only prints, so a slow runner cannot fail it.

use std::time::Instant;

/// The served side may take this many times its oracle's median.
const RATIO_CEILING: f64 = 10.0;
/// A served case under this many milliseconds passes whatever the ratio.
const FLOOR_MS: f64 = 50.0;
/// No served case may take longer than this, whatever the ratio.
const ABSOLUTE_MS: f64 = 500.0;
const SCAN_MS: f64 = 5_000.0;
const CATCH_UP_MS: f64 = 2_000.0;

pub fn gated() -> bool {
    std::env::var("PROVENANCE_AB_GATE").is_ok_and(|value| value == "1")
}

/// Timed runs per case after one warm-up run.
pub fn runs() -> usize {
    if gated() {
        9
    } else {
        5
    }
}

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

impl Row {
    fn ratio(&self) -> f64 {
        if self.oracle_ms > 0.0 {
            self.served_ms / self.oracle_ms
        } else {
            f64::INFINITY
        }
    }

    fn within_ceiling(&self) -> bool {
        let allowed = (self.oracle_ms * RATIO_CEILING).max(FLOOR_MS);
        self.served_ms <= allowed && self.served_ms <= ABSOLUTE_MS
    }
}

pub fn print_rows(corpus: &str, rows: &[Row], scan_ms: f64, rebuild_ms: f64, catch_up_ms: f64) {
    println!("A/B timings over {corpus}: operation request oracle_ms served_ms ratio");
    for row in rows {
        println!(
            "{} {} {:.1} {:.1} {:.2}",
            row.operation,
            row.request,
            row.oracle_ms,
            row.served_ms,
            row.ratio()
        );
    }
    let mut operations: Vec<&'static str> = rows.iter().map(|row| row.operation).collect();
    operations.dedup();
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
            if oracle > 0.0 {
                served / oracle
            } else {
                f64::INFINITY
            }
        );
    }
    println!("scan_ms {scan_ms:.1}");
    println!("rebuild_ms {rebuild_ms:.1}");
    println!("catch_up_ms {catch_up_ms:.1}");
}

/// Refuses a case past its ceiling when the gate is set.
pub fn check_ceilings(corpus: &str, rows: &[Row], scan_ms: f64, catch_up_ms: f64) {
    if !gated() {
        return;
    }
    let over: Vec<String> = rows
        .iter()
        .filter(|row| !row.within_ceiling())
        .map(|row| {
            format!(
                "{} {}: served {:.1} ms against oracle {:.1} ms",
                row.operation, row.request, row.served_ms, row.oracle_ms
            )
        })
        .collect();
    assert!(
        over.is_empty(),
        "served cases over their ceiling on {corpus}: {over:?}"
    );
    assert!(
        scan_ms <= SCAN_MS,
        "the scan of {corpus} took {scan_ms:.1} ms, over {SCAN_MS} ms"
    );
    assert!(
        catch_up_ms <= CATCH_UP_MS,
        "catch-up over {corpus} took {catch_up_ms:.1} ms, over {CATCH_UP_MS} ms"
    );
}
