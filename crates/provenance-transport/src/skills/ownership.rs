pub(super) fn may_replace(existing: &[u8], desired: &[u8]) -> bool {
    if existing == desired {
        return true;
    }
    let Some(existing) = std::str::from_utf8(existing)
        .ok()
        .and_then(crate::legacy_cleanup::managed_skill_stamp)
    else {
        return false;
    };
    std::str::from_utf8(desired)
        .ok()
        .and_then(crate::legacy_cleanup::managed_skill_stamp)
        .is_some_and(|desired| existing.version < desired.version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::stamp::fnv1a64;

    fn installed(version: &str, content: &str) -> Vec<u8> {
        let insertion = content.find("\n---\n").unwrap() + "\n---\n".len();
        let stamp = format!(
            "<!-- Installed by provenance {version}; content hash fnv1a64:{} -->\n",
            fnv1a64(content)
        );
        format!(
            "{}{}{}",
            &content[..insertion],
            stamp,
            &content[insertion..]
        )
        .into_bytes()
    }

    #[test]
    #[provenance_macros::verifies("rule_init_upgrades_hash_owned_skills", examples)]
    fn replacement_requires_an_intact_older_typed_skill_stamp() {
        let current = env!("CARGO_PKG_VERSION");
        let desired = installed(current, "---\nname: skill\n---\ncurrent\n");
        let cases = [
            (installed("0.2.1", "---\nname: skill\n---\nold\n"), true),
            (
                installed("99.0.0", "---\nname: skill\n---\nfuture\n"),
                false,
            ),
            (
                installed(current, "---\nname: skill\n---\ndifferent\n"),
                false,
            ),
            (
                installed("not-semver", "---\nname: skill\n---\nold\n"),
                false,
            ),
            (b"---\nname: skill\n---\nunstamped\n".to_vec(), false),
        ];

        for (existing, expected) in cases {
            assert_eq!(may_replace(&existing, &desired), expected, "{existing:?}");
        }

        let mut edited = installed("0.2.1", "---\nname: skill\n---\nold\n");
        edited.extend_from_slice(b"edited\n");
        assert!(!may_replace(&edited, &desired));
    }
}
