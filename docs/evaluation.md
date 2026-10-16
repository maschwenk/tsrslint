# tsrslint evaluation (2026-10-02)

> **Update (later on 2026-10-02):** the full port has landed: all 59 rules are ported and pass tsgolint's test suite
> (7,341 cases pass, 28 skipped upstream, 0 fail). On the monolith (tsrs `lint-api` `60a8d12`, quieter machine, 3 alternating
> pairs through oxlint) the normal config gives 0 diagnostics with both backends; the tsgolint segment takes 8.4 s vs
> 25.9 s (median) at 8.1 GB vs 26.7 GB peak RSS. With all 59 rules enabled both report the same 181,827 diagnostics
> (identical per-rule counts and positions); the sections below describe the original five-rule slice.
> Hand-measured monolith numbers for tsrslint 0.1.1 and main on 2026-10-04 are in the
> [addendum](#addendum-the-monolith-on-tsrslint-011-and-main-2026-10-04).

Question: is a Rust port of tsgolint on tsrs worth it, and what would a full port cost? Everything below was
measured on an internal monolith (origin/main `(redacted)`, 28,573 linted files, 39,145 program files) on an
18-core Apple Silicon Mac with 128 GB RAM that was **shared with another agent's builds and benchmarks**: load
average 10–65 during every run. All timings are alternating stock/tsrslint pairs; medians are reported and the
raw rows are in [data/](data/).

Versions: oxlint 1.73.0, stock `@oxlint-tsgolint/darwin-arm64@7.0.2002` (TypeScript-Go 7.0, `2bd066d8`), tsrslint
`main` on tsrs `lint-api` (`fe2aa45`, TypeScript 7.1-dev `b85298b6`).

## 1. Where stock tsgolint spends its time

Full monolith payload (13 rules) captured from `oxlint --type-aware`; `tsgolint headless -cpuprof -trace` plus an
`OXC_LOG=debug` run for phase timestamps and a `GODEBUG=gctrace=1` run for heap growth.

**Wall time (31.1 s run):**

| phase | wall | share |
| --- | ---: | ---: |
| read + parse payload | 0.01 s | 0% |
| file → tsconfig assignment (`FindTsConfigParallel`) | 3.23 s | 10% |
| program creation: parse, module resolution, bind | 1.94 s | 6% |
| linting: rule listeners + the checker work they trigger (4 checkers) | 25.89 s | 83% |
| inferred program (108 unmatched files) | 0.06 s | 0% |

**CPU (pprof, 178.8 CPU-s in 33.5 s):** every sample classified by its stack.

| bucket | CPU | share |
| --- | ---: | ---: |
| checker work called from rule listeners¹ | 49.7 s | 27.8% |
| page commit: `runtime.madvise` under `mheap.allocSpan` (on the allocating goroutine) | 59.4 s | 33.2% |
| GC (background mark, sweep, scavenge) | 33.4 s | 18.7% |
| tsconfig resolution (18 workers, mostly `stat`) | 21.9 s | 12.3% |
| program creation (parse, module resolution) | 8.1 s | 4.5% |
| bind | 1.3 s | 0.7% |
| rule logic outside the checker, including the AST walk | 0.7 s | 0.4% |
| protocol (JSON + framing) | 0.2 s | 0.1% |
| other runtime (tracer, scheduler) | 3.9 s | 2.2% |

¹ 34.6 s of it are samples whose 64-frame stacks were truncated inside the checker (Go's profiler limit); their
roots are checker frames (`checkExpressionWorker`, `Relater.isRelatedTo`, `getTypeOfSymbol`, …).

The live heap is 3.3 GB after program creation and 22 GB (27.5 GB heap, 30.7 GB RSS) at the end, so over 80% of
the allocation, page-commit and GC cost is incurred by checker work during linting.

**Answer:** about 89% of wall time and about 70–85% of CPU is program creation plus type checking (the work tsrs
does faster and in less memory). Rule logic and the protocol are under 1%. File-to-program assignment is the
remaining 10% of wall time; it is independent of the checker (tsrslint's port of it takes 1.5 s).

`-debug timings` per-rule numbers are misleading for this question: they include the checker work the rule
happened to trigger first (no-floating-promises 32.6 s, promise-function-async 30.7 s, unbound-method 21.3 s,
no-misused-promises 14.5 s, return-await 12.5 s; the other 8 rules together under 1 s). The 5-rule slice payload
costs stock tsgolint as much as the 13-rule payload (median 38.1 s vs 33.7 s), because type checking is shared.

## 2. Protocol

[headless-protocol.md](headless-protocol.md).

## 3. Checker API: tsgolint's shim usage mapped to tsrs

Every `checker.*` / `TypeChecker.*` call in tsgolint's non-test code (rules + utils), mapped by name to tsrs
(`tools/bench/api_inventory.py`, raw rows in [data/api-inventory.json](data/api-inventory.json)):

| status in tsrs `main` | count | notes |
| --- | ---: | --- |
| public method | 62 | includes every hot one: `GetTypeAtLocation` (48 call sites), `GetSymbolAtLocation` (22), `TypeToString` (18, `type_to_string_exported`), `getTypeArguments` (14), `getPropertyOfType` (13), `IsTupleType`, `getSignaturesOfType`, `getContextualType`, `getResolvedSignature`, `isTypeAssignableTo`, `getAwaitedType`, … |
| public struct field / accessor | 14 | `Type.flags/objectFlags/symbol/alias`, `Signature.parameters/declaration`, `TupleType.combinedFlags/readonly`, `TypePredicate.kind/t/parameterIndex`, `Checker.numberType/booleanType/globalRegExpType` |
| exists, `pub(crate)` | 19 | needs `pub`; 2 done on branch `lint-api` |
| missing | 0 | |

The 19 `pub(crate)` ones: `getAccessedPropertyName` ✅, `isArrayOrTupleType` ✅, `extractDefinitelyFalsyTypes`,
`getApparentTypeOfContextualType`, `getAssignmentReducedType`, `getContextFreeTypeOfExpression`,
`getDeclarationOfAliasSymbol`, `getFlowTypeOfReference`, `getIndexedAccessTypeOrUndefined`,
`getPropertyNameForKnownSymbolName`, `getTypeOfPropertyOrIndexSignatureOfType`, `getTypeWithDefault`,
`hasTypeFacts`, `isDeprecatedSymbol`, `isSpreadableProperty`, `isTypeIdenticalTo`, `removeDefinitelyFalsyTypes`,
`getMappedTypeModifiers`, `isNonDeferredTypeReference`. All are one-word visibility changes.

The operations the slice needed:

| operation (Go) | tsrs | status |
| --- | --- | --- |
| `NewProgram`, `BindSourceFiles`, `SourceFiles`, `ForEachCheckerParallel`, `GetProgramDiagnostics`, `Get{Syntactic,Semantic}Diagnostics`, `IsSourceFileDefaultLibrary`, `Options`, `Host` | `tsrs_compiler::Program` | public |
| `tsoptions.GetParsedCommandLineOfConfigFile`, `ParsedCommandLine.{FileNames, ResolvedProjectReferencePaths}`, `GetDefaultLibFileName`, `GetLibFileName` | `tsrs_tsoptions` | public |
| inferred-program config (`ParsedCommandLine` from options + files) | `tsoptions::new_parsed_command_line` | public |
| `project.ConfigFileRegistryBuilder.ComputeConfigFileName` | `tsrs_project` | `pub(crate)`; reimplemented in tsrslint (30 lines) |
| `GetTypeAtLocation`, `GetSymbolAtLocation`, `GetTypeOfSymbolAtLocation`, `TypeToString` | `get_type_at_location`, `get_symbol_at_location_exported`, `get_type_of_symbol_at_location`, `type_to_string_exported` | public |
| `getApparentType`, `getTypeArguments`, `isArrayType`, `IsTupleType`, `getPropertyOfType`, `getSignaturesOfType`, `getReturnTypeOfSignature`, `getBaseTypes`, `getDeclaredTypeOfSymbol`, `getBaseConstraintOfType`, `getIndexTypeOfType` | same names, snake_case | public |
| `getAccessedPropertyName` | `get_accessed_property_name` | was `pub(crate)`, now public on `lint-api` |
| `isArrayOrTupleType` | `is_array_or_tuple_type` | was `pub(crate)`, now public on `lint-api` (not used by the slice; used by 4 other rules) |
| AST: `ForEachChild`, `SkipParentheses`, `GetOperatorPrecedence`, `IsAssignmentExpression`, `GetEnclosingBlockScopeContainer`, `Locals`, … ; scanner `GetRangeOfTokenAtPosition`, `GetECMALineAndUTF16CharacterOfPosition` | `tsrs_ast`, `tsrs_scanner` | public |

tsgolint's TypeScript-Go patch stack has no tsrs counterpart to maintain: 0001 (single-run project service) and
0003 (early return on invalid tsconfig) are behavior tsrslint implements itself, 0002 is the visibility above,
0004/0005 are Go-only build and cache fixes.

## 4. The slice

Rust, about 3.4k lines after rustfmt (`crates/tsrslint/src`): protocol, headless driver, tsconfig assignment, configured and
inferred programs, tsconfig/program/`--type-check` diagnostics, source overrides, fixes and suggestions, timings,
the listener walk (`visitLintNodes` including the destructuring-pattern listener kinds), the used utils, and 5 rules
(1,549 Go lines → 1,361 Rust lines, 0.88×; about 1.0× counting only non-blank, non-comment lines): `no-floating-promises` (all options except
`allowForKnownSafeCalls`/`allowForKnownSafePromises`, which need `type_matches_specifier.go`),
`promise-function-async`, `return-await`, `no-unnecessary-boolean-literal-compare`, `no-implied-eval`. The first
three are 3 of the 5 rules that drive the monolith's checker cost.

## 5. Parity

Output compared message by message (`tools/bench/compare.py`: every field of every diagnostic, including labels
with printed types, help text, fixes and suggestions; order ignored).

| corpus | files / programs | stock | tsrslint | identical |
| --- | --- | ---: | ---: | ---: |
| Monolith, slice rules with its options (oxlint's own payload, filtered) | 28,573 / 1 + inferred | 16 | 16 | 16 |
| Internal packages with deps, half strict options, `-fix -fix-suggestions` | 1,565 / 55 + inferred | 955 | 955 | 955 (827 with fixes, 14 with suggestions, 8 with labels) |
| smoke file covering each message variant, `-fix -fix-suggestions` | 1 | 10 | 10 | 10 |
| `--type-check` (`report_syntactic`/`report_semantic`) | 2 | 9 | 9 | 9 |
| V1 payload | 1 | 4 | 4 | 4 |
| invalid tsconfig | 1 | 2 | 2 | 1 (the other differs only by TS 7.1 listing `es2026` as a valid target) |

tsgolint's own tests for the 5 rules (452 cases, extracted from the Go tests): 415 pass, 37 skipped (36 use the
unported allowlist options, 1 is skipped upstream), 0 fail.

End to end, `oxlint -c .oxlintrc.tsrslint.json --type-aware --deny-warnings` (the monolith's config with the 8 other
type-aware rules turned off) with `OXLINT_TSGOLINT_PATH=<tsrslint>` sends the same 28,573 files and 5 rules and exits 0, like
stock (the monolith's suppressions and disable directives hide all 16 diagnostics in both).

Note the two backends implement different TypeScript versions (7.0 vs 7.1-dev); no rule diagnostic differed on these
corpora, but the full rule set could surface 7.0/7.1 checker differences.

## 6. Timing

**Direct** (`tsgolint headless < payload`, slice payload, 7 alternating pairs in two rounds, load 10–31):

| | wall (median) | wall (range) | user CPU (median) | peak RSS (median) |
| --- | ---: | ---: | ---: | ---: |
| stock tsgolint 7.0.2002 | 38.1 s | 31.3–59.8 s | 179.3 s | 26.7 GB |
| tsrslint | 12.4 s | 10.9–13.1 s | 36.0 s | 7.8 GB |
| ratio | **3.1× faster** | per-pair 2.9–4.7× | 5.0× less | 3.4× less |

tsrslint phases (one run, load 24): assignment 1.5 s, program creation 2.8 s, linting 9.1 s, inferred 0.1 s.

**End to end through oxlint** (3 pairs with the tsgolint segment timed by a wrapper, load 20–32):

| | oxlint wall (median) | tsgolint segment (median) | oxlint's own pass |
| --- | ---: | ---: | ---: |
| stock | 66.2 s | 44.8 s | 21–22 s |
| tsrslint | 44.3 s | 15.7 s | 24–29 s |
| ratio | 1.5× | 2.9× | (same work; spread is load) |

oxlint runs its own (JS-plugin-heavy) pass first and the type-aware backend after it, so the end-to-end gain is
capped by oxlint's ~21 s pass. A first end-to-end round without segment timing ran at load 10–65 and is too
contaminated to use (stock 79–97 s, tsrslint 28–75 s); it is in [data/](data/) for completeness.

## 7. What a full port would take

| item | size | notes |
| --- | --- | --- |
| remaining rules | 54 of 59 at v7.0.2002 (main has 60) | 26.2k Go lines of rule code → ~23k Rust lines at the slice's 0.88× ratio. Largest: prefer-optional-chain 3.9k, no-unnecessary-condition 2.3k, no-unnecessary-type-assertion 1.5k, no-misused-promises 1.1k, prefer-nullish-coalescing 1.0k, no-deprecated 0.9k |
| remaining utils | ~3.5k Go lines | incl. `type_matches_specifier.go` (626, the allowlist options several rules share) |
| checker API | 17 visibility changes in tsrs | no missing operations |
| tests | 5,519 cases | extracted automatically from all 59 rules (2,802 valid, 2,717 invalid); 133 entries in 3 rules (prefer-nullish-coalescing, prefer-optional-chain, unbound-method) build cases with helpers/loops and need extractor work or hand conversion |
| Monolith-only scope | 8 more rules, 6.9k Go lines | consistent-type-exports, no-array-delete, no-misused-promises, no-unsafe-unary-minus, no-useless-default-assignment, prefer-includes, prefer-optional-chain (3.9k), unbound-method |

Total about 26k Rust lines on top of the existing ~3.4k. Ported the way tsrs was (agent-driven, rule by rule, gated
by the extracted tests and by diffing against stock tsgolint on real code), that is roughly 2–4 weeks of agent time
plus human review; the monolith-only scope is roughly a quarter of that.

**Ongoing cost.** tsgolint `main` had 81 commits touching rules/utils/linter/cmd in the last 90 days (36 fixes, 21
perf, 7 features; about 3.0k non-test lines added and 1.1k removed), so about 6 commits a week to review and port.
Because this repository keeps tsgolint's history, `git merge upstream/main` plus re-running the extractor turns most
upstream behavior changes into failing Rust tests. The protocol is internal and unversioned (V2 payloads, labeled
ranges and timings all arrived in 2026), so each oxlint upgrade needs a protocol check. TypeScript version tracking is
tsrs's job and already happens there. tsgolint's TypeScript-Go submodule/patch/shim maintenance (7 commits in 90 days)
goes away.

## 8. Recommendation

**Go, but scoped to the rules the monolith enables, not as a general-purpose replacement for tsgolint.**

* The win is real and comes from the part tsrs already ports: about 3× faster on the type-aware backend, 5× less CPU,
  3.4× less memory (27 GB → 8 GB, which matters for CI runner sizing), with identical output on ~990 diagnostics
  across 30k files. End to end the gain is 1.5× because oxlint's own pass is unchanged.
* The rule code is cheap relative to the checker (under 1% of runtime) and mechanical to port with the extracted tests.
* A full general-purpose port means owning 60 rules and tracking about 6 upstream commits a week indefinitely for rules
  we do not use. Port the 8 remaining monolith rules (6.9k Go lines), run tsrslint in shadow mode in CI next
  to stock tsgolint (diff the decoded outputs) for a couple of weeks, then switch the monolith over with the
  `oxlint-tsgolint` catalog entry pointing at a tsrslint build. Expand only when another app enables a rule
  tsrslint lacks.
* Before committing, compare against the fairer baseline: tsgolint rebuilt on TypeScript 7.1 (the other half of this
  investigation). Unmeasured here.

## Unverified

* Linux/CI performance (all numbers are from one shared macOS machine at load 10–65).
* The language-server path (`source_overrides` from oxlint's LSP) is implemented but untested.
* Windows.
* Rule behavior on TypeScript 7.0-vs-7.1 checker differences beyond the corpora above.

## Addendum: the monolith on tsrslint 0.1.1 and main (2026-10-04)

Moved here from the README, which now shows only the public CI benchmark ([bench/README.md](../bench/README.md)).
The monolith is private, so it is not in CI; these numbers were measured by hand on 2026-10-04 and have not been
refreshed since. 28,611 linted files, Linux x86_64 (18 vCPU Xeon 8375C, 47 GiB, otherwise idle), PGO release builds,
medians of interleaved runs; every run reports the same diagnostics:

| | stock tsgolint 7.0.2002 | tsrslint 0.1.1 | tsrslint main (2026-10-04) |
| --- | ---: | ---: | ---: |
| payload oxlint sends for the monolith's config (11 rules), wall | 61.7 s | 16.8 s | 10.4 s |
| same, peak RSS | 26.0 GiB | 7.99 GiB | 6.15 GiB |
| all 59 rules, wall | 89.2 s | 25.0 s | 15.1 s |
| all 59 rules, peak RSS | 29.8 GiB | 9.48 GiB | 7.14 GiB |
| `oxlint --type-aware --deny-warnings` end to end, wall | 92.3 s | | 36.2 s |

On an 18-core M5 Max (the monolith's config): 0.1.1 6.6 s / 7.7 GiB, main 4.2 s / 5.9 GiB.

Where the time goes: almost all of it is the type checking the rules ask for, shared between rules (each of the
expensive rules alone costs about 60% of the full run); rule code, the AST walk and the protocol are about 2% with the
monolith's config and 6.5% with all 59 rules ([perf-lint-cpu.md](perf-lint-cpu.md)). What makes main faster than
0.1.1: tsrs main instead of an old branch, linting each file with the checker tsrs's import-graph assignment picks
plus tail work stealing (`crates/tsrslint/src/sched.rs`), 6 checkers instead of 4 on machines with 8 or more hardware
threads, and less file-system work before linting (each tsconfig parsed once; on macOS at most 4 threads read files).
Knobs and diagnostics: `TSRSLINT_CHECKERS=<n>` (4 checkers: about 1 GiB less peak memory on the monolith, about 20%
slower), `TSRSLINT_SCHEDULE=locality`, `TSRSLINT_CHECKER_STATS=1`, `TSRSLINT_MEM_STATS=1`.
