//! The review row findings of `provenance check`.

use std::process::Output;

/// Each finding of the review row section of `output`, in output order. The
/// section is the line that ends with `review row finding(s):` and the
/// `- ` lines after it. The first section of the output counts. With `json`,
/// standard error is one `operation_failed` object whose message holds the
/// section; else standard error is text.
pub fn review_row_findings(output: &Output, json: bool) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stderr = if json {
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        let refusal: serde_json::Value = serde_json::from_str(&stderr)
            .unwrap_or_else(|error| panic!("{error}: standard error is not one object: {stderr}"));
        assert_eq!(refusal["error"]["kind"], "operation_failed", "{refusal}");
        assert!(refusal["meta"].is_object(), "{refusal}");
        refusal["error"]["message"].as_str().unwrap().to_owned()
    } else {
        stderr
    };
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), stderr);
    text.lines()
        .skip_while(|line| !line.trim_end().ends_with("review row finding(s):"))
        .skip(1)
        .map_while(|line| line.trim_start().strip_prefix("- "))
        .map(str::to_owned)
        .collect()
}
