//! The review row findings of `provenance check`.

use std::process::Output;

/// Each finding of the review row section of `output`, in output order. The
/// section is the line that ends with `review row finding(s):` and the
/// `- ` lines after it. The first section of the output counts. When standard
/// error is one refusal object, its message holds the section.
pub fn review_row_findings(output: &Output) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let refusal = serde_json::from_str::<serde_json::Value>(&stderr)
        .ok()
        .and_then(|refusal| refusal["error"]["message"].as_str().map(str::to_owned));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        refusal.unwrap_or(stderr)
    );
    text.lines()
        .skip_while(|line| !line.trim_end().ends_with("review row finding(s):"))
        .skip(1)
        .map_while(|line| line.trim_start().strip_prefix("- "))
        .map(str::to_owned)
        .collect()
}
