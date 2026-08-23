# `@quality-sh/create-provenance`

Install Provenance as a development dependency and initialize a TypeScript
project:

```sh
npx --yes @quality-sh/create-provenance@latest
```

The initializer detects npm, pnpm, Yarn, Bun, Deno, or Nub from the
`packageManager` field or the project lockfile. Use an explicit selection when
a project contains lockfiles from more than one manager:

```sh
npx --yes @quality-sh/create-provenance@latest --package-manager bun
```

Use `--path <path>` to initialize a project outside the current directory. The
initializer installs the matching `@quality-sh/provenance` release, creates the
default scope, validates the new state, and adds `.provenance/cache/` to
`.gitignore`.
