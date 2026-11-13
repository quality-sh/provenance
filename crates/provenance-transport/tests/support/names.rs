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

/// A UUID with or without a word before it, a hex digest, a signed value
/// or a time stamp.
pub fn generated(text: &str) -> bool {
    let hex = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_hexdigit());
    let last = text.rsplit('_').next().unwrap_or(text);
    let uuid = last.len() == 36 && last.split('-').map(str::len).eq([8, 4, 4, 4, 12]);
    let digest = text.len() >= 40 && hex(text.trim_start_matches("sha256:"));
    let time = text.len() >= 20
        && text.as_bytes()[4] == b'-'
        && text.as_bytes()[10] == b'T'
        && text.bytes().take(4).all(|b| b.is_ascii_digit());
    uuid || digest || time
}
