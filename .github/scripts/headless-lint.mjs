#!/usr/bin/env node
// Runs a tsrslint binary over tsgolint's headless protocol (docs/headless-protocol.md) on a project, with all 59
// rules enabled and default options (one V2 config, like oxlint sends for one rule set). Used by the PGO training
// workload (.github/scripts/pgo-train.sh) and to benchmark binaries (docs/perf-pgo.md).
//
//   node .github/scripts/headless-lint.mjs run <tsgolint> <project dir> [options]   lint, print a summary
//   node .github/scripts/headless-lint.mjs payload <project dir> [options] > p.json  only write the payload
//   node .github/scripts/headless-lint.mjs decode <framed stdout file>              sorted diagnostics, one per line
//
// Options: --ext ts,tsx (file extensions, default ts,tsx,mts,cts; .d.ts files are never linted),
// --include <dir> (repeatable; default: the whole project), --out <file> (run: write the sorted diagnostics).
// The files are the project's git-tracked files (git ls-files), so the project must be a git checkout.
// `run` fails if the binary exits non-zero or crashes, sends an error frame, reports an internal diagnostic
// (tsconfig/program errors: that program would not be linted), or reports no rule diagnostic at all.

import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const [mode, ...rest] = process.argv.slice(2);

if (mode === "run") {
    const [exe, dir, ...opts] = rest;
    if (!exe || !dir) usage();
    const { payload, cwd } = buildPayload(dir, opts);
    const t0 = performance.now();
    const result = spawnSync(path.resolve(exe), ["headless"], {
        cwd,
        input: JSON.stringify(payload),
        maxBuffer: 1 << 30,
        stdio: ["pipe", "pipe", "inherit"],
    });
    const seconds = ((performance.now() - t0) / 1000).toFixed(1);
    if (result.error) fail(`failed to run ${exe}: ${result.error.message}`);
    if (result.status !== 0) fail(`${exe} headless exited with ${result.status ?? result.signal}`);
    const diagnostics = decode(result.stdout);
    const internal = diagnostics.filter(d => d.kind !== 0);
    const perRule = new Map();
    for (const d of diagnostics) if (d.kind === 0) perRule.set(d.rule, (perRule.get(d.rule) ?? 0) + 1);
    console.log(
        `${path.basename(cwd)}: ${payload.configs[0].file_paths.length} files, ${diagnostics.length - internal.length} `
            + `rule diagnostics from ${perRule.size} rules, ${internal.length} internal, ${seconds} s`,
    );
    if (opts.includes("--out")) fs.writeFileSync(opts[opts.indexOf("--out") + 1], sortedLines(diagnostics));
    if (internal.length) fail(`internal diagnostics: ${JSON.stringify(internal.slice(0, 5), undefined, 2)}`);
    if (perRule.size === 0) fail("no rule diagnostics");
} else if (mode === "payload") {
    const [dir, ...opts] = rest;
    if (!dir) usage();
    process.stdout.write(JSON.stringify(buildPayload(dir, opts).payload));
} else if (mode === "decode") {
    if (!rest[0]) usage();
    process.stdout.write(sortedLines(decode(fs.readFileSync(rest[0]))));
} else {
    usage();
}

function buildPayload(dir, opts) {
    const cwd = path.resolve(dir);
    const value = name => (opts.includes(name) ? opts[opts.indexOf(name) + 1] : undefined);
    const exts = (value("--ext") ?? "ts,tsx,mts,cts").split(",");
    const include = opts.flatMap((o, i) => (o === "--include" ? [opts[i + 1]] : []));
    const files = execFileSync("git", ["ls-files", "-z", "--", ...(include.length ? include : ["."])], {
        cwd,
        encoding: "utf8",
        maxBuffer: 1 << 30,
    })
        .split("\0")
        .filter(f => f && exts.includes(path.extname(f).slice(1)) && !/\.d\.[mc]?ts$/.test(f))
        .map(f => path.join(cwd, f));
    if (!files.length) fail(`no .${exts.join("/.")} files in ${cwd}`);
    // Every ported rule: one case file per rule (tests/cases/<rule-name>.json).
    const rules = fs.readdirSync(path.join(repo, "crates/tsrslint/tests/cases"))
        .filter(f => f.endsWith(".json"))
        .map(f => ({ name: f.slice(0, -".json".length) }))
        .sort((a, b) => a.name.localeCompare(b.name));
    const payload = {
        version: 2,
        configs: [{ file_paths: files, rules }],
        source_overrides: null,
        report_syntactic: false,
        report_semantic: false,
    };
    return { payload, cwd };
}

// Framed stdout: | u32 LE length | u8 type | JSON |. Type 0 = error (fatal), 1 = diagnostic, 2 = timing.
function decode(out) {
    const diagnostics = [];
    for (let offset = 0; offset < out.length;) {
        if (offset + 5 > out.length) fail(`truncated frame header at byte ${offset}`);
        const length = out.readUInt32LE(offset);
        const type = out[offset + 4];
        const start = offset + 5;
        if (start + length > out.length) fail(`truncated frame payload at byte ${offset}`);
        const body = JSON.parse(out.subarray(start, start + length).toString("utf8"));
        if (type === 0) fail(`error message: ${body.error}`);
        if (type === 1) diagnostics.push(body);
        offset = start + length;
    }
    return diagnostics;
}

// Diagnostics arrive in any order (one writer, many checker threads); sorted JSON lines compare byte for byte.
function sortedLines(diagnostics) {
    const lines = diagnostics.map(d => JSON.stringify(d)).sort();
    return lines.length ? lines.join("\n") + "\n" : "";
}

function usage() {
    console.error(
        "usage: headless-lint.mjs run <tsgolint> <project dir> [--ext ts,tsx] [--include <dir>]... [--out <file>]\n"
            + "       headless-lint.mjs payload <project dir> [--ext ts,tsx] [--include <dir>]...\n"
            + "       headless-lint.mjs decode <framed stdout file>",
    );
    process.exit(2);
}

function fail(message) {
    console.error(`headless-lint: ${message}`);
    process.exit(1);
}
