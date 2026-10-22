# npm packaging

Modeled on maschwenk/tsrs's npm packaging (TypeScript 7's scheme):

| package | contents |
| --- | --- |
| `@maschwenk/tsrslint` | `bin/tsrslint` (Node launcher), `bin/tsrslint-path` (prints the binary path), `lib/index.cjs` (`binaryPath()`, `version`, `tsrsCommit`), `optionalDependencies` on every platform package |
| `@maschwenk/tsrslint-<os>-<arch>` | the native binary `tsrslint` (`tsrslint.exe` on Windows) for one platform, with `os`/`cpu` (and `libc: glibc` on Linux) so package managers install only the matching one |

Platforms: `darwin-arm64`, `linux-x64`, `linux-arm64` (glibc), and `win32-x64`. The bin is `tsrslint` rather than
`tsgolint` so the package can sit next to `oxlint-tsgolint` (shadow mode); oxlint is pointed at the native binary
with `OXLINT_TSGOLINT_PATH`. Cargo still names the binary `tsgolint`; `build.mjs` renames it in the platform package.

`binaryPath()` (and the launcher) use `TSRSLINT_BINARY` when set, otherwise resolve
`<name>-<process.platform>-<process.arch>/package.json` from the main package and return `tsrslint` next to it. The
launcher `execve`s the binary on Node >= 22.15 and otherwise spawns it with inherited stdio, forwarding signals and
the exit code.

## Version

One source of truth: `[workspace.package] version` in the workspace `Cargo.toml` (also what `tsgolint --version`
prints). `build.mjs` records the tsrs commit from `Cargo.lock` as `tsrslint.tsrsCommit` in the main package.json.

## Building and testing locally

```sh
cargo build --release
node npm/smoke.mjs target/release/tsgolint              # headless protocol on npm/smoke/fixture
node npm/build.mjs --binary aarch64-apple-darwin=target/release/tsgolint --pack
node npm/smoke-install.mjs npm/dist                    # install the tarballs in a temp project, run bin + API + smoke
```

On Windows, use `target\release\tsgolint.exe` for the binary and `x86_64-pc-windows-msvc` for the target triple.

## Releasing

`.github/workflows/release.yml`: a tag push `v<version>` builds the four platforms (`dist` profile, with PGO on Unix
and without PGO on Windows), smoke-tests each natively, packs, and publishes the platform packages first, then the
main package using npm trusted publishing. Configure the trusted publisher for `@maschwenk/tsrslint-win32-x64`
as for the existing platform packages before publishing the first Windows release.
`gh workflow run release.yml -f dry_run=true` does everything with `npm publish --dry-run`.
