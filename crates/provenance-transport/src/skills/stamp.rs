//! The one line provenance stamps into every skill file it installs, and the
//! reader that takes it back apart.
//!
//! The stamp is the only evidence `legacy_cleanup` has that a file on disk is
//! provenance's to delete, so the writer and the reader have to agree on it
//! down to the byte. They agree by construction here: the three literals
//! below are the whole format, `header` is the only place a stamp is written,
//! and `parse_header` is the only place one is read.

use semver::Version;

/// Everything before the version.
const PREFIX: &str = "<!-- Installed by provenance ";
/// Everything between the version and the hash.
const HASH_PREFIX: &str = "; content hash fnv1a64:";
/// Everything after the hash.
const SUFFIX: &str = " -->";
/// The hash is `fnv1a64` rendered as fixed-width lowercase hex.
const HASH_LENGTH: usize = 16;

/// The stamp for a file whose installed bytes - frontmatter and payload, this
/// line excluded - are `content`.
pub fn header(content: &str) -> String {
    format!(
        "{PREFIX}{}{HASH_PREFIX}{}{SUFFIX}",
        env!("CARGO_PKG_VERSION"),
        fnv1a64(content)
    )
}

#[derive(Debug, PartialEq, Eq)]
pub struct SkillStamp<'a> {
    pub version: Version,
    pub hash: &'a str,
}

pub fn parse_header(header: &str) -> Option<SkillStamp<'_>> {
    let header = header.strip_prefix(PREFIX)?;
    let (version, hash) = header.split_once(HASH_PREFIX)?;
    let hash = hash.strip_suffix(SUFFIX)?;
    let version = Version::parse(version).ok()?;
    (hash.len() == HASH_LENGTH
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(SkillStamp { version, hash })
}

/// The hash a `header` line claims, or `None` when the line is not a stamp:
/// no version, a malformed hash, or anything else in place of the format
/// `header` writes.
pub fn header_hash(header: &str) -> Option<&str> {
    parse_header(header).map(|stamp| stamp.hash)
}

/// Content hash stamped into installed skill files and read back when proving
/// ownership before deleting them.
pub fn fnv1a64(content: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The writer's output is what the reader accepts, and the hash it hands
    /// back is the one the writer put in.
    #[test]
    fn the_reader_takes_back_apart_what_the_writer_wrote() {
        for content in ["", "---\nname: old\n---\npayload\n", "é 日本語\r\n"] {
            let stamp = header(content);
            assert_eq!(header_hash(&stamp), Some(fnv1a64(content).as_str()));
        }
    }

    /// The format is pinned here as well as in the installed files that carry
    /// it, so a change to either literal shows up as a failing test rather
    /// than as skill files the next release refuses to recognise.
    #[test]
    fn the_stamp_keeps_its_published_shape() {
        assert_eq!(
            header("payload\n"),
            format!(
                "<!-- Installed by provenance {}; content hash fnv1a64:acb27c196e1c811d -->",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}
