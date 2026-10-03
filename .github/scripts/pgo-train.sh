#!/usr/bin/env bash
# PGO training workload for the release binaries (.github/workflows/release.yml; method and numbers:
# docs/perf-pgo.md). Runs PGO-instrumented binaries and merges their raw profiles into <out.profdata>.
#
#   pgo-train.sh <instrumented tsgolint> <instrumented rule_tests> <work-dir> <out.profdata>
#   pgo-train.sh --setup-only <work-dir>          # only clone + install the projects (also for benchmarking)
#
# Workload (public code only; never train on private code):
#   - tsgolint headless with all 59 rules (.github/scripts/headless-lint.mjs) on two open-source projects, the ones
#     maschwenk/tsrs trains on: xstate (TypeScript monorepo) and webpack (JavaScript with JSDoc types, checkJs).
#     Real code with real dependencies: the hot paths of the checker and the rules, and the binary's own code
#     (protocol, tsconfig resolution, multi-threaded programs). Cloning and installing them needs network.
#   - rule_tests, the rule test suite (crates/tsrslint/tests/rule_tests.rs): every case of all 59 rules through
#     linter::lint_files, in-process, so rule paths the two projects never take are in the profile too. It links the
#     very same crate units as the tsgolint binary (cargo builds the tsrslint lib and the tsrs crates once for both,
#     with the same metadata hashes), so its counts apply to them. Needs the repository's node_modules (pnpm install:
#     @types/node for some no-deprecated cases).
# The projects' profiles are merged with weight 8: unweighted, the suite's 7,341 tiny programs are ~65% of all
# counts, and the profile then measured ~1% worse on vscode than one from the projects alone; weighted, it is as
# good as the projects alone and still covers the suite's paths (docs/perf-pgo.md).
# llvm-profdata comes from rustup's llvm-tools component (override: LLVM_PROFDATA). The script fails if a run fails
# or crashes, or writes no profile.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

# name|repo|commit|install command (commits and install commands as in maschwenk/tsrs bench/projects.json)
projects=(
  "xstate|https://github.com/statelyai/xstate.git|fbee62e7c1586315ed478c2fedf530d7e0ff5a3e|PUPPETEER_SKIP_CHROMIUM_DOWNLOAD=true npx --yes \$(node -p 'require(\"./package.json\").packageManager') install"
  "webpack|https://github.com/webpack/webpack.git|b54c5e2aa26dd0267bc904d45402f811f100186c|npx --yes yarn@1.22.22 install --ignore-scripts --ignore-engines && npx --yes yarn@1.22.22 link --link-folder \"\$PWD/node_modules/.yarn-link\" && npx --yes yarn@1.22.22 link --link-folder \"\$PWD/node_modules/.yarn-link\" webpack"
)

setup() { # $1 = work dir; shallow clone of each project's commit + install, skipped when already done
  local name repo commit install dest i
  for p in "${projects[@]}"; do
    IFS='|' read -r name repo commit install <<< "$p"
    dest="$1/$name"
    if [ "$(cat "$dest/.pgo-setup" 2>/dev/null)" = "$commit $install" ]; then
      echo "$name: already set up at ${commit:0:12}"
      continue
    fi
    for i in 1 2 3; do # network steps get a few attempts
      rm -rf "$dest" && mkdir -p "$dest"
      if git -C "$dest" init -q && git -C "$dest" fetch -q --depth 1 "$repo" "$commit" \
        && git -C "$dest" -c advice.detachedHead=false checkout -q FETCH_HEAD \
        && (cd "$dest" && CI=true HUSKY=0 bash -c "$install"); then
        echo "$commit $install" > "$dest/.pgo-setup"
        break
      fi
      [ "$i" = 3 ] && { echo "::error::setting up $name failed"; exit 1; }
      echo "$name: attempt $i failed, retrying"; sleep $((10 * i))
    done
  done
}

if [ "${1:-}" = --setup-only ]; then
  mkdir -p "$2" && setup "$(cd "$2" && pwd)"
  exit 0
fi

[ $# = 4 ] || { echo "usage: pgo-train.sh <tsgolint> <rule_tests> <work-dir> <out.profdata>"; exit 2; }
tsgolint=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
rule_tests=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
mkdir -p "$3"
work=$(cd "$3" && pwd)
out=$4
profdata=${LLVM_PROFDATA:-$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata}
raw="$work/raw"
rm -rf "$raw" && mkdir -p "$raw"

setup "$work"

time LLVM_PROFILE_FILE="$raw/rule-tests-%p.profraw" "$rule_tests" --quiet

time LLVM_PROFILE_FILE="$raw/xstate-%p.profraw" node "$here/headless-lint.mjs" run "$tsgolint" "$work/xstate" \
  --include packages
time LLVM_PROFILE_FILE="$raw/webpack-%p.profraw" node "$here/headless-lint.mjs" run "$tsgolint" "$work/webpack" \
  --ext js --include lib --include bin

ls -l "$raw"
for f in rule-tests xstate webpack; do
  compgen -G "$raw/$f-*.profraw" > /dev/null || { echo "::error::no profile from $f"; exit 1; }
done

weighted=()
for f in "$raw"/xstate-*.profraw "$raw"/webpack-*.profraw; do weighted+=("--weighted-input=8,$f"); done
"$profdata" merge -o "$out" "$raw"/rule-tests-*.profraw "${weighted[@]}"
"$profdata" show "$out" | tail -5
