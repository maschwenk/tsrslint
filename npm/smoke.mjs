#!/usr/bin/env node
// Smoke test of a tsrslint binary over tsgolint's headless protocol (docs/headless-protocol.md): sends a V2 payload
// for npm/smoke/fixture, parses the framed stdout and checks that the expected rule diagnostics come back.
//
//   node npm/smoke.mjs target/release/tsgolint
//   node npm/smoke.mjs node_modules/.bin/tsrslint      # through the npm launcher

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const arg = process.argv[2];
if (!arg) {
    console.error("usage: node npm/smoke.mjs <tsrslint binary or launcher>");
    process.exit(2);
}
// The binary runs with the fixture as cwd, so a relative path is resolved here (a bare name is looked up on PATH).
const exe = arg.includes("/") ? path.resolve(arg) : arg;

const fixture = path.join(path.dirname(fileURLToPath(import.meta.url)), "smoke", "fixture");
const file = path.join(fixture, "index.ts");
const payload = {
    version: 2,
    configs: [{ file_paths: [file], rules: [{ name: "no-floating-promises" }, { name: "no-array-delete" }] }],
    source_overrides: null,
    report_syntactic: false,
    report_semantic: false,
};

const version = spawnSync(exe, ["--version"], { encoding: "utf8" });
if (version.status !== 0) fail(`${exe} --version exited with ${version.status}: ${version.stderr}${version.error ?? ""}`);
console.log(version.stdout.trim());

const result = spawnSync(exe, ["headless"], { cwd: fixture, input: JSON.stringify(payload), maxBuffer: 64 << 20 });
if (result.error) fail(`failed to run ${exe}: ${result.error.message}`);
if (result.status !== 0) fail(`${exe} headless exited with ${result.status}\n${result.stderr}`);

const messages = [];
const out = result.stdout;
for (let offset = 0; offset < out.length;) {
    if (offset + 5 > out.length) fail(`truncated frame header at byte ${offset}`);
    const length = out.readUInt32LE(offset);
    const type = out[offset + 4];
    const start = offset + 5;
    if (start + length > out.length) fail(`truncated frame payload at byte ${offset}`);
    messages.push({ type, body: JSON.parse(out.subarray(start, start + length).toString("utf8")) });
    offset = start + length;
}

const errors = messages.filter(m => m.type === 0);
if (errors.length) fail(`error messages: ${JSON.stringify(errors)}`);
const diagnostics = messages.filter(m => m.type === 1).map(m => m.body);
const internal = diagnostics.filter(d => d.kind !== 0);
if (internal.length) fail(`unexpected internal diagnostics: ${JSON.stringify(internal)}`);
const rules = diagnostics.map(d => d.rule).sort();
const expected = ["no-array-delete", "no-floating-promises"];
if (JSON.stringify(rules) !== JSON.stringify(expected)) {
    fail(`expected diagnostics for ${expected.join(", ")}, got ${JSON.stringify(diagnostics, undefined, 2)}`);
}
for (const d of diagnostics) {
    if (d.file_path !== file) fail(`diagnostic for unexpected file ${d.file_path}`);
    console.log(`ok ${d.rule} ${d.range.pos}-${d.range.end}: ${d.message.description}`);
}
console.log("smoke test passed");

function fail(message) {
    console.error(`smoke test failed: ${message}`);
    process.exit(1);
}
