# The oxlint ⇄ tsgolint headless protocol

Sources: oxc `crates/oxc_linter/src/tsgolint.rs` at tag `oxlint_v1.73.0` (commit `39677ba5`) and tsgolint
`cmd/tsgolint/{headless.go,payload.go}` + `internal/linter/linter.go` at tag `v7.0.2002` (commit `acd88e1b`).
The protocol is internal to oxlint and tsgolint: it has no version negotiation, and the two are released in
lockstep (oxlint pins a compatible `oxlint-tsgolint`). A third-party backend has to track both repositories.

## 1. Finding and starting the backend

oxlint (`try_find_tsgolint_executable`) looks for the executable in this order:

1. `OXLINT_TSGOLINT_PATH`: a file, or a directory containing `tsgolint`. If set but missing, it is an error
   (no fallback).
2. `node_modules/.bin/tsgolint` in the cwd and every ancestor directory (`tsgolint.CMD`/`tsgolint.exe` on Windows).
3. `tsgolint` on `PATH`.

It spawns `<exe> headless [flags]` with the cwd of oxlint, stdin and stdout piped, stderr inherited. Flags:

| flag | when | meaning |
| --- | --- | --- |
| `-debug timings` (two argv entries) | `oxlint --debug timings` | emit one timing message at the end |
| `-fix` | `--fix` (and other fix kinds that include fixes) | include `fixes` in rule diagnostics |
| `-fix-suggestions` | `--fix-suggestions` | include `suggestions` in rule diagnostics |
| `-trace=<file>` | env `OXLINT_TSGOLINT_TRACE` | Go runtime execution trace |
| `-cpuprof=<file>` | env `OXLINT_TSGOLINT_CPU` | Go pprof CPU profile |
| `-heap=<file>` / `-allocs=<file>` | env `OXLINT_TSGOLINT_HEAP` / `OXLINT_TSGOLINT_ALLOCS` | Go heap / allocation profiles |

The backend itself reads `OXC_LOG=debug` (debug log to stderr) and
`OXLINT_TSGOLINT_DANGEROUSLY_SUPPRESS_PROGRAM_DIAGNOSTICS=true` (lint programs even when the tsconfig or the
program has errors).

oxlint writes the whole payload to stdin, closes it, then reads stdout on a separate thread while the process
runs. After the process exits, a non-zero exit status is an error (`exit status: N[, error: <error message>]`);
oxlint prints `Error running tsgolint: …` and fails the run.

## 2. stdin: the payload (one JSON document)

```json
{
  "version": 2,
  "configs": [
    {
      "file_paths": ["/abs/path/a.ts", "/abs/path/b.tsx"],
      "rules": [
        { "name": "no-floating-promises", "options": { "ignoreVoid": true, "checkThenables": false } },
        { "name": "return-await", "options": "always" },
        { "name": "no-array-delete" }
      ]
    }
  ],
  "source_overrides": null,
  "report_syntactic": false,
  "report_semantic": false
}
```

* `version`: `2`. A payload without `version` (or `0`) is the legacy V1 shape
  `{"files": [{"file_path": "...", "rules": ["rule-name", ...]}]}`, which tsgolint converts to V2 (one config per
  file, no options). Any other version is an error.
* `configs`: oxlint resolves the oxlint config of every file it lints and groups files by their set of enabled
  type-aware rules (a `BTreeSet` ordered by rule name, then by the options serialized as JSON), so each distinct rule
  set appears once. Only files with a JS/TS source type are sent; files with no enabled type-aware rule still appear
  (with `"rules": []`). `file_paths` are absolute, as oxlint walked them.
* `rules[].name`: the rule name without the `typescript/` prefix. Only rules with warn/deny severity that are
  marked `IS_TSGOLINT_RULE` in oxlint are sent; severity itself is not sent (oxlint applies it on the way back).
* `rules[].options`: the rule's configuration as oxlint serializes it (`Rule::to_configuration`): an object for most
  rules, a string for enum options (`return-await`), omitted when the rule has none. oxlint fills defaults, so in
  practice every option field is present; tsgolint still applies its own defaults for missing fields.
* `source_overrides`: `{ "/abs/path": "file contents" }` for unsaved editor buffers (the language server's
  `lint_source`), otherwise `null`. tsgolint layers these over the OS file system.
* `report_syntactic` / `report_semantic`: `oxlint --type-check`. When set, the backend also reports TypeScript's
  syntactic / semantic diagnostics of each linted file as internal diagnostics (`id` = `TS<code>`).

## 3. stdout: framed messages

```
| payload length: u32 little-endian | message type: u8 | payload: UTF-8 JSON (length bytes) |
```

Messages are streamed in any order (tsgolint emits them from one writer goroutine as workers report them; there is
no ordering by file). oxlint parses incrementally and stops at the first malformed frame.

| type | name | payload |
| ---: | --- | --- |
| 0 | error | `{"error": "<text>"}`: fatal; oxlint stops reading and fails with this text |
| 1 | diagnostic | see below |
| 2 | timing | `{"rules": [{"rule_name": "no-floating-promises", "duration": <ns u64>, "calls": <u64>}]}`, once, last |

