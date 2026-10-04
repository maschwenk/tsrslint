#!/usr/bin/env python3
"""Writes or checks the rule coverage table in README.md (between the rules markers).

    python3 tools/rule_coverage.py --write README.md     # regenerate the table
    python3 tools/rule_coverage.py --check README.md     # exit 1 if the table is stale (CI)
    python3 tools/rule_coverage.py --update-upstream <tsgolint checkout> <ref>   # re-pin tools/upstream-rules.json

Inputs, all in this repository (no network): the rules tsgolint v7.0.2002 has (internal/rules/<rule>/, kept from
tsgolint's history), the ported rules (crates/tsrslint/src/rules/mod.rs), their test cases
(crates/tsrslint/tests/cases/<rule>.json, extracted from tsgolint's Go tests), the newest CI benchmark result
(bench/results/*.json, written by .depot/workflows/bench.yml) for the diagnostics column, and the rule list of a
pinned upstream tsgolint ref (tools/upstream-rules.json) for rules added after v7.0.2002.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CASES = REPO / "crates" / "tsrslint" / "tests" / "cases"
RULES_RS = REPO / "crates" / "tsrslint" / "src" / "rules" / "mod.rs"
GO_RULES = REPO / "internal" / "rules"
UPSTREAM = REPO / "tools" / "upstream-rules.json"
RESULTS = REPO / "bench" / "results"
START, END = "<!-- rules:start -->", "<!-- rules:end -->"
BASE_TAG = "v7.0.2002"


def go_rule_dirs(names: list[str]) -> list[str]:
    """Rule names from tsgolint's internal/rules/<snake_case>/ directories (fixtures is not a rule)."""
    return sorted(n.replace("_", "-") for n in names if n != "fixtures" and "." not in n)


def case_counts(path: Path) -> dict:
    data = json.loads(path.read_text())
    cases = [c for g in data["groups"] for c in g["valid"] + g["invalid"]]
    return {"cases": len(cases), "skipped": sum(1 for c in cases if c.get("skip")),
            "only": sum(1 for c in cases if c.get("only")), "unconverted": len(data.get("unconverted", []))}


def ported_rules() -> set[str]:
    text = RULES_RS.read_text()
    names = re.search(r"pub const RULE_NAMES: &\[&str\] = &\[(.*?)\];", text, re.S)
    if not names:
        sys.exit(f"no RULE_NAMES in {RULES_RS}")
    stubs = set(re.findall(r'"([a-z-]+)"\s*=>\s*Err\(not_ported', text))
    return set(re.findall(r'"([a-z-]+)"', names.group(1))) - stubs


def bench_counts() -> tuple[dict, dict | None]:
    """Per-rule diagnostics of the newest CI bench run: rule -> {tsrslint, stock, projects (with any)}, and the run."""
    newest = None
    for f in sorted(RESULTS.glob("*.json")):
        r = json.loads(f.read_text())
        if r.get("machine", {}).get("ci") and (newest is None or r["date"] > newest["date"]):
            newest = r
    if newest is None:
        return {}, None
    counts: dict = {}
    for pr in newest["projects"].values():
        for c in ("tsrslint", "stock"):
            for rule, n in pr.get(c, {}).get("per_rule", {}).items():
                e = counts.setdefault(rule, {"tsrslint": 0, "stock": 0, "projects": 0})
                e[c] += n
                if c == "tsrslint" and n:
                    e["projects"] += 1
    return counts, newest


