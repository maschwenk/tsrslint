#!/usr/bin/env node
// Installs packed packages in a fresh project and checks npm's shims, the JS API and the headless protocol.
import assert from 'node:assert/strict';
import { execFileSync, execSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const npmDir = path.dirname(fileURLToPath(import.meta.url));
const dist = path.resolve(process.argv[2] ?? path.join(npmDir, 'dist'));
const { packages } = JSON.parse(fs.readFileSync(path.join(dist, 'packages.json'), 'utf8'));
const dependencies = Object.fromEntries(packages.map(p => [p.name, pathToFileURL(p.tarball).href]));
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tsrslint-smoke-'));

function npm(command) {
  const options = { cwd: dir, encoding: 'utf8' };
  // Commands are fixed strings below; paths are passed through package.json, not interpolated into a shell.
  return process.platform === 'win32'
    ? execSync(`npm ${command}`, options)
    : execFileSync('npm', command.split(' '), options);
}

function node(args) {
  return execFileSync(process.execPath, args, { cwd: dir, encoding: 'utf8' });
}

try {
  fs.writeFileSync(
    path.join(dir, 'package.json'),
    JSON.stringify({ name: 'tsrslint-smoke', version: '1.0.0', private: true, dependencies }),
  );
  console.log(npm('install --no-audit --no-fund'));
  console.log(npm('exec --no -- tsrslint --version').trim());
  const bin = npm('exec --no -- tsrslint-path').trim();
  console.log(`binary: ${bin}`);
  assert.ok(fs.statSync(bin).isFile());
  const api = JSON.parse(node([
    '-e',
    'const m = require("@maschwenk/tsrslint"); console.log(JSON.stringify({ path: m.binaryPath(), version: m.version, tsrsCommit: m.tsrsCommit }))',
  ]));
  assert.equal(api.path, bin);
  assert.ok(api.version);
  assert.ok(api.tsrsCommit);
  const launcher = path.join(dir, 'node_modules', '@maschwenk', 'tsrslint', 'bin', 'tsrslint');
  console.log(node([path.join(npmDir, 'smoke.mjs'), '--node', launcher]));
  console.log(node([path.join(npmDir, 'smoke.mjs'), bin]));
} finally {
  fs.rmSync(dir, { recursive: true, force: true });
}
