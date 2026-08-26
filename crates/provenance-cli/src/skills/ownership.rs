pub(super) fn may_replace(existing: &[u8], desired: &[u8]) -> bool {
    existing == desired
        || std::str::from_utf8(existing).is_ok_and(crate::legacy_cleanup::valid_managed_skill)
}
