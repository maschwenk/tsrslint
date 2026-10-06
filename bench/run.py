#!/usr/bin/env python3
"""Benchmarks tsrslint against stock tsgolint 7.0.2002 on open-source projects, all 59 rules, headless protocol.

See bench/README.md. Typical use:

    python3 bench/run.py --local                                  # every project in bench/projects.json
    python3 bench/run.py --local --projects xstate,webpack --reps 1
    python3 bench/run.py --setup-only                             # clone + install only (CI cache warm-up)

Adapted from maschwenk/tsrs bench/run.py (same project pins, checkout markers and cache keys).
"""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import struct
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BENCH = REPO / "bench"
HEADLESS = REPO / ".github" / "scripts" / "headless-lint.mjs"
PGO_TRAIN = REPO / ".github" / "scripts" / "pgo-train.sh"
# Same marker file and content as maschwenk/tsrs's bench, so a work dir shared with it reuses its checkouts.
MARKER = ".tsrs-bench.json"
# pgo-train.sh skips cloning a project whose <dir>/.pgo-setup reads "<commit> <install>".
PGO_MARKER = ".pgo-setup"
START, END = "<!-- bench:start -->", "<!-- bench:end -->"
# --tsrslint-build: how the measured tsrslint binary was built, for the table header.
TSRSLINT_BUILDS = {
    "release": "`cargo build --release`, not the PGO build that ships",
    "pgo-dist": "the PGO `dist` build, built like the npm release binaries",
}


