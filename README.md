# tsrslint

An exploratory Rust port of [tsgolint](https://github.com/oxc-project/tsgolint) (oxlint's type-aware lint
backend) on top of [tsrs](https://github.com/maschwenk/tsrs), the Rust port of the TypeScript 7 checker.

It speaks tsgolint's headless protocol ([docs/headless-protocol.md](docs/headless-protocol.md)), so oxlint uses it
unchanged through `OXLINT_TSGOLINT_PATH`.

**Status: complete port.** All 59 of tsgolint's v7.0.2002 rules are ported and pass tsgolint's own test suite (see
[Rule coverage](#rule-coverage)). Everything around the rules is ported too: payload V1/V2, file → tsconfig assignment (nearest
config, project references, ancestor configs), the inferred program for unmatched files, tsconfig/program
diagnostics, `--type-check` diagnostics, source overrides, `-fix` / `-fix-suggestions` and `-debug timings`. Go's
profiling flags (`-cpuprof` & co.) are accepted and ignored. tsrs ports TypeScript 7.1.0-dev.20260929, so printed
types can differ from stock tsgolint 7.0.2002 (built on TS 7.0) where TS 7.1 prints them differently (union member
order, `typeof import("…")` specifiers). On an internal monolith with all 59 rules enabled, tsrslint reports the
same 181,827 diagnostics as stock (same rules, files and positions); only some printed types differ, as above.

[docs/evaluation.md](docs/evaluation.md) has the original evaluation of the five-rule slice (profile of stock
tsgolint, parity and timing on a 28.5k-file codebase, full-port estimate).

<!-- bench:start -->
## Benchmark: tsrslint vs tsgolint 7.0.2002

Each row lints one open-source project with all 59 rules (default options) through tsgolint's headless protocol, the way `oxlint --type-aware` runs its backend, with stock tsgolint 7.0.2002 (npm `@oxlint-tsgolint/linux-x64@7.0.2002`) and with tsrslint at commit `8805a2a3a347` (the PGO `dist` build, built like the npm release binaries). Both get the same payload; median of 3 interleaved runs, on Depot CI `depot-ubuntu-24.04-8` (8 vCPU, 31 GB RAM, Linux x86_64, AMD EPYC 9R45 96-Core Processor).

| project | in PGO training | files | diagnostics, stock / tsrslint | same diagnostics | text differs | stock wall (s) | tsrslint wall (s) | speedup | stock peak RSS | tsrslint peak RSS | memory, tsrslint / stock |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| vscode | no | 9,401 | 282,717–282,719 / 282,719 | yes; stock's runs differ by 2 | 179 | 14.34 | 3.36 | 4.27x | 9.45 GiB | 2.67 GiB | 0.28x |
| mui-docs | no | 1,004 | 3,607 / 3,607 | yes | 10 | 10.67 | 1.92 | 5.56x | 8.32 GiB | 1.78 GiB | 0.21x |
| xstate | yes | 253 | 6,513 / 6,513 | yes | 13 | 0.91 | 0.32 | 2.86x | 913 MiB | 638 MiB | 0.70x |
| webpack | yes | 777 | 34,799 / 34,802 | **no: 3 differ** | 16 | 1.00 | 0.27 | 3.73x | 1001 MiB | 553 MiB | 0.55x |

in PGO training: the release binaries' profile is trained on these files (`.github/scripts/pgo-train.sh`), which flatters tsrslint there; the other projects are held out. diagnostics: rule diagnostics each binary reports. same diagnostics: both report the same multiset of (file, range, rule, message id); a bold cell is a disagreement. When one binary's runs report different sets (multi-threaded nondeterminism), the closest pair is compared and the cell says by how many diagnostics that binary's runs differ. text differs: diagnostics both report whose message text differs; expected in small numbers, since tsrs ports TypeScript 7.1-dev and tsgolint 7.0.2002 is built on TypeScript 7.0, which print some types differently. wall: process wall-clock time. speedup: stock wall / tsrslint wall. peak RSS: maximum resident set size; memory below 1x = tsrslint uses less.

Runner: Depot CI `depot-ubuntu-24.04-8` (8 vCPU, 31 GB RAM, Linux x86_64, AMD EPYC 9R45 96-Core Processor). Date: 2026-10-05 22:20 UTC. tsrslint commit: `8805a2a3a347`. Numbers from shared CI machines are noisy; compare trends, not single runs. How it is measured: [`bench/README.md`](bench/README.md).
<!-- bench:end -->

### An internal monolith (private, measured by hand)

The CI benchmark above runs on public projects. The monolith is private, so it is not in CI; these numbers
were measured by hand on 2026-10-04. 28,611 linted files, Linux x86_64 (18 vCPU Xeon 8375C, 47 GiB, otherwise idle),
PGO release builds, medians of interleaved runs; every run reports the same diagnostics:

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

All 59 rules run every case extracted from tsgolint's Go tests; per-rule counts are in
[Rule coverage](#rule-coverage).

CI runs on Depot ([.depot/workflows/ci.yml](.depot/workflows/ci.yml): fmt, release build, rule tests, headless and npm
packaging smoke tests, a check that the rule coverage table below is current, and the lint ratchet of
[docs/rust-practices.md](docs/rust-practices.md)). The benchmark at the top runs in
its own Depot workflow ([.depot/workflows/bench.yml](.depot/workflows/bench.yml), [bench/README.md](bench/README.md)). Releases: [.github/workflows/release.yml](.github/workflows/release.yml), see
[npm/README.md](npm/README.md#releasing).

The release binaries are PGO builds: profile `dist` (fat LTO) trained on the rule tests and two open-source projects
with all 59 rules. That is ~15% fewer instructions and less CPU time than the same build without PGO, with the same
diagnostics ([docs/perf-pgo.md](docs/perf-pgo.md)). `cargo build --release` stays the development build.

## Rule coverage

Generated by `python3 tools/rule_coverage.py --write README.md`; CI fails when it is stale.

<!-- rules:start -->
59 of tsgolint v7.0.2002's 59 rules are ported. Their test cases are tsgolint's own Go tests, extracted to `crates/tsrslint/tests/cases/<rule>.json`: 7,341 pass and 28 are skipped (the cases upstream marks `Skip: true`) of 7,369. CI's rule-test step fails on any failing case, so passed = cases - skipped on every green commit.

| rule | ported | test cases | passed | skipped | bench diagnostics |
| --- | --- | ---: | ---: | ---: | ---: |
| `await-thenable` | yes | 126 | 126 | 0 | 309 (3/4) |
| `consistent-return` | yes | 31 | 31 | 0 | 1,421 (3/4) |
| `consistent-type-exports` | yes | 49 | 49 | 0 | 22 (2/4) |
| `dot-notation` | yes | 75 | 75 | 0 | 3,127 (2/4) |
| `no-array-delete` | yes | 31 | 31 | 0 | 7 (1/4) |
| `no-base-to-string` | yes | 381 | 380 | 1 | 241 (4/4) |
| `no-confusing-void-expression` | yes | 110 | 109 | 1 | 11,709 (4/4) |
| `no-deprecated` | yes | 235 | 223 | 12 | 2,510 (4/4) |
| `no-duplicate-type-constituents` | yes | 82 | 80 | 2 | 199 (4/4) |
| `no-floating-promises` | yes | 179 | 178 | 1 | 6,893 (4/4) |
| `no-for-in-array` | yes | 24 | 24 | 0 | 7 (2/4) |
| `no-implied-eval` | yes | 70 | 70 | 0 | 6 (3/4) |
| `no-meaningless-void-operator` | yes | 6 | 6 | 0 | 11 (2/4) |
| `no-misused-promises` | yes | 222 | 221 | 1 | 2,067 (4/4) |
| `no-misused-spread` | yes | 131 | 130 | 1 | 84 (3/4) |
| `no-mixed-enums` | yes | 51 | 51 | 0 | 3 (1/4) |
| `no-redundant-type-constituents` | yes | 104 | 104 | 0 | 589 (3/4) |
| `no-unnecessary-boolean-literal-compare` | yes | 50 | 50 | 0 | 556 (3/4) |
| `no-unnecessary-condition` | yes | 350 | 350 | 0 | 11,121 (4/4) |
| `no-unnecessary-qualifier` | yes | 18 | 18 | 0 | 65 (2/4) |
| `no-unnecessary-template-expression` | yes | 183 | 183 | 0 | 171 (3/4) |
| `no-unnecessary-type-arguments` | yes | 111 | 110 | 1 | 313 (4/4) |
| `no-unnecessary-type-assertion` | yes | 366 | 366 | 0 | 4,050 (4/4) |
| `no-unnecessary-type-conversion` | yes | 77 | 77 | 0 | 583 (4/4) |
| `no-unnecessary-type-parameters` | yes | 165 | 165 | 0 | 392 (3/4) |
| `no-unsafe-argument` | yes | 43 | 43 | 0 | 3,628 (4/4) |
| `no-unsafe-assignment` | yes | 95 | 94 | 1 | 7,789 (4/4) |
| `no-unsafe-call` | yes | 36 | 36 | 0 | 2,718 (4/4) |
| `no-unsafe-enum-comparison` | yes | 93 | 93 | 0 | 1,752 (3/4) |
| `no-unsafe-member-access` | yes | 36 | 36 | 0 | 9,706 (4/4) |
| `no-unsafe-return` | yes | 62 | 62 | 0 | 1,385 (4/4) |
| `no-unsafe-type-assertion` | yes | 101 | 101 | 0 | 18,665 (4/4), stock 18,663 |
| `no-unsafe-unary-minus` | yes | 23 | 23 | 0 | 0 |
| `no-useless-default-assignment` | yes | 83 | 83 | 0 | 113 (4/4) |
| `non-nullable-type-assertion-style` | yes | 26 | 26 | 0 | 1,876 (4/4), stock 1,873 |
| `only-throw-error` | yes | 91 | 91 | 0 | 72 (2/4) |
| `prefer-find` | yes | 47 | 47 | 0 | 66 (2/4) |
| `prefer-includes` | yes | 65 | 65 | 0 | 408 (3/4) |
| `prefer-nullish-coalescing` | yes | 186 | 184 | 2 | 5,160 (4/4) |
| `prefer-optional-chain` | yes | 1,170 | 1,170 | 0 | 1,725 (4/4) |
| `prefer-promise-reject-errors` | yes | 162 | 161 | 1 | 192 (4/4) |
| `prefer-readonly` | yes | 162 | 162 | 0 | 3,373 (2/4) |
| `prefer-readonly-parameter-types` | yes | 135 | 135 | 0 | 160,481 (4/4) |
| `prefer-reduce-type-parameter` | yes | 31 | 31 | 0 | 24 (2/4) |
| `prefer-regexp-exec` | yes | 36 | 36 | 0 | 308 (4/4) |
| `prefer-return-this-type` | yes | 22 | 22 | 0 | 63 (2/4) |
| `prefer-string-starts-ends-with` | yes | 132 | 132 | 0 | 253 (3/4) |
| `promise-function-async` | yes | 53 | 53 | 0 | 11,383 (4/4) |
| `related-getter-setter-pairs` | yes | 23 | 23 | 0 | 12 (2/4) |
| `require-array-sort-compare` | yes | 43 | 43 | 0 | 47 (3/4) |
| `require-await` | yes | 90 | 90 | 0 | 9,568 (4/4) |
| `restrict-plus-operands` | yes | 122 | 122 | 0 | 53 (2/4) |
| `restrict-template-expressions` | yes | 72 | 72 | 0 | 723 (4/4) |
| `return-await` | yes | 100 | 100 | 0 | 292 (1/4) |
| `strict-boolean-expressions` | yes | 218 | 218 | 0 | 31,962 (4/4) |
| `strict-void-return` | yes | 215 | 214 | 1 | 4,935 (4/4) |
| `switch-exhaustiveness-check` | yes | 107 | 107 | 0 | 723 (4/4) |
| `unbound-method` | yes | 205 | 203 | 2 | 982 (3/4) |
| `use-unknown-in-catch-callback-variable` | yes | 57 | 56 | 1 | 748 (4/4) |

bench diagnostics: what tsrslint reports for the rule (default options) in the newest CI benchmark run (2026-10-05 22:20 UTC, commit `8805a2a3a347`), summed over its projects (vscode, mui-docs, xstate, webpack), and in how many of them it fires; ", stock N" where stock tsgolint v7.0.2002 reports a different number. 0: those projects never trigger the rule (its test cases still run).

Upstream: oxc-project/tsgolint at `f69fe736c071` (2026-10-02, v7.0.2003-5-gf69fe73) has 60 rules. Not ported yet (added after v7.0.2002): `no-generated-empty-object-type` (since v7.0.2003). Commits after v7.0.2002 also change 16 existing rules (tsrslint ports their v7.0.2002 code; commits per rule): `await-thenable` 1, `no-deprecated` 1, `no-duplicate-type-constituents` 1, `no-meaningless-void-operator` 1, `no-misused-promises` 2, `no-misused-spread` 1, `no-unnecessary-condition` 3, `no-unnecessary-type-assertion` 3, `no-unnecessary-type-parameters` 1, `no-unsafe-return` 1, `no-useless-default-assignment` 2, `only-throw-error` 1, `require-array-sort-compare` 1, `return-await` 1, `strict-void-return` 1, `unbound-method` 1.
<!-- rules:end -->

## License and attribution

MIT, like tsgolint: [LICENSE](LICENSE) (Copyright VoidZero Inc. & contributors, typescript-eslint contributors).
The Rust code is a derivative of tsgolint's Go code. tsrs is Apache-2.0 and a derivative of microsoft/TypeScript
(see [THIRD-PARTY-LICENSE](THIRD-PARTY-LICENSE)). Not affiliated with oxc-project, typescript-eslint or Microsoft.
