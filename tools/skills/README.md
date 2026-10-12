# Bundled skill sources

Each bundled skill has one tracked source: `skills/<name>/SKILL.md`.
The CLI build reads those files in a workspace. It embeds their bytes through
Cargo's output directory. An extracted Cargo package reads its crate-local
`skills/` files instead.

Before `cargo package` or `cargo publish`, run this command from the repository
root:

```sh
node tools/skills/prepare.mjs
```

The command copies all four root sources into `crates/provenance-cli/skills/`.
It replaces stale bytes. These files have no installation stamp and must not
be tracked. Cargo includes them in the package. The build helper cannot add
archive members because Cargo collects package files before it runs the helper.

`node tools/operation-codegen/ensure-generated.mjs` also runs this preparation.
The release and package workflows run it before they collect package files.

`provenance init` installs the bundled content in `.agents/skills/` and creates
Claude links or copies. The bundled installed directories are ignored. Other
skills in these directories are not affected.

Run `node --test tools/skills/*.test.mjs` to check package preparation. The
`cli_skills_single_source` process tests also check source edits, rebuilds,
external installation, an extracted package, and Git status after init.
