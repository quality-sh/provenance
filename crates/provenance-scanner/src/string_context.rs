/// A marker is hidden only when a quote region opens before it and is still
/// open at the marker: the region must close somewhere after the marker, so
/// a lone unmatched quote earlier in a comment line never hides a directive.
///
/// Double quotes and backticks both open hiding regions in every language: a
/// paired span quoting a directive is quotation, not declaration, whether it
/// is a Go raw string or a doc-comment code span. Single quotes never open a
/// hiding region. A pair that closes before the marker is skipped as a
/// char-literal-like span (quotes inside it stay inert); an apostrophe whose
/// mate sits past the marker is prose, as in contractions straddling the
/// directive.
pub const fn marker_is_inside_quoted_region(text: &str, marker: usize) -> bool {
    let bytes = text.as_bytes();
    let mut quote = None;
    let mut index = 0;
    loop {
        if index >= marker {
            return quote.is_some();
        }
        if index >= bytes.len() {
            return false;
        }
        if let Some(active_quote) = quote {
            match bytes[index] {
                b'\\' if active_quote != b'`' => index += 2,
                byte if byte == active_quote => {
                    quote = None;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }
        let candidate = bytes[index];
        if candidate == b'\'' {
            match quote_end(bytes, index + 1, candidate) {
                Some(end) if end < marker => index = end + 1,
                _ => index += 1,
            }
            continue;
        }
        if (candidate == b'"' || candidate == b'`')
            && quote_end(bytes, index + 1, candidate).is_some()
        {
            quote = Some(candidate);
        }
        index += 1;
    }
}

const fn quote_end(bytes: &[u8], mut index: usize, quote: u8) -> Option<usize> {
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if quote != b'`' => index += 2,
            byte if byte == quote => return Some(index),
            _ => index += 1,
        }
    }
    None
}
