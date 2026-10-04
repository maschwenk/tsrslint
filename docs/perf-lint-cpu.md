# perf-lint-cpu: where the lint phase's CPU goes

Question: in the lint phase (rule listeners plus the checker work they trigger), how much CPU is rule code, the walk,
the protocol and the allocator, and which checker calls do the rules pay for? Answer: on the payload its CI runs,
98% of lint-phase CPU is first-time type checking that the rules ask for; rule code, the walk and the output are
1.9% together, and repeated checker queries are 0.1%. There is no rule-side change that moves the normal payload by
more than run-to-run noise. On the 59-rule stress payload the non-checker share is 6.5% and repeated queries are
about 10%, most of which cannot be cached without changing results (below). One fix landed: type printing is
memoized per file (all59: 59% fewer prints, peak footprint -2.4%).

## Method

- Binary: tsrslint `main` 056a72d (files linted by the checker tsrs assigns them) on tsrs `claude/lint-exports`
  (4da8206), `cargo build --release` (no PGO). Corpus and payloads as in `tsrslint-bench/README.md`: `monolith-normal`
  (11-13 rules, 27,398 files, what its CI runs) and `monolith-all59` (all 59 rules, default options).
- Profiles: `samply record -s --unstable-presymbolicate` (1 kHz), one run per payload, samples weighted by their
  thread CPU delta. A small reader of the profile JSON (not committed) classifies every sample by its stack: the rule
  is the outermost `tsrslint::rules::*` frame, the checker entry is the first tsrs frame below it, the util is the
  `tsrslint::utils` frame that made that call, and "allocator" means the leaf is mimalloc, memcpy/memmove or
  hash-table growth. The lint phase is the 4 checker threads (`for_each_checker_group` workers); everything else is
  pre-lint.
- Repeated queries: an instrumented build (experiment only, not committed) wrapped 8 checker APIs that rules call
  (`get_type_at_location`, `get_symbol_at_location_exported`, `type_to_string_exported`, `get_contextual_type`,
  `get_contextual_type_for_argument_at_index`, `get_type_of_symbol_at_location`, `get_resolved_signature_exported`,
  `get_return_type_of_signature`), counted top-level calls with thread CPU time, split first calls from repeats (same
  checker, same arguments), and compared each repeat's result with the first one. The per-call clock reads (~0.1 us)
  inflate the cost of cheap calls.
- Machine: the shared 18-core Mac, load 23-58 (one run of 166) during these runs; the numbers are CPU shares and
  counts, which load moves less than wall time.

## Whole process (normal, profiled run, load 23)

|                                |                                                                                                                              CPU |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------: |
| lint phase (4 checker threads) |                                                                                                                           21.0 s |
| pre-lint threads               | 22.8 s, of which 17.4 s are `open`/`stat`/`read`/`getdirentries` syscalls (tsconfig assignment and program build, not this lane) |

## Lint phase by bucket

| bucket                                         | normal (21.0 s) | all59 (41.0 s) |
| ---------------------------------------------- | --------------: | -------------: |
| checker work called from a rule                |           91.1% |          87.1% |
| allocator leaves inside that checker work      |            7.0% |           6.4% |
| rule logic (tsrslint frames, no checker below) |            0.5% |           4.2% |
| walk and dispatch (`Walker`, `for_each_child`) |            1.2% |           1.2% |
| tsrs AST/scanner helpers called by rules       |            0.2% |           0.8% |
| protocol (diagnostic conversion and output)    |            0.0% |           0.1% |
| other                                          |            0.1% |           0.3% |

Rule logic in all59 is spread over many functions: the largest is `no_deprecated::check_identifier` at 0.08 s
(0.2%). `Walker::run` is the largest walk frame (0.09 s normal, 0.25 s all59), mostly the indirect `visit` call and
loading the dispatch slot.

## Checker work by rule and entry point

Each rule pays for the checker work it triggers first, so these shares say which rule asks first, not which rule
needs the work.

normal:

| rule                   | share | entry point                                                                              |
| ---------------------- | ----: | ---------------------------------------------------------------------------------------- |
| no-floating-promises   | 30.0% | `get_type_at_location` of expression statements                                          |
| promise-function-async | 28.7% | `get_type_at_location` of functions 26.7%, return types 1.9%                             |
| no-misused-promises    | 15.1% | `get_type_at_location` 11.1%, via `is_thenable_type` 3.1%                                |
| unbound-method         | 14.2% | `get_type_at_location` 10.0%, `get_symbol_at_location` (checks the property access) 4.0% |
| return-await           | 10.4% | `get_type_at_location`                                                                   |
| the other 6-8 rules    |  0.3% |                                                                                          |

Under those calls the time is ordinary call checking: `resolve_call` 72% inclusive, `infer_type_arguments` 53%,
`check_function_expression_or_object_literal_method` 49%. For scale, a full `tsrs --noEmit --checkers 4` of
the monolith (tsrs main 9ba8f6d, one run) takes 30 s of user CPU including parse (7.8 s check wall), so the normal
rules ask for very roughly two thirds of a full type check.

