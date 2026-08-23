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

Deno blocks packages that are newer than its minimum dependency age. The
initializer disables that age check for the Deno add operation so a new
Provenance release can install immediately. For Nub, it approves the new
Provenance package name and excludes only the Provenance package family from
the release age check. Other Nub dependencies keep the project's age policy.
