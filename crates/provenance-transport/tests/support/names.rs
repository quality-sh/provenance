//! Generated values, named in the order they first show, so two runs whose
//! record ids, digests and times differ can compare their answers.

use std::collections::BTreeMap;

#[derive(Default)]
pub struct Names {
    known: BTreeMap<String, String>,
}

impl Names {
    pub fn name(&mut self, text: &str) -> String {
        if !generated(text) {
            return text.to_owned();
        }
        let next = self.known.len() + 1;
        self.known
            .entry(text.to_owned())
            .or_insert_with(|| format!("<generated {next}>"))
            .clone()
    }
}

/// A UUID with or without a word before it, a hex digest, a page cursor or
/// a time stamp. A page cursor is two base64url parts joined by a dot, and
/// its second part is a signature of 32 bytes.
pub fn generated(text: &str) -> bool {
    let hex = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_hexdigit());
    let base64url = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    let last = text.rsplit('_').next().unwrap_or(text);
    let uuid = last.len() == 36 && last.split('-').map(str::len).eq([8, 4, 4, 4, 12]);
    let digest = text.len() >= 40 && hex(text.trim_start_matches("sha256:"));
    let cursor = text.split_once('.').is_some_and(|(body, signature)| {
        base64url(body) && signature.len() == 43 && base64url(signature)
    });
    let time = text.len() >= 20
        && text.as_bytes()[4] == b'-'
        && text.as_bytes()[10] == b'T'
        && text.bytes().take(4).all(|b| b.is_ascii_digit());
    uuid || digest || cursor || time
}