def table() -> str:
    base = go_rule_dirs([p.name for p in GO_RULES.iterdir() if p.is_dir()])
    case_files = sorted(p.stem for p in CASES.glob("*.json"))
    ported = ported_rules()
    upstream = json.loads(UPSTREAM.read_text())
    bench, run = bench_counts()
    rules = sorted(set(base) | set(case_files) | ported)
    n_bench_projects = len(run["projects"]) if run else 0

    head = "| rule | ported | test cases | passed | skipped |" + (" bench diagnostics |" if bench else "")
    lines = [head, "| --- | --- | ---: | ---: | ---: |" + (" ---: |" if bench else "")]
    tot = {"cases": 0, "passed": 0, "skipped": 0}
    problems = []
    for rule in rules:
        c = case_counts(CASES / f"{rule}.json") if rule in case_files else None
        if rule not in base:
            problems.append(f"{rule}: not a tsgolint {BASE_TAG} rule")
        if c is None:
            problems.append(f"{rule}: no test cases")
        elif c["unconverted"] or c["only"]:
            problems.append(f"{rule}: {c['unconverted']} unconverted, {c['only']} `only` cases")
        cases, skipped = (c["cases"], c["skipped"]) if c else (0, 0)
        passed = cases - skipped if rule in ported else 0
        tot["cases"] += cases
        tot["passed"] += passed
        tot["skipped"] += skipped
        row = f"| `{rule}` | {'yes' if rule in ported else '**no**'} | {cases:,} | {passed:,} | {skipped} |"
        if bench:
            b = bench.get(rule)
            if not b or not b["tsrslint"]:
                cell = "0"
            else:
                cell = f"{b['tsrslint']:,} ({b['projects']}/{n_bench_projects})"
            if b and b["stock"] != b["tsrslint"]:
                cell += f", stock {b['stock']:,}"
            row += f" {cell} |"
        lines.append(row)
    n_ported = sum(1 for r in base if r in ported)
    missing = [r for r in upstream["rules"] if r not in ported]
    out = [
        f"{n_ported} of tsgolint {BASE_TAG}'s {len(base)} rules are ported. Their test cases are tsgolint's own Go "
        f"tests, extracted to `crates/tsrslint/tests/cases/<rule>.json`: {tot['passed']:,} pass and "
        f"{tot['skipped']} are skipped (the cases upstream marks `Skip: true`) of {tot['cases']:,}. CI's rule-test "
        "step fails on any failing case, so passed = cases - skipped on every green commit.",
        "",
        *lines,
    ]
    if run:
        out += ["", f"bench diagnostics: what tsrslint reports for the rule (default options) in the newest CI "
                    f"benchmark run ({run['date']}, commit `{run['tsrslint']['commit'][:12]}`), summed over its "
                    f"projects ({', '.join(run['projects'])}), and in how many of them it fires; \", stock N\" where "
                    f"stock tsgolint {BASE_TAG} reports a different number. 0: those projects never trigger the "
                    "rule (its test cases still run)."]
    added = upstream.get("added_after_base", {})
    changed = upstream.get("changed_after_base", {})
    out += ["", f"Upstream: oxc-project/tsgolint at `{upstream['ref'][:12]}` ({upstream['date']}, "
                f"{upstream['describe']}) has {len(upstream['rules'])} rules. "
            + ("Not ported yet (added after " + BASE_TAG + "): "
               + ", ".join(f"`{r}`" + (f" ({added[r]})" if r in added else "") for r in missing) + "."
               if missing else f"All of them are ported.")
            + (f" Commits after {BASE_TAG} also change {len(changed)} existing rules (tsrslint ports their {BASE_TAG} "
               f"code; commits per rule): " + ", ".join(f"`{r}` {n}" for r, n in changed.items()) + "." if changed else "")]
    if problems:
        sys.exit("rule coverage: " + "; ".join(problems))
    return "\n".join(out) + "\n"


def splice(text: str, block: str) -> str:
    if START not in text or END not in text:
        sys.exit(f"README has no {START} ... {END} markers")
    pre, rest = text.split(START, 1)
    _, post = rest.split(END, 1)
    return f"{pre}{START}\n{block}{END}{post}"


def update_upstream(checkout: Path, ref: str) -> None:
    git = lambda *a: subprocess.run(["git", "-C", str(checkout), *a], capture_output=True, text=True,
                                    check=True).stdout
    sha = git("rev-parse", f"{ref}^{{commit}}").strip()
    rules = go_rule_dirs(git("ls-tree", "--name-only", f"{sha}:internal/rules").split())
    base = set(go_rule_dirs(git("ls-tree", "--name-only", f"{BASE_TAG}:internal/rules").split()))
    tags = git("tag", "--list", "v*", "--sort=v:refname").split()
    added = {}
    for r in sorted(set(rules) - base):
        d = f"internal/rules/{r.replace('-', '_')}"
        added[r] = next((f"since {t}" for t in tags
                         if subprocess.run(["git", "-C", str(checkout), "cat-file", "-e", f"{t}:{d}"],
                                           capture_output=True).returncode == 0), "not in a release yet")
    # Commits after the base tag that change an existing rule's directory (fixes the port does not have yet).
    changed: dict = {}
    log = git("log", "--format=%x00%h", "--name-only", f"{BASE_TAG}..{sha}", "--", "internal/rules")
    for entry in log.split("\0")[1:]:
        _, *files = entry.split("\n")
        for r in {f.split("/")[2].replace("_", "-") for f in files if f.count("/") >= 3}:
            if r in base:
                changed[r] = changed.get(r, 0) + 1
    data = {
        "repository": "https://github.com/oxc-project/tsgolint",
        "ref": sha,
        "date": git("show", "-s", "--format=%cs", sha).strip(),
        "describe": git("describe", "--tags", sha).strip(),
        "rules": rules,
        "added_after_base": added,
        "changed_after_base": dict(sorted(changed.items())),
    }
    UPSTREAM.write_text(json.dumps(data, indent=2) + "\n")
    print(f"wrote {UPSTREAM.relative_to(REPO)}: {len(rules)} rules at {sha[:12]}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--write", type=Path, metavar="README")
    g.add_argument("--check", type=Path, metavar="README")
    g.add_argument("--print", action="store_true")
    g.add_argument("--update-upstream", nargs=2, metavar=("CHECKOUT", "REF"))
    args = ap.parse_args()
    if args.update_upstream:
        update_upstream(Path(args.update_upstream[0]), args.update_upstream[1])
        return
    block = table()
    if args.print:
        print(block, end="")
    elif args.write:
        args.write.write_text(splice(args.write.read_text(), block))
    else:
        text = args.check.read_text()
        if splice(text, block) != text:
            sys.exit(f"{args.check}: the rule table is stale; run `python3 tools/rule_coverage.py --write {args.check}`")
        print(f"{args.check}: rule table is up to date")


if __name__ == "__main__":
    main()
