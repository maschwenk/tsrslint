import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const build = fileURLToPath(new URL('./build.mjs', import.meta.url));

function workspace(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tsrslint-build-test-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function json(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

test('release artifacts include Windows .exe and preserve Unix package names', t => {
  const dir = workspace(t);
  const artifacts = path.join(dir, 'artifacts');
  const out = path.join(dir, 'packages');
  const targets = [
    ['aarch64-apple-darwin', 'darwin', 'arm64'],
    ['x86_64-unknown-linux-gnu', 'linux', 'x64'],
    ['aarch64-unknown-linux-gnu', 'linux', 'arm64'],
    ['x86_64-pc-windows-msvc', 'win32', 'x64'],
  ];
  for (const [triple, os] of targets) {
    const target = path.join(artifacts, triple);
    fs.mkdirSync(target, { recursive: true });
    fs.writeFileSync(path.join(target, os === 'win32' ? 'tsgolint.exe' : 'tsgolint'), triple);
  }
  execFileSync(process.execPath, [build, '--artifacts', artifacts, '--out', out]);
  const main = json(path.join(out, 'tsrslint', 'package.json'));
  assert.equal(Object.keys(main.optionalDependencies).length, targets.length);
  for (const [triple, os, cpu] of targets) {
    const dir = path.join(out, `tsrslint-${os}-${cpu}`);
    const pkg = json(path.join(dir, 'package.json'));
    const exe = os === 'win32' ? 'tsrslint.exe' : 'tsrslint';
    assert.deepEqual(pkg.os, [os]);
    assert.deepEqual(pkg.cpu, [cpu]);
    assert.deepEqual(pkg.libc, os === 'linux' ? ['glibc'] : undefined);
    assert.ok(pkg.files.includes(exe));
    assert.equal(fs.readFileSync(path.join(dir, exe), 'utf8'), triple);
    assert.equal(main.optionalDependencies[pkg.name], pkg.version);
  }
  assert.equal(json(path.join(out, 'packages.json')).packages.at(-1).name, main.name);
});

test('explicit Windows binary includes only the Windows optional dependency', t => {
  const dir = workspace(t);
  const binary = path.join(dir, 'binary with spaces.exe');
  const out = path.join(dir, 'packages with spaces');
  fs.writeFileSync(binary, 'Windows binary');
  execFileSync(process.execPath, [build, '--binary', `x86_64-pc-windows-msvc=${binary}`, '--out', out]);
  const main = json(path.join(out, 'tsrslint', 'package.json'));
  assert.deepEqual(main.optionalDependencies, { '@maschwenk/tsrslint-win32-x64': main.version });
  assert.equal(fs.readFileSync(path.join(out, 'tsrslint-win32-x64', 'tsrslint.exe'), 'utf8'), 'Windows binary');
});
