# Porting tsgolint rules to tsrslint

tsrslint ports tsgolint **v7.0.2002** (this repository's Go sources under `internal/`, `cmd/`) to Rust on top of
tsrs (`maschwenk/tsrs`, the Rust port of the TypeScript 7 checker). The Go code is the spec: port behavior,
messages, ranges, fixes and suggestions exactly, so tsrslint output is byte-for-byte what stock tsgolint reports.

## Layout

- `crates/tsrslint/src/rules/<rule>.rs`: one file per rule, named after `internal/rules/<rule>/`. Every rule
  already has a file and a registry entry in `rules/mod.rs`; unported rules are stubs whose `create()` returns
  `not_ported(...)`. Porting a rule = replacing its stub. Do not edit `rules/mod.rs` (except, if a rule needs a
  different constructor signature, its own match arm).
- `crates/tsrslint/src/utils/`: ports of `internal/utils/*.go`. New helpers go into the submodule named after the
  Go file they port (`go_utils.rs` is `utils.go`), with the Go name in snake_case. Check `utils/mod.rs` and the
  submodules first; never port the same Go helper twice. Helpers that exist only in a rule's Go package stay in
  that rule's file.
- `crates/tsrslint/src/rule.rs`: the `Rule` / `RuleVisitor` / `Ctx` API (port of `internal/rule/rule.go`).
- `crates/tsrslint/tests/cases/<rule>.json`: tsgolint's own test cases, extracted from
  `internal/rules/<rule>/<rule>_test.go` by `tools/extract_rule_tests` (`GOWORK=off go build`, then
  `extract_rule_tests internal/rules/<rule> crates/tsrslint/tests/cases/<rule>.json`). Cases the extractor cannot
  evaluate statically are listed under `unconverted`.
- `crates/tsrslint/tests/rule_tests.rs`: runs the cases. One `#[test]` per rule; stubs are skipped.

## Workflow per rule

1. Read `internal/rules/<rule>/*.go` and the helpers it calls (`internal/utils`, `shim/*`, patched
   typescript-go). Port the options struct (defaults from the Go `UnmarshalJSON`), listeners, messages (same ids and
   text), fixes and suggestions.
2. `cargo test --release --test rule_tests <rule_snake_name> -- --nocapture` until every converted case passes.
   A case may be skipped only if it is skipped upstream. Do not edit extracted cases to make them pass.
3. If cases are `unconverted`, convert them (extend the extractor only in your own branch, or dump them at runtime
   from the Go tests) so the rule is fully covered.
4. `cargo fmt`, no new warnings, and `tools/lint/ratchet.py` passes: no new clippy findings
   ([rust-practices.md](rust-practices.md)).

## tsrs API

tsgolint calls checker internals through shims; tsrs has the same functions, sometimes `pub(crate)`. Make them
`pub` (visibility only, no behavior changes) on your tsrs branch (based on `lint-api`). Point your worktree at
your local tsrs checkout with an untracked `.cargo/config.toml`:

```toml
[patch."https://github.com/maschwenk/tsrs"]
tsrs_core = { path = "/path/to/tsrs/crates/tsrs_core" }
# ... one line per tsrs crate in Cargo.toml (and tsrs_parser for the tests)
```

If tsrs is genuinely missing behavior a rule needs, port it into tsrs from the TypeScript Go source
(`microsoft/TypeScript` `tsc/` at the commit tsrs ports) and note it in your report.
