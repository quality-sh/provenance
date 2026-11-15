//! The review row findings of `provenance check`.

use std::process::Output;

/// Each finding of the review row section of `output`, in output order. The
/// section is the line that ends with `review row finding(s):` and the
/// `- ` lines after it. The first section of the output counts.
pub fn review_row_findings(output: &Output) -> Vec<String> {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines()
        .skip_while(|line| !line.trim_end().ends_with("review row finding(s):"))
        .skip(1)
        .map_while(|line| line.trim_start().strip_prefix("- "))
        .map(str::to_owned)
        .collect()
}
