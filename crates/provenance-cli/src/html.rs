//! Shared HTML text and attribute escaping.

pub(crate) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn escape_attr(text: &str) -> String {
    escape_html(text).replace('"', "&quot;")
}