### Diagnostic payload

```json
{
  "kind": 0,
  "range": { "pos": 50, "end": 53 },
  "message": { "id": "floatingVoid", "description": "Promises must be awaited, …", "help": "The promise must …" },
  "file_path": "/abs/path/a.ts",
  "rule": "no-floating-promises",
  "labeled_ranges": [ { "label": "This unhandled promise-like value has type `Promise<number>`.", "range": { "pos": 50, "end": 53 } } ],
  "fixes": [ { "text": "await ", "range": { "pos": 50, "end": 50 } } ],
  "suggestions": [ { "message": { "id": "floatingFixVoid", "description": "Add void operator to ignore." }, "fixes": [ … ] } ]
}
```

* `kind`: `0` = rule diagnostic, `1` = internal diagnostic (tsconfig/program errors, `--type-check` errors).
* `range`: `pos`/`end` are **UTF-8 byte offsets** into the file text (TypeScript-Go's and tsrs's positions, and
  oxc's `Span`). Rule diagnostics use the node range with leading trivia skipped
  (`scanner.GetRangeOfTokenAtPosition(file, node.Pos()).WithEnd(node.End())`). Omitted when invalid.
* `message`: `id` is the typescript-eslint message id; `help` is omitted when empty.
* `file_path`: always present (`null` allowed for internal diagnostics without a file).
* `rule`: required for `kind: 0`, absent for `kind: 1`.
* `labeled_ranges`: optional. oxlint shows them as labels; when present, the primary `range` is added as an
  extra (unlabeled) label unless it is empty.
* `fixes`: only meaningful with `-fix`; `suggestions` only with `-fix-suggestions`. Empty lists may be omitted.

### What oxlint does with each diagnostic

* Rule diagnostics: the severity is looked up from the resolved config of `file_path` by `rule` (a diagnostic
  for a rule that was not requested for that file is dropped). Disable directives are checked for `<rule>`,
  `typescript-eslint/<rule>` and `@typescript-eslint/<rule>` at `range` (or the first labeled range if `range`
  is empty). Then bulk suppressions (`oxlint-suppressions`) apply. The diagnostic is rendered as
  `typescript(<rule>)` with URL `https://oxc.rs/docs/guide/usage/linter/rules/typescript/<rule>.html`.
  Fixes and suggestions are merged with `CompositeFix::merge_fixes` against the file text.
* Internal diagnostics: rendered as errors `typescript(<id>)` (`tsconfig-error`, `TS2322`, …), with a label
  only if `file_path` and `range` are both set.

## 4. Backend behavior oxlint relies on (tsgolint semantics a replacement must match)

1. **File → program assignment** (`utils.TsConfigResolver`): for each file, the nearest `tsconfig.json` (then
   `jsconfig.json`) in its directory or an ancestor (stopping at `node_modules`), then a breadth-first search through
   that config's project references for a config whose file list contains the file; if none does, the same from the
   next ancestor config. Files no configured program contains go to one **inferred program** with fixed options
   (`allowJs`, `module: esnext`, `moduleResolution: bundler`, `target: es2022`, `jsx: react-jsx`,
   `allowImportingTsExtensions`, `strictNullChecks`, `strictFunctionTypes`, `sourceMap`, `esModuleInterop`,
   `allowNonTsExtensions`, `resolveJsonModule`).
2. **Program creation**: `useSourceOfProjectReference: true`, multi-threaded, TypeScript's default checker count
   (4, or `--checkers` in the tsconfig). tsconfig parse errors, config errors and program diagnostics become
   `kind: 1` diagnostics with `id: "tsconfig-error"`, `description: "Invalid tsconfig"` and the TypeScript message as
   `help` (plus a pointer to tsgolint#351 for removed options), and that program is skipped.
3. **Linting**: no full type check. Rules ask the checker lazily (`getTypeAtLocation` & co.); every checker pulls
   files from one shared queue in program order. Per file, every rule's listeners run in one AST walk
   (`visitLintNodes`: enter/exit listeners per node kind, plus "allow pattern" variants for destructuring
   assignment targets).
4. **Errors**: an unknown rule name makes tsgolint panic (non-zero exit, Go stack trace on stderr); a file missing
   from the program it was assigned to panics too.

tsrslint implements all of the above except the Go profiling flags (accepted and ignored) and, for speed, the
checker scheduling: each checker lints the files tsrs's import-graph locality assignment gives it, a checker that runs
out takes not-yet-started files from the busiest one, and programs get 6 checkers on machines with 8 or more hardware
threads (`crates/tsrslint/src/sched.rs`; `TSRSLINT_CHECKERS`, `TSRSLINT_SCHEDULE=locality`,
`TSRSLINT_CHECKER_STATS=1`). The diagnostics are the same; with another checker count a few printed type labels can
list the members of an object-literal union in another order.
