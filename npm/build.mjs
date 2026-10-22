#!/usr/bin/env node
// Assembles the npm packages for tsrslint, modeled on tsrs's npm/build.mjs (TypeScript 7's scheme): one main package
// (Node launcher `bin/tsrslint`, `binaryPath()` API) whose `optionalDependencies` name one package per platform,
// each holding only that platform's native binary, renamed from Cargo's `tsgolint` to `tsrslint`.
//
//   node npm/build.mjs --binary aarch64-apple-darwin=target/release/tsgolint --pack
//   node npm/build.mjs --artifacts dir/ --pack     # dir/<target triple>/tsgolint, as the release workflow lays out
//   node npm/build.mjs --print-version             # also --check-tag <tag>
//
// The package name comes from npm/tsrslint/package.json; platform packages are `<name>-<os>-<cpu>`. The version is
// `[workspace.package] version` from the workspace Cargo.toml. The main package lists exactly the platforms given
// here, so a release never points at a platform package that was not published.

import { execFileSync, execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const npmDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(npmDir, "..");

// Rust target triple -> Node's process.platform / process.arch.
const TARGETS = {
    "aarch64-apple-darwin": { os: "darwin", cpu: "arm64" },
    "x86_64-unknown-linux-gnu": { os: "linux", cpu: "x64", libc: "glibc" },
    "aarch64-unknown-linux-gnu": { os: "linux", cpu: "arm64", libc: "glibc" },
    "x86_64-pc-windows-msvc": { os: "win32", cpu: "x64" },
};
// Cargo's binary name (crates/tsrslint/Cargo.toml) and the file name inside the platform packages.
const CARGO_EXE = "tsgolint";
const PACKAGE_EXE = "tsrslint";

function usage(message) {
    if (message) console.error(`error: ${message}\n`);
    console.error(
        "usage: node npm/build.mjs [--binary <triple>=<path>]... [--artifacts <dir>] [--out <dir>] [--pack]\n" +
            "                          [--check-tag <git tag>] [--print-version]\n" +
            `triples: ${Object.keys(TARGETS).join(", ")}`,
    );
    process.exit(message ? 2 : 0);
}

function parseArgs(argv) {
    const opts = { binaries: new Map(), out: path.join(npmDir, "dist"), pack: false };
    for (let i = 0; i < argv.length; i++) {
        const arg = argv[i];
        const value = () => argv[++i] ?? usage(`${arg} needs a value`);
        switch (arg) {
            case "--binary": {
                const [triple, file] = value().split(/=(.*)/s);
                if (!TARGETS[triple]) usage(`unknown target triple ${triple}`);
                if (!file) usage(`--binary needs <triple>=<path>`);
                opts.binaries.set(triple, path.resolve(file));
                break;
            }
            case "--artifacts": {
                const dir = path.resolve(value());
                for (const triple of Object.keys(TARGETS)) {
                    const suffix = TARGETS[triple].os === "win32" ? ".exe" : "";
                    for (const exe of [CARGO_EXE, PACKAGE_EXE]) {
                        const file = path.join(dir, triple, exe + suffix);
                        if (fs.existsSync(file)) opts.binaries.set(triple, file);
                    }
                }
                break;
            }
            case "--out":
                opts.out = path.resolve(value());
                break;
            case "--pack":
                opts.pack = true;
                break;
            case "--check-tag":
                opts.checkTag = value();
                break;
            case "--print-version":
                opts.printVersion = true;
                break;
            case "-h":
            case "--help":
                usage();
            default:
                usage(`unknown argument ${arg}`);
        }
    }
    return opts;
}

// The version, from the workspace Cargo.toml (single source of truth), and the tsrs commit, from Cargo.lock.
function readVersions() {
    let table = "";
    let version;
    for (const raw of fs.readFileSync(path.join(repoRoot, "Cargo.toml"), "utf8").split("\n")) {
        const line = raw.replace(/#.*$/, "").trim();
        const header = line.match(/^\[([^\]]+)\]$/);
        if (header) table = header[1];
        const kv = line.match(/^version\s*=\s*"([^"]*)"$/);
        if (kv && table === "workspace.package") version = kv[1];
    }
    if (!version) throw new Error("Cargo.toml needs [workspace.package] version");
    const lock = fs.readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8");
    const tsrsCommit = lock.match(/source = "git\+https:\/\/github\.com\/maschwenk\/tsrs[^#"]*#([0-9a-f]{40})"/)?.[1];
    if (!tsrsCommit) throw new Error("Cargo.lock has no maschwenk/tsrs git dependency");
    return { version, tsrsCommit };
}

function writeJson(file, value) {
    fs.writeFileSync(file, JSON.stringify(value, undefined, 4) + "\n");
}

function copyDir(src, dst) {
    fs.mkdirSync(dst, { recursive: true });
    for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
        const from = path.join(src, entry.name);
        const to = path.join(dst, entry.name);
        if (entry.isDirectory()) copyDir(from, to);
        else fs.copyFileSync(from, to);
    }
}

