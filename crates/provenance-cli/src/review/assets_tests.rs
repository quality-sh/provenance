use super::content_type;

#[test]
fn a_known_extension_gets_its_media_type() {
    for (path, expected) in [
        ("/index.html", "text/html; charset=utf-8"),
        ("/assets/app.mjs", "text/javascript; charset=utf-8"),
        ("/assets/app.js.map", "application/json"),
        ("/assets/font.woff2", "font/woff2"),
        ("/assets/logo.jpeg", "image/jpeg"),
    ] {
        assert_eq!(content_type(path), expected, "{path}");
    }
}

#[test]
fn an_unknown_or_missing_extension_is_served_as_bytes() {
    assert_eq!(content_type("/assets/data.bin"), "application/octet-stream");
    assert_eq!(content_type("/LICENSE"), "application/octet-stream");
}
