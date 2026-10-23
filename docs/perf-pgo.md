# perf-pgo: PGO for the release binaries

Question: is profile-guided optimization worth it for the shipped `tsgolint` binaries, on top of the `dist` profile
(fat LTO, one codegen unit)? Answer: yes. On two codebases outside the training set, all 59 rules enabled, the PGO
binary executes 15-16% fewer instructions and spends 14-15% less user CPU time, with the same diagnostics and the same
peak memory. Release builds use it since 2026-10-03 (`.github/workflows/release.yml`, training script
`.github/scripts/pgo-train.sh`); `cargo build --release` (development) is unchanged.

tsrs's own release binaries are PGO builds (maschwenk/tsrs notes/perf-pgo.md), but tsrslint compiles the tsrs crates
itself (git dependencies), so that profile does not carry over: tsrslint trains its own, with the lint rules in the
workload.

## How the release builds it

1. Instrumented build (`RUSTFLAGS=-Cprofile-generate=…`, separate `CARGO_TARGET_DIR`):
   `cargo test --profile dist -p tsrslint --test rule_tests --no-run`, which builds both the `tsgolint` binary and the
   rule test binary.
2. Training (`.github/scripts/pgo-train.sh`, below), then `llvm-profdata merge` (rustup's `llvm-tools` component;
   `cargo pgo` is not needed).
3. Final build: `cargo build --profile dist -p tsrslint` with `-Cprofile-use=<sha256-prefix>.profdata` (the hash in
   the path makes cargo rebuild whatever rust-cache restored whenever the profile changes).

On macOS the workflow sets `CFLAGS=-fno-profile-generate` / `-fno-profile-use`: cc forwards rustc's PGO flags to
clang (not to gcc), and Apple clang's instrumentation does not have to match rustc's LLVM (tsrs saw the instrumented
mimalloc crash at startup with rustc 1.99). So mimalloc's C code is built without PGO on every platform, as tsrs does.

