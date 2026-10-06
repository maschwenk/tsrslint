# Benchmarks: tsrslint vs stock tsgolint

`bench/run.py` lints open-source projects with all 59 rules through the headless protocol, once with stock tsgolint
7.0.2002 and once with tsrslint, and reports wall time, peak memory, the diagnostic count of each and whether they
report the same diagnostics. The Depot CI workflow `.depot/workflows/bench.yml` runs it on every push to `main` and
rewrites the table at the top of `README.md`. Adapted from maschwenk/tsrs's `bench/` (same project pins, checkout
markers and cache layout).

(`benchmarks/` is upstream tsgolint's ESLint-vs-tsgolint benchmark, kept from tsgolint's history; it is not run.)

```sh
cargo build --release
python3 bench/run.py --local                                   # all projects, 3 reps
python3 bench/run.py --local --projects xstate,webpack --reps 1
python3 bench/run.py --local --work-dir ~/bench-cache          # reuse clones across worktrees
python3 bench/run.py --local --readme README.md                # also rewrite the README block
python3 bench/run.py --local --tsrslint path/to/tsgolint --tsrslint-build pgo-dist   # another binary
```

Results go to `bench/results/<date>-<commit>[-local].{json,md}`; per-run stderr to `<work-dir>/logs/<timestamp>/`.
The first run clones and installs the projects into `<work-dir>/solutions` (default `bench/.work`, ~5 GB) and stock
tsgolint into `<work-dir>/stock`; later runs reuse them as long as `bench/projects.json` is unchanged. The checkouts
use the same marker file as maschwenk/tsrs's bench, so a work dir shared with it reuses vscode, mui-docs and webpack
(tsrs names the xstate checkout `xstate-main`; a copy named `xstate` works).

## Projects

`bench/projects.json`: the projects maschwenk/tsrs benchmarks the type checker on, with the same commits, install
commands and overlays: four from [microsoft/typescript-benchmarking](https://github.com/microsoft/typescript-benchmarking)
(minus the two that live inside the suite repository) and four large applications built on Zod or Effect Schema and
tested with Vitest. Linted files: the project's git-tracked files under the listed directories
(`.github/scripts/headless-lint.mjs payload <dir> <lint options>`), never `.d.ts`.

| project | repository @ commit | linted | PGO training |
| --- | --- | --- | --- |
| `vscode` | microsoft/vscode @ `9cf0128b9822` | `src/**/*.{ts,tsx,mts,cts}` (9,401 files) | no |
| `mui-docs` | mui/material-ui @ `6780195595c1` | `docs/**/*.{ts,tsx,mts,cts}` (1,001 files) | no |
| `xstate` | statelyai/xstate @ `fbee62e7c158` | `packages/**/*.{ts,tsx,mts,cts}` (253 files) | yes |
| `webpack` | webpack/webpack @ `b54c5e2aa26d` | `{lib,bin}/**/*.js` (777 files, JSDoc types under `checkJs`) | yes |
| `cal-diy` | calcom/cal.diy @ `54343aa685ae` | `apps/web/**/*.{ts,tsx,mts,cts}` (984 files; Zod 3, tRPC) | no |
| `formbricks-web` | formbricks/formbricks @ `ec45c28e4d20` | `apps/web/**/*.{ts,tsx,mts,cts}` (3,408 files; Zod 4, Vitest) | no |
| `supabase-studio` | supabase/supabase @ `12afe3999951` | `apps/studio/**/*.{ts,tsx,mts,cts}` (4,824 files; Zod 3, Vitest) | no |
| `t3code-server` | pingdotgg/t3code @ `9bd1d8009a6b` | `apps/server/**/*.{ts,tsx,mts,cts}` (1,326 files; Effect) | no |

The application projects' install commands also run the type generation their own `typecheck` needs (Prisma clients,
workspace package declarations, Next.js route types), and cal.diy and Formbricks get an overlay
(`bench/overlays/<name>/`, copied over the checkout before the install) that replaces tsconfig options TypeScript 7
removed. maschwenk/tsrs's `bench/README.md` ("Application projects") documents each step; the overlay files are
identical in both repositories, and their content hash is part of the checkout marker and the CI cache key, so a
shared work dir reuses the checkouts.

PGO training: `.github/scripts/pgo-train.sh` trains the release binaries' profile on exactly these xstate and webpack
files, so their tsrslint numbers are flattered; vscode and mui-docs are held out. `run.py` reads the training set
from `pgo-train.sh` and refuses to run if the two pin different commits or install commands.

## What is measured

- One payload per project, the same for both binaries: version 2, one config with every rule in
  `crates/tsrslint/tests/cases/` (all 59) and default options, as `headless-lint.mjs payload` writes it (the PGO
  training workload uses the same script). Each run is `<binary> headless < payload`, cwd = the project.
- Stock: the native binary of npm `@oxlint-tsgolint/<os>-<arch>@7.0.2002` (what `oxlint-tsgolint@7.0.2002`'s Node
  launcher runs), checked to be that version and a native executable.
- tsrslint: by default `target/release/tsgolint`. CI measures what the npm package ships: the PGO build of the `dist`
  profile, built with the commands of `.github/workflows/release.yml` and trained with `.github/scripts/pgo-train.sh`
  on the bench runner itself (`--tsrslint <binary> --tsrslint-build pgo-dist`; docs/perf-pgo.md). The table header
  names the build.
