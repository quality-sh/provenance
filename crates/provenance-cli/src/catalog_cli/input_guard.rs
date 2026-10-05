/// Refuses a malformed review edit guard and names the required `ETag` form.
#[provenance_macros::rule("rule_cli_guard_guidance")]
pub(super) fn validate_review_etag(value: &str) -> anyhow::Result<()> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    anyhow::ensure!(
        valid,
        "--if-match must equal data.edit.etag from the latest record read; \
         expected sha256:<64 lowercase hexadecimal characters>"
    );
    Ok(())
}