def log(msg: str) -> None:
    print(f"[bench {time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def sh(cmd: str | list[str], cwd: Path | None = None, env: dict | None = None) -> None:
    shell = isinstance(cmd, str)
    log(f"$ {cmd if shell else ' '.join(cmd)}" + (f"   (in {cwd})" if cwd else ""))
    subprocess.run(cmd, cwd=cwd, shell=shell, check=True, env=env, executable="/bin/bash" if shell else None)


def retry(what: str, fn, attempts: int = 3) -> None:
    """Network steps (clone, package installs) get a few attempts: the CI job runs unattended."""
    for i in range(1, attempts + 1):
        try:
            return fn()
        except subprocess.CalledProcessError as e:
            if i == attempts:
                raise
            log(f"{what}: attempt {i} failed ({e}); retrying in {10 * i} s")
            time.sleep(10 * i)


def git_checkout(repo: str, commit: str, dest: Path) -> None:
    """Shallow checkout of exactly one commit (GitHub serves fetches by SHA)."""
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    sh(["git", "init", "-q", str(dest)])
    sh(["git", "-C", str(dest), "fetch", "-q", "--depth", "1", repo, commit])
    sh(["git", "-C", str(dest), "-c", "advice.detachedHead=false", "checkout", "-q", "FETCH_HEAD"])


def read_marker(d: Path) -> dict | None:
    try:
        return json.loads((d / MARKER).read_text())
    except (OSError, ValueError):
        return None


def pgo_training_projects() -> dict[str, dict]:
    """The projects .github/scripts/pgo-train.sh trains the release profile on: name -> {repo, commit, install}."""
    block = re.search(r"^projects=\(\n(.*?)^\)", PGO_TRAIN.read_text(), re.M | re.S)
    if not block:
        sys.exit(f"no projects=( ... ) array in {PGO_TRAIN}")
    out = {}
    for line in block.group(1).splitlines():
        line = line.strip()
        if line.startswith('"') and line.endswith('"'):
            # One bash double-quoted string; `read -r` splits it on the first three `|`.
            name, repo, commit, install = line[1:-1].split("|", 3)
            out[name] = {"repo": repo, "commit": commit, "install": install.replace('\\"', '"').replace("\\$", "$")}
    if not out:
        sys.exit(f"no projects parsed from {PGO_TRAIN}")
    return out


def overlay_files(p: dict) -> list[tuple[str, bytes]]:
    """The files of a project's `overlay` directory (bench/overlays/<name>), as (relative path, content)."""
    if "overlay" not in p:
        return []
    root = REPO / p["overlay"]
    files = sorted((f.relative_to(root).as_posix(), f.read_bytes()) for f in root.rglob("*") if f.is_file())
    if not files:
        sys.exit(f"{p['name']}: overlay {p['overlay']} has no files")
    return files


def overlay_digest(p: dict) -> str | None:
    """Content hash of a project's overlay, computed like maschwenk/tsrs's bench so the two share checkout markers."""
    files = overlay_files(p)
    if not files:
        return None
    h = hashlib.sha256()
    for rel, data in files:
        h.update(rel.encode() + b"\0" + hashlib.sha256(data).digest())
    return h.hexdigest()[:16]


def ensure_checkout(p: dict, dest: Path, pgo: bool) -> None:
    want = {"repo": p["repo"], "commit": p["commit"], "install": p["install"]}
    overlay = overlay_files(p)
    if overlay:
        want["overlay"] = overlay_digest(p)
    if read_marker(dest) == want:
        log(f"{p['name']}: cached checkout {p['commit'][:12]}")
    else:
        log(f"{p['name']}: setting up {p['repo']} @ {p['commit'][:12]}")
        t0 = time.perf_counter()

        def attempt() -> None:
            git_checkout(p["repo"], p["commit"], dest)
            # Overlay files replace or add checkout files before the install (bench/README.md "Projects").
            for rel, data in overlay:
                (dest / rel).parent.mkdir(parents=True, exist_ok=True)
                (dest / rel).write_bytes(data)
            sh(p["install"], cwd=dest, env=dict(os.environ, CI="true", HUSKY="0"))

        retry(p["name"], attempt)
        (dest / MARKER).write_text(json.dumps(want))
        log(f"{p['name']}: setup took {time.perf_counter() - t0:.0f} s")
    if pgo:
        # Same checkout as pgo-train.sh's setup() makes (checked in main()), so the training run reuses it.
        pgo_marker = f"{p['commit']} {p['install']}\n"
        if (dest / PGO_MARKER).exists() and (dest / PGO_MARKER).read_text() == pgo_marker:
            return
        (dest / PGO_MARKER).write_text(pgo_marker)


def npm_platform() -> str:
    arch = {"x86_64": "x64", "amd64": "x64", "arm64": "arm64", "aarch64": "arm64"}[platform.machine().lower()]
    return f"{'darwin' if sys.platform == 'darwin' else 'linux'}-{arch}"


def ensure_stock(cfg: dict, work: Path) -> Path:
    """Installs the native tsgolint of npm oxlint-tsgolint@<version> and returns it.

    oxlint-tsgolint's bin is a Node launcher; oxlint itself runs the platform package's binary
    (@oxlint-tsgolint/<os>-<arch>/tsgolint), and so does the harness.
    """
    pkg, version = cfg["package"].format(platform=npm_platform()), cfg["version"]
    d = work / "stock" / version
    pkg_dir = d / "node_modules" / pkg
    exe = pkg_dir / "tsgolint"
    if not exe.exists():
        d.mkdir(parents=True, exist_ok=True)
        (d / "package.json").write_text('{"name": "tsgolint-stock-bench", "private": true}\n')
        retry(f"{pkg}@{version}", lambda: sh(["npm", "install", "--no-audit", "--no-fund", "--no-save", f"{pkg}@{version}"], cwd=d))
    installed = json.loads((pkg_dir / "package.json").read_text()).get("version")
    if installed != version:
        sys.exit(f"{pkg_dir} is version {installed}, expected {version}")
    with open(exe, "rb") as f:
        magic = f.read(4)
    if magic not in (b"\x7fELF", b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe"):
        sys.exit(f"{exe} is not a native executable (magic {magic!r})")
    return exe


def make_payload(p: dict, cwd: Path, work: Path) -> tuple[Path, int, int]:
    """The payload both binaries get: headless-lint.mjs's (all 59 rules, default options, one config)."""
    out = work / "payloads" / f"{p['name']}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "wb") as f:
        subprocess.run(["node", str(HEADLESS), "payload", str(cwd), *p.get("lint", [])], stdout=f, check=True)
    payload = json.loads(out.read_text())
    return out, len(payload["configs"][0]["file_paths"]), len(payload["configs"][0]["rules"])


def decode(path: Path) -> tuple[list[dict], str | None]:
    """Framed stdout (docs/headless-protocol.md): | u32 LE length | u8 type | JSON |; type 0 = error, 1 = diagnostic."""
    b = path.read_bytes()
    diags, error, i = [], None, 0
    while i < len(b):
        if i + 5 > len(b):
            return diags, f"truncated frame header at byte {i}"
        (n,) = struct.unpack_from("<I", b, i)
        t = b[i + 4]
        if i + 5 + n > len(b):
            return diags, f"truncated frame at byte {i}"
        body = json.loads(b[i + 5:i + 5 + n])
        if t == 0:
            error = body.get("error", "error frame")
        elif t == 1:
            diags.append(body)
        i += 5 + n
    return diags, error


def diag_key(d: dict) -> tuple:
    r = d.get("range") or {}
    return (d.get("file_path"), r.get("pos"), r.get("end"), d.get("rule"), (d.get("message") or {}).get("id"))


def diag_full(d: dict) -> str:
    d = {k: v for k, v in d.items() if v not in (None, [], "")}
    return json.dumps(d, sort_keys=True)


# Spawns, times and reaps one measured process, in a fresh interpreter. On Linux a child's ru_maxrss starts from the
# high-water RSS of the process it was forked from (fork copies the memory map, exec records its peak): spawned from
# this harness, which holds the decoded diagnostics of earlier runs (over 1 GiB after vscode), every run would report
# at least the harness's peak. The helper's own peak is ~10 MB.
MEASURE = r"""
import json, os, subprocess, sys, threading, time
argv, cwd, stdin, stdout, stderr, timeout = json.loads(sys.argv[1])
timed_out = []
with open(stdin, "rb") as i, open(stdout, "wb") as o, open(stderr, "wb") as e:
    t0 = time.perf_counter()
    p = subprocess.Popen(argv, cwd=cwd, stdin=i, stdout=o, stderr=e)
    timer = threading.Timer(timeout, lambda: (timed_out.append(1), p.kill()))
    timer.start()
    _, status, ru = os.wait4(p.pid, 0)
    wall = time.perf_counter() - t0
    timer.cancel()
print(json.dumps({"exit": os.waitstatus_to_exitcode(status), "wall_s": wall, "cpu_s": ru.ru_utime + ru.ru_stime,
                  "maxrss": ru.ru_maxrss, "timed_out": bool(timed_out)}))
"""


def run_once(exe: Path, cwd: Path, payload: Path, logs: Path, tag: str, timeout: float) -> dict:
    out_path, err_path = logs / f"{tag}.out", logs / f"{tag}.stderr"
    logs.mkdir(parents=True, exist_ok=True)
    spec = json.dumps([[str(exe), "headless"], str(cwd), str(payload), str(out_path), str(err_path), timeout])
    m = json.loads(subprocess.run([sys.executable, "-c", MEASURE, spec], capture_output=True, text=True,
                                  check=True).stdout)
    # ru_maxrss: KiB on Linux, bytes on macOS (the number `/usr/bin/time -v` / `-l` print).
    peak = m["maxrss"] * (1 if sys.platform == "darwin" else 1024)
    r: dict = {"exit": m["exit"], "wall_s": round(m["wall_s"], 3), "cpu_s": round(m["cpu_s"], 2),
               "peak_rss_bytes": peak, "timed_out": m["timed_out"]}
    diags, error = decode(out_path)
    out_path.unlink()  # up to ~100 MB per run; the decoded summary below is what is kept
    rule = [d for d in diags if d.get("kind") == 0]
    r["diagnostics"] = len(rule)
    r["internal"] = len(diags) - len(rule)
    r["error"] = error
    r["per_rule"] = dict(sorted(collections.Counter(d.get("rule") for d in rule).items()))
    keys = collections.Counter(diag_key(d) for d in diags)
    r["keys_sha"] = hashlib.sha256(repr(sorted(keys.elements())).encode()).hexdigest()[:16]
    r["ok"] = r["exit"] == 0 and error is None and not r["timed_out"]
    r["_keys"] = keys
    r["_full"] = collections.Counter(diag_full(d) for d in diags)
    if r["internal"]:
        r["internal_sample"] = [d for d in diags if d.get("kind") != 0][:5]
    return r


def count_instructions(exe: Path, cwd: Path, payload: Path, logs: Path, name: str, timeout: float) -> dict | None:
    """User-space instructions and peak RSS of one single-checker lint (bench/count.py), or None where it cannot
    count.

    Untimed and separate from the timed runs. One checker (TSRSLINT_CHECKERS=1) and one rayon thread make the count
    reproducible, so one run is enough (bench/README.md "Regression flag")."""
    if sys.platform != "linux":
        return None
    out = logs / f"{name}-instructions.json"
    logs.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, TSRSLINT_CHECKERS="1", RAYON_NUM_THREADS="1")
    with open(payload, "rb") as stdin, open(logs / f"{name}-instructions.stderr", "wb") as stderr:
        subprocess.run([sys.executable, str(BENCH / "count.py"), str(out), "--", str(exe), "headless"], cwd=cwd,
                       stdin=stdin, stdout=subprocess.DEVNULL, stderr=stderr, env=env, timeout=timeout)
    r = json.loads(out.read_text())
    if r.get("exit") != 0 or r.get("instructions") is None:
        return None
    return {"instructions": r["instructions"], "max_rss_bytes": r["max_rss_bytes"]}


def median(xs: list) -> float | None:
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def summarize(runs: list[dict]) -> dict:
    ok = [r for r in runs if r["ok"]]
    s: dict = {"runs": len(runs), "ok_runs": len(ok)}
    for k in ("wall_s", "cpu_s", "peak_rss_bytes", "diagnostics", "internal"):
        s[k] = median([r.get(k) for r in ok])
    s["diagnostic_counts"] = sorted({r["diagnostics"] for r in ok})
    # Diagnostics come from several checker threads; the (file, range, rule, message id) set must not vary.
    s["stable_keys"] = len({r["keys_sha"] for r in ok}) <= 1
    s["per_rule"] = ok[0]["per_rule"] if ok else {}
    if ok and ok[0].get("internal_sample"):
        s["internal_sample"] = ok[0]["internal_sample"]
    failed = [r for r in runs if not r["ok"]]
    if failed:
        s["failure"] = {k: failed[0].get(k) for k in ("exit", "error", "timed_out")}
    return s


def compare(stock: dict, tsrs: dict, cwd: Path) -> dict:
    """Parity of one stock run and one tsrslint run: key multisets, then the full diagnostic text."""
    only_stock = stock["_keys"] - tsrs["_keys"]
    only_tsrs = tsrs["_keys"] - stock["_keys"]
    differ = sum(only_stock.values()) + sum(only_tsrs.values())
    # Diagnostics whose key both report but whose text (message, labels; printed types) differs.
    text_differs = max(0, sum((tsrs["_full"] - stock["_full"]).values()) - sum(only_tsrs.values()))
    rel = lambda f: os.path.relpath(f, cwd) if isinstance(f, str) else f
    sample = lambda c: [[rel(k[0]), *k[1:]] for k in sorted(c.elements(), key=repr)[:10]]
    return {"same_keys": differ == 0, "keys_differ": differ, "text_differs": text_differs,
            "stock_only_sample": sample(only_stock), "tsrslint_only_sample": sample(only_tsrs)}


def rustc_version() -> str | None:
    """`rustc -V` in the checkout: the toolchain rust-toolchain.toml pins, which the bench workflow builds with."""
    try:
        return subprocess.run(["rustc", "-V"], capture_output=True, text=True, cwd=REPO, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def machine_info(local: bool, label: str | None) -> dict:
    info = {"platform": f"{platform.system()} {platform.machine()}", "cpus": os.cpu_count()}
    if sys.platform == "darwin":
        q = lambda k: subprocess.run(["sysctl", "-n", k], capture_output=True, text=True).stdout.strip()
        info["cpu"] = q("machdep.cpu.brand_string")
        info["memory_gb"] = round(int(q("hw.memsize")) / 2**30)
    else:
        try:
            info["cpu"] = next(l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith("model name"))
        except (OSError, StopIteration):
            info["cpu"] = platform.processor()
        try:
            kb = next(int(l.split()[1]) for l in open("/proc/meminfo") if l.startswith("MemTotal"))
            info["memory_gb"] = round(kb / 2**20)
        except (OSError, StopIteration):
            pass
        libc, version = platform.libc_ver()
        if libc:
            info["libc"] = f"{libc} {version}"
    try:
        info["load_avg_1m"] = round(os.getloadavg()[0], 1)
    except OSError:
        pass
    if label:
        info["label"] = label
    elif os.environ.get("GITHUB_ACTIONS") == "true" and not local:
        info["ci"] = True
        runner = os.environ.get("BENCH_RUNNER") or os.environ.get("RUNNER_NAME", "hosted runner")
        info["label"] = (f"{runner} ({info['cpus']} vCPU, {info.get('memory_gb')} GB RAM, {platform.system()} "
                         f"{platform.machine()}, {info['cpu']})")
    else:
        info["label"] = f"local machine ({info['cpu']}, {info['cpus']} cores, {info.get('memory_gb')} GB)"
    return info


def fmt_mem(b: float | None) -> str:
    if b is None:
        return "n/a"
    return f"{b / 2**30:.2f} GiB" if b >= 2**30 else f"{b / 2**20:.0f} MiB"


def fmt_num(x: float | None) -> str:
    return "n/a" if x is None else f"{x:.2f}"


def fmt_ratio(x: float | None) -> str:
    return "n/a" if x is None else f"{x:.2f}x"


def fmt_int(x: float | None) -> str:
    return "n/a" if x is None else f"{int(x):,}"


def markdown(result: dict) -> str:
    m = result["machine"]
    sv = result["stock"]["version"]
    commit = result["tsrslint"]["commit"][:12]
    n_rules = result["rules"]
    lines = [
        f"## Benchmark: tsrslint vs tsgolint {sv}",
        "",
        f"Each row lints one open-source project with all {n_rules} rules (default options) through tsgolint's "
        f"headless protocol, the way `oxlint --type-aware` runs its backend, with stock tsgolint {sv} (npm "
        f"`{result['stock']['package']}@{sv}`) and with tsrslint at commit `{commit}` "
        f"({TSRSLINT_BUILDS[result['tsrslint']['build']]}). Both get the same payload; median of {result['reps']} "
        f"interleaved runs, on {m['label']}.",
        "",
        "| project | in PGO training | files | diagnostics, stock / tsrslint | same diagnostics | text differs | "
        "stock wall (s) | tsrslint wall (s) | speedup | stock peak RSS | tsrslint peak RSS | memory, tsrslint / stock |",
        "| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for name, pr in result["projects"].items():
        g, t = pr["stock"], pr["tsrslint"]
        head = f"| {name} | {'yes' if pr['pgo_training'] else 'no'} | {pr['files']:,} |"
        if not g["ok_runs"] or not t["ok_runs"]:
            failed = [c for c, s in (("stock", g), ("tsrslint", t)) if not s["ok_runs"]]
            lines.append(f"{head} **FAILED: {', '.join(failed)}** | | | | | | | | |")
            continue
        counts = " / ".join("–".join(f"{c:,}" for c in s["diagnostic_counts"][::max(1, len(s["diagnostic_counts"]) - 1)])
                            for s in (g, t))
        internal = [f"{c} {int(s['internal'])}" for c, s in (("stock", g), ("tsrslint", t)) if s["internal"]]
        if internal:
            counts += f" (internal: {', '.join(internal)})"
        par = pr["parity"]
        same = "yes" if par["same_keys"] else f"**no: {par['keys_differ']:,} differ**"
        for c, s in (("stock", g), ("tsrslint", t)):
            if "varies" in s:
                same += f"; {c}'s runs differ by {s['varies']['keys_differ']:,}"
        speed = g["wall_s"] / t["wall_s"] if g["wall_s"] and t["wall_s"] else None
        mem = t["peak_rss_bytes"] / g["peak_rss_bytes"] if g["peak_rss_bytes"] and t["peak_rss_bytes"] else None
        lines.append(f"{head} {counts} | {same} | {par['text_differs']:,} | {fmt_num(g['wall_s'])} | "
                     f"{fmt_num(t['wall_s'])} | {fmt_ratio(speed)} | {fmt_mem(g['peak_rss_bytes'])} | "
                     f"{fmt_mem(t['peak_rss_bytes'])} | {fmt_ratio(mem)} |")
    lines += [
        "",
        "in PGO training: the release binaries' profile is trained on these files (`.github/scripts/pgo-train.sh`), "
        "which flatters tsrslint there; the other projects are held out. diagnostics: rule diagnostics each binary "
        "reports. same diagnostics: both report the same multiset of (file, range, rule, message id); a bold cell is "
        "a disagreement. When one binary's runs report different sets (multi-threaded nondeterminism), the closest pair is compared and the cell says by how many diagnostics that binary's runs differ. text differs: diagnostics both report whose message text differs; expected in small numbers, "
        "since tsrs ports TypeScript 7.1-dev and tsgolint 7.0.2002 is built on TypeScript 7.0, which print some types "
        "differently. wall: process wall-clock time. speedup: stock wall / tsrslint wall. peak RSS: maximum resident "
        "set size; memory below 1x = tsrslint uses less.",
        "",
        f"Runner: {m['label']}. Date: {result['date']}. tsrslint commit: `{commit}`. "
        + ("Numbers from shared CI machines are noisy; compare trends, not single runs. " if m.get("ci") else "")
        + "How it is measured: [`bench/README.md`](bench/README.md).",
    ]
    return "\n".join(lines).rstrip() + "\n"


def update_readme(readme: Path, table: str) -> None:
    text = readme.read_text()
    block = f"{START}\n{table}{END}"
    if START in text and END in text:
        pre, rest = text.split(START, 1)
        _, post = rest.split(END, 1)
        text = pre + block + post
    else:
        text = f"{block}\n\n{text}"
    readme.write_text(text)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--local", action="store_true", help="label results as measured on this machine")
    ap.add_argument("--label", help="machine label for the table header")
    ap.add_argument("--work-dir", type=Path, default=BENCH / ".work", help="clones, installs, logs (default bench/.work)")
    ap.add_argument("--tsrslint", type=Path, default=REPO / "target" / "release" / "tsgolint")
    ap.add_argument("--tsrslint-build", choices=sorted(TSRSLINT_BUILDS), default="release",
                    help="how --tsrslint was built (named in the table header; CI: pgo-dist, see .depot/workflows/bench.yml)")
    ap.add_argument("--stock", type=Path, help="stock tsgolint binary (default: install it from npm)")
    ap.add_argument("--projects", help="comma-separated subset of bench/projects.json")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--timeout", type=float, default=900, help="per-run timeout in seconds")
    ap.add_argument("--no-warmup", action="store_true")
    ap.add_argument("--no-instructions", action="store_true",
                    help="skip the untimed instruction-count run of tsrslint (Linux only; bench/regressions.py compares it)")
    ap.add_argument("--setup-only", action="store_true", help="clone and install the projects and stock tsgolint")
    ap.add_argument("--print-cache-keys", action="store_true",
                    help="print `<name>=<key>` lines (GITHUB_OUTPUT format) for the CI caches of the work dir")
    ap.add_argument("--out-dir", type=Path, default=BENCH / "results")
    ap.add_argument("--readme", type=Path, help="rewrite the bench block of this README")
    ap.add_argument("--apply-table", type=Path,
                    help="only rewrite --readme's bench block from this results .md (no benchmarking)")
    args = ap.parse_args()
    if args.apply_table:
        if not args.readme:
            ap.error("--apply-table needs --readme")
        update_readme(args.readme, args.apply_table.read_text())
        return

    cfg = json.loads((BENCH / "projects.json").read_text())
    projects = cfg["projects"]
    if args.projects:
        want = args.projects.split(",")
        unknown = set(want) - {p["name"] for p in projects}
        if unknown:
            sys.exit(f"unknown projects: {', '.join(sorted(unknown))}")
        projects = [p for p in projects if p["name"] in want]
    work = args.work_dir.resolve()
    if args.print_cache_keys:
        digest = lambda x: hashlib.sha256(json.dumps(x, sort_keys=True).encode()).hexdigest()[:16]
        print(f"stock={digest([cfg['stock'], npm_platform()])}")
        for p in projects:
            key = [p["repo"], p["commit"], p["install"]] + ([overlay_digest(p)] if "overlay" in p else [])
            print(f"{p['name']}={p['commit'][:12]}-{digest(key)}")
        return

    pgo = pgo_training_projects()
    for p in projects:
        if p["name"] in pgo and pgo[p["name"]] != {k: p[k] for k in ("repo", "commit", "install")}:
            sys.exit(f"{p['name']}: bench/projects.json and {PGO_TRAIN.relative_to(REPO)} pin different checkouts")
    stock = (args.stock or ensure_stock(cfg["stock"], work)).resolve()
    for p in projects:
        ensure_checkout(p, work / "solutions" / p["name"], p["name"] in pgo)
    if args.setup_only:
        return

    tsrslint = args.tsrslint.resolve()
    commit = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "-C", str(REPO), "status", "--porcelain", "--untracked-files=no"],
                                capture_output=True, text=True).stdout.strip())
    now = dt.datetime.now(dt.timezone.utc)
    result: dict = {
        "date": now.strftime("%Y-%m-%d %H:%M UTC"),
        "machine": machine_info(args.local, args.label),
        "tsrslint": {"commit": commit, "dirty": dirty, "build": args.tsrslint_build, "rustc": rustc_version(),
                     "binary": str(tsrslint).replace(str(Path.home()), "~")},
        "stock": {"package": cfg["stock"]["package"].format(platform=npm_platform()),
                  "version": cfg["stock"]["version"], "binary": str(stock).replace(str(Path.home()), "~")},
        "reps": args.reps,
        "rules": None,
        "projects": {},
        "raw": [],
    }
    logs = work / "logs" / now.strftime("%Y%m%d-%H%M%S")
    binaries = {"stock": stock, "tsrslint": tsrslint}
    t_start = time.perf_counter()
    for p in projects:
        name = p["name"]
        cwd = work / "solutions" / name
        payload, n_files, n_rules = make_payload(p, cwd, work)
        result["rules"] = n_rules
        log(f"{name}: {n_files} files, {n_rules} rules")
        if not args.no_warmup:
            log(f"{name}: warm-up (stock, untimed)")
            r = run_once(stock, cwd, payload, logs, f"{name}-warmup", args.timeout)
            log(f"{name} warm-up: wall {r['wall_s']:.2f} s, {r['diagnostics']} diagnostics, exit {r['exit']}")
        runs: dict = {"stock": [], "tsrslint": []}
        for rep in range(args.reps):
            order = ["stock", "tsrslint"] if rep % 2 == 0 else ["tsrslint", "stock"]
            for c in order:
                r = run_once(binaries[c], cwd, payload, logs, f"{name}-{c}-{rep}", args.timeout)
                log(f"{name} {c:8} rep {rep}: wall {r['wall_s']:.2f} s, cpu {r['cpu_s']:.1f} s, "
                    f"peak {fmt_mem(r['peak_rss_bytes'])}, {r['diagnostics']} diagnostics + {r['internal']} internal, "
                    f"exit {r['exit']}" + (f", error {r['error']!r}" if r["error"] else "")
                    + (", TIMED OUT" if r["timed_out"] else ""))
                if not r["ok"]:
                    log(f"  FAILED; stderr: {logs / f'{name}-{c}-{rep}.stderr'}")
                # The (large) multisets are only kept for the first good run of each distinct output.
                if not r["ok"] or any(x["keys_sha"] == r["keys_sha"] and "_keys" in x for x in runs[c]):
                    r.pop("_keys"), r.pop("_full")
                runs[c].append(r)
                result["raw"].append({"project": name, "binary": c, "rep": rep,
                                      **{k: v for k, v in r.items() if not k.startswith("_") and k not in ("per_rule", "internal_sample")}})
        pr: dict = {"commit": p["commit"], "lint": p.get("lint", []), "files": n_files,
                    "pgo_training": name in pgo,
                    "stock": summarize(runs["stock"]), "tsrslint": summarize(runs["tsrslint"])}
        # Distinct (file, range, rule, message id) multisets per binary: more than one when its runs disagree
        # (multi-threaded nondeterminism). Parity is the closest stock / tsrslint pair.
        outs = {c: [r for r in runs[c] if "_keys" in r] for c in runs}
        for c in outs:
            if len(outs[c]) > 1:
                pr[c]["varies"] = compare(outs[c][0], outs[c][1], cwd)
                log(f"{name}: {c} runs differ: {pr[c]['varies']['keys_differ']} diagnostics")
        if outs["stock"] and outs["tsrslint"]:
            pr["parity"] = min((compare(g, t, cwd) for g in outs["stock"] for t in outs["tsrslint"]),
                               key=lambda x: x["keys_differ"])
            log(f"{name}: same diagnostics: {pr['parity']['same_keys']} ({pr['parity']['keys_differ']} differ), "
                f"text differs: {pr['parity']['text_differs']}")
        for c in runs:
            for r in runs[c]:
                r.pop("_keys", None), r.pop("_full", None)
        if not args.no_instructions:
            counted = count_instructions(tsrslint, cwd, payload, logs, name, args.timeout)
            if counted is not None:
                pr["tsrslint"].update(counted)
                log(f"{name} tsrslint: {counted['instructions'] / 1e9:.3f} G instructions, peak "
                    f"{fmt_mem(counted['max_rss_bytes'])} (one checker, untimed)")
        result["projects"][name] = pr
    result["duration_s"] = round(time.perf_counter() - t_start)

    table = markdown(result)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    stem = f"{now.strftime('%Y-%m-%d')}-{commit[:12]}" + ("-local" if args.local else "")
    (args.out_dir / f"{stem}.json").write_text(json.dumps(result, indent=1) + "\n")
    (args.out_dir / f"{stem}.md").write_text(table)
    if args.readme:
        update_readme(args.readme, table)
    print(table)
    log(f"wrote {args.out_dir / stem}.json/.md in {result['duration_s']} s; logs in {logs}")
    bad = [n for n, pr in result["projects"].items()
           if not pr["stock"]["ok_runs"] or not pr["tsrslint"]["ok_runs"] or not pr.get("parity", {}).get("same_keys")]
    if bad:
        log(f"WARNING: failed runs or different diagnostics: {', '.join(bad)}")


if __name__ == "__main__":
    main()