- Per project: one untimed warm-up run (stock), then N reps (default 3); within a rep the binaries alternate and the
  order flips every rep. Tables report medians.
- Wall: process wall clock. Peak: max RSS from `wait4` rusage. Each run is spawned by a fresh helper interpreter, not
  by the harness: on Linux a child's `ru_maxrss` starts at the peak RSS of the process it was forked from, and the
  harness holds over 1 GiB of decoded diagnostics after vscode (the first CI run reported 1.19 GiB for both binaries
  on xstate and webpack because of it). The JSON also has CPU time (user + sys) per run.
- Diagnostics: the rule diagnostics (`kind` 0) of each run; internal diagnostics (tsconfig/program errors) are counted
  separately and shown when there are any. A run fails on a non-zero exit, an error frame or the timeout (900 s).
- Same diagnostics: the multisets of (file, range, rule, message id) of the two binaries are compared; "no: N
  differ" counts the diagnostics only one of them reports (the JSON has a sample). When the runs of one binary report
  different sets (multi-threaded nondeterminism; seen for stock tsgolint on vscode), each distinct set is kept, the
  closest stock / tsrslint pair is compared, and the cell says by how many diagnostics that binary's runs differ. "text differs" counts diagnostics
  both report whose text (message, labels) differs: tsrs ports TypeScript 7.1-dev, stock tsgolint 7.0.2002 is built
  on TypeScript 7.0, and the two print some types differently (union member order, `typeof import("…")`). Like
  maschwenk/tsrs's bench, which finds tsgo 7.0.2 and 7.1-dev disagreeing on webpack's type errors, a difference
  between 7.0 and 7.1 can also change what a rule reports; a "no" cell is a signal to investigate, not proof of a
  tsrslint bug.
- Per-rule diagnostic counts of each binary are in the JSON; `tools/rule_coverage.py` puts the newest CI run's into
  the README's rule table.

## Regression flag

Wall time on a shared VM moves a few percent between runs with no code change, so it cannot show a small regression.
Each run therefore also counts the user-space instructions of one tsrslint lint per project with a single checker
(`bench/count.py`, Linux `perf_event_open`; untimed, after the timed runs; `TSRSLINT_CHECKERS=1`,
`RAYON_NUM_THREADS=1`). That count repeats to about 0.01% (four runs on the bench machine: 0.006% on webpack, 0.014%
on xstate), so a change of more than 1% comes from the code. Stock tsgolint is not counted: the Go runtime makes its
count vary by a few percent.

`bench/regressions.py --latest` compares the counts with the newest earlier result from the same runner label and
build, and only when the CPU model and C library match. When the Rust compiler differs (results record
`tsrslint.rustc`; `rust-toolchain.toml` pins it), it prints the change as the upgrade's measurement and flags nothing.
A project up more than 1% is a regression: the step prints a warning and comments on the pull request merged in
between, or, when the run covers more than three merges, on the commit. It never fails the job. Results store the count under `projects.<name>.tsrslint.instructions`. `count.py` and
`regressions.py` are copies of maschwenk/tsrs's (`regressions.py` differs in the constants at the top).

The same untimed run records the process's peak RSS (`ru_maxrss` via `wait4`), stored as
`projects.<name>.tsrslint.max_rss_bytes`. With one checker it still moves more than tsrs's (the tsconfig pool and the
file readers run several threads): five identical runs spread 0.65% on mui-docs, 1.37% on webpack and 1.79% (5.7 MiB)
on xstate. Peak memory is therefore flagged when it rises by more than 4% and more than 12 MiB.

## CI

`.depot/workflows/bench.yml` on [Depot CI](https://depot.dev/docs/ci/overview), the mechanism `ci.yml` already uses.
It runs on every push to `main` except pushes that touch only `README.md` / `bench/results/**`, and on
`workflow_dispatch` (`depot ci dispatch --repo maschwenk/tsrslint --workflow bench.yml --ref <branch>`; such a run
commits its results to that branch). One bench at a time, never cancelled (a push during a run queues the newest
commit), network steps retried, and the results commit re-created on the branch's latest head before pushing, with
the bench table and the rule table regenerated there.

**Fixed machine spec**: `depot-ubuntu-24.04-8`, 8 vCPU, 32 GB RAM, Linux x86_64, as in maschwenk/tsrs, so numbers
are comparable over time; the first step fails the job on any other machine. tsrslint uses 6 checkers on it (8 or
more hardware threads), stock tsgolint its default.

Caching (Depot Cache serves the `actions/cache` API): `Swatinem/rust-cache` for `~/.cargo` and the dependency part of
`target/`, which holds the PGO-instrumented build (stable RUSTFLAGS, so the tsrs crates are reused); the final PGO
build lives outside `target/` because its RUSTFLAGS contain the profile's hash, which changes every run. One cache for
stock tsgolint and one per project (checkout + `node_modules`) keyed on its commit and install command
(`bench/run.py --print-cache-keys`), so changing one pin re-installs only that project. The PGO training run reuses
the xstate and webpack checkouts (`run.py` writes the `.pgo-setup` marker `pgo-train.sh` checks).
