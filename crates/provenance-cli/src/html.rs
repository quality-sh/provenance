//! Shared HTML text and attribute escaping.

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn escape_attr(text: &str) -> String {
    escape_html(text).replace('"', "&quot;")
}