function npmPack(dir, out) {
    // Each package is a direct child of out. Keep shell arguments fixed when invoking npm.cmd on Windows.
    const options = { cwd: dir, encoding: "utf8" };
    const stdout = process.platform === "win32"
        ? execSync("npm pack --json --pack-destination ..", options)
        : execFileSync("npm", ["pack", "--json", "--pack-destination", out], options);
    const [info] = JSON.parse(stdout);
    return path.join(out, info.filename);
}

function main() {
    const opts = parseArgs(process.argv.slice(2));
    const { version, tsrsCommit } = readVersions();

    if (opts.checkTag !== undefined) {
        const tag = opts.checkTag.replace(/^refs\/tags\//, "");
        if (tag !== `v${version}`) throw new Error(`git tag ${tag} does not match the package version: expected v${version}`);
    }
    if (opts.printVersion) {
        console.log(version);
        return;
    }
    if (opts.checkTag !== undefined && opts.binaries.size === 0) return;
    if (opts.binaries.size === 0) usage("no binaries given (--binary or --artifacts)");

    const template = JSON.parse(fs.readFileSync(path.join(npmDir, "tsrslint", "package.json"), "utf8"));
    const name = template.name;
    const baseName = name.split("/").pop();
    const license = fs.readFileSync(path.join(repoRoot, "LICENSE"));
    const notice = fs.readFileSync(path.join(repoRoot, "NOTICE"));
    const thirdParty = fs.readFileSync(path.join(repoRoot, "THIRD-PARTY-LICENSE"));

    fs.rmSync(opts.out, { recursive: true, force: true });
    fs.mkdirSync(opts.out, { recursive: true });

    const common = {
        version,
        license: template.license,
        repository: template.repository,
        publishConfig: template.publishConfig,
    };

    const packageDirs = [];
    const optionalDependencies = {};
    for (const [triple, binary] of [...opts.binaries].sort()) {
        if (!fs.existsSync(binary)) throw new Error(`binary for ${triple} not found: ${binary}`);
        const { os, cpu, libc } = TARGETS[triple];
        const exe = PACKAGE_EXE + (os === "win32" ? ".exe" : "");
        const platformName = `${name}-${os}-${cpu}`;
        const dir = path.join(opts.out, `${baseName}-${os}-${cpu}`);
        fs.mkdirSync(dir, { recursive: true });

        fs.copyFileSync(binary, path.join(dir, exe));
        fs.chmodSync(path.join(dir, exe), 0o755);
        fs.writeFileSync(path.join(dir, "LICENSE"), license);
        fs.writeFileSync(path.join(dir, "NOTICE"), notice);
        fs.writeFileSync(path.join(dir, "THIRD-PARTY-LICENSE"), thirdParty);
        const readme = fs.readFileSync(path.join(npmDir, "platform", "README.md"), "utf8")
            .replaceAll("{{name}}", platformName)
            .replaceAll("{{mainName}}", name)
            .replaceAll("{{os}}", os)
            .replaceAll("{{cpu}}", cpu)
            .replaceAll("{{version}}", version)
            .replaceAll("{{tsrsCommit}}", tsrsCommit.slice(0, 12));
        fs.writeFileSync(path.join(dir, "README.md"), readme);
        writeJson(path.join(dir, "package.json"), {
            name: platformName,
            ...common,
            description: `The ${os}-${cpu} binary for ${name}`,
            preferUnplugged: true,
            files: [exe, "NOTICE", "THIRD-PARTY-LICENSE"],
            exports: { "./package.json": "./package.json" },
            os: [os],
            cpu: [cpu],
            ...(libc ? { libc: [libc] } : {}),
        });
        optionalDependencies[platformName] = version;
        packageDirs.push(dir);
    }

    const mainDir = path.join(opts.out, baseName);
    copyDir(path.join(npmDir, "tsrslint"), mainDir);
    for (const bin of Object.values(template.bin)) fs.chmodSync(path.join(mainDir, bin), 0o755);
    fs.writeFileSync(path.join(mainDir, "LICENSE"), license);
    fs.writeFileSync(path.join(mainDir, "NOTICE"), notice);
    fs.writeFileSync(path.join(mainDir, "THIRD-PARTY-LICENSE"), thirdParty);
    writeJson(path.join(mainDir, "package.json"), {
        ...template,
        version,
        tsrslint: { tsrsCommit },
        optionalDependencies,
    });
    // Platform packages first: they must be published before the main package that depends on them.
    packageDirs.push(mainDir);

    console.log(`version ${version}`);
    const manifest = [];
    for (const dir of packageDirs) {
        const entry = { name: JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8")).name, dir };
        if (opts.pack) entry.tarball = npmPack(dir, opts.out);
        console.log(`${opts.pack ? "packed" : "staged"} ${path.relative(process.cwd(), entry.tarball ?? dir)}`);
        manifest.push(entry);
    }
    // Publish order for the release workflow.
    writeJson(path.join(opts.out, "packages.json"), { version, packages: manifest });
}

try {
    main();
}
catch (e) {
    console.error(`error: ${e.message}`);
    process.exit(1);
}
