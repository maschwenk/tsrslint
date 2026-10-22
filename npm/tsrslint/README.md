# @maschwenk/tsrslint

A Rust port of [tsgolint](https://github.com/oxc-project/tsgolint), oxlint's type-aware lint backend, built on
[tsrs](https://github.com/maschwenk/tsrs) (a Rust port of the TypeScript 7 checker). It implements all 59 tsgolint
rules and speaks tsgolint's headless protocol, so oxlint runs it in place of `oxlint-tsgolint`.

```sh
pnpm add -D @maschwenk/tsrslint
OXLINT_TSGOLINT_PATH="$(pnpm exec tsrslint-path)" pnpm exec oxlint --type-aware
```

The bin is `tsrslint`, not `tsgolint`, so it can be installed next to `oxlint-tsgolint` (whose bin oxlint finds
by default) and selected explicitly through `OXLINT_TSGOLINT_PATH`. Point that variable at the native binary, not at
the Node launcher in `node_modules/.bin`:

* shell: `pnpm exec tsrslint-path` prints the binary's absolute path;
* Node: `require("@maschwenk/tsrslint").binaryPath()` returns it (also exports `version` and `tsrsCommit`).

`tsrslint ...` itself execs the native binary with the same arguments and stdio (`tsrslint --version`,
`tsrslint headless < payload.json`).

Supported platforms: macOS arm64, Linux x64/arm64 (glibc), Windows x64. The binary comes from the optional dependency
`@maschwenk/tsrslint-<os>-<arch>`, installed automatically for the current platform. To use a locally built binary
through the launcher and `binaryPath()`, set `TSRSLINT_BINARY=/path/to/binary`.

MIT, like tsgolint (see `LICENSE`); tsrs is Apache-2.0 (see `THIRD-PARTY-LICENSE`). Not affiliated with
oxc-project, typescript-eslint or Microsoft.