all59: no-unsafe-assignment 31.5%, no-misused-promises 20.5% (12.2% of it in
`get_contextual_type_for_argument_at_index`, 93% of that resolving the call's signature for the first time),
no-floating-promises 15.5%, no-unsafe-return 8.6%, no-deprecated 3.0%, no-unnecessary-type-assertion 2.5%, every
other rule under 2%. Type printing (`type_to_string_ex`) was 2.8%, mostly no-unsafe-type-assertion and
no-unsafe-return labels.

Hottest checker leaves (normal, two profiles): `SymbolMap::position` 4.7-4.8%, `instantiate_type_with_alias_worker`
4.4-4.7%, `reserve_rehash` 2.3-2.6% (cache growth across many maps), `get_apparent_type` 2.1-2.4%, then a flat tail
(all59 has the same top four). The
samples sit on loads (for example 17% of `get_apparent_type`'s on the first read of the type's flags; in
`SymbolMap::position` on reading the symbol's name after a fingerprint match): the checker is memory-bound here,
and none of these is a lint-only path.

## Repeated queries

| API                                         | normal calls / repeats / repeat CPU | all59 calls / repeats / repeat CPU | repeats with a different result (all59) |
| ------------------------------------------- | ----------------------------------- | ---------------------------------- | --------------------------------------: |
| `get_type_at_location`                      | 1,031,034 / 60,892 / 0.02 s         | 11,692,090 / 6,220,480 / 1.82 s    |                                  35,637 |
| `get_contextual_type`                       | 2,831 / 289 / 0.00 s                | 5,379,929 / 2,245,312 / 0.66 s     |                                   7,934 |
| `type_to_string_exported`                   | 1 / 0                               | 162,295 / 131,076 / 0.51 s         |                                       0 |
| `get_contextual_type_for_argument_at_index` | 0                                   | 4,241,762 / 2,359,761 / 0.39 s     |                                       0 |
| `get_type_of_symbol_at_location`            | 20,047 / 0                          | 6,563,052 / 2,136,698 / 0.37 s     |                                       0 |
| `get_return_type_of_signature`              | 148,994 / 19,310 / 0.00 s           | 2,136,009 / 1,874,049 / 0.26 s     |                                       0 |
| `get_symbol_at_location_exported`           | 151,121 / 0                         | 4,488,289 / 761,085 / 0.22 s       |                                       2 |
| `get_resolved_signature_exported`           | 0                                   | 1,424,407 / 463,049 / 0.07 s       |                                       0 |

- normal: nothing worth caching.
- all59: `get_type_at_location` and `get_contextual_type` return a different type on repeat in 0.6% / 0.4% of
  cases (fresh object/array literal and similar types are created per check), so a cache would change what later
  rules see. Not done.
- The repeats that return the same result are mostly cheap calls whose cost here is largely the instrumentation;
  `type_to_string_exported` is the exception (about 4 us per print), hence the fix below. Most repeats are across
  rules (no-misused-promises first, then strict-void-return, no-unsafe-call, strict-boolean-expressions, ...); the
  only large within-rule ones are no-misused-promises asking the same argument's contextual type once per signature
  parameter (2.36M repeats, ~0.15 s after subtracting instrumentation).

## Fix: print each type once per file

`utils::type_to_string` memoizes `type_to_string_exported` per file (cleared at the start of each file, so a cached
string never crosses files or checkers); all 45 rule and util call sites go through it. 95,400 of 162,295 prints on
all59 are served from the cache (per checker 131,076 would be; per file keeps it trivially safe and small).
Printing builds synthesized type nodes in tsrs's leak arena, so each print also costs memory that is never returned:

| payload, checkers | binary | instructions G      | peak footprint GiB         | page reclaims |
| ----------------- | ------ | ------------------- | -------------------------- | ------------- |
| all59, 4          | main   | 537.9 [534.8-546.0] | 6.65 [6.64-6.66]           | 438.6k        |
|                   | fix    | 533.1 [530.5-537.3] | 6.49 [6.48-6.51] **-2.4%** | 427.2k        |
| all59, 1          | main   | 429.7 [428.8-434.4] | 5.08 [5.08-5.09]           | 335.6k        |
|                   | fix    | 440.6 [439.3-440.9] | 4.92 [4.91-4.92] **-3.1%** | 323.4k        |
| normal, 4         | main   | 376.9 [375.9-381.0] | 5.44 [5.43-5.44]           |               |
|                   | fix    | 375.8 [372.5-386.4] | 5.44 [5.42-5.45]           |               |

3 interleaved pairs each, medians [min-max], load 32-166 (all59 4 checkers 36-57, normal 34-43). The instruction
counts include kernel instructions and follow system time here (the single-checker fix runs had 2x the involuntary
context switches and 2x the sys time of their pairs), so the instruction effect of removing ~0.4 s of printing is
below what this machine resolves; the footprint and page-reclaim drops are consistent in every pair. The expected CPU
saving is the measured repeat cost, ~0.4 s of all59 lint CPU (~1%). Output: same diagnostic keys as `bump` on both
payloads and the same full output hash as main in every run (all59 4 checkers `78a72615f3a5`, 1 checker
`c3eeac7f2cfc`, normal `3c5cc6d071bc`); rule tests 59/59.

## What is left, and where

- 1 checker vs 4 on the same binary (`single_threaded`, a local bench switch): normal 294.5 G vs 376-381 G
  instructions, all59 429-434 G vs 535-546 G, about 20% fewer. Most of that is presumably the 4 checkers resolving
  the same declarations, but `single_threaded` also makes program creation sequential, so the split is not
  measured (scheduling lane).
- Pre-lint FS syscalls: 17.4 s CPU on the normal payload, as much as the whole lint phase (tsconfig/FS lane).
- Memory: every `type_to_string` leaks its synthesized nodes into the arena (~1.8 KB per print by the footprint
  drop above); printing in a scoped arena would remove the rest (memory lane).
- Checker throughput itself: flat, memory-bound profile; the cpu-checker passes in tsrs already took the obvious
  items. Not attempted here.
