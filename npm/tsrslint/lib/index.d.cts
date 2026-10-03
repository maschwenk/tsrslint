/**
 * Absolute path of the native tsrslint binary for this platform (from the optional dependency
 * `@maschwenk/tsrslint-<os>-<arch>`, or `TSRSLINT_BINARY` when set). Point `OXLINT_TSGOLINT_PATH` at it.
 * Throws when the platform package is not installed.
 */
export declare function binaryPath(): string;
/** The npm package version (the Cargo workspace version). */
export declare const version: string;
/** The maschwenk/tsrs commit the binary was built against. */
export declare const tsrsCommit: string | undefined;
