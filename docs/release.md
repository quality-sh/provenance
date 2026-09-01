# Release

Build the local binary with:

```sh
cargo build --release -p provenance-cli --features scanner
```

Distribute `target/release/provenance`. Users should commit `.provenance/state/` and ignore `.provenance/cache/`.

## Never `--all-features`

Release builds must enumerate features explicitly. The `dogfood` feature is
dev-only (internal agent feedback capture) and must never ship in a released
binary; `--all-features` would compile it in. CI enforces this by building
with the release feature set and asserting the binary contains no `dogfood`
marker string:

```sh
! grep -q dogfood target/release/provenance
```
