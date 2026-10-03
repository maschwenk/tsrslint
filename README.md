# tsrslint

An exploratory Rust port of [tsgolint](https://github.com/oxc-project/tsgolint) (oxlint's type-aware lint
backend) on top of [tsrs](https://github.com/maschwenk/tsrs), the Rust port of the TypeScript 7 checker.

The binary is named `tsgolint` and speaks tsgolint's headless protocol
([docs/headless-protocol.md](docs/headless-protocol.md)), so oxlint uses it unchanged:

```sh
cargo build --release
OXLINT_TSGOLINT_PATH=$PWD/target/release/tsgolint oxlint --type-aware
```

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

## License and attribution

MIT, like tsgolint: [LICENSE](LICENSE) (Copyright VoidZero Inc. & contributors, typescript-eslint contributors).
The Rust code is a derivative of tsgolint's Go code. tsrs is Apache-2.0 and a derivative of microsoft/TypeScript
(see [THIRD-PARTY-LICENSE](THIRD-PARTY-LICENSE)). Not affiliated with oxc-project, typescript-eslint or Microsoft.