Windows x64 uses the same instrumented build, training workload and weighted profile merge through Git Bash.
The training script converts profile paths to native Windows paths and enables Git long paths for the training
checkouts. `main` returns an `ExitCode` instead of calling `process::exit`, allowing the Windows CRT to flush LLVM's
profile data. The rule-test harness normalizes its absolute fixture path without `canonicalize`, which would add
the Windows `\\?\` prefix. Empty profiles fail training rather than silently losing a workload.

The Windows pipeline was verified locally with Rust 1.99.0: all 60 rule tests passed in both the instrumented and
profile-use builds. XState (253 files, 6,513 diagnostics) and Webpack (777 files, 34,802 diagnostics) produced
byte-identical sorted diagnostics to the non-PGO `dist` build. This establishes build and output compatibility,
not a Windows performance measurement; the speedup figures below are from macOS.

## Training workload

Public code only (never private code), fast enough for CI (~15 s locally once the projects are installed):

- `tsgolint headless` with all 59 rules (`.github/scripts/headless-lint.mjs`: one V2 payload, default options) on the
  two open-source projects tsrs trains on, at the same commits and with the same install commands: xstate
  (`packages/`, 253 TypeScript files, 6,513 diagnostics from 48 rules) and webpack (`lib/` + `bin/`, 777 JavaScript
  files with JSDoc types under `checkJs`, 34,802 diagnostics from 46 rules). 55 of the 59 rules report something
  (not: no-array-delete, no-mixed-enums, no-unsafe-unary-minus, return-await), and every rule's listeners run on
  every file. This is the real workload: lazy type queries from the rules, multi-threaded programs, the protocol and
  tsconfig resolution of the binary itself.
- The rule test suite (`crates/tsrslint/tests/rule_tests.rs`: 7,341 valid and invalid cases of all 59 rules,
  in-process through `linter::lint_files`), so report paths, options and corner cases the projects never reach are in
  the profile and not laid out as cold code.

The rule test binary is a different executable, but it links the same crate units as `tsgolint`: in one cargo
invocation the tsrslint lib and every tsrs crate are built once, with the same metadata (and so the same symbol
hashes) as in the final `cargo build -p tsrslint`; the test's dev-dependency (`tsrs_parser`) is a normal dependency
of the tsrs crates already, with the same features. Checked:

- `cargo test --profile dist --test rule_tests --no-run` after `cargo build --profile dist -p tsrslint` in the same
  target directory compiles only the test target; `libtsrslint-*.rlib` and every `libtsrs_*` are reused.
- Final build with `-Cllvm-args=-pgo-warn-missing-function` and a `RUSTC_WRAPPER` that drops cargo's `--cap-lints`
  (otherwise rustc's warnings for git dependencies are hidden): **0 hash mismatches**. 2,124 functions of the
  binary's crates have no profile data (1,533 of them in tsrs*/tsrslint, 2 in the binary's own crate); the profile
  has data for 12,744 functions. Functions without data were never executed in training or were inlined before
  instrumentation.
- With a profile from the rule tests alone, 1,726 tsrs*/tsrslint functions lack data (vs 1,533). Had the test's crate
  units differed from the binary's, nearly all ~12,000 would.

Weights: the rule tests' tiny programs add up to ~65% of all counts unweighted (5.8 G of 8.9 G), which made the
profile ~1% worse on vscode than one from the projects alone. `pgo-train.sh` merges the projects' profiles with
weight 8 (rule tests ~19% of the counts); that measured as good as projects alone on both codebases. Weights scale
counts only, so the coverage numbers above hold for the weighted profile.

## Results

Apple M5 Max (18 cores), shared machine; rustc 1.99.0 (the release runners' stable). (a) = `dist` as released in
v0.1.0; (b) = (a) + PGO as above (= the release workflow's commands, without `--target`). Each run is one
`tsgolint headless` process on a fixed payload, timed by `/usr/bin/time -l`; 5 rounds, interleaved (a, b, …).
Medians [min-max]; % vs (a). Neither codebase is in the training set:

- An internal monolith (private; measuring is fine, training is not) with all 59 rules (the stress config of the
  v0.1.0 parity check): 28,728 files, the payload oxlint 1.73.0 sends for it.
- vscode (`src/`, commit 9cf0128b, `npm ci --ignore-scripts`), all 59 rules: 9,401 files.

| codebase | binary | user+sys s | user s | wall s | instructions G | cycles G | peak RSS GiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| monolith, 59 rules | a | 67.34 [63.45-68.57] | 47.26 [43.83-47.58] | 12.92 [12.31-14.19] | 671.1 [668.4-679.4] | 284.2 [267.9-289.6] | 9.76 |
| | b | 58.19 [52.95-61.24] **-13.6%** | 40.05 [37.50-40.81] -15.3% | 11.18 [10.27-11.78] -13.5% | 571.0 [564.3-573.7] **-14.9%** | 246.9 [224.2-258.7] -13.1% | 9.76 |
| vscode, 59 rules | a | 20.86 [20.30-24.23] | 16.73 [16.43-17.01] | 4.38 [4.24-4.74] | 227.3 [226.5-235.7] | 87.9 [85.4-102.4] | 3.34 |
| | b | 19.61 [18.78-20.13] **-6.0%** | 14.30 [13.88-14.86] -14.5% | 3.94 [3.73-4.24] -10.0% | 190.3 [187.9-191.1] **-16.3%** | 82.1 [79.2-84.7] -6.5% | 3.34 |

- Instructions are the stable metric (spread ~1-3%, partly from multi-threaded scheduling); (b)'s maximum
  instruction count is below (a)'s minimum on both codebases. Machine load was 6-9 during these runs.
- The gain is user time. System time (page faults of a 3-10 GB heap) does not change, which is why vscode's
  user+sys moves less than its user time; its cycles also count kernel time.
- An earlier round at load ~27 (7 vscode and 5 monolith rounds, unweighted profile) gave the same picture: -16.6% /
  -14.5% instructions, -11.9% / -11.6% user+sys.
- With one checker (`"checkers": 1` in the tsconfigs, the deterministic setup below): monolith wall 25.6-27.3 s ->
  21.4-23.6 s, vscode 14.1-16.0 s -> 11.3-12.3 s (2 runs each).
- Peak RSS is unchanged.

Training variants (instructions G, medians; round 1 at load ~27, round 2 = the table above):

| profile | vscode r1 | vscode r2 | monolith r1 | monolith r2 |
| --- | --- | --- | --- | --- |
| none (a) | 230.2 | 227.3 | 677.7 | 671.1 |
| rule tests only | 220.2 -4.4% | | 646.3 -4.6% | |
| projects only | 190.3 -17.4% | 189.8 -16.5% | 574.3 -15.3% | 572.6 -14.7% |
| both, unweighted | 192.1 -16.6% | 192.2 -15.4% | 579.8 -14.5% | 570.8 -14.9% |
| both, projects x8 (shipped) | | 190.3 -16.3% | | 571.0 -14.9% |

The rule tests alone are a poor profile (many tiny programs, lib-file setup, rare branches) but cheap insurance for
code real projects rarely reach; the projects carry the gain.

The macOS arm64 binary of the release dry run (Build time, below; the artifact as it would ship), same machine, 5
interleaved rounds against (a): monolith 675.3 -> 568.4 G instructions (**-15.8%**), user+sys -9.4%, wall -15.8%; vscode
230.9 -> 189.8 G (**-17.8%**), user+sys -9.9%, wall -16.7%; peak RSS unchanged. Every run had the same (file, range,
rule, message id) multiset as (a), and 3 of its 5 monolith runs and all 5 vscode runs matched a run of (a) byte for
byte.

Binary size (arm64): a 14.5 MB (12.3 MB stripped), b 16.6 MB (14.4 MB stripped).

## Output is unchanged

- oxlint 1.73.0 on the monolith with the 59-rule stress config: 181,827 diagnostics with (a) and with (b), and the same
  (file, line, column, offset, length, rule, message, severity) for every one (`compare.py`: 0 only in a, 0 only in b).
- Raw headless output, 4 checkers, all timing runs (monolith: 23 of a and 40 PGO runs; vscode: 17 and 46; all training
  variants and the CI-built binary): every run has exactly the same multiset of (file, range, rule, message id):
  195,119 diagnostics on the monolith, 282,719 on vscode. The text of up to 3 (monolith) / 1 (vscode) of them differs from
  run to run, for (a) too: no-unsafe-type-assertion labels that print object-literal unions, whose synthesized
  `?: undefined` members come in a different order (sometimes with `readonly`) depending on which checker thread
  creates those types first. In round 2, two runs of (a) differ in up to 3 diagnostics, runs of different binaries in
  median 2, at most 3, and the same complete output occurs for (a) and PGO binaries (one monolith output came from
  runs of all four binaries). A few PGO runs printed an ordering no run of (a) printed (at most one per binary), at
  the same two diagnostics where (a) itself printed 7 (monolith) and 2 (vscode) orderings, and each is a reordering of
  an (a) text (same characters).
- With deterministic scheduling (`"checkers": 1`), (a) and (b) produce byte-identical sorted output on the monolith and
  vscode (2 runs each).
- The rule test suite, built with the (unweighted) PGO profile, passes (59 rules, 7,341 cases); so does the
  instrumented suite in every release training run.

## Build time

Local (same machine, clean target directories): (a) 59 s. PGO: instrumented build of both binaries 2 min 18 s,
training ~15 s (+2.5 min the first time to clone and install the two projects), final build 1 min 27 s - 1 min 37 s:
~4.2 min, ~3 min more than (a).

On GitHub runners (a release dry run, rustc 1.99.0, nothing cached yet for the new rust-cache key):

| target | job | instrumented build | training | final build | job before (v0.1.0 run, warm / first dry run, cold) |
| --- | --- | --- | --- | --- | --- |
| x86_64-unknown-linux-gnu | 9.7 min | 4.0 min | 1.6 min | 3.2 min | 2.0 / 3.5 min |
| aarch64-unknown-linux-gnu | 8.1 min | 3.0 min | 1.5 min | 2.7 min | 1.8 / 3.0 min |
| aarch64-apple-darwin | 8.6 min | 3.3 min | 1.3 min | 3.2 min | 1.5 / 2.8 min |

So ~5-6 min more per target than a cold non-PGO build (~6-8 min more than a warm one); the jobs run in parallel,
so the release takes ~6 min longer. About 1 min of the training is cloning and installing the two projects
(uncached). A warm rust-cache will not change much: the instrumented build lives outside `target/`, and the profile
(so `RUSTFLAGS`) differs on every run, since multi-threaded training counts are not deterministic, so the final
build recompiles everything. In that run the training reported the same diagnostic counts as locally on all three
targets, the rule tests passed, and the merged profiles have data for 12,744 (macOS) and ~14,800 (Linux) functions.

## Reproducing

```sh
export CARGO_TARGET_DIR=/tmp/pgo-target
RUSTFLAGS=-Cprofile-generate=/tmp/pgo-raw CFLAGS=-fno-profile-generate \
  cargo test --profile dist --locked -p tsrslint --test rule_tests --no-run   # prints the rule_tests path
.github/scripts/pgo-train.sh /tmp/pgo-target/dist/tsgolint /tmp/pgo-target/dist/deps/rule_tests-<hash> \
  /tmp/pgo-work /tmp/pgo.profdata
unset CARGO_TARGET_DIR
RUSTFLAGS=-Cprofile-use=/tmp/pgo.profdata CFLAGS=-fno-profile-use cargo build --profile dist --locked -p tsrslint
# lint a project with all 59 rules, or write the payload for timing runs:
node .github/scripts/headless-lint.mjs run target/dist/tsgolint <git checkout> --out diagnostics.txt
node .github/scripts/headless-lint.mjs payload <git checkout> > payload.json
```

(`CFLAGS` only matters on macOS. The rule tests need `pnpm install` first, and llvm-profdata comes from
`rustup component add llvm-tools`.)
