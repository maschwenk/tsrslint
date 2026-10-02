# tsrslint

An exploratory Rust port of [tsgolint](https://github.com/oxc-project/tsgolint) (oxlint's type-aware lint
backend) on top of [tsrs](https://github.com/maschwenk/tsrs), the Rust port of the TypeScript 7 checker.

The binary is named `tsgolint` and speaks tsgolint's headless protocol
([docs/headless-protocol.md](docs/headless-protocol.md)), so oxlint uses it unchanged:

```sh
cargo build --release
OXLINT_TSGOLINT_PATH=$PWD/target/release/tsgolint oxlint --type-aware
```

**Status: vertical slice.** Five of tsgolint's ~60 rules are ported:
`no-floating-promises` (without `allowForKnownSafeCalls` / `allowForKnownSafePromises`), `no-implied-eval`,
`no-unnecessary-boolean-literal-compare`, `promise-function-async` and `return-await`. A payload that enables any
other rule fails with `unknown rule` (tsgolint itself panics on unknown rules). Everything around the rules is
ported: payload V1/V2, file → tsconfig assignment (nearest config, project references, ancestor configs), the
inferred program for unmatched files, tsconfig/program diagnostics, `--type-check` diagnostics, source overrides,
`-fix` / `-fix-suggestions` and `-debug timings`. Go's profiling flags (`-cpuprof` & co.) are accepted and ignored.

[docs/evaluation.md](docs/evaluation.md) has the measurements (profile of stock tsgolint, parity and timing on a
28.5k-file codebase) and the estimate for a full port.

## Layout

* `crates/tsrslint/`: the Rust crate (`src/linter.rs` = headless.go + linter.go, `src/tsconfig.rs` =
  find_tsconfig.go, `src/rule.rs` = rule.go, `src/utils.rs` = the used parts of internal/utils, `src/rules/`).
* `crates/tsrslint/tests/rule_tests.rs`: port of tsgolint's rule tester; runs the cases in `tests/cases/*.json`,
  extracted from tsgolint's Go tests by `tools/extract_rule_tests` (`GOWORK=off go run ./tools/extract_rule_tests
  internal/rules/<rule> crates/tsrslint/tests/cases/<rule-name>.json`).
* Everything else is tsgolint at `v7.0.2002` (this repository starts from tsgolint's history), kept as the porting
  reference and for the fixtures the tests use; tsgolint's README is [README.tsgolint.md](README.tsgolint.md).

## Tests

```sh
cargo test --release --test rule_tests -- --nocapture
```

415 of tsgolint's 452 test cases for the five rules pass; 37 are skipped (36 use the unported
`allowForKnownSafe*` options, 1 is skipped upstream too); none fail.

## License and attribution

MIT, like tsgolint: [LICENSE](LICENSE) (Copyright VoidZero Inc. & contributors, typescript-eslint contributors).
The Rust code is a derivative of tsgolint's Go code. tsrs is Apache-2.0 and a derivative of microsoft/TypeScript
(see [THIRD-PARTY-LICENSE](THIRD-PARTY-LICENSE)). Not affiliated with oxc-project, typescript-eslint or Microsoft.
