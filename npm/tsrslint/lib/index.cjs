"use strict";
// Finds the native tsrslint binary. Same scheme as TypeScript 7's `typescript` package and tsrs: the binary ships in a
// per-platform package `<this package's name>-<process.platform>-<process.arch>`, listed in `optionalDependencies`
// so the package manager installs only the one whose `os`/`cpu` match.

const fs = require("node:fs");
const path = require("node:path");
const pkg = require("../package.json");

const EXE_NAME = process.platform === "win32" ? "tsrslint.exe" : "tsrslint";

function binaryPath() {
    const override = process.env.TSRSLINT_BINARY;
    if (override) {
        if (!fs.existsSync(override)) {
            throw new Error(`TSRSLINT_BINARY is set to ${override}, which does not exist.`);
        }
        return override;
    }

    const platformPackageName = `${pkg.name}-${process.platform}-${process.arch}`;
    let packageDir;
    try {
        packageDir = path.dirname(require.resolve(`${platformPackageName}/package.json`));
    }
    catch {
        const supported = Object.keys(pkg.optionalDependencies ?? {});
        throw new Error(
            [
                `Cannot find the optional dependency ${platformPackageName}, which contains the tsrslint binary for ${process.platform}-${process.arch}.`,
                supported.includes(platformPackageName)
                    ? `It is listed in ${pkg.name}'s optionalDependencies but was not installed. Common causes: installing with --no-optional / --omit=optional, ` +
                        `a lockfile or node_modules created on another platform, or pnpm \`supportedArchitectures\` excluding this platform. Reinstall to fix.`
                    : `This platform is not supported. Supported: ${supported.map(n => n.slice(pkg.name.length + 1)).join(", ")}.`,
                `To use a locally built binary instead, set TSRSLINT_BINARY=/path/to/tsgolint (cargo build --release puts it in target/release).`,
            ].join("\n"),
        );
    }

    const exe = path.join(packageDir, EXE_NAME);
    if (!fs.existsSync(exe)) {
        throw new Error(`${platformPackageName} is installed at ${packageDir} but has no ${EXE_NAME}.`);
    }
    return exe;
}

exports.binaryPath = binaryPath;
exports.version = pkg.version;
exports.tsrsCommit = pkg.tsrslint?.tsrsCommit;
