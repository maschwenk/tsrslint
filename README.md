# tsrslint

An exploratory Rust port of [tsgolint](https://github.com/oxc-project/tsgolint) (oxlint's type-aware lint
backend) on top of [tsrs](https://github.com/maschwenk/tsrs), the Rust port of the TypeScript 7 checker.

It speaks tsgolint's headless protocol ([docs/headless-protocol.md](docs/headless-protocol.md)), so oxlint uses it
unchanged through `OXLINT_TSGOLINT_PATH`.

**Status: complete port.** All 59 of tsgolint's v7.0.2002 rules are ported and pass tsgolint's own test suite (see
[Tests](#tests)). Everything around the rules is ported too: payload V1/V2, file → tsconfig assignment (nearest
config, project references, ancestor configs), the inferred program for unmatched files, tsconfig/program
diagnostics, `--type-check` diagnostics, source overrides, `-fix` / `-fix-suggestions` and `-debug timings`. Go's
profiling flags (`-cpuprof` & co.) are accepted and ignored. tsrs ports TypeScript 7.1.0-dev.20260929, so printed
types can differ from stock tsgolint 7.0.2002 (built on TS 7.0) where TS 7.1 prints them differently (union member
order, `typeof import("…")` specifiers). On an internal monolith with all 59 rules enabled, tsrslint reports the
same 181,827 diagnostics as stock (same rules, files and positions); only some printed types differ, as above.

[docs/evaluation.md](docs/evaluation.md) has the original evaluation of the five-rule slice (profile of stock
tsgolint, parity and timing on a 28.5k-file codebase, full-port estimate).

## Performance

an internal monolith (28,611 linted files), Linux x86_64 (18 vCPU Xeon 8375C, 47 GiB, otherwise idle), PGO
release builds, medians of interleaved runs; every run reports the same diagnostics:

| | stock tsgolint 7.0.2002 | tsrslint 0.1.1 | tsrslint main (2026-10-04) |
| --- | ---: | ---: | ---: |
| payload oxlint sends for the monolith's config (11 rules), wall | 61.7 s | 16.8 s | 10.4 s |
| same, peak RSS | 26.0 GiB | 7.99 GiB | 6.15 GiB |
| all 59 rules, wall | 89.2 s | 25.0 s | 15.1 s |
| all 59 rules, peak RSS | 29.8 GiB | 9.48 GiB | 7.14 GiB |
| `oxlint --type-aware --deny-warnings` end to end, wall | 92.3 s | | 36.2 s |

On an 18-core M5 Max (the monolith's config): 0.1.1 6.6 s / 7.7 GiB, main 4.2 s / 5.9 GiB.

Where the time goes: almost all of it is the type checking the rules ask for, shared between rules (each of the
expensive rules alone costs about 60% of the full run); rule code, the AST walk and the protocol are about 2% with the monolith's config and 6.5% with all
59 rules ([docs/perf-lint-cpu.md](docs/perf-lint-cpu.md)). What makes main faster than 0.1.1: tsrs main instead of an old
branch, linting each file with the checker tsrs's import-graph assignment picks plus tail work stealing
(`src/sched.rs`), 6 checkers instead of 4 on machines with 8 or more hardware threads, and less file-system work
before linting (each tsconfig parsed once; on macOS at most 4 threads read files). Knobs and diagnostics:
`TSRSLINT_CHECKERS=<n>` (4 checkers: about 1 GiB less peak memory on the monolith, about 20% slower),
`TSRSLINT_SCHEDULE=locality`, `TSRSLINT_CHECKER_STATS=1`, `TSRSLINT_MEM_STATS=1`.

## Install and use

From npm (macOS arm64, Linux x64/arm64):

```sh
pnpm add -D @maschwenk/tsrslint
OXLINT_TSGOLINT_PATH="$(pnpm exec tsrslint-path)" pnpm exec oxlint --type-aware
```

The package's bin is `tsrslint` (not `tsgolint`), so it installs next to `oxlint-tsgolint` without replacing it;
oxlint only uses tsrslint when `OXLINT_TSGOLINT_PATH` points at the native binary. Get that path with
`pnpm exec tsrslint-path` or `require("@maschwenk/tsrslint").binaryPath()`. Packaging details:
[npm/README.md](npm/README.md).

From source (Cargo names the binary `tsgolint`):

```sh
cargo build --release
OXLINT_TSGOLINT_PATH=$PWD/target/release/tsgolint oxlint --type-aware
```

## Layout

* `crates/tsrslint/`: the Rust crate (`src/linter.rs` = headless.go + linter.go, `src/tsconfig.rs` =
  find_tsconfig.go, `src/rule.rs` = rule.go, `src/utils/` = internal/utils/*.go with one module per Go file,
  `src/rules/<rule>.rs` = internal/rules/<rule>/); [docs/porting.md](docs/porting.md) has the conventions.
* `crates/tsrslint/tests/rule_tests.rs`: port of tsgolint's rule tester; runs the cases in `tests/cases/*.json`,
  extracted from tsgolint's Go tests by `tools/extract_rule_tests` (`GOWORK=off go run ./tools/extract_rule_tests
  internal/rules/<rule> crates/tsrslint/tests/cases/<rule-name>.json`).
* Everything else is tsgolint at `v7.0.2002` (this repository starts from tsgolint's history), kept as the porting
  reference and for the fixtures the tests use; tsgolint's README is [README.tsgolint.md](README.tsgolint.md).

## Tests

```sh
pnpm install --frozen-lockfile   # @types/node, needed by some no-deprecated cases
cargo test --release --test rule_tests -- --nocapture
```

All 59 rules run every case extracted from tsgolint's Go tests: 7,341 pass, 28 are skipped (exactly the cases
marked `Skip: true` upstream), none fail and none are unconverted.

CI runs on Depot ([.depot/workflows/ci.yml](.depot/workflows/ci.yml): fmt, release build, rule tests, headless and npm
packaging smoke tests). Releases: [.github/workflows/release.yml](.github/workflows/release.yml), see
[npm/README.md](npm/README.md#releasing).

The release binaries are PGO builds: profile `dist` (fat LTO) trained on the rule tests and two open-source projects
with all 59 rules. That is ~15% fewer instructions and less CPU time than the same build without PGO, with the same
diagnostics ([docs/perf-pgo.md](docs/perf-pgo.md)). `cargo build --release` stays the development build.

## License and attribution

MIT, like tsgolint: [LICENSE](LICENSE) (Copyright VoidZero Inc. & contributors, typescript-eslint contributors).
The Rust code is a derivative of tsgolint's Go code. tsrs is Apache-2.0 and a derivative of microsoft/TypeScript
(see [THIRD-PARTY-LICENSE](THIRD-PARTY-LICENSE)). Not affiliated with oxc-project, typescript-eslint or Microsoft.
