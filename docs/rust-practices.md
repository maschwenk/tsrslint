# Rust practices

[porting.md](porting.md) says how tsgolint's Go becomes Rust. This file says what the Rust must not do and how that
is checked. The rules are the ones tsrs uses; `docs/RUST.md` in maschwenk/tsrs has the reason for each lint, where it
was taken from (Bun, oxc, Rolldown, rust-analyzer, Ruff, Biome, Turborepo, swc), and the list of performance
techniques that are in place, rejected or untried there.

## The lint ratchet

```sh
tools/lint/ratchet.py        # must pass before a change lands; CI runs it as the `lint-ratchet` job
```

`[workspace.lints]` in `Cargo.toml` and `clippy.toml` set the lints. `tools/lint/baseline.tsv` records the findings
that existed when they were turned on, as one count per lint and file. The ratchet runs clippy and fails when a
(lint, file) count is above its baseline: existing findings are not errors, but a change cannot add one.

When it fails, it prints the new findings, on the lines the branch changed when git can tell.

- Fix the finding. `cargo +<toolchain> clippy` shows clippy's suggestion.
- If the flagged code is intended, put `#[expect(clippy::<lint>, reason = "...")]` on the smallest item or statement
  that covers it. For `inline_always` and the unchecked operations the reason cites the measurement. Use `#[expect]`,
  not `#[allow]`: it warns once the lint stops firing. A bare `#[allow]` is itself a finding.
- Do not raise `baseline.tsv`.

Paying findings down is separate work: `tools/lint/ratchet.py --list clippy::<lint>`, fix them, run the rule tests,
then `tools/lint/ratchet.py --update`. `--update` only lowers counts, and it lists the rows it left alone because
another OS may still have the findings.

Details:

- Findings depend on the clippy version, so the ratchet runs the toolchain pinned as `TOOLCHAIN` in the script. Keep
  the pin equal to tsrs's. To bump it: change the pin, run `--update --allow-increase` on macOS and on Linux, commit
  the result. From a Mac, the Linux half is
  `docker run --rm -v "$PWD":/w -w /w -e CARGO_TARGET_DIR=/tmp/t rust:<toolchain> bash -c 'rustup component add clippy && python3 tools/lint/ratchet.py --update --allow-increase'`.
- It covers the library and the binary, not the tests. The tsrs crates are dependencies and are linted in tsrs.
- `tools/lint/ratchet.py` is a copy of the one in tsrs; the differences are the constants at the top. Fix a bug in
  both.

## Source checks

`tools/lint/source.py` (CI: the `lint-ratchet` job; a copy of tsrs's) checks what clippy cannot express. A new
`unsafe impl Send` / `Sync` fails until `tools/lint/source.py --update` records it in `tools/lint/send_sync.tsv` (none
today), so every cross-thread escape is a reviewed diff. A `Relaxed` / `Acquire` / `Release` / `AcqRel` ordering needs
a `//` comment on its line or in the three lines above saying why it is enough (`tools/lint/atomics.tsv`: none
without one today).

## What the lints check

Clippy's default groups stay on, as porting.md already asked ("`cargo clippy` clean for your files"). Added on top,
the set tsrs uses:

| Group | Lints | Why |
| --- | --- | --- |
| Clones | `redundant_clone`, `clone_on_copy`, `implicit_clone`, `clone_on_ref_ptr`, `assigning_clones`, `cloned_instead_of_copied` | Go copies pointers; a port turns many of those into `.clone()`. With these on, a `.clone()` that remains is a real copy, and a reference-count bump reads `Arc::clone(&x)`. |
| Allocations | `format_push_string`, `inefficient_to_string`, `needless_collect`, `unnecessary_box_returns`, `rc_buffer`, `vec_box`, `stable_sort_primitive` | A temporary `String`, `Vec` or `Box` that the code did not need. |
| Arguments and stack | `needless_pass_by_value`, `trivially_copy_pass_by_ref`, `large_types_passed_by_value`, `large_stack_arrays`, `large_stack_frames`, `inline_always` | `&str` and `&[T]` in, small `Copy` values by value. `#[inline(always)]` needs a measurement. |
| Hashing | `disallowed_types` (`std::collections::HashMap` / `HashSet`), `set_contains_or_insert`, `zero_sized_map_values`, `iter_over_hash_type` | std's maps use SipHash; use `rustc_hash`. Iterating a hash map or set must never decide the order of diagnostics: output has to match tsgolint byte for byte. |
| Locks and threads | `mutex_atomic`, `mutex_integer`, `rc_mutex`, `significant_drop_in_scrutinee`, `non_send_fields_in_send_ty` | A mutex around a flag or counter, a lock guard held across a whole `match`, a new `unsafe impl Send` over non-`Send` fields. |
| `unsafe` | `unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`, `missing_safety_doc`, `ptr_as_ptr`, `ptr_cast_constness`, `ref_as_ptr`, `borrow_as_ptr`, `cast_ptr_alignment`, `as_ptr_cast_mut`, `transmute_ptr_to_ptr`, `mem_forget`, `disallowed_methods` | Every `unsafe` block states its invariant in a `// SAFETY:` comment. `clippy.toml` lists the unchecked operations (`get_unchecked`, `unwrap_unchecked`, `unreachable_unchecked`, `from_utf8_unchecked`): each needs a reason that cites a measurement. |
| Leftovers | `dbg_macro`, `todo`, `unimplemented`, `disallowed_macros`, `allow_attributes`, `allow_attributes_without_reason` | Debugging output and placeholders do not land; suppressions are `#[expect]` with a reason. |

`or_fun_call` is off, as in tsrs: its fix makes an eager call lazy (`unwrap_or(f())` → `unwrap_or_else(f)`), and Go
evaluates `f` eagerly, so the rewrite can skip a side effect the Go code performs.

## Rules that only review checks

1. **A performance claim comes with numbers.** Instructions retired and peak memory, before and after, same machine
   and payload (`/usr/bin/time -l` on macOS, `perf stat` on Linux). Wall time on a shared machine is noise. Write the
   result under `docs/`, as [perf-lint-cpu.md](perf-lint-cpu.md) and [perf-pgo.md](perf-pgo.md) do.
2. **Check where the time goes before optimizing a rule.** On the normal payload 98% of lint-phase CPU is first-time
   type checking that the rules ask for; rule code, the walk and the output are 1.9% together
   ([perf-lint-cpu.md](perf-lint-cpu.md)). A rule-side change does not move that payload; a checker-side change
   belongs in tsrs.
3. **Build settings are measurements, not defaults.** `[profile.dist]` (fat LTO, one codegen unit) with PGO is
   15 to 16% fewer instructions than without ([perf-pgo.md](perf-pgo.md)). Change a setting only with a measurement.
4. **Comparators are total orders.** Rust's `sort_by` may panic on a comparator that is not one; Go's does not.
   Where the Go comparator is not total, complete it with a tie-breaker.
