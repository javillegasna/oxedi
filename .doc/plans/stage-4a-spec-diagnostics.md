# Stage 4a — Spec `segments` + diagnósticos estructurales · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The spec learns the names and types of every segment's elements and rejects malformed or ambiguous specs in plain words; the engine says which segment opened each loop; and a first consumer of the event stream, the `EnvelopeChecker`, reports structural (SNIP 1) problems as self-explanatory `Diagnostic`s.

**Architecture:** `spec` gains a shape pre-check over the raw `serde_json::Value`, a global `segments` section (elements keyed by canonical position, typed, optionally composite), a per-loop `control` object for envelopes, and stricter validation (empty ids anywhere, overlapping sibling triggers). `engine` adds the trigger index to `Event::LoopOpened`; `tree` mirrors it in `Node::opened_by`. The new `diagnostic` module holds `Diagnostic`, `Rule`, `SnipLevel` and `LoopRef` as owned values whose `Display` is the contract. The new `check` module holds `EnvelopeChecker`, which follows the engine's events segment by segment and reads every envelope rule from the spec. Nothing in `src/` names `ST`, `SE` or any other 835 segment.

**Tech Stack:** Rust edition 2024 (stable, 1.88+ for `let` chains), `serde` 1 and `serde_json` 1 (no new dependencies); `proptest` and `criterion` already present.

**Spec:** `.doc/architectural-commitment.md` — §7 "Stage 4 · Proyección a dominio + validación": T11 (`Diagnostic`), T12 (`LoopOpened.segment`), T13 (`segments` section and the validation pass that closes #7, #17, #18), T17 level 1 (envelope rules), argued from N1, N3, P1, P2, P6, P7, P10. Columns, `tables`, the `Projector`, SNIP 2 emission and the `Processor` (T14–T16, T17 level 2) are plan 4b.

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/`.

## Global Constraints

- Edition 2024, stable toolchain. `[dependencies]` of `edi835_core` stays exactly `serde` and `serde_json`; nothing that does I/O, threads or a runtime. `Cargo.toml` does not change in this plan.
- `git add` names the task's files; never `git add -A` or `git add .`.
- Every commit passes the four gates: `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, `cargo bench --workspace --no-run --locked`.
- No `unwrap`, `expect`, `panic!` or fallible indexing on input in `src/` outside `#[cfg(test)]`. One documented exception: `Spec::builtin_835()`. Indexing with a `LoopId` (`ordinals[id.index()]`, `spec.get(id)`) is allowed because only the spec that owns the vector creates them. Arithmetic on counts uses `saturating_sub` / `checked_*`.
- Comments and doc comments describe implementation only: no stage numbers, principle codes (N1, P10), decision codes (T12), issue numbers or history.
- Every error and diagnostic names the rule that failed, where (loop and key as written for specs; segment index, element/component position and loop path for files) and the offending datum. Each new `SpecError` variant and each `Rule` variant has a full-text `Display` test; `source()` returns the serde error where there is one and `None` otherwise.
- `Event` stays `Copy` and three machine words. `feed` and `EnvelopeChecker::on` return slices of reused buffers; the checker allocates only when it emits a diagnostic or opens an envelope that has a `control`.
- Golden files are regenerated with `UPDATE_GOLDEN=1`, inspected with the command given in Task 1, and committed with the code; never edited by hand. Fixtures and samples are never modified.
- `tests/common/mod.rs` helpers (`all_files`, `load_fixture`, `load_sample`, `run_engine`, `run_engine_keeping`, `events_of`) are reused; Task 7 adds `diagnostics_of`.

## Review Focus

1. **An implicit opening names the descendant's trigger, and every opening's index is the segment captured right after it** (fragment `ST` → `open! interchange #0`, `open! group #0`). Tests: `missing_ancestors_open_implicitly`, `every_explicit_opening_names_its_own_trigger` (Task 1), `every_opening_names_the_segment_captured_right_after_it` over the eleven files (Task 1); the golden diff touches only `open` lines.
2. **A spec that is not an object where an object belongs is rejected before serde, with the path as written** (`[]` at the root, `loops` as an array, a trigger as an array, a patched trigger replaced by an array). Tests: `a_spec_that_is_not_an_object_says_so_in_plain_words`, `every_object_of_the_loop_schema_is_checked_with_its_path`, `a_patched_spec_goes_through_the_same_shape_check` (Task 2), `every_object_of_the_segment_schema_is_checked_with_its_path` (Task 3).
3. **Two siblings that one segment can satisfy, with neither more specific than the other, are rejected naming both loops and both triggers; a bare catch-all beside a conditioned sibling, a strict superset of conditions, and `N1 {1:PR}` beside `N1 {1:PE}` all load.** Tests: `siblings_testing_different_positions_overlap_and_are_rejected`, `a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads`, `a_strict_superset_of_conditions_does_not_overlap`, `siblings_that_differ_at_a_shared_position_do_not_overlap`, `more_specific_trigger_wins_over_a_bare_one` (Task 4).
4. **Segment counts include unmatched segments and exclude empty ones; counts with leading zeros pass; a non-numeric count is a mismatch carrying the text.** Tests: `empty_segments_are_not_counted`, `leading_zeros_in_a_count_are_accepted`, `a_count_that_is_not_a_number_is_a_mismatch_with_the_text_as_datum` (Task 7), and trizetto's `SE01 declares "15" but the count is 18` (the `XX` is counted) in `the_known_anomalies_are_reported_exactly_and_nothing_else` (Task 7).
5. **An implicit envelope never reports its missing end; an explicit one does, at the segment that closed it or at the end of the stream.** Tests: `implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end`, `a_loop_closed_by_an_outer_end_segment_is_unterminated`, `loops_still_open_at_the_end_of_the_stream_are_unterminated` (Task 7), and blue_cross's three diagnostics (Task 7).

---

## File Structure

```
crates/edi835_core/
├── specs/
│   └── 835.json               # + "control" on interchange/group/transaction, + "segments" (29 ids)
├── src/
│   ├── lib.rs                 # + pub mod check, diagnostic; re-exports; crate docs
│   ├── spec.rs                # shape pre-check, segments section, control, empty ids, overlapping triggers
│   ├── engine.rs              # Event::LoopOpened { segment }
│   ├── tree.rs                # Node::opened_by
│   ├── diagnostic.rs          # NEW: Diagnostic, Rule, SnipLevel, LoopRef
│   └── check.rs               # NEW: EnvelopeChecker
├── benches/
│   └── tokenize.rs            # + "check" group
└── tests/
    ├── common/mod.rs          # + diagnostics_of
    ├── engine_golden.rs       # open lines carry " #<index>"
    ├── engine_invariants.rs   # + every opening names the next captured segment
    ├── check_envelope.rs      # NEW: pinned diagnostics over the eleven files
    ├── samples/README.md      # + note on the two excerpt samples and their SE01
    └── golden/*.events.txt    # regenerated once (only `open` lines change)
```

`diagnostic` depends on `spec` (for `ElementType`) and `document` (for `Span`); `check` depends on `engine`, `spec`, `segment`, `element` and `diagnostic`. Nothing below `spec` changes; `engine` does not know `check` or `diagnostic` exists.

## Facts this plan relies on (verified against the files before writing it)

| File | Envelope | What the checker must say |
|---|---|---|
| trizetto_sample.rmt | ST #2 … SE #19, `SE01=15`; 18 segments ST..SE (the bogus `XX` #7 included) | `UnknownSegment` `XX` #7 in `1000A#1`; `ControlCountMismatch` SE01 `15` vs 18 |
| multi_claim_sample.txt | counts and control numbers agree | four `UnknownSegment` (`N3` #19, `N4` #20 in `2000#1/2100#1`; `N3` #34, `N4` #35 in `2000#2/2100#2`) |
| blue_cross_nc_sample.txt | fragment: ST #0 … SE #31, `SE01=33` vs 32; no GS/GE/ISA/IEA | two `ImplicitLoop` naming `ST` at #0; `ControlCountMismatch` SE01 `33` vs 32; no `UnterminatedLoop` (its envelope loops are implicit) |
| edi835_test_file.RMT | ST #2 … SE #77, `SE01=1202` vs 76 (an anonymized excerpt of a larger file) | `ControlCountMismatch` SE01 `1202` vs 76 |
| edi835_test_not_available_claim_id.RMT | ST #2 … SE #256, `SE01=302` vs 255 | `ControlCountMismatch` SE01 `302` vs 255 |
| emedny, united_legacy, davisvision, eyemed, united, versant | SE01 = count; ST02 = SE02; GE01 = 1 = number of ST; GS06 = GE02; IEA01 = 1; ISA13 = IEA02 | nothing |

The three real count mismatches are properties of the committed samples (never edited), not defects of the checker; the integration test pins them.

---
## Task 1: `LoopOpened` carries the segment that opened the loop

**Implementer tier:** Sonnet — four files plus nine regenerated golden files, and the golden diff must be inspected and judged before committing.

**Files:**
- Modify: `crates/edi835_core/src/engine.rs`
- Modify: `crates/edi835_core/src/tree.rs`
- Modify: `crates/edi835_core/tests/engine_golden.rs`
- Modify: `crates/edi835_core/tests/engine_invariants.rs`
- Regenerate: `crates/edi835_core/tests/golden/*.events.txt` (the two `.summary.txt` files must not change)

**Interfaces:**
- Consumes: `LoopEngine`, `LoopTree`, `Spec` as they are today.
- Produces:
  - `Event::LoopOpened { id: LoopId, implicit: bool, segment: usize }` — `segment` is the loop's own trigger, or for an implicit opening the trigger of the descendant that forced the chain. `Event` stays `Copy` and `size_of::<Event>() == 3 * size_of::<usize>()`.
  - `Node::opened_by: Option<usize>` — `Some(segment)` for every loop node, `None` only for the root.
  - Golden line format: `open <loop> #<index>` and `open! <loop> #<index>` (e.g. `open 2100 #12`, `open! group #0`). Summaries already strip the ` #…` suffix, so they do not change.

- [ ] **Step 1: Update the engine tests to the new event shape and add two tests**

In `crates/edi835_core/src/engine.rs`, inside `mod tests`, add the field `segment` to every `Event::LoopOpened { … }` literal. The values, test by test (`implicit` stays as it is):

| Test | Literal | `segment` |
|---|---|---|
| `trigger_opens_child_and_captures_it` | `LoopOpened { id: id(&spec, "A"), .. }` | `0` |
| `sibling_trigger_closes_and_reopens` | `B` | `3` |
| `trigger_condition_must_match` | `C` (second run, `AA~BB~CC*X~`) | `2` |
| `missing_ancestors_open_implicitly` | `A` (implicit), `B` (implicit), `C` in the first `assert_eq!` | `0`, `0`, `0` |
| `missing_ancestors_open_implicitly` | `B` (implicit) in the second `assert_eq!` (`AA~CC*X~`) | `1` |
| `a_reachable_trigger_wins_over_capture_in_the_current_loop` | `D` | `3` |
| `implicit_open_keeps_the_nearest_open_ancestor_below_the_top` | `P` (implicit), `T` | `2`, `2` |
| `a_trigger_matching_at_two_depths_opens_under_the_innermost_parent` | `Y` | `2` |

For example the first one becomes:

```rust
                Event::LoopOpened {
                    id: id(&spec, "A"),
                    implicit: false,
                    segment: 0
                },
```

and the implicit chain of `missing_ancestors_open_implicitly` becomes:

```rust
    #[test]
    fn missing_ancestors_open_implicitly() {
        let spec = spec();
        let (events, path) = run(&spec, b"CC*X~");
        assert_eq!(
            events,
            vec![
                Event::LoopOpened {
                    id: id(&spec, "A"),
                    implicit: true,
                    segment: 0
                },
                Event::LoopOpened {
                    id: id(&spec, "B"),
                    implicit: true,
                    segment: 0
                },
                Event::LoopOpened {
                    id: id(&spec, "C"),
                    implicit: false,
                    segment: 0
                },
                Event::Captured {
                    id: id(&spec, "C"),
                    segment: 0
                },
            ]
        );
        assert_eq!(path, vec!["A", "B", "C"]);
        let (events, _) = run(&spec, b"AA~CC*X~");
        assert_eq!(
            events[0],
            Event::LoopOpened {
                id: id(&spec, "B"),
                implicit: true,
                segment: 1
            },
            "only the missing ancestor is implicit"
        );
    }
```

Then add these two tests before `feed_returns_the_events_of_that_call_only`:

```rust
    #[test]
    fn an_event_stays_three_words() {
        assert_eq!(
            std::mem::size_of::<Event>(),
            3 * std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn every_explicit_opening_names_its_own_trigger() {
        let spec = spec();
        let mut engine = LoopEngine::new(&spec);
        let opened: Vec<(&str, bool, usize)> = segs(b"AA~BB~B1~BB~CC*X~")
            .iter()
            .flat_map(|segment| engine.feed(segment).to_vec())
            .filter_map(|event| match event {
                Event::LoopOpened {
                    id,
                    implicit,
                    segment,
                } => Some((spec.loop_name(id), implicit, segment)),
                _ => None,
            })
            .collect();
        assert_eq!(
            opened,
            vec![
                ("A", false, 0),
                ("B", false, 1),
                ("B", false, 3),
                ("C", false, 4)
            ]
        );
    }
```

- [ ] **Step 2: Add the tree test**

In `crates/edi835_core/src/tree.rs`, inside `mod tests`, add before `empty_input_is_a_lone_root`:

```rust
    #[test]
    fn every_node_but_the_root_knows_the_segment_that_opened_it() {
        let (spec, explicit) = tree(b"AA~A1~BB~B1~BB~AE~AA~");
        assert_eq!(explicit.node(explicit.root()).opened_by, None);
        let opened: Vec<(&str, Option<usize>)> = explicit.nodes()[1..]
            .iter()
            .map(|node| {
                (
                    node.loop_id.map_or("", |id| spec.loop_name(id)),
                    node.opened_by,
                )
            })
            .collect();
        assert_eq!(
            opened,
            vec![
                ("A", Some(0)),
                ("B", Some(2)),
                ("B", Some(4)),
                ("A", Some(6))
            ]
        );
        let (spec, implicit) = tree(b"BB~");
        let a = implicit
            .nodes_of(spec.loop_id("A").unwrap())
            .next()
            .unwrap();
        assert_eq!(
            implicit.node(a).opened_by,
            Some(0),
            "the implicit A was opened for BB"
        );
    }
```

- [ ] **Step 3: Add the invariant over the eleven files and the golden line format**

In `crates/edi835_core/tests/engine_invariants.rs` add before `feeding_from_a_document_or_a_tokenizer_gives_the_same_events`:

```rust
#[test]
fn every_opening_names_the_segment_captured_right_after_it() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let events = common::events_of(&spec, &bytes, delims);
        for (at, event) in events.iter().enumerate() {
            let Event::LoopOpened { segment, .. } = *event else {
                continue;
            };
            let next_capture = events[at..].iter().find_map(|later| match *later {
                Event::Captured { segment, .. } => Some(segment),
                _ => None,
            });
            assert_eq!(
                next_capture,
                Some(segment),
                "{name}: event #{at} {event:?} names a segment other than the next capture"
            );
        }
    }
}
```

In `crates/edi835_core/tests/engine_golden.rs`, in `fn line`, replace the `LoopOpened` arm with:

```rust
        Event::LoopOpened {
            id: l,
            implicit,
            segment,
        } => format!(
            "open{} {} #{segment}",
            if implicit { "!" } else { "" },
            spec.loop_name(l)
        ),
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib engine tree`
Expected: compile errors `pattern does not mention field 'segment'` / `variant 'Event::LoopOpened' has no field named 'segment'` and `struct 'Node' has no field named 'opened_by'`.

- [ ] **Step 5: Implement the event field**

In `crates/edi835_core/src/engine.rs` replace the `LoopOpened` variant with:

```rust
    /// A loop started; `implicit` when it was opened to host a descendant
    /// whose ancestors were absent from the input.
    LoopOpened {
        /// The loop.
        id: LoopId,
        /// `true` when no segment of its own opened it.
        implicit: bool,
        /// Index of the trigger segment that caused the opening: the loop's
        /// own trigger, or for an implicit opening the trigger of the
        /// descendant that needed it.
        segment: usize,
    },
```

Replace `fn open` with:

```rust
    fn open(&mut self, id: LoopId, implicit: bool, segment: usize) {
        self.stack.push(id);
        self.events.push(Event::LoopOpened {
            id,
            implicit,
            segment,
        });
    }
```

and pass the index of the segment being fed at the three call sites in `feed`:

```rust
                self.close_to(depth);
                self.open(child, false, index);
                self.capture(child, index);
```

```rust
            for &ancestor in &chain[first_missing..] {
                self.open(ancestor, true, index);
            }
            self.open(target, false, index);
```

- [ ] **Step 6: Implement `Node::opened_by`**

In `crates/edi835_core/src/tree.rs` add the field to `Node`, after `implicit`:

```rust
    /// Index of the trigger segment that caused the opening (for an implicit
    /// node, the trigger of the descendant that needed it); `None` only for
    /// the root.
    pub opened_by: Option<usize>,
```

In `LoopTree::apply` replace the `LoopOpened` arm's head and the pushed node with:

```rust
            Event::LoopOpened {
                id,
                implicit,
                segment,
            } => {
                let node = NodeId(self.nodes.len());
                self.nodes.push(Node {
                    loop_id: Some(id),
                    implicit,
                    opened_by: Some(segment),
                    parent: Some(current),
```

and in `Node::root` add `opened_by: None,` after `implicit: false,`.

- [ ] **Step 7: Run the unit tests and the invariants**

Run: `cargo test -p edi835_core --lib && cargo test -p edi835_core --test engine_invariants`
Expected: all pass (`engine` 18 tests, `tree` 5, `engine_invariants` 5).

Run: `cargo test -p edi835_core --test engine_golden`
Expected: `event_streams_match_the_golden_files` FAILS with `line 1: actual "open interchange #0", expected "open interchange"` (or the equivalent first `open` line of each of the nine `.events.txt` files). The two summaries are not in the failure list.

- [ ] **Step 8: Regenerate the goldens and inspect the diff**

Run: `UPDATE_GOLDEN=1 cargo test -p edi835_core --test engine_golden`
Then run:

```bash
git diff --stat -- crates/edi835_core/tests/golden
git diff -U0 -- crates/edi835_core/tests/golden | grep -E '^[+-][^+-]' | grep -vE '^[+-]open!? [^ ]+( #[0-9]+)?$'
```

Expected: the stat lists exactly the nine `.events.txt` files with as many insertions as deletions (759/759) and no `.summary.txt`; the second command prints nothing (every changed line is an `open` line). Spot check: `head -3 crates/edi835_core/tests/golden/blue_cross_nc_sample.txt.events.txt` prints `open! interchange #0`, `open! group #0`, `open transaction #0`. Then:

Run: `cargo test -p edi835_core --test engine_golden`
Expected: 4 passed.

- [ ] **Step 9: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/engine.rs crates/edi835_core/src/tree.rs crates/edi835_core/tests/engine_golden.rs crates/edi835_core/tests/engine_invariants.rs crates/edi835_core/tests/golden/*.events.txt
git commit -m "engine: LoopOpened carries the index of the segment that opened the loop

Implicit openings carry the trigger of the descendant that needed them.
LoopTree records it as Node::opened_by. Event stays three words.
Golden event streams regenerated: only open lines gain ' #<index>'."
```

---
## Task 2: Shape pre-check — objects where the schema has objects

**Implementer tier:** Haiku — one file, and this step holds every line of code and test.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`

**Interfaces:**
- Consumes: `Spec::from_value` (every load and every patch goes through it).
- Produces:
  - `SpecError::NotAnObject { path: String, found: &'static str }` — `path` joins the keys as written with `.` (`loops.2100.trigger`); the top level has the empty path. `found` is `an array`, `a string`, `a number`, `a boolean` or `null`.
  - `Display`: top level → `the spec must be a JSON object; found an array`; anywhere else → `spec: the value at loops.2100.trigger must be a JSON object; found an array`.
  - Private `fn kind_of(&Value) -> &'static str`, `fn object_at(&Value, &str) -> Result<&Map, SpecError>`, `fn check_shape(&Value) -> Result<(), SpecError>`; Task 3 extends `check_shape` with the `segments` section and Task 7 with `control`.

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`:

1. In `a_malformed_top_level_is_a_schema_error_without_a_loop`, change the input from `"[]"` to `r#"{"name":"t"}"#` (a top-level `[]` is now a `NotAnObject`; a missing `loops` key is still a schema error without a loop). The rest of the test stays.

2. Add before `a_spec_without_loops_is_rejected`:

```rust
    #[test]
    fn a_spec_that_is_not_an_object_says_so_in_plain_words() {
        let err = Spec::from_json("[]").unwrap_err();
        assert!(
            matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path.is_empty()),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found an array"
        );
    }

    #[test]
    fn every_object_of_the_loop_schema_is_checked_with_its_path() {
        let cases = [
            (r#"{"name":"t","loops":[]}"#, "loops", "an array"),
            (r#"{"name":"t","loops":{"a":"AA"}}"#, "loops.a", "a string"),
            (
                r#"{"name":"t","loops":{"a":{"trigger":["AA"]}}}"#,
                "loops.a.trigger",
                "an array",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":1}}}}"#,
                "loops.a.trigger.where",
                "a number",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":null}}}}"#,
                "loops.a.trigger.where",
                "null",
            ),
        ];
        for (json, expected_path, expected_found) in cases {
            let err = Spec::from_json(json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{json}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patched_spec_goes_through_the_same_shape_check() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2100":{"trigger":["CLP"]}}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
    }

    #[test]
    fn every_kind_of_json_value_is_named() {
        use serde_json::json;
        assert_eq!(kind_of(&json!(null)), "null");
        assert_eq!(kind_of(&json!(true)), "a boolean");
        assert_eq!(kind_of(&json!(1)), "a number");
        assert_eq!(kind_of(&json!("x")), "a string");
        assert_eq!(kind_of(&json!([])), "an array");
        assert_eq!(kind_of(&json!({})), "an object");
    }
```

3. Add before `unknown_parent_displays_both_names`:

```rust
    #[test]
    fn not_an_object_displays_the_path_and_what_was_found() {
        let err = SpecError::NotAnObject {
            path: "loops.2100.trigger".into(),
            found: "an array",
        };
        assert_eq!(
            err.to_string(),
            "spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::NotAnObject {
            path: String::new(),
            found: "a string",
        };
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found a string"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: compile errors `no variant named 'NotAnObject'` and `cannot find function 'kind_of'`.

- [ ] **Step 3: Implement the variant**

In `enum SpecError`, add before `Patch`:

```rust
    /// A value the schema requires to be an object is something else.
    NotAnObject {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.2100.trigger`); empty for the top level.
        path: String,
        /// What was found instead: `an array`, `a string`, `a number`,
        /// `a boolean` or `null`.
        found: &'static str,
    },
```

In `impl fmt::Display for SpecError`, add before the `SpecError::Patch` arm:

```rust
            SpecError::NotAnObject { path, found } if path.is_empty() => {
                write!(f, "the spec must be a JSON object; found {found}")
            }
            SpecError::NotAnObject { path, found } => write!(
                f,
                "spec: the value at {path} must be a JSON object; found {found}"
            ),
```

`source()` needs no change: the new variant falls into the `_ => None` arm.

- [ ] **Step 4: Implement the pre-check**

Add these functions above `fn check_cycles`:

```rust
/// How a JSON value is described when it is not the object a spec expects.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The value as an object, or [`SpecError::NotAnObject`] naming `path`.
fn object_at<'v>(
    value: &'v Value,
    path: &str,
) -> Result<&'v serde_json::Map<String, Value>, SpecError> {
    value.as_object().ok_or_else(|| SpecError::NotAnObject {
        path: path.to_string(),
        found: kind_of(value),
    })
}

/// Requires an object everywhere the schema has one, before serde sees the
/// value: serde would accept an array in place of a struct, and its message
/// for the wrong kind of value does not say that an object was expected.
/// Missing keys are left to the schema.
fn check_shape(source: &Value) -> Result<(), SpecError> {
    let root = object_at(source, "")?;
    if let Some(loops) = root.get("loops") {
        for (name, def) in object_at(loops, "loops")? {
            let at = format!("loops.{name}");
            let def = object_at(def, &at)?;
            if let Some(trigger) = def.get("trigger") {
                let at = format!("{at}.trigger");
                let trigger = object_at(trigger, &at)?;
                if let Some(conditions) = trigger.get("where") {
                    object_at(conditions, &format!("{at}.where"))?;
                }
            }
        }
    }
    Ok(())
}
```

and make it the first line of `Spec::from_value`:

```rust
    pub(crate) fn from_value(source: Value) -> Result<Spec, SpecError> {
        check_shape(&source)?;
        let raw: RawSpec =
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: all pass (45 tests in `spec::tests`).

- [ ] **Step 6: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/spec.rs
git commit -m "spec: require objects where the schema has objects, and say so in plain words"
```

---
## Task 3: The `segments` section — element names, types, lengths and composites

**Implementer tier:** Sonnet — one large file plus `lib.rs`; the code is complete here, but serde attributes, the recursive raw type and the error plumbing must be wired without drifting from it.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `check_shape`, `object_at` (Task 2).
- Produces:
  - `pub enum ElementType { An, Id, N(u8), R { scale: u8 }, Dt, Tm }` (`Copy`, `Eq`, `Display`: `AN (string)`, `ID (code)`, `N2 (integer with 2 implied decimals)`, `R (decimal, scale 2)`, `DT (date CCYYMMDD or YYMMDD)`, `TM (time HHMM, HHMMSS or HHMMSSD..)`), read from `"AN"`, `"ID"`, `"N0"`…`"N9"`, `"R"` (+ optional `"scale"`, default 2), `"DT"`, `"TM"`.
  - `pub struct ElementDef { pub name: String, pub kind: ElementType, pub required: bool, pub min: Option<usize>, pub max: Option<usize>, pub composite: BTreeMap<usize, ElementDef> }`
  - `pub struct SegmentDef { pub elements: BTreeMap<usize, ElementDef> }`
  - `pub enum ElementDefError { NonCanonicalPosition, EmptyName, DuplicateName { name, first }, UnknownType { found }, ScaleWithoutR { kind }, MinAboveMax { min, max }, CompositeOnNonAn { kind }, NestedComposite }` with `Display`.
  - `SpecError::SegmentSchema { segment: String, source: serde_json::Error }` and `SpecError::BadElementDef { segment: String, position: String, reason: ElementDefError }`; a component's `position` is written `<element>.composite.<component>` (e.g. `1.composite.2`).
  - `Spec::segment(&self, id: &[u8]) -> Option<&SegmentDef>`, `Spec::segments(&self) -> impl Iterator<Item = (&[u8], &SegmentDef)>` (ordered by id).
  - JSON shape: `"segments": { "CLP": { "elements": { "1": { "name": "claim_submitter_identifier", "type": "AN", "required": true, "min": 1, "max": 38 } } } }`. `required` defaults to `false`; `elements` defaults to empty; unknown keys are schema errors.
  - Names must be unique among siblings (the elements of a segment; the components of one composite). A segment a loop lists with no entry here is valid and opaque.

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `patch_adds_a_loop`:

```rust
    const CLP_ONLY: &str = r#"{"name":"t",
        "loops":{"2100":{"trigger":{"segment":"CLP"},"segments":["ZZ1"]}},
        "segments":{"CLP":{"elements":{
            "1":{"name":"claim_submitter_id","type":"AN","required":true,"min":1,"max":38},
            "3":{"name":"total_claim_charge_amount","type":"R","required":true},
            "12":{"name":"drg_weight","type":"R","scale":4}
        }},
        "SVC":{"elements":{
            "1":{"name":"procedure","type":"AN","required":true,"composite":{
                "1":{"name":"qualifier","type":"ID","required":true,"min":2,"max":2},
                "2":{"name":"code","type":"AN","required":true}
            }},
            "5":{"name":"units","type":"N0"}
        }}}
    }"#;

    fn element_error(elements: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{{"AA":{{"elements":{elements}}}}}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn segments_are_keyed_by_id_and_elements_by_position() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        let first = &clp.elements[&1];
        assert_eq!(first.name, "claim_submitter_id");
        assert_eq!(first.kind, ElementType::An);
        assert!(first.required);
        assert_eq!((first.min, first.max), (Some(1), Some(38)));
        assert_eq!(clp.elements[&3].kind, ElementType::R { scale: 2 });
        assert!(!clp.elements[&12].required, "required defaults to false");
        assert_eq!(clp.elements[&12].kind, ElementType::R { scale: 4 });
        assert_eq!(
            clp.elements.keys().copied().collect::<Vec<_>>(),
            vec![1, 3, 12]
        );
        let svc = spec.segment(b"SVC").unwrap();
        let procedure = &svc.elements[&1];
        assert_eq!(procedure.composite[&1].kind, ElementType::Id);
        assert_eq!(procedure.composite[&2].name, "code");
        assert_eq!(svc.elements[&5].kind, ElementType::N(0));
        let ids: Vec<&[u8]> = spec.segments().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![&b"CLP"[..], &b"SVC"[..]]);
    }

    #[test]
    fn every_type_code_is_read() {
        let cases = [
            ("AN", None, ElementType::An),
            ("ID", None, ElementType::Id),
            ("N0", None, ElementType::N(0)),
            ("N2", None, ElementType::N(2)),
            ("N9", None, ElementType::N(9)),
            ("R", None, ElementType::R { scale: 2 }),
            ("R", Some(6), ElementType::R { scale: 6 }),
            ("DT", None, ElementType::Dt),
            ("TM", None, ElementType::Tm),
        ];
        for (code, scale, expected) in cases {
            assert_eq!(ElementType::parse(code, scale), Ok(expected), "{code}");
        }
        for code in ["an", "N", "N10", "NA", "R2", "", "B"] {
            assert_eq!(
                ElementType::parse(code, None),
                Err(ElementDefError::UnknownType {
                    found: code.to_string()
                }),
                "{code:?}"
            );
        }
    }

    #[test]
    fn element_types_display_their_code_and_meaning() {
        assert_eq!(ElementType::An.to_string(), "AN (string)");
        assert_eq!(ElementType::Id.to_string(), "ID (code)");
        assert_eq!(
            ElementType::N(2).to_string(),
            "N2 (integer with 2 implied decimals)"
        );
        assert_eq!(
            ElementType::R { scale: 2 }.to_string(),
            "R (decimal, scale 2)"
        );
        assert_eq!(ElementType::Dt.to_string(), "DT (date CCYYMMDD or YYMMDD)");
        assert_eq!(
            ElementType::Tm.to_string(),
            "TM (time HHMM, HHMMSS or HHMMSSD..)"
        );
    }

    #[test]
    fn a_segment_a_loop_lists_without_a_definition_stays_opaque() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        assert!(spec.get(spec.loop_id("2100").unwrap()).accepts(b"ZZ1"));
        assert_eq!(spec.segment(b"ZZ1"), None);
    }

    #[test]
    fn a_spec_without_segments_has_none() {
        let spec =
            Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}}}"#).unwrap();
        assert_eq!(spec.segments().count(), 0);
    }

    #[test]
    fn bad_element_definitions_are_rejected_with_segment_position_and_reason() {
        let cases = [
            (
                r#"{"01":{"name":"a","type":"AN"}}"#,
                "01",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"0":{"name":"a","type":"AN"}}"#,
                "0",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"1":{"name":"","type":"AN"}}"#,
                "1",
                ElementDefError::EmptyName,
            ),
            (
                r#"{"1":{"name":"a","type":"AN"},"2":{"name":"a","type":"ID"}}"#,
                "2",
                ElementDefError::DuplicateName {
                    name: "a".into(),
                    first: "1".into(),
                },
            ),
            (
                r#"{"1":{"name":"a","type":"XX"}}"#,
                "1",
                ElementDefError::UnknownType { found: "XX".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"N2","scale":2}}"#,
                "1",
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","min":5,"max":2}}"#,
                "1",
                ElementDefError::MinAboveMax { min: 5, max: 2 },
            ),
            (
                r#"{"1":{"name":"a","type":"ID","composite":{"1":{"name":"b","type":"AN"}}}}"#,
                "1",
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"2":{"name":"b","type":"AN","composite":{"1":{"name":"c","type":"AN"}}}}}}"#,
                "1.composite.2",
                ElementDefError::NestedComposite,
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"1":{"name":"b","type":"AN"},"2":{"name":"b","type":"AN"}}}}"#,
                "1.composite.2",
                ElementDefError::DuplicateName {
                    name: "b".into(),
                    first: "1".into(),
                },
            ),
        ];
        for (elements, expected_position, expected_reason) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == expected_position && *reason == expected_reason),
                "{elements}: {err:?}"
            );
        }
    }

    #[test]
    fn names_only_need_to_be_unique_among_siblings() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{
                "1":{"name":"code","type":"AN","composite":{"1":{"name":"code","type":"AN"}}}
            }}}}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
    }

    #[test]
    fn a_segment_definition_that_breaks_the_schema_names_the_segment() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"AN","lenght":3}}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::SegmentSchema { segment, .. } if segment == "AA"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("segment \"AA\" does not match the schema: "),
            "{err}"
        );
        assert!(err.to_string().contains("lenght"), "{err}");
    }

    #[test]
    fn every_object_of_the_segment_schema_is_checked_with_its_path() {
        let cases = [
            (r#"[]"#, "segments", "an array"),
            (r#"{"CLP":[]}"#, "segments.CLP", "an array"),
            (
                r#"{"CLP":{"elements":[]}}"#,
                "segments.CLP.elements",
                "an array",
            ),
            (
                r#"{"CLP":{"elements":{"1":"claim_id"}}}"#,
                "segments.CLP.elements.1",
                "a string",
            ),
            (
                r#"{"SVC":{"elements":{"1":{"name":"p","type":"AN","composite":{"2":7}}}}}"#,
                "segments.SVC.elements.1.composite.2",
                "a number",
            ),
        ];
        for (segments, expected_path, expected_found) in cases {
            let json = format!(
                r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{segments}}}"#
            );
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{segments}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patch_retouches_one_element_and_keeps_the_rest() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(r#"{"segments":{"CLP":{"elements":{"1":{"max":30}}}}}"#)
            .unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        assert_eq!(clp.elements[&1].max, Some(30));
        assert_eq!(clp.elements[&1].name, "claim_submitter_id");
        assert_eq!(clp.elements.len(), 3);
    }

    #[test]
    fn a_patch_adds_a_segment_definition() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(
                r#"{"segments":{"ZZ1":{"elements":{"1":{"name":"payer_note","type":"AN"}}}}}"#,
            )
            .unwrap();
        assert_eq!(
            spec.segment(b"ZZ1").unwrap().elements[&1].name,
            "payer_note"
        );
    }

    #[test]
    fn to_json_round_trips_the_segments_section() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert!(spec.segments().eq(again.segments()));
        assert_eq!(again.segments().count(), 2);
    }

    #[test]
    fn segment_schema_error_displays_the_segment_and_the_serde_message() {
        let err = SpecError::SegmentSchema {
            segment: "CLP".into(),
            source: serde_json::from_value::<RawSegment>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "segment \"CLP\" does not match the schema: invalid type: integer `1`, expected a segment object"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn bad_element_def_displays_segment_position_and_every_reason() {
        let cases = [
            (
                ElementDefError::NonCanonicalPosition,
                "segment \"CLP\" element \"01\": positions are 1-based integers written in canonical form",
            ),
            (
                ElementDefError::EmptyName,
                "segment \"CLP\" element \"01\": \"name\" is empty",
            ),
            (
                ElementDefError::DuplicateName {
                    name: "claim_id".into(),
                    first: "1".into(),
                },
                "segment \"CLP\" element \"01\": name \"claim_id\" is already used by position \"1\"",
            ),
            (
                ElementDefError::UnknownType { found: "XX".into() },
                "segment \"CLP\" element \"01\": type \"XX\" is not one of AN, ID, N0 to N9, R, DT, TM",
            ),
            (
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
                "segment \"CLP\" element \"01\": \"scale\" applies only to type R; found type \"N2\"",
            ),
            (
                ElementDefError::MinAboveMax { min: 5, max: 2 },
                "segment \"CLP\" element \"01\": \"min\" 5 is greater than \"max\" 2",
            ),
            (
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
                "segment \"CLP\" element \"01\": \"composite\" requires type AN; found type \"ID\"",
            ),
            (
                ElementDefError::NestedComposite,
                "segment \"CLP\" element \"01\": a component cannot declare its own \"composite\"",
            ),
        ];
        for (reason, expected) in cases {
            let err = SpecError::BadElementDef {
                segment: "CLP".into(),
                position: "01".into(),
                reason,
            };
            assert_eq!(err.to_string(), expected);
            assert!(std::error::Error::source(&err).is_none());
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: compile errors `cannot find type 'ElementType'`, `no method named 'segment' found for struct 'Spec'`, `no variant named 'BadElementDef'`.

- [ ] **Step 3: Add the public types**

Update the module doc of `spec.rs` (its last sentence) to:

```rust
//! and an optional segment that closes it. An optional `segments` section
//! names and types the elements of each segment id, wherever the segment
//! appears. Loading compiles that into index-based definitions so the engine
//! never compares strings, and keeps the JSON value so patches can be applied
//! on top.
```

Insert after `impl LoopDef { … }` (before `/// Why a spec could not be loaded`):

```rust
/// The data type of an element, as the X12 standard names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// `AN`: a string.
    An,
    /// `ID`: a code from a list.
    Id,
    /// `N0` to `N9`: an integer with that many implied decimal places.
    N(u8),
    /// `R`: a decimal number with an explicit point, kept at `scale` places.
    R {
        /// Decimal places; 2 unless the spec says otherwise.
        scale: u8,
    },
    /// `DT`: a date, `CCYYMMDD` or `YYMMDD`.
    Dt,
    /// `TM`: a time, `HHMM` optionally followed by seconds and decimal seconds.
    Tm,
}

impl ElementType {
    /// Reads the type code of an element definition; `scale` is the
    /// definition's `scale` key, which only `R` accepts.
    fn parse(code: &str, scale: Option<u8>) -> Result<ElementType, ElementDefError> {
        let kind = match code.as_bytes() {
            b"AN" => ElementType::An,
            b"ID" => ElementType::Id,
            b"R" => ElementType::R {
                scale: scale.unwrap_or(2),
            },
            b"DT" => ElementType::Dt,
            b"TM" => ElementType::Tm,
            [b'N', digit @ b'0'..=b'9'] => ElementType::N(digit - b'0'),
            _ => {
                return Err(ElementDefError::UnknownType {
                    found: code.to_string(),
                });
            }
        };
        if scale.is_some() && !matches!(kind, ElementType::R { .. }) {
            return Err(ElementDefError::ScaleWithoutR {
                kind: code.to_string(),
            });
        }
        Ok(kind)
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementType::An => write!(f, "AN (string)"),
            ElementType::Id => write!(f, "ID (code)"),
            ElementType::N(places) => {
                write!(f, "N{places} (integer with {places} implied decimals)")
            }
            ElementType::R { scale } => write!(f, "R (decimal, scale {scale})"),
            ElementType::Dt => write!(f, "DT (date CCYYMMDD or YYMMDD)"),
            ElementType::Tm => write!(f, "TM (time HHMM, HHMMSS or HHMMSSD..)"),
        }
    }
}

/// One element of a segment, or one component of a composite element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDef {
    /// Name used for the element downstream, e.g. `claim_submitter_id`.
    pub name: String,
    /// Data type.
    pub kind: ElementType,
    /// `true` when the element must be present and non-empty.
    pub required: bool,
    /// Minimum length, when the spec sets one.
    pub min: Option<usize>,
    /// Maximum length, when the spec sets one.
    pub max: Option<usize>,
    /// Components by 1-based position; empty for a simple element.
    pub composite: BTreeMap<usize, ElementDef>,
}

/// The elements of one segment id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// Elements by 1-based position. Positions with no entry are opaque.
    pub elements: BTreeMap<usize, ElementDef>,
}

/// Why an element definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementDefError {
    /// The position key is not a 1-based integer in canonical form.
    NonCanonicalPosition,
    /// `name` is the empty string.
    EmptyName,
    /// Another element at the same level already has this name.
    DuplicateName {
        /// The repeated name.
        name: String,
        /// Position key of the element that used it first, as written.
        first: String,
    },
    /// `type` is not one of the known codes.
    UnknownType {
        /// The code as written.
        found: String,
    },
    /// `scale` was given for a type other than `R`.
    ScaleWithoutR {
        /// The type code as written.
        kind: String,
    },
    /// `min` is greater than `max`.
    MinAboveMax {
        /// The minimum as written.
        min: usize,
        /// The maximum as written.
        max: usize,
    },
    /// `composite` was given for a type other than `AN`.
    CompositeOnNonAn {
        /// The type code as written.
        kind: String,
    },
    /// A component declares a `composite` of its own.
    NestedComposite,
}

impl fmt::Display for ElementDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementDefError::NonCanonicalPosition => write!(
                f,
                "positions are 1-based integers written in canonical form"
            ),
            ElementDefError::EmptyName => write!(f, "\"name\" is empty"),
            ElementDefError::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by position {first:?}")
            }
            ElementDefError::UnknownType { found } => write!(
                f,
                "type {found:?} is not one of AN, ID, N0 to N9, R, DT, TM"
            ),
            ElementDefError::ScaleWithoutR { kind } => {
                write!(f, "\"scale\" applies only to type R; found type {kind:?}")
            }
            ElementDefError::MinAboveMax { min, max } => {
                write!(f, "\"min\" {min} is greater than \"max\" {max}")
            }
            ElementDefError::CompositeOnNonAn { kind } => {
                write!(f, "\"composite\" requires type AN; found type {kind:?}")
            }
            ElementDefError::NestedComposite => {
                write!(f, "a component cannot declare its own \"composite\"")
            }
        }
    }
}
```

- [ ] **Step 4: Add the error variants**

Change the doc line of `enum SpecError` to `/// Why a spec could not be loaded. Each variant names where in the spec the fault is.` and add before `AmbiguousTrigger`:

```rust
    /// A segment definition does not match the schema.
    SegmentSchema {
        /// The segment id as written.
        segment: String,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// An element definition is invalid.
    BadElementDef {
        /// The segment id as written.
        segment: String,
        /// The element's key as written; a component is `<element>.composite.<component>`.
        position: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
```

In `Display`, add before the `SpecError::AmbiguousTrigger` arm:

```rust
            SpecError::SegmentSchema { segment, source } => {
                write!(f, "segment {segment:?} does not match the schema: {source}")
            }
            SpecError::BadElementDef {
                segment,
                position,
                reason,
            } => write!(f, "segment {segment:?} element {position:?}: {reason}"),
```

In `source()`, replace the first arm with:

```rust
            SpecError::Json(e)
            | SpecError::Schema { source: e, .. }
            | SpecError::SegmentSchema { source: e, .. } => Some(e),
```

- [ ] **Step 5: Deserialize and compile the section**

Add the `segments` field to `struct RawSpec` (its attributes stay as they are), and add the two raw types after `struct RawTrigger`:

```rust
struct RawSpec {
    name: String,
    loops: BTreeMap<String, Value>,
    #[serde(default)]
    segments: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a segment object")]
struct RawSegment {
    #[serde(default)]
    elements: BTreeMap<String, RawElement>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "an element object")]
struct RawElement {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    required: bool,
    min: Option<usize>,
    max: Option<usize>,
    scale: Option<u8>,
    #[serde(default)]
    composite: BTreeMap<String, RawElement>,
}
```

Add the field to `Spec`:

```rust
/// A loaded, validated loop structure.
#[derive(Debug, Clone)]
pub struct Spec {
    name: String,
    loops: Vec<LoopDef>,
    roots: Vec<LoopId>,
    segments: BTreeMap<Vec<u8>, SegmentDef>,
    source: Value,
}
```

Add the accessors to `impl Spec`, before `children`:

```rust
    /// The element definitions of a segment id, if the spec has any.
    pub fn segment(&self, id: &[u8]) -> Option<&SegmentDef> {
        self.segments.get(id)
    }

    /// Every defined segment id with its definition, ordered by id.
    pub fn segments(&self) -> impl Iterator<Item = (&[u8], &SegmentDef)> {
        self.segments.iter().map(|(id, def)| (id.as_slice(), def))
    }
```

In `from_value`, replace the parsing of each `where` key with the shared helper:

```rust
            for (position, value) in &def.trigger.conditions {
                let parsed = parse_position(position).ok_or_else(|| SpecError::BadPosition {
                    loop_name: name.clone(),
                    position: position.clone(),
                })?;
                conditions.push((parsed, value.as_bytes().to_vec()));
            }
```

and replace the construction of `spec` at the end of `from_value` (before `spec.check_ambiguity()?;`) with:

```rust
        let mut segments = BTreeMap::new();
        for (id, value) in &raw.segments {
            let def: RawSegment =
                serde_json::from_value(value.clone()).map_err(|e| SpecError::SegmentSchema {
                    segment: id.clone(),
                    source: e,
                })?;
            let elements = compile_elements(id, &def.elements, None)?;
            segments.insert(id.as_bytes().to_vec(), SegmentDef { elements });
        }

        let spec = Spec {
            name: raw.name,
            loops,
            roots,
            segments,
            source,
        };
```

Add above `fn kind_of`:

```rust
/// A 1-based element position written in canonical form (`"1"`, never
/// `"01"` or `"+1"`), so no two keys can name the same position.
fn parse_position(key: &str) -> Option<usize> {
    key.parse::<usize>()
        .ok()
        .filter(|&p| p >= 1 && p.to_string() == key)
}

/// Compiles the elements of `segment`, or the components of the element at
/// `parent` (its key as written) when one is given.
fn compile_elements(
    segment: &str,
    raw: &BTreeMap<String, RawElement>,
    parent: Option<&str>,
) -> Result<BTreeMap<usize, ElementDef>, SpecError> {
    let mut elements = BTreeMap::new();
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, def) in raw {
        let position_text = match parent {
            Some(parent) => format!("{parent}.composite.{key}"),
            None => key.clone(),
        };
        let fail = |reason| SpecError::BadElementDef {
            segment: segment.to_string(),
            position: position_text.clone(),
            reason,
        };
        let position =
            parse_position(key).ok_or_else(|| fail(ElementDefError::NonCanonicalPosition))?;
        if def.name.is_empty() {
            return Err(fail(ElementDefError::EmptyName));
        }
        if let Some(first) = names.insert(def.name.as_str(), key.as_str()) {
            return Err(fail(ElementDefError::DuplicateName {
                name: def.name.clone(),
                first: first.to_string(),
            }));
        }
        let kind = ElementType::parse(&def.kind, def.scale).map_err(fail)?;
        if let (Some(min), Some(max)) = (def.min, def.max)
            && min > max
        {
            return Err(fail(ElementDefError::MinAboveMax { min, max }));
        }
        let composite = if def.composite.is_empty() {
            BTreeMap::new()
        } else if parent.is_some() {
            return Err(fail(ElementDefError::NestedComposite));
        } else if kind != ElementType::An {
            return Err(fail(ElementDefError::CompositeOnNonAn {
                kind: def.kind.clone(),
            }));
        } else {
            compile_elements(segment, &def.composite, Some(key))?
        };
        elements.insert(
            position,
            ElementDef {
                name: def.name.clone(),
                kind,
                required: def.required,
                min: def.min,
                max: def.max,
                composite,
            },
        );
    }
    Ok(elements)
}
```

- [ ] **Step 6: Extend the shape pre-check**

Replace `fn check_shape` with the version that also walks `segments`, and add `check_elements_shape` after it:

```rust
/// Requires an object everywhere the schema has one, before serde sees the
/// value: serde would accept an array in place of a struct, and its message
/// for the wrong kind of value does not say that an object was expected.
/// Missing keys are left to the schema.
fn check_shape(source: &Value) -> Result<(), SpecError> {
    let root = object_at(source, "")?;
    if let Some(loops) = root.get("loops") {
        for (name, def) in object_at(loops, "loops")? {
            let at = format!("loops.{name}");
            let def = object_at(def, &at)?;
            if let Some(trigger) = def.get("trigger") {
                let at = format!("{at}.trigger");
                let trigger = object_at(trigger, &at)?;
                if let Some(conditions) = trigger.get("where") {
                    object_at(conditions, &format!("{at}.where"))?;
                }
            }
        }
    }
    if let Some(segments) = root.get("segments") {
        for (id, def) in object_at(segments, "segments")? {
            let at = format!("segments.{id}");
            let def = object_at(def, &at)?;
            if let Some(elements) = def.get("elements") {
                check_elements_shape(elements, &format!("{at}.elements"))?;
            }
        }
    }
    Ok(())
}

/// Requires every element (and every component) definition to be an object.
fn check_elements_shape(elements: &Value, at: &str) -> Result<(), SpecError> {
    for (position, def) in object_at(elements, at)? {
        let at = format!("{at}.{position}");
        let def = object_at(def, &at)?;
        if let Some(composite) = def.get("composite") {
            check_elements_shape(composite, &format!("{at}.composite"))?;
        }
    }
    Ok(())
}
```

- [ ] **Step 7: Export the types**

In `crates/edi835_core/src/lib.rs` replace the `spec` re-export with:

```rust
pub use spec::{
    ElementDef, ElementDefError, ElementType, LoopDef, LoopId, SegmentDef, Spec, SpecError,
    Trigger, merge_patch,
};
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: all pass (59 tests in `spec::tests`).

- [ ] **Step 9: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/spec.rs crates/edi835_core/src/lib.rs
git commit -m "spec: segments section with typed, positional element definitions

Elements are keyed by canonical 1-based position so a patch retouches one
element. Types AN, ID, N0-N9, R (scale), DT, TM; composites on AN only.
Invalid definitions name the segment, the position as written and the reason."
```

---
## Task 4: Empty segment ids anywhere, and overlapping sibling triggers

**Implementer tier:** Haiku — one file, every line of code given; the only care needed is replacing the three existing tests listed below verbatim.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`

**Interfaces:**
- Consumes: `from_value`, `check_ambiguity` (existing), `Trigger`.
- Produces:
  - `SpecError::EmptySegmentId { loop_name: Option<String>, key: String }` (replaces the one-field variant). `key` is `trigger.segment`, `segments[<i>]` or `end` for a loop, and `segments.""` for an empty id used as a key of the `segments` section (then `loop_name` is `None`). Display: `loop "2100" has an empty segment id at segments[3]` / `the spec has an empty segment id at segments.""`.
  - `SpecError::OverlappingTriggers { parent: Option<String>, a: String, b: String, conditions_a: String, conditions_b: String }`, raised when two siblings trigger on the same segment id, no position present in both `where` maps requires different values, and neither condition set is a strict superset of the other (identical triggers keep raising `AmbiguousTrigger`). The `conditions_*` fields hold the whole trigger as text: `"N1" where {1: "PR"}` or `"N1" with no conditions`. Display: `loops "other" and "payer" under "transaction" can open on the same segment: "other" on "N1" where {2: "X"}, "payer" on "N1" where {1: "PR"}, no position they both test requires different values, and neither trigger is more specific than the other`.
  - Allowed: `N1 {}` beside `N1 {1:PR}` (the bare trigger is the catch-all; the engine already prefers the trigger with more conditions), `N1 {1:PR}` beside `N1 {1:PR, 2:X}`, `N1 {1:PR}` beside `N1 {1:PE}`. Rejected: `N1 {1:PR}` beside `N1 {2:X}`.

- [ ] **Step 1: Replace the tests that used the old shapes**

In `mod tests` of `crates/edi835_core/src/spec.rs`:

`more_specific_trigger_wins_over_a_bare_one` stays exactly as it is: a bare trigger beside a conditioned sibling remains valid.

1. Replace `empty_trigger_segment_is_rejected` with:

```rust
    #[test]
    fn empty_segment_ids_are_rejected_with_the_loop_and_the_key() {
        let cases = [
            (r#"{"trigger":{"segment":""}}"#, "trigger.segment"),
            (
                r#"{"trigger":{"segment":"AA"},"segments":["A1",""]}"#,
                "segments[1]",
            ),
            (r#"{"trigger":{"segment":"AA"},"end":""}"#, "end"),
        ];
        for (def, expected_key) in cases {
            let json = format!(r#"{{"name":"t","loops":{{"a":{def}}}}}"#);
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::EmptySegmentId { loop_name: Some(name), key } if name == "a" && key == expected_key),
                "{def}: {err:?}"
            );
        }
    }

    #[test]
    fn an_empty_segment_id_in_the_segments_section_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"":{"elements":{}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == "segments.\"\""),
            "{err:?}"
        );
    }
```

2. Replace `patch_error_displays_the_inner_error_once` with:

```rust
    #[test]
    fn patch_error_displays_the_inner_error_once() {
        let err = SpecError::Patch {
            source: Box::new(SpecError::EmptySegmentId {
                loop_name: Some("a".into()),
                key: "end".into(),
            }),
        };
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"a\" has an empty segment id at end"
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"a\" has an empty segment id at end")
        );
    }
```

3. Replace `empty_segment_id_displays_the_loop` with:

```rust
    #[test]
    fn empty_segment_id_displays_the_loop_and_the_key() {
        let err = SpecError::EmptySegmentId {
            loop_name: Some("2100".into()),
            key: "segments[3]".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" has an empty segment id at segments[3]"
        );
        let err = SpecError::EmptySegmentId {
            loop_name: None,
            key: "segments.\"\"".into(),
        };
        assert_eq!(
            err.to_string(),
            "the spec has an empty segment id at segments.\"\""
        );
    }
```

- [ ] **Step 2: Add the overlap tests**

Add before `to_json_round_trips_through_from_json`:

```rust
    #[test]
    fn siblings_testing_different_positions_overlap_and_are_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "other":{"parent":"transaction","trigger":{"segment":"N1","where":{"2":"X"}}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::OverlappingTriggers { parent: Some(parent), a, b, .. } if parent == "transaction" && a == "other" && b == "payer"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "loops \"other\" and \"payer\" under \"transaction\" can open on the same segment: \"other\" on \"N1\" where {2: \"X\"}, \"payer\" on \"N1\" where {1: \"PR\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "any":{"parent":"transaction","trigger":{"segment":"N1"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        );
        assert!(spec.is_ok(), "{spec:?}");
    }

    #[test]
    fn a_strict_superset_of_conditions_does_not_overlap() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR"}}},
                "acme":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}}
            }}"#,
        )
        .unwrap();
        let segments = segs(b"N1*PR*X~N1*PR*Y~");
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            spec.loop_id("acme"),
            "the more specific trigger wins"
        );
        assert_eq!(
            spec.matching_child(None, &segments[1]),
            spec.loop_id("payer")
        );
    }

    #[test]
    fn siblings_that_differ_at_a_shared_position_do_not_overlap() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}},
                "payee":{"trigger":{"segment":"N1","where":{"1":"PE"}}},
                "other":{"trigger":{"segment":"N3"}}
            }}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
        let builtin = Spec::builtin_835();
        assert!(builtin.loop_id("1000A").is_some() && builtin.loop_id("1000B").is_some());
    }

    #[test]
    fn overlapping_triggers_display_both_loops_and_their_triggers() {
        let err = SpecError::OverlappingTriggers {
            parent: Some("transaction".into()),
            a: "1000A".into(),
            b: "1000C".into(),
            conditions_a: "\"N1\" where {1: \"PR\"}".into(),
            conditions_b: "\"N1\" where {2: \"X\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"1000A\" and \"1000C\" under \"transaction\" can open on the same segment: \"1000A\" on \"N1\" where {1: \"PR\"}, \"1000C\" on \"N1\" where {2: \"X\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::OverlappingTriggers {
            parent: None,
            a: "a".into(),
            b: "b".into(),
            conditions_a: "\"AA\" where {1: \"X\"}".into(),
            conditions_b: "\"AA\" where {2: \"Y\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root can open on the same segment: \"a\" on \"AA\" where {1: \"X\"}, \"b\" on \"AA\" where {2: \"Y\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn triggers_render_with_their_conditions_in_position_order() {
        let bare = Trigger {
            segment: b"N1".to_vec(),
            conditions: Vec::new(),
        };
        assert_eq!(render_trigger(&bare), "\"N1\" with no conditions");
        let conditioned = Trigger {
            segment: b"N1".to_vec(),
            conditions: vec![(1, b"PR".to_vec()), (3, b"X".to_vec())],
        };
        assert_eq!(
            render_trigger(&conditioned),
            "\"N1\" where {1: \"PR\", 3: \"X\"}"
        );
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: compile errors (`EmptySegmentId` has no field `key`, no variant `OverlappingTriggers`, no function `render_trigger`).

- [ ] **Step 4: Implement the variants**

In `enum SpecError` replace `EmptySegmentId` with:

```rust
    /// A segment id is the empty string.
    EmptySegmentId {
        /// The loop holding it; `None` for a key of the `segments` section.
        loop_name: Option<String>,
        /// Where it sits, as written: `trigger.segment`, `segments[<i>]`,
        /// `end`, or `segments.""` for the section.
        key: String,
    },
```

and add after `AmbiguousTrigger` (last variant):

```rust
    /// Two loops with the same parent trigger on the same segment id, no
    /// position tested by both requires different values, and neither set of
    /// conditions contains the other: one segment can satisfy both and
    /// neither trigger is more specific.
    OverlappingTriggers {
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// First loop, in spec order.
        a: String,
        /// Second loop, in spec order.
        b: String,
        /// The first loop's trigger, e.g. `"N1" where {1: "PR"}`.
        conditions_a: String,
        /// The second loop's trigger, written the same way.
        conditions_b: String,
    },
}
```

(The closing `}` above is the end of the enum.) In `Display`, replace the `EmptySegmentId` arm with:

```rust
            SpecError::EmptySegmentId {
                loop_name: Some(loop_name),
                key,
            } => write!(f, "loop {loop_name:?} has an empty segment id at {key}"),
            SpecError::EmptySegmentId {
                loop_name: None,
                key,
            } => write!(f, "the spec has an empty segment id at {key}"),
```

and add after the `AmbiguousTrigger` arm (the last arm of the `match`):

```rust
            SpecError::OverlappingTriggers {
                parent,
                a,
                b,
                conditions_a,
                conditions_b,
            } => {
                write!(f, "loops {a:?} and {b:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(
                    f,
                    " can open on the same segment: {a:?} on {conditions_a}, {b:?} on \
                     {conditions_b}, no position they both test requires different values, \
                     and neither trigger is more specific than the other"
                )
            }
```

- [ ] **Step 5: Validate every segment id**

In `from_value`, replace the trigger check at the top of the `for (name, def) in &defs` loop with:

```rust
        for (name, def) in &defs {
            let empty_at = |key: String| SpecError::EmptySegmentId {
                loop_name: Some(name.clone()),
                key,
            };
            if def.trigger.segment.is_empty() {
                return Err(empty_at("trigger.segment".into()));
            }
            if let Some(i) = def.segments.iter().position(String::is_empty) {
                return Err(empty_at(format!("segments[{i}]")));
            }
            if def.end.as_deref() == Some("") {
                return Err(empty_at("end".into()));
            }
```

and at the top of the `for (id, value) in &raw.segments` loop add:

```rust
            if id.is_empty() {
                return Err(SpecError::EmptySegmentId {
                    loop_name: None,
                    key: "segments.\"\"".into(),
                });
            }
```

- [ ] **Step 6: Reject overlapping siblings**

Replace `fn check_ambiguity` with:

```rust
    fn check_ambiguity(&self) -> Result<(), SpecError> {
        let groups = std::iter::once(self.roots.as_slice())
            .chain(self.loops.iter().map(|def| def.children.as_slice()));
        for siblings in groups {
            for (i, &first) in siblings.iter().enumerate() {
                for &second in &siblings[i + 1..] {
                    let trigger = &self.loops[first.0].trigger;
                    let other = &self.loops[second.0].trigger;
                    if trigger.segment != other.segment {
                        continue;
                    }
                    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                    let parent = self.loops[first.0]
                        .parent
                        .map(|parent| self.loops[parent.0].name.clone());
                    if trigger == other {
                        return Err(SpecError::AmbiguousTrigger {
                            first: self.loops[first.0].name.clone(),
                            second: self.loops[second.0].name.clone(),
                            parent,
                            segment: text(&trigger.segment),
                            conditions: trigger
                                .conditions
                                .iter()
                                .map(|(position, value)| (*position, text(value)))
                                .collect(),
                        });
                    }
                    // Siblings are told apart when a shared position requires
                    // different values, or when one trigger's conditions
                    // contain the other's: the engine then prefers the one
                    // with more conditions and the other is the catch-all.
                    let excluded = trigger.conditions.iter().any(|(position, value)| {
                        other
                            .conditions
                            .iter()
                            .any(|(p, v)| p == position && v != value)
                    });
                    let contains = |big: &Trigger, small: &Trigger| {
                        small
                            .conditions
                            .iter()
                            .all(|condition| big.conditions.contains(condition))
                    };
                    let nested = contains(trigger, other) || contains(other, trigger);
                    if !excluded && !nested {
                        return Err(SpecError::OverlappingTriggers {
                            parent,
                            a: self.loops[first.0].name.clone(),
                            b: self.loops[second.0].name.clone(),
                            conditions_a: render_trigger(trigger),
                            conditions_b: render_trigger(other),
                        });
                    }
                }
            }
        }
        Ok(())
    }
```

Add above `fn parse_position`:

```rust
/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
fn render_trigger(trigger: &Trigger) -> String {
    let segment = String::from_utf8_lossy(&trigger.segment);
    if trigger.conditions.is_empty() {
        return format!("{segment:?} with no conditions");
    }
    let parts: Vec<String> = trigger
        .conditions
        .iter()
        .map(|(position, value)| format!("{position}: {:?}", String::from_utf8_lossy(value)))
        .collect();
    format!("{segment:?} where {{{}}}", parts.join(", "))
}
```

The rendered triggers are kept as text so the variant stays below clippy's `result_large_err` threshold.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: all pass (66 tests in `spec::tests`), including `builtin_835_loads` (1000A `{1: "PR"}` and 1000B `{1: "PE"}` differ at position 1).

- [ ] **Step 8: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/spec.rs
git commit -m "spec: reject empty segment ids anywhere and sibling triggers one segment can satisfy

EmptySegmentId names the loop and the key as written (trigger.segment,
segments[i], end) or the segments section. OverlappingTriggers names both
loops and both triggers; identical triggers stay AmbiguousTrigger."
```

---
## Task 5: The built-in 835 defines its 29 segments

**Implementer tier:** Opus — the element definitions below come from the X12 005010X221 (835) base standard and must be checked against it, not merely transcribed: names, `ID`/`AN`/`R`/`N0`/`DT`/`TM` types, `M` → `"required": true`, and min/max lengths.

**Files:**
- Modify: `crates/edi835_core/specs/835.json`
- Modify: `crates/edi835_core/src/spec.rs` (tests only)

**Interfaces:**
- Consumes: the `segments` section (Task 3).
- Produces: `Spec::builtin_835().segment(id)` is `Some` for every id any loop lists as trigger, held segment or end: ISA, GS, ST, BPR, TRN, CUR, REF, DTM, N1, N3, N4, PER, RDM, LX, TS3, TS2, CLP, CAS, NM1, MIA, MOA, AMT, QTY, SVC, LQ, PLB, SE, GE, IEA.

Conventions used in the data (they are the rules to check the JSON against):
- `required` is `true` only where the base standard marks the element Mandatory; X (conditional) and O are `false`. Implementation-guide situational rules are not modelled.
- Composites are declared on an `AN` element and list their components by position: `REF04` (C040), `SVC01` and `SVC06` (C003, 8 components), `PLB03`, `05`, `07`, `09`, `11`, `13` (C042). A component's `required` applies when the composite is present.
- `R` uses scale 2 (money) unless written: `CUR03` exchange rate 6, `CLP12` DRG weight 4, `CLP13` discharge fraction 4, `MOA01` reimbursement rate 4, `TS216` average DRG weight 4.
- `TS3` lists only the positions 5010 uses (01–05, 13, 15, 17, 18, 20–24); the others stay opaque. `QTY03` (composite unit of measure) and `RDM04`/`RDM05` are not defined and stay opaque.
- `ISA11` is the repetition separator in 5010 and the standards identifier in 4010; it is typed `AN` 1/1 to fit both.

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `ancestors_are_listed_root_first`:

```rust
    #[test]
    fn builtin_835_defines_every_segment_its_loops_name() {
        let spec = Spec::builtin_835();
        for def in spec.loops() {
            let ids = std::iter::once(&def.trigger.segment)
                .chain(&def.segments)
                .chain(def.end.as_ref());
            for id in ids {
                assert!(
                    spec.segment(id).is_some(),
                    "loop {} names {} with no definition",
                    def.name,
                    String::from_utf8_lossy(id)
                );
            }
        }
        let ids: Vec<String> = spec
            .segments()
            .map(|(id, _)| String::from_utf8_lossy(id).into_owned())
            .collect();
        assert_eq!(
            ids,
            vec![
                "AMT", "BPR", "CAS", "CLP", "CUR", "DTM", "GE", "GS", "IEA", "ISA", "LQ", "LX",
                "MIA", "MOA", "N1", "N3", "N4", "NM1", "PER", "PLB", "QTY", "RDM", "REF", "SE",
                "ST", "SVC", "TRN", "TS2", "TS3"
            ]
        );
    }

    #[test]
    fn builtin_835_element_names_are_unique_among_siblings() {
        fn check(at: &str, elements: &BTreeMap<usize, ElementDef>) {
            let mut seen = std::collections::BTreeSet::new();
            for (position, def) in elements {
                assert!(
                    seen.insert(def.name.as_str()),
                    "{at}: name {} repeats at position {position}",
                    def.name
                );
                check(&format!("{at}{position:02}"), &def.composite);
            }
        }
        for (id, def) in Spec::builtin_835().segments() {
            check(&String::from_utf8_lossy(id), &def.elements);
        }
    }

    #[test]
    fn builtin_835_types_the_elements_the_envelope_checks_rely_on() {
        let spec = Spec::builtin_835();
        let element = |id: &[u8], position: usize| &spec.segment(id).unwrap().elements[&position];
        assert_eq!(element(b"CLP", 1).name, "claim_submitter_identifier");
        assert_eq!(element(b"CLP", 3).kind, ElementType::R { scale: 2 });
        assert_eq!(element(b"SE", 1).kind, ElementType::N(0));
        assert_eq!(element(b"ISA", 13).kind, ElementType::N(0));
        assert_eq!(element(b"DTM", 2).kind, ElementType::Dt);
        let procedure = element(b"SVC", 1);
        assert!(procedure.required);
        assert_eq!(procedure.composite.len(), 8);
        assert_eq!(procedure.composite[&2].name, "procedure_code");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::tests::builtin_835`
Expected: `builtin_835_defines_every_segment_its_loops_name` fails with `loop 1000A names N1 with no definition`; `builtin_835_types_the_elements_the_envelope_checks_rely_on` panics on `unwrap()` of `None`.

- [ ] **Step 3: Write the definitions**

Replace `crates/edi835_core/specs/835.json` with:

```json
{
  "name": "835",
  "loops": {
    "interchange": {
      "trigger": { "segment": "ISA" },
      "end": "IEA"
    },
    "group": {
      "parent": "interchange",
      "trigger": { "segment": "GS" },
      "end": "GE"
    },
    "transaction": {
      "parent": "group",
      "trigger": { "segment": "ST" },
      "segments": ["BPR", "TRN", "CUR", "REF", "DTM", "PLB"],
      "end": "SE"
    },
    "1000A": {
      "parent": "transaction",
      "trigger": { "segment": "N1", "where": { "1": "PR" } },
      "segments": ["N3", "N4", "REF", "PER"]
    },
    "1000B": {
      "parent": "transaction",
      "trigger": { "segment": "N1", "where": { "1": "PE" } },
      "segments": ["N3", "N4", "REF", "RDM"]
    },
    "2000": {
      "parent": "transaction",
      "trigger": { "segment": "LX" },
      "segments": ["TS3", "TS2"]
    },
    "2100": {
      "parent": "2000",
      "trigger": { "segment": "CLP" },
      "segments": ["CAS", "NM1", "MIA", "MOA", "REF", "DTM", "PER", "AMT", "QTY"]
    },
    "2110": {
      "parent": "2100",
      "trigger": { "segment": "SVC" },
      "segments": ["DTM", "CAS", "REF", "AMT", "QTY", "LQ"]
    }
  },
  "segments": {
    "ISA": { "elements": {
      "1": {"name": "authorization_information_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
      "2": {"name": "authorization_information", "type": "AN", "required": true, "min": 10, "max": 10},
      "3": {"name": "security_information_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
      "4": {"name": "security_information", "type": "AN", "required": true, "min": 10, "max": 10},
      "5": {"name": "interchange_sender_id_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
      "6": {"name": "interchange_sender_id", "type": "AN", "required": true, "min": 15, "max": 15},
      "7": {"name": "interchange_receiver_id_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
      "8": {"name": "interchange_receiver_id", "type": "AN", "required": true, "min": 15, "max": 15},
      "9": {"name": "interchange_date", "type": "DT", "required": true, "min": 6, "max": 6},
      "10": {"name": "interchange_time", "type": "TM", "required": true, "min": 4, "max": 4},
      "11": {"name": "repetition_separator", "type": "AN", "required": true, "min": 1, "max": 1},
      "12": {"name": "interchange_control_version_number", "type": "ID", "required": true, "min": 5, "max": 5},
      "13": {"name": "interchange_control_number", "type": "N0", "required": true, "min": 9, "max": 9},
      "14": {"name": "acknowledgment_requested", "type": "ID", "required": true, "min": 1, "max": 1},
      "15": {"name": "interchange_usage_indicator", "type": "ID", "required": true, "min": 1, "max": 1},
      "16": {"name": "component_element_separator", "type": "AN", "required": true, "min": 1, "max": 1}
    } },
    "GS": { "elements": {
      "1": {"name": "functional_identifier_code", "type": "ID", "required": true, "min": 2, "max": 2},
      "2": {"name": "application_sender_code", "type": "AN", "required": true, "min": 2, "max": 15},
      "3": {"name": "application_receiver_code", "type": "AN", "required": true, "min": 2, "max": 15},
      "4": {"name": "date", "type": "DT", "required": true, "min": 8, "max": 8},
      "5": {"name": "time", "type": "TM", "required": true, "min": 4, "max": 8},
      "6": {"name": "group_control_number", "type": "N0", "required": true, "min": 1, "max": 9},
      "7": {"name": "responsible_agency_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "8": {"name": "version_release_industry_identifier_code", "type": "AN", "required": true, "min": 1, "max": 12}
    } },
    "ST": { "elements": {
      "1": {"name": "transaction_set_identifier_code", "type": "ID", "required": true, "min": 3, "max": 3},
      "2": {"name": "transaction_set_control_number", "type": "AN", "required": true, "min": 4, "max": 9},
      "3": {"name": "implementation_convention_reference", "type": "AN", "min": 1, "max": 35}
    } },
    "BPR": { "elements": {
      "1": {"name": "transaction_handling_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "2": {"name": "total_actual_provider_payment_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "3": {"name": "credit_debit_flag_code", "type": "ID", "required": true, "min": 1, "max": 1},
      "4": {"name": "payment_method_code", "type": "ID", "required": true, "min": 3, "max": 3},
      "5": {"name": "payment_format_code", "type": "ID", "min": 1, "max": 10},
      "6": {"name": "sender_dfi_id_number_qualifier", "type": "ID", "min": 2, "max": 2},
      "7": {"name": "sender_dfi_identification_number", "type": "AN", "min": 3, "max": 12},
      "8": {"name": "sender_account_number_qualifier", "type": "ID", "min": 1, "max": 3},
      "9": {"name": "sender_account_number", "type": "AN", "min": 1, "max": 35},
      "10": {"name": "originating_company_identifier", "type": "AN", "min": 10, "max": 10},
      "11": {"name": "originating_company_supplemental_code", "type": "AN", "min": 9, "max": 9},
      "12": {"name": "receiver_dfi_id_number_qualifier", "type": "ID", "min": 2, "max": 2},
      "13": {"name": "receiver_dfi_identification_number", "type": "AN", "min": 3, "max": 12},
      "14": {"name": "receiver_account_number_qualifier", "type": "ID", "min": 1, "max": 3},
      "15": {"name": "receiver_account_number", "type": "AN", "min": 1, "max": 35},
      "16": {"name": "check_issue_or_eft_effective_date", "type": "DT", "min": 8, "max": 8},
      "17": {"name": "business_function_code", "type": "ID", "min": 1, "max": 3},
      "18": {"name": "dfi_id_number_qualifier_3", "type": "ID", "min": 2, "max": 2},
      "19": {"name": "dfi_identification_number_3", "type": "AN", "min": 3, "max": 12},
      "20": {"name": "account_number_qualifier_3", "type": "ID", "min": 1, "max": 3},
      "21": {"name": "account_number_3", "type": "AN", "min": 1, "max": 35}
    } },
    "TRN": { "elements": {
      "1": {"name": "trace_type_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "2": {"name": "check_or_eft_trace_number", "type": "AN", "required": true, "min": 1, "max": 50},
      "3": {"name": "payer_identifier", "type": "AN", "min": 10, "max": 10},
      "4": {"name": "originating_company_supplemental_code", "type": "AN", "min": 1, "max": 50}
    } },
    "CUR": { "elements": {
      "1": {"name": "entity_identifier_code", "type": "ID", "required": true, "min": 2, "max": 3},
      "2": {"name": "currency_code", "type": "ID", "required": true, "min": 3, "max": 3},
      "3": {"name": "exchange_rate", "type": "R", "min": 4, "max": 10, "scale": 6}
    } },
    "REF": { "elements": {
      "1": {"name": "reference_identification_qualifier", "type": "ID", "required": true, "min": 2, "max": 3},
      "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50},
      "3": {"name": "description", "type": "AN", "min": 1, "max": 80},
      "4": {"name": "reference_identifier", "type": "AN", "composite": {
        "1": {"name": "reference_identification_qualifier", "type": "ID", "required": true, "min": 2, "max": 3},
        "2": {"name": "reference_identification", "type": "AN", "required": true, "min": 1, "max": 50},
        "3": {"name": "reference_identification_qualifier_2", "type": "ID", "min": 2, "max": 3},
        "4": {"name": "reference_identification_2", "type": "AN", "min": 1, "max": 50},
        "5": {"name": "reference_identification_qualifier_3", "type": "ID", "min": 2, "max": 3},
        "6": {"name": "reference_identification_3", "type": "AN", "min": 1, "max": 50}
      }}
    } },
    "DTM": { "elements": {
      "1": {"name": "date_time_qualifier", "type": "ID", "required": true, "min": 3, "max": 3},
      "2": {"name": "date", "type": "DT", "min": 8, "max": 8},
      "3": {"name": "time", "type": "TM", "min": 4, "max": 8},
      "4": {"name": "time_code", "type": "ID", "min": 2, "max": 2},
      "5": {"name": "date_time_period_format_qualifier", "type": "ID", "min": 2, "max": 3},
      "6": {"name": "date_time_period", "type": "AN", "min": 1, "max": 35}
    } },
    "N1": { "elements": {
      "1": {"name": "entity_identifier_code", "type": "ID", "required": true, "min": 2, "max": 3},
      "2": {"name": "name", "type": "AN", "min": 1, "max": 60},
      "3": {"name": "identification_code_qualifier", "type": "ID", "min": 1, "max": 2},
      "4": {"name": "identification_code", "type": "AN", "min": 2, "max": 80},
      "5": {"name": "entity_relationship_code", "type": "ID", "min": 2, "max": 2},
      "6": {"name": "entity_identifier_code_2", "type": "ID", "min": 2, "max": 3}
    } },
    "N3": { "elements": {
      "1": {"name": "address_information", "type": "AN", "required": true, "min": 1, "max": 55},
      "2": {"name": "address_information_2", "type": "AN", "min": 1, "max": 55}
    } },
    "N4": { "elements": {
      "1": {"name": "city_name", "type": "AN", "min": 2, "max": 30},
      "2": {"name": "state_or_province_code", "type": "ID", "min": 2, "max": 2},
      "3": {"name": "postal_code", "type": "ID", "min": 3, "max": 15},
      "4": {"name": "country_code", "type": "ID", "min": 2, "max": 3},
      "5": {"name": "location_qualifier", "type": "ID", "min": 1, "max": 2},
      "6": {"name": "location_identifier", "type": "AN", "min": 1, "max": 30},
      "7": {"name": "country_subdivision_code", "type": "ID", "min": 1, "max": 3}
    } },
    "PER": { "elements": {
      "1": {"name": "contact_function_code", "type": "ID", "required": true, "min": 2, "max": 2},
      "2": {"name": "name", "type": "AN", "min": 1, "max": 60},
      "3": {"name": "communication_number_qualifier", "type": "ID", "min": 2, "max": 2},
      "4": {"name": "communication_number", "type": "AN", "min": 1, "max": 256},
      "5": {"name": "communication_number_qualifier_2", "type": "ID", "min": 2, "max": 2},
      "6": {"name": "communication_number_2", "type": "AN", "min": 1, "max": 256},
      "7": {"name": "communication_number_qualifier_3", "type": "ID", "min": 2, "max": 2},
      "8": {"name": "communication_number_3", "type": "AN", "min": 1, "max": 256},
      "9": {"name": "contact_inquiry_reference", "type": "AN", "min": 1, "max": 20}
    } },
    "RDM": { "elements": {
      "1": {"name": "report_transmission_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "2": {"name": "name", "type": "AN", "min": 1, "max": 60},
      "3": {"name": "communication_number", "type": "AN", "min": 1, "max": 256}
    } },
    "LX": { "elements": {
      "1": {"name": "assigned_number", "type": "N0", "required": true, "min": 1, "max": 6}
    } },
    "TS3": { "elements": {
      "1": {"name": "provider_identifier", "type": "AN", "required": true, "min": 1, "max": 50},
      "2": {"name": "facility_type_code", "type": "AN", "required": true, "min": 1, "max": 2},
      "3": {"name": "fiscal_period_date", "type": "DT", "required": true, "min": 8, "max": 8},
      "4": {"name": "total_claim_count", "type": "R", "required": true, "min": 1, "max": 15},
      "5": {"name": "total_claim_charge_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "13": {"name": "total_msp_payer_amount", "type": "R", "min": 1, "max": 18},
      "15": {"name": "total_non_lab_charge_amount", "type": "R", "min": 1, "max": 18},
      "17": {"name": "total_hcpcs_reported_charge_amount", "type": "R", "min": 1, "max": 18},
      "18": {"name": "total_hcpcs_payable_amount", "type": "R", "min": 1, "max": 18},
      "20": {"name": "total_professional_component_amount", "type": "R", "min": 1, "max": 18},
      "21": {"name": "total_msp_patient_liability_met_amount", "type": "R", "min": 1, "max": 18},
      "22": {"name": "total_patient_reimbursement_amount", "type": "R", "min": 1, "max": 18},
      "23": {"name": "total_pip_claim_count", "type": "R", "min": 1, "max": 15},
      "24": {"name": "total_pip_adjustment_amount", "type": "R", "min": 1, "max": 18}
    } },
    "TS2": { "elements": {
      "1": {"name": "total_drg_amount", "type": "R", "min": 1, "max": 18},
      "2": {"name": "total_federal_specific_amount", "type": "R", "min": 1, "max": 18},
      "3": {"name": "total_hospital_specific_amount", "type": "R", "min": 1, "max": 18},
      "4": {"name": "total_disproportionate_share_amount", "type": "R", "min": 1, "max": 18},
      "5": {"name": "total_capital_amount", "type": "R", "min": 1, "max": 18},
      "6": {"name": "total_indirect_medical_education_amount", "type": "R", "min": 1, "max": 18},
      "7": {"name": "total_outlier_day_count", "type": "R", "min": 1, "max": 15},
      "8": {"name": "total_day_outlier_amount", "type": "R", "min": 1, "max": 18},
      "9": {"name": "total_cost_outlier_amount", "type": "R", "min": 1, "max": 18},
      "10": {"name": "average_drg_length_of_stay", "type": "R", "min": 1, "max": 15},
      "11": {"name": "total_discharge_count", "type": "R", "min": 1, "max": 15},
      "12": {"name": "total_cost_report_day_count", "type": "R", "min": 1, "max": 15},
      "13": {"name": "total_covered_day_count", "type": "R", "min": 1, "max": 15},
      "14": {"name": "total_noncovered_day_count", "type": "R", "min": 1, "max": 15},
      "15": {"name": "total_msp_pass_through_amount", "type": "R", "min": 1, "max": 18},
      "16": {"name": "average_drg_weight", "type": "R", "min": 1, "max": 15, "scale": 4},
      "17": {"name": "total_pps_capital_fsp_drg_amount", "type": "R", "min": 1, "max": 18},
      "18": {"name": "total_pps_capital_hsp_drg_amount", "type": "R", "min": 1, "max": 18},
      "19": {"name": "total_pps_dsh_drg_amount", "type": "R", "min": 1, "max": 18}
    } },
    "CLP": { "elements": {
      "1": {"name": "claim_submitter_identifier", "type": "AN", "required": true, "min": 1, "max": 38},
      "2": {"name": "claim_status_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "3": {"name": "total_claim_charge_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "4": {"name": "claim_payment_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "5": {"name": "patient_responsibility_amount", "type": "R", "min": 1, "max": 18},
      "6": {"name": "claim_filing_indicator_code", "type": "ID", "min": 1, "max": 2},
      "7": {"name": "payer_claim_control_number", "type": "AN", "min": 1, "max": 50},
      "8": {"name": "facility_type_code", "type": "AN", "min": 1, "max": 2},
      "9": {"name": "claim_frequency_type_code", "type": "ID", "min": 1, "max": 1},
      "10": {"name": "patient_status_code", "type": "ID", "min": 1, "max": 2},
      "11": {"name": "drg_code", "type": "ID", "min": 1, "max": 4},
      "12": {"name": "drg_weight", "type": "R", "min": 1, "max": 15, "scale": 4},
      "13": {"name": "discharge_fraction", "type": "R", "min": 1, "max": 10, "scale": 4},
      "14": {"name": "yes_no_condition_or_response_code", "type": "ID", "min": 1, "max": 1}
    } },
    "CAS": { "elements": {
      "1": {"name": "claim_adjustment_group_code", "type": "ID", "required": true, "min": 1, "max": 2},
      "2": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 1, "max": 5},
      "3": {"name": "adjustment_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "4": {"name": "adjustment_quantity", "type": "R", "min": 1, "max": 15},
      "5": {"name": "adjustment_reason_code_2", "type": "ID", "min": 1, "max": 5},
      "6": {"name": "adjustment_amount_2", "type": "R", "min": 1, "max": 18},
      "7": {"name": "adjustment_quantity_2", "type": "R", "min": 1, "max": 15},
      "8": {"name": "adjustment_reason_code_3", "type": "ID", "min": 1, "max": 5},
      "9": {"name": "adjustment_amount_3", "type": "R", "min": 1, "max": 18},
      "10": {"name": "adjustment_quantity_3", "type": "R", "min": 1, "max": 15},
      "11": {"name": "adjustment_reason_code_4", "type": "ID", "min": 1, "max": 5},
      "12": {"name": "adjustment_amount_4", "type": "R", "min": 1, "max": 18},
      "13": {"name": "adjustment_quantity_4", "type": "R", "min": 1, "max": 15},
      "14": {"name": "adjustment_reason_code_5", "type": "ID", "min": 1, "max": 5},
      "15": {"name": "adjustment_amount_5", "type": "R", "min": 1, "max": 18},
      "16": {"name": "adjustment_quantity_5", "type": "R", "min": 1, "max": 15},
      "17": {"name": "adjustment_reason_code_6", "type": "ID", "min": 1, "max": 5},
      "18": {"name": "adjustment_amount_6", "type": "R", "min": 1, "max": 18},
      "19": {"name": "adjustment_quantity_6", "type": "R", "min": 1, "max": 15}
    } },
    "NM1": { "elements": {
      "1": {"name": "entity_identifier_code", "type": "ID", "required": true, "min": 2, "max": 3},
      "2": {"name": "entity_type_qualifier", "type": "ID", "required": true, "min": 1, "max": 1},
      "3": {"name": "name_last_or_organization_name", "type": "AN", "min": 1, "max": 60},
      "4": {"name": "name_first", "type": "AN", "min": 1, "max": 35},
      "5": {"name": "name_middle", "type": "AN", "min": 1, "max": 25},
      "6": {"name": "name_prefix", "type": "AN", "min": 1, "max": 10},
      "7": {"name": "name_suffix", "type": "AN", "min": 1, "max": 10},
      "8": {"name": "identification_code_qualifier", "type": "ID", "min": 1, "max": 2},
      "9": {"name": "identification_code", "type": "AN", "min": 2, "max": 80},
      "10": {"name": "entity_relationship_code", "type": "ID", "min": 2, "max": 2},
      "11": {"name": "entity_identifier_code_2", "type": "ID", "min": 2, "max": 3},
      "12": {"name": "name_last_or_organization_name_2", "type": "AN", "min": 1, "max": 60}
    } },
    "MIA": { "elements": {
      "1": {"name": "covered_days_or_visits_count", "type": "R", "required": true, "min": 1, "max": 15},
      "2": {"name": "pps_operating_outlier_amount", "type": "R", "min": 1, "max": 18},
      "3": {"name": "lifetime_psychiatric_days_count", "type": "R", "min": 1, "max": 15},
      "4": {"name": "claim_drg_amount", "type": "R", "min": 1, "max": 18},
      "5": {"name": "claim_payment_remark_code", "type": "AN", "min": 1, "max": 50},
      "6": {"name": "claim_disproportionate_share_amount", "type": "R", "min": 1, "max": 18},
      "7": {"name": "claim_msp_pass_through_amount", "type": "R", "min": 1, "max": 18},
      "8": {"name": "claim_pps_capital_amount", "type": "R", "min": 1, "max": 18},
      "9": {"name": "pps_capital_fsp_drg_amount", "type": "R", "min": 1, "max": 18},
      "10": {"name": "pps_capital_hsp_drg_amount", "type": "R", "min": 1, "max": 18},
      "11": {"name": "pps_capital_dsh_drg_amount", "type": "R", "min": 1, "max": 18},
      "12": {"name": "old_capital_amount", "type": "R", "min": 1, "max": 18},
      "13": {"name": "pps_capital_ime_amount", "type": "R", "min": 1, "max": 18},
      "14": {"name": "pps_operating_hospital_specific_drg_amount", "type": "R", "min": 1, "max": 18},
      "15": {"name": "cost_report_day_count", "type": "R", "min": 1, "max": 15},
      "16": {"name": "pps_operating_federal_specific_drg_amount", "type": "R", "min": 1, "max": 18},
      "17": {"name": "claim_pps_capital_outlier_amount", "type": "R", "min": 1, "max": 18},
      "18": {"name": "claim_indirect_teaching_amount", "type": "R", "min": 1, "max": 18},
      "19": {"name": "nonpayable_professional_component_amount", "type": "R", "min": 1, "max": 18},
      "20": {"name": "claim_payment_remark_code_2", "type": "AN", "min": 1, "max": 50},
      "21": {"name": "claim_payment_remark_code_3", "type": "AN", "min": 1, "max": 50},
      "22": {"name": "claim_payment_remark_code_4", "type": "AN", "min": 1, "max": 50},
      "23": {"name": "claim_payment_remark_code_5", "type": "AN", "min": 1, "max": 50},
      "24": {"name": "pps_capital_exception_amount", "type": "R", "min": 1, "max": 18}
    } },
    "MOA": { "elements": {
      "1": {"name": "reimbursement_rate", "type": "R", "min": 1, "max": 10, "scale": 4},
      "2": {"name": "hcpcs_payable_amount", "type": "R", "min": 1, "max": 18},
      "3": {"name": "claim_payment_remark_code", "type": "AN", "min": 1, "max": 50},
      "4": {"name": "claim_payment_remark_code_2", "type": "AN", "min": 1, "max": 50},
      "5": {"name": "claim_payment_remark_code_3", "type": "AN", "min": 1, "max": 50},
      "6": {"name": "claim_payment_remark_code_4", "type": "AN", "min": 1, "max": 50},
      "7": {"name": "claim_payment_remark_code_5", "type": "AN", "min": 1, "max": 50},
      "8": {"name": "esrd_payment_amount", "type": "R", "min": 1, "max": 18},
      "9": {"name": "nonpayable_professional_component_amount", "type": "R", "min": 1, "max": 18}
    } },
    "AMT": { "elements": {
      "1": {"name": "amount_qualifier_code", "type": "ID", "required": true, "min": 1, "max": 3},
      "2": {"name": "monetary_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "3": {"name": "credit_debit_flag_code", "type": "ID", "min": 1, "max": 1}
    } },
    "QTY": { "elements": {
      "1": {"name": "quantity_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
      "2": {"name": "quantity", "type": "R", "min": 1, "max": 15},
      "4": {"name": "free_form_information", "type": "AN", "min": 1, "max": 30}
    } },
    "SVC": { "elements": {
      "1": {"name": "composite_medical_procedure", "type": "AN", "required": true, "composite": {
        "1": {"name": "product_or_service_id_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "procedure_code", "type": "AN", "required": true, "min": 1, "max": 48},
        "3": {"name": "procedure_modifier_1", "type": "AN", "min": 2, "max": 2},
        "4": {"name": "procedure_modifier_2", "type": "AN", "min": 2, "max": 2},
        "5": {"name": "procedure_modifier_3", "type": "AN", "min": 2, "max": 2},
        "6": {"name": "procedure_modifier_4", "type": "AN", "min": 2, "max": 2},
        "7": {"name": "description", "type": "AN", "min": 1, "max": 80},
        "8": {"name": "product_or_service_id", "type": "AN", "min": 1, "max": 48}
      }},
      "2": {"name": "line_item_charge_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "3": {"name": "line_item_provider_payment_amount", "type": "R", "min": 1, "max": 18},
      "4": {"name": "national_uniform_billing_committee_revenue_code", "type": "AN", "min": 1, "max": 48},
      "5": {"name": "units_of_service_paid_count", "type": "R", "min": 1, "max": 15},
      "6": {"name": "original_composite_medical_procedure", "type": "AN", "composite": {
        "1": {"name": "original_product_or_service_id_qualifier", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "original_procedure_code", "type": "AN", "required": true, "min": 1, "max": 48},
        "3": {"name": "original_procedure_modifier_1", "type": "AN", "min": 2, "max": 2},
        "4": {"name": "original_procedure_modifier_2", "type": "AN", "min": 2, "max": 2},
        "5": {"name": "original_procedure_modifier_3", "type": "AN", "min": 2, "max": 2},
        "6": {"name": "original_procedure_modifier_4", "type": "AN", "min": 2, "max": 2},
        "7": {"name": "original_description", "type": "AN", "min": 1, "max": 80},
        "8": {"name": "original_product_or_service_id", "type": "AN", "min": 1, "max": 48}
      }},
      "7": {"name": "original_units_of_service_count", "type": "R", "min": 1, "max": 15}
    } },
    "LQ": { "elements": {
      "1": {"name": "code_list_qualifier_code", "type": "ID", "min": 1, "max": 3},
      "2": {"name": "industry_code", "type": "AN", "min": 1, "max": 30}
    } },
    "PLB": { "elements": {
      "1": {"name": "provider_identifier", "type": "AN", "required": true, "min": 1, "max": 50},
      "2": {"name": "fiscal_period_date", "type": "DT", "required": true, "min": 8, "max": 8},
      "3": {"name": "adjustment_identifier", "type": "AN", "required": true, "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "4": {"name": "provider_adjustment_amount", "type": "R", "required": true, "min": 1, "max": 18},
      "5": {"name": "adjustment_identifier_2", "type": "AN", "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "6": {"name": "provider_adjustment_amount_2", "type": "R", "min": 1, "max": 18},
      "7": {"name": "adjustment_identifier_3", "type": "AN", "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "8": {"name": "provider_adjustment_amount_3", "type": "R", "min": 1, "max": 18},
      "9": {"name": "adjustment_identifier_4", "type": "AN", "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "10": {"name": "provider_adjustment_amount_4", "type": "R", "min": 1, "max": 18},
      "11": {"name": "adjustment_identifier_5", "type": "AN", "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "12": {"name": "provider_adjustment_amount_5", "type": "R", "min": 1, "max": 18},
      "13": {"name": "adjustment_identifier_6", "type": "AN", "composite": {
        "1": {"name": "adjustment_reason_code", "type": "ID", "required": true, "min": 2, "max": 2},
        "2": {"name": "reference_identification", "type": "AN", "min": 1, "max": 50}
      }},
      "14": {"name": "provider_adjustment_amount_6", "type": "R", "min": 1, "max": 18}
    } },
    "SE": { "elements": {
      "1": {"name": "number_of_included_segments", "type": "N0", "required": true, "min": 1, "max": 10},
      "2": {"name": "transaction_set_control_number", "type": "AN", "required": true, "min": 4, "max": 9}
    } },
    "GE": { "elements": {
      "1": {"name": "number_of_transaction_sets_included", "type": "N0", "required": true, "min": 1, "max": 6},
      "2": {"name": "group_control_number", "type": "N0", "required": true, "min": 1, "max": 9}
    } },
    "IEA": { "elements": {
      "1": {"name": "number_of_included_functional_groups", "type": "N0", "required": true, "min": 1, "max": 5},
      "2": {"name": "interchange_control_number", "type": "N0", "required": true, "min": 9, "max": 9}
    } }
  }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: all pass (69 tests in `spec::tests`), including `to_json_round_trips_through_from_json` and every patch test on the built-in.

Run: `cargo test --workspace --locked`
Expected: green; the goldens do not change (the engine reads only `loops`).

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/specs/835.json crates/edi835_core/src/spec.rs
git commit -m "spec: the built-in 835 names and types the elements of its 29 segments"
```

---
## Task 6: `Diagnostic` — a finding that explains itself

**Implementer tier:** Haiku — one new file plus two lines of `lib.rs`, all given below.

**Files:**
- Create: `crates/edi835_core/src/diagnostic.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `ElementType` (Task 3), `Document::spans`, `Span`.
- Produces:
  - `pub enum SnipLevel { L1, L2, L3 }`, `Display` `SNIP 1` / `SNIP 2` / `SNIP 3`.
  - `pub struct LoopRef { pub name: String, pub ordinal: usize }`, `Display` `2100#3`; the ordinal counts every instance of that loop in the stream, from 1.
  - `pub enum Rule` with level-1 variants `UnknownSegment { id }`, `ImplicitLoop { loop_name, caused_by }`, `UnterminatedLoop { loop_name, expected_end }`, `ControlCountMismatch { segment_id, element, expected, found }`, `ControlNumberMismatch { opener, opener_element, closer, closer_element, opener_value, closer_value }`, and level-2 variants `RequiredElementMissing { segment_id, element, component, name }`, `TypeMismatch { segment_id, element, component, name, expected: ElementType }`, `LengthOutOfRange { segment_id, element, component, name, min, max, length }`, `CompositeShape { segment_id, element, name, declared, found }` (declared now, emitted by the projector later). `Rule::level(&self) -> SnipLevel`.
  - `pub struct Diagnostic { pub rule, pub level, pub segment: Option<usize>, pub element: Option<usize>, pub component: Option<usize>, pub path: Vec<LoopRef>, pub datum: Vec<u8> }` (`Clone`, `Debug`, `PartialEq`, `Eq`), `Diagnostic::new(rule, segment, element, component, path, datum)` (level taken from the rule), `Diagnostic::span(&self, &Document<'_>) -> Option<Span>`.
  - One-line `Display`: `SNIP 1 · <rule text> · segment #<i>[, element <e>[, component <c>]] · at <path joined by "/"> · datum "<lossy UTF-8>"`. With no segment the location reads `end of stream`; with an empty path it reads `at the root`. Element references are X12 style: `CLP01`, `SVC01-2`.

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/diagnostic.rs` with only the module doc, the imports and the tests:

```rust
//! Findings about a file's data, each one readable on its own.
//!
//! A diagnostic names the rule that failed, where it failed (segment index,
//! element and component position, and the open loops with the ordinal of
//! each instance) and the offending value as it appears in the file. It holds
//! owned values only, so it can be printed, stored or sent elsewhere without
//! the spec or the document that produced it.

use std::fmt;

use crate::document::{Document, Span};
use crate::spec::ElementType;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Delimiters;

    fn path(loops: &[(&str, usize)]) -> Vec<LoopRef> {
        loops
            .iter()
            .map(|&(name, ordinal)| LoopRef {
                name: name.to_string(),
                ordinal,
            })
            .collect()
    }

    const TRANSACTION: &[(&str, usize)] = &[("interchange", 1), ("group", 1), ("transaction", 1)];

    #[test]
    fn levels_display_as_snip_numbers() {
        assert_eq!(SnipLevel::L1.to_string(), "SNIP 1");
        assert_eq!(SnipLevel::L2.to_string(), "SNIP 2");
        assert_eq!(SnipLevel::L3.to_string(), "SNIP 3");
    }

    #[test]
    fn a_loop_ref_displays_name_and_ordinal() {
        let at = LoopRef {
            name: "2100".into(),
            ordinal: 3,
        };
        assert_eq!(at.to_string(), "2100#3");
    }

    #[test]
    fn unknown_segment_displays_id_index_path_and_datum() {
        let diagnostic = Diagnostic::new(
            Rule::UnknownSegment { id: b"XX".to_vec() },
            Some(7),
            None,
            None,
            path(&[
                ("interchange", 1),
                ("group", 1),
                ("transaction", 1),
                ("1000A", 1),
            ]),
            b"XX".to_vec(),
        );
        assert_eq!(diagnostic.level, SnipLevel::L1);
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · segment \"XX\" is not part of the structure: no open loop holds it and it opens no loop · segment #7 · at interchange#1/group#1/transaction#1/1000A#1 · datum \"XX\""
        );
    }

    #[test]
    fn implicit_loop_displays_the_loop_and_the_segment_that_needed_it() {
        let diagnostic = Diagnostic::new(
            Rule::ImplicitLoop {
                loop_name: "group".into(),
                caused_by: b"ST".to_vec(),
            },
            Some(0),
            None,
            None,
            path(&[("interchange", 1), ("group", 1)]),
            b"ST".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"group\" opened without its own trigger to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\""
        );
    }

    #[test]
    fn unterminated_loop_displays_the_expected_end_and_the_closing_segment() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "transaction".into(),
                expected_end: b"SE".to_vec(),
            },
            Some(4),
            None,
            None,
            path(TRANSACTION),
            b"GE".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"transaction\" closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
        );
    }

    #[test]
    fn a_finding_at_the_end_of_the_stream_says_so() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "interchange".into(),
                expected_end: b"IEA".to_vec(),
            },
            None,
            None,
            None,
            path(&[("interchange", 1)]),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"interchange\" closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\""
        );
    }

    #[test]
    fn control_count_mismatch_displays_the_element_the_value_and_the_count() {
        let diagnostic = Diagnostic::new(
            Rule::ControlCountMismatch {
                segment_id: b"SE".to_vec(),
                element: 1,
                expected: 18,
                found: b"15".to_vec(),
            },
            Some(19),
            Some(1),
            None,
            path(TRANSACTION),
            b"15".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · SE01 declares \"15\" but the count is 18 · segment #19, element 1 · at interchange#1/group#1/transaction#1 · datum \"15\""
        );
    }

    #[test]
    fn control_number_mismatch_displays_both_elements_and_values() {
        let diagnostic = Diagnostic::new(
            Rule::ControlNumberMismatch {
                opener: b"ST".to_vec(),
                opener_element: 2,
                closer: b"SE".to_vec(),
                closer_element: 2,
                opener_value: b"0001".to_vec(),
                closer_value: b"0002".to_vec(),
            },
            Some(4),
            Some(2),
            None,
            path(TRANSACTION),
            b"0002".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\""
        );
    }

    #[test]
    fn required_element_missing_displays_the_element_and_its_name() {
        let diagnostic = Diagnostic::new(
            Rule::RequiredElementMissing {
                segment_id: b"CLP".to_vec(),
                element: 1,
                component: None,
                name: "claim_submitter_identifier".into(),
            },
            Some(12),
            Some(1),
            None,
            path(&[("transaction", 1), ("2000", 1), ("2100", 1)]),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.level.to_string(),
            "SNIP 2",
            "element rules are level 2"
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · required element CLP01 (claim_submitter_identifier) is missing or empty · segment #12, element 1 · at transaction#1/2000#1/2100#1 · datum \"\""
        );
    }

    #[test]
    fn type_mismatch_displays_the_component_and_the_declared_type() {
        let diagnostic = Diagnostic::new(
            Rule::TypeMismatch {
                segment_id: b"CLP".to_vec(),
                element: 3,
                component: None,
                name: "total_claim_charge_amount".into(),
                expected: ElementType::R { scale: 2 },
            },
            Some(12),
            Some(3),
            None,
            path(&[("2100", 2)]),
            b"12A".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element CLP03 (total_claim_charge_amount) is not a valid R (decimal, scale 2) · segment #12, element 3 · at 2100#2 · datum \"12A\""
        );
        let component = Rule::TypeMismatch {
            segment_id: b"SVC".to_vec(),
            element: 1,
            component: Some(1),
            name: "product_or_service_id_qualifier".into(),
            expected: ElementType::Id,
        };
        assert_eq!(
            component.to_string(),
            "element SVC01-1 (product_or_service_id_qualifier) is not a valid ID (code)"
        );
    }

    #[test]
    fn length_out_of_range_displays_the_length_and_the_bounds() {
        let rule = |min, max| Rule::LengthOutOfRange {
            segment_id: b"CLP".to_vec(),
            element: 1,
            component: None,
            name: "claim_submitter_identifier".into(),
            min,
            max,
            length: 40,
        };
        let diagnostic = Diagnostic::new(
            rule(Some(1), Some(38)),
            Some(12),
            Some(1),
            None,
            Vec::new(),
            b"0123456789012345678901234567890123456789".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element CLP01 (claim_submitter_identifier) has length 40; the spec allows 1 to 38 · segment #12, element 1 · at the root · datum \"0123456789012345678901234567890123456789\""
        );
        assert!(
            rule(Some(41), None)
                .to_string()
                .ends_with("allows at least 41")
        );
        assert!(
            rule(None, Some(38))
                .to_string()
                .ends_with("allows at most 38")
        );
        assert!(rule(None, None).to_string().ends_with("allows any length"));
    }

    #[test]
    fn composite_shape_displays_found_and_declared_components() {
        let diagnostic = Diagnostic::new(
            Rule::CompositeShape {
                segment_id: b"SVC".to_vec(),
                element: 1,
                name: "composite_medical_procedure".into(),
                declared: 8,
                found: 9,
            },
            Some(17),
            Some(1),
            Some(9),
            path(&[("2110", 1)]),
            b"X".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element SVC01 (composite_medical_procedure) has 9 components; the spec declares 8 · segment #17, element 1, component 9 · at 2110#1 · datum \"X\""
        );
    }

    #[test]
    fn invalid_utf8_in_a_datum_is_shown_with_replacement_characters() {
        let diagnostic = Diagnostic::new(
            Rule::UnknownSegment {
                id: vec![b'Z', 0xFF],
            },
            Some(1),
            None,
            None,
            Vec::new(),
            vec![b'Z', 0xFF],
        );
        assert!(
            diagnostic
                .to_string()
                .ends_with("at the root · datum \"Z\u{FFFD}\""),
            "{diagnostic}"
        );
    }

    #[test]
    fn span_resolves_the_segment_bytes_from_the_document() {
        let document =
            Document::with_delimiters(&b"AA*1~BB*2~"[..], Delimiters::new(b'*', b':', b'~'));
        let at = |segment| {
            Diagnostic::new(
                Rule::UnknownSegment { id: b"BB".to_vec() },
                segment,
                None,
                None,
                Vec::new(),
                b"BB".to_vec(),
            )
        };
        let span = at(Some(1)).span(&document).unwrap();
        assert_eq!(&document.as_bytes()[span.raw], b"BB*2~");
        assert_eq!(at(Some(9)).span(&document), None);
        assert_eq!(at(None).span(&document), None);
    }
}
```

In `crates/edi835_core/src/lib.rs` add `pub mod diagnostic;` after `pub mod delimiters;` and, after the `delimiters` re-export:

```rust
pub use diagnostic::{Diagnostic, LoopRef, Rule, SnipLevel};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib diagnostic::`
Expected: compile errors (`cannot find type 'Diagnostic'`, `'Rule'`, `'SnipLevel'`, `'LoopRef'`); the `pub use` line fails too.

- [ ] **Step 3: Implement the module**

Insert between `use crate::spec::ElementType;` and `#[cfg(test)]`:

```rust
/// The SNIP validation level a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnipLevel {
    /// Integrity: envelopes, control numbers and counts, segment structure.
    L1,
    /// Requirements: required elements, types and lengths.
    L2,
    /// Balancing: amounts that must add up.
    L3,
}

impl fmt::Display for SnipLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self {
            SnipLevel::L1 => 1,
            SnipLevel::L2 => 2,
            SnipLevel::L3 => 3,
        };
        write!(f, "SNIP {level}")
    }
}

/// One open loop instance: the loop's name and the 1-based ordinal of the
/// instance among every instance of that loop in the stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopRef {
    /// Loop name as the spec writes it, e.g. `2100`.
    pub name: String,
    /// 1 for the first instance of the loop, 2 for the second, and so on.
    pub ordinal: usize,
}

impl fmt::Display for LoopRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.name, self.ordinal)
    }
}

/// The rule a diagnostic reports, with the values its message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// No open loop holds the segment and it opens no loop.
    UnknownSegment {
        /// The segment id.
        id: Vec<u8>,
    },
    /// A loop was opened without its own trigger, to hold a descendant.
    ImplicitLoop {
        /// The loop that was opened.
        loop_name: String,
        /// Id of the segment whose loop needed it.
        caused_by: Vec<u8>,
    },
    /// A loop that declares an end segment closed without capturing it.
    UnterminatedLoop {
        /// The loop.
        loop_name: String,
        /// The end segment the spec declares for it.
        expected_end: Vec<u8>,
    },
    /// A closing segment's count element does not match what it counts.
    ControlCountMismatch {
        /// The closing segment id, e.g. `SE`.
        segment_id: Vec<u8>,
        /// 1-based position of the count element.
        element: usize,
        /// The count observed in the stream.
        expected: usize,
        /// The count element as written.
        found: Vec<u8>,
    },
    /// A closing segment's control number differs from its opener's.
    ControlNumberMismatch {
        /// The opening segment id, e.g. `ST`.
        opener: Vec<u8>,
        /// 1-based position of the control number in the opener.
        opener_element: usize,
        /// The closing segment id, e.g. `SE`.
        closer: Vec<u8>,
        /// 1-based position of the control number in the closer.
        closer_element: usize,
        /// The opener's control number as written.
        opener_value: Vec<u8>,
        /// The closer's control number as written.
        closer_value: Vec<u8>,
    },
    /// A required element (or component) is absent or empty.
    RequiredElementMissing {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
    },
    /// A value does not parse as its declared type.
    TypeMismatch {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
        /// The declared type.
        expected: ElementType,
    },
    /// A value is shorter or longer than its definition allows.
    LengthOutOfRange {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
        /// Declared minimum length.
        min: Option<usize>,
        /// Declared maximum length.
        max: Option<usize>,
        /// The value's length.
        length: usize,
    },
    /// A composite element has more components than its definition declares.
    CompositeShape {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// The element's name in the spec.
        name: String,
        /// Highest component position the definition declares.
        declared: usize,
        /// Components found in the file.
        found: usize,
    },
}

impl Rule {
    /// The SNIP level the rule belongs to.
    pub fn level(&self) -> SnipLevel {
        match self {
            Rule::UnknownSegment { .. }
            | Rule::ImplicitLoop { .. }
            | Rule::UnterminatedLoop { .. }
            | Rule::ControlCountMismatch { .. }
            | Rule::ControlNumberMismatch { .. } => SnipLevel::L1,
            Rule::RequiredElementMissing { .. }
            | Rule::TypeMismatch { .. }
            | Rule::LengthOutOfRange { .. }
            | Rule::CompositeShape { .. } => SnipLevel::L2,
        }
    }
}

/// Bytes from the file, shown as text (invalid UTF-8 replaced) and quoted.
struct Quoted<'a>(&'a [u8]);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(self.0))
    }
}

/// An element reference in X12 style: `CLP01`, or `SVC01-2` for a component.
struct ElementRef<'a> {
    segment_id: &'a [u8],
    element: usize,
    component: Option<usize>,
}

impl fmt::Display for ElementRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:02}",
            String::from_utf8_lossy(self.segment_id),
            self.element
        )?;
        match self.component {
            Some(component) => write!(f, "-{component}"),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::UnknownSegment { id } => write!(
                f,
                "segment {} is not part of the structure: no open loop holds it and it opens no loop",
                Quoted(id)
            ),
            Rule::ImplicitLoop {
                loop_name,
                caused_by,
            } => write!(
                f,
                "loop {loop_name:?} opened without its own trigger to hold segment {}",
                Quoted(caused_by)
            ),
            Rule::UnterminatedLoop {
                loop_name,
                expected_end,
            } => write!(
                f,
                "loop {loop_name:?} closed without its end segment {}",
                Quoted(expected_end)
            ),
            Rule::ControlCountMismatch {
                segment_id,
                element,
                expected,
                found,
            } => write!(
                f,
                "{} declares {} but the count is {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                },
                Quoted(found)
            ),
            Rule::ControlNumberMismatch {
                opener,
                opener_element,
                closer,
                closer_element,
                opener_value,
                closer_value,
            } => write!(
                f,
                "{} {} does not match {} {}",
                ElementRef {
                    segment_id: closer,
                    element: *closer_element,
                    component: None
                },
                Quoted(closer_value),
                ElementRef {
                    segment_id: opener,
                    element: *opener_element,
                    component: None
                },
                Quoted(opener_value)
            ),
            Rule::RequiredElementMissing {
                segment_id,
                element,
                component,
                name,
            } => write!(
                f,
                "required element {} ({name}) is missing or empty",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::TypeMismatch {
                segment_id,
                element,
                component,
                name,
                expected,
            } => write!(
                f,
                "element {} ({name}) is not a valid {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::LengthOutOfRange {
                segment_id,
                element,
                component,
                name,
                min,
                max,
                length,
            } => {
                write!(
                    f,
                    "element {} ({name}) has length {length}; the spec allows ",
                    ElementRef {
                        segment_id,
                        element: *element,
                        component: *component
                    }
                )?;
                match (min, max) {
                    (Some(min), Some(max)) => write!(f, "{min} to {max}"),
                    (Some(min), None) => write!(f, "at least {min}"),
                    (None, Some(max)) => write!(f, "at most {max}"),
                    (None, None) => write!(f, "any length"),
                }
            }
            Rule::CompositeShape {
                segment_id,
                element,
                name,
                declared,
                found,
            } => write!(
                f,
                "element {} ({name}) has {found} components; the spec declares {declared}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                }
            ),
        }
    }
}

/// One finding about the data of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What failed.
    pub rule: Rule,
    /// The SNIP level of `rule`.
    pub level: SnipLevel,
    /// Index of the segment at fault; `None` when the finding is about the
    /// end of the stream.
    pub segment: Option<usize>,
    /// 1-based element position inside the segment, when the finding is.
    pub element: Option<usize>,
    /// 1-based component position inside the element, when the finding is.
    pub component: Option<usize>,
    /// Open loops at the time, outermost first.
    pub path: Vec<LoopRef>,
    /// The offending value as it appears in the file.
    pub datum: Vec<u8>,
}

impl Diagnostic {
    /// A diagnostic whose level is the rule's own.
    pub fn new(
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        component: Option<usize>,
        path: Vec<LoopRef>,
        datum: Vec<u8>,
    ) -> Diagnostic {
        Diagnostic {
            level: rule.level(),
            rule,
            segment,
            element,
            component,
            path,
            datum,
        }
    }

    /// Where the segment at fault lives in `document`; `None` when the
    /// diagnostic names no segment or the document has no such segment.
    pub fn span(&self, document: &Document<'_>) -> Option<Span> {
        self.segment
            .and_then(|index| document.spans().get(index))
            .cloned()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} · {} · ", self.level, self.rule)?;
        match self.segment {
            Some(segment) => write!(f, "segment #{segment}")?,
            None => write!(f, "end of stream")?,
        }
        if let Some(element) = self.element {
            write!(f, ", element {element}")?;
            if let Some(component) = self.component {
                write!(f, ", component {component}")?;
            }
        }
        write!(f, " · at ")?;
        if self.path.is_empty() {
            write!(f, "the root")?;
        }
        for (i, open) in self.path.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{open}")?;
        }
        write!(f, " · datum {}", Quoted(&self.datum))
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib diagnostic::`
Expected: 14 passed.

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/diagnostic.rs crates/edi835_core/src/lib.rs
git commit -m "diagnostic: Diagnostic, Rule, SnipLevel and LoopRef with a self-contained Display

Owned values only; the byte range resolves from a Document by segment index.
Level-2 rules are declared and their text is fixed; nothing emits them yet."
```

---
## Task 7: `EnvelopeChecker` — structural diagnostics from the events, rules from the spec

**Implementer tier:** Opus — the checker is a state machine that must stay in step with the engine's stack (ordinals, segment counts, child counts, end capture, implicit loops), and the spec gains a validated `control` object; judgment is needed wherever a real file disagrees with an expectation.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`
- Modify: `crates/edi835_core/specs/835.json`
- Create: `crates/edi835_core/src/check.rs`
- Modify: `crates/edi835_core/src/lib.rs`
- Modify: `crates/edi835_core/tests/common/mod.rs`
- Create: `crates/edi835_core/tests/check_envelope.rs`
- Modify: `crates/edi835_core/tests/samples/README.md`

**Interfaces:**
- Consumes: `Event` (Task 1), `Diagnostic`, `Rule`, `LoopRef` (Task 6), `Spec::get`, `Spec::loop_name`, `Segment::element`, `Element::simple`.
- Produces:
  - `pub enum ControlCount { Segments, Children }`, `pub struct Control { pub opener_element: usize, pub closer_element: usize, pub count_element: usize, pub count: ControlCount }`, `LoopDef::control: Option<Control>`.
  - JSON: `"control": { "opener_element": 2, "closer_element": 2, "count_element": 1, "count": "segments" }` on a loop that has `end`. All four keys are required.
  - `pub enum ControlError { ZeroPosition { key: &'static str }, UnknownCount { found: String }, NoEnd }` and `SpecError::BadControl { loop_name: String, reason: ControlError }`, Display `loop "transaction" has an invalid "control": "count" must be "segments" or "children"; found "segs"`.
  - `pub struct EnvelopeChecker<'s>` with `new(&'s Spec)`, `on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic]` (the events the engine returned for that segment; slice of a reused buffer) and `finish(&mut self) -> &[Diagnostic]` (closes what is still open and resets, like `LoopEngine::finish`).
  - Rules emitted: `UnknownSegment` for `Unmatched`; `ImplicitLoop` for `LoopOpened { implicit: true }` (datum and `caused_by` = the id of the segment being fed); `UnterminatedLoop` when a loop with `end` closes without having captured it — at the segment whose arrival closed it, or with `segment: None` at `finish` — except for implicit loops; on capture of a loop's `end`, `ControlCountMismatch` (count element vs. segments from trigger to end inclusive, or vs. child instances opened by their own trigger) and `ControlNumberMismatch` (closer element vs. the trigger's element, compared byte for byte; skipped for an implicit loop, which has no trigger).
  - Counting: every segment that yields `Captured` or `Unmatched` counts; `Empty` segments do not. A count is ASCII digits with optional leading zeros; anything else (empty, sign, space, overflow) is a mismatch whose `found` and datum are the text as written.
  - `LoopRef::ordinal` numbers the instances of each loop across the stream (`2000#2/2100#2` for the second claim under the second LX).
  - `tests/common::diagnostics_of(&Spec, &[u8], Delimiters) -> Vec<Diagnostic>`.

- [ ] **Step 1: Write the failing spec tests for `control`**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `patch_adds_a_loop`:

```rust
    fn control_error(control: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{"env":{{"trigger":{{"segment":"HD"}},"end":"TR","control":{control}}}}}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn builtin_835_declares_the_envelope_controls() {
        let spec = Spec::builtin_835();
        let control = |name: &str| spec.get(spec.loop_id(name).unwrap()).control;
        assert_eq!(
            control("interchange"),
            Some(Control {
                opener_element: 13,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Children,
            })
        );
        assert_eq!(
            control("group"),
            Some(Control {
                opener_element: 6,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Children,
            })
        );
        assert_eq!(
            control("transaction"),
            Some(Control {
                opener_element: 2,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Segments,
            })
        );
        assert_eq!(control("2100"), None);
    }

    #[test]
    fn bad_controls_are_rejected_with_the_loop_and_the_reason() {
        let cases = [
            (
                r#"{"opener_element":0,"closer_element":2,"count_element":1,"count":"segments"}"#,
                ControlError::ZeroPosition {
                    key: "opener_element",
                },
            ),
            (
                r#"{"opener_element":2,"closer_element":2,"count_element":0,"count":"segments"}"#,
                ControlError::ZeroPosition {
                    key: "count_element",
                },
            ),
            (
                r#"{"opener_element":2,"closer_element":2,"count_element":1,"count":"segs"}"#,
                ControlError::UnknownCount {
                    found: "segs".into(),
                },
            ),
        ];
        for (control, expected) in cases {
            let err = control_error(control);
            assert!(
                matches!(&err, SpecError::BadControl { loop_name, reason } if loop_name == "env" && *reason == expected),
                "{control}: {err:?}"
            );
        }
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"env":{"trigger":{"segment":"HD"},"control":{"opener_element":2,"closer_element":2,"count_element":1,"count":"segments"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                SpecError::BadControl {
                    reason: ControlError::NoEnd,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn a_control_that_is_not_an_object_or_misses_a_key_is_rejected() {
        let err = control_error("[2,2,1]");
        assert!(
            matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path == "loops.env.control"),
            "{err:?}"
        );
        let err = control_error(r#"{"opener_element":2,"closer_element":2,"count":"segments"}"#);
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "env"),
            "{err:?}"
        );
    }

    #[test]
    fn bad_control_displays_the_loop_and_every_reason() {
        let cases = [
            (
                ControlError::ZeroPosition {
                    key: "opener_element",
                },
                "loop \"transaction\" has an invalid \"control\": \"opener_element\" must be a 1-based element position; found 0",
            ),
            (
                ControlError::UnknownCount {
                    found: "segs".into(),
                },
                "loop \"transaction\" has an invalid \"control\": \"count\" must be \"segments\" or \"children\"; found \"segs\"",
            ),
            (
                ControlError::NoEnd,
                "loop \"transaction\" has an invalid \"control\": the loop has no \"end\" segment to check",
            ),
        ];
        for (reason, expected) in cases {
            let err = SpecError::BadControl {
                loop_name: "transaction".into(),
                reason,
            };
            assert_eq!(err.to_string(), expected);
            assert!(std::error::Error::source(&err).is_none());
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: compile errors (`cannot find type 'Control'`, `no field 'control' on type '&LoopDef'`, no variant `BadControl`).

- [ ] **Step 3: Implement `control` in the spec**

Add the field to `LoopDef`, after `end`:

```rust
    /// How the end segment checks the loop it closes, for envelope loops.
    pub control: Option<Control>,
```

and add after `struct LoopDef { … }` (before `impl LoopDef`):

```rust
/// What a loop's end segment counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCount {
    /// Every segment from the trigger to the end segment, both included.
    Segments,
    /// The child loop instances opened by their own trigger.
    Children,
}

/// The control elements an envelope loop's trigger and end segment carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// 1-based position of the control number in the trigger, e.g. `ST02`.
    pub opener_element: usize,
    /// 1-based position of the same control number in the end segment, e.g. `SE02`.
    pub closer_element: usize,
    /// 1-based position of the count in the end segment, e.g. `SE01`.
    pub count_element: usize,
    /// What the count counts.
    pub count: ControlCount,
}

/// Why a loop's `control` was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// A position key holds 0.
    ZeroPosition {
        /// The key, e.g. `opener_element`.
        key: &'static str,
    },
    /// `count` is not `segments` or `children`.
    UnknownCount {
        /// The value as written.
        found: String,
    },
    /// The loop has no `end` segment to carry the count and control number.
    NoEnd,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based element position; found 0")
            }
            ControlError::UnknownCount { found } => {
                write!(
                    f,
                    "\"count\" must be \"segments\" or \"children\"; found {found:?}"
                )
            }
            ControlError::NoEnd => write!(f, "the loop has no \"end\" segment to check"),
        }
    }
}
```

Add to `enum SpecError`, after `BadElementDef`:

```rust
    /// A loop's `control` is invalid.
    BadControl {
        /// The loop.
        loop_name: String,
        /// What is wrong with it.
        reason: ControlError,
    },
```

and to `Display`, after the `BadElementDef` arm:

```rust
            SpecError::BadControl { loop_name, reason } => {
                write!(f, "loop {loop_name:?} has an invalid \"control\": {reason}")
            }
```

Add the field to `struct RawLoop` and the raw type after it:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a loop object")]
struct RawLoop {
    parent: Option<String>,
    trigger: RawTrigger,
    #[serde(default)]
    segments: Vec<String>,
    end: Option<String>,
    control: Option<RawControl>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a control object")]
struct RawControl {
    opener_element: usize,
    closer_element: usize,
    count_element: usize,
    count: String,
}
```

In `from_value`, right after `conditions.sort();`, compile the control:

```rust
            let control = match &def.control {
                None => None,
                Some(raw) => Some(compile_control(raw, def.end.is_some()).map_err(|reason| {
                    SpecError::BadControl {
                        loop_name: name.clone(),
                        reason,
                    }
                })?),
            };
```

and pass it into the `LoopDef` (`control,` after `end: …,`). Add above `fn render_trigger`:

```rust
/// Validates a loop's `control`; `has_end` says whether the loop declares an end segment.
fn compile_control(raw: &RawControl, has_end: bool) -> Result<Control, ControlError> {
    if !has_end {
        return Err(ControlError::NoEnd);
    }
    let positions = [
        ("opener_element", raw.opener_element),
        ("closer_element", raw.closer_element),
        ("count_element", raw.count_element),
    ];
    if let Some((key, _)) = positions.iter().find(|(_, position)| *position == 0) {
        return Err(ControlError::ZeroPosition { key });
    }
    let count = match raw.count.as_str() {
        "segments" => ControlCount::Segments,
        "children" => ControlCount::Children,
        _ => {
            return Err(ControlError::UnknownCount {
                found: raw.count.clone(),
            });
        }
    };
    Ok(Control {
        opener_element: raw.opener_element,
        closer_element: raw.closer_element,
        count_element: raw.count_element,
        count,
    })
}
```

In `check_shape`, inside the loop over `loops`, after the `trigger` block:

```rust
            if let Some(control) = def.get("control") {
                object_at(control, &format!("{at}.control"))?;
            }
```

In `crates/edi835_core/specs/835.json`, give the three envelope loops their control (the rest of the file is unchanged):

```json
    "interchange": {
      "trigger": { "segment": "ISA" },
      "end": "IEA",
      "control": { "opener_element": 13, "closer_element": 2, "count_element": 1, "count": "children" }
    },
    "group": {
      "parent": "interchange",
      "trigger": { "segment": "GS" },
      "end": "GE",
      "control": { "opener_element": 6, "closer_element": 2, "count_element": 1, "count": "children" }
    },
    "transaction": {
      "parent": "group",
      "trigger": { "segment": "ST" },
      "segments": ["BPR", "TRN", "CUR", "REF", "DTM", "PLB"],
      "end": "SE",
      "control": { "opener_element": 2, "closer_element": 2, "count_element": 1, "count": "segments" }
    },
```

In `crates/edi835_core/src/lib.rs`, extend the `spec` re-export to:

```rust
pub use spec::{
    Control, ControlCount, ControlError, ElementDef, ElementDefError, ElementType, LoopDef, LoopId,
    SegmentDef, Spec, SpecError, Trigger, merge_patch,
};
```

Run: `cargo test -p edi835_core --lib spec::`
Expected: 73 passed.

- [ ] **Step 4: Write the failing checker tests**

Create `crates/edi835_core/src/check.rs` with only the module doc, the imports and the tests:

```rust
//! Envelope and structure checks over the engine's events.
//!
//! The checker follows the loops the engine opens and closes and reports what
//! the events alone reveal: segments no loop holds, loops opened without their
//! trigger, loops that close without their end segment, and end segments whose
//! count or control number disagrees with the loop they close. Which loops are
//! envelopes, and which elements carry the count and the control number, is
//! read from each loop's `control` in the spec.

use crate::diagnostic::{Diagnostic, LoopRef, Rule};
use crate::element::Element;
use crate::engine::Event;
use crate::segment::Segment;
use crate::spec::{ControlCount, LoopId, Spec};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, LoopEngine, Tokenizer};

    const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240101*1200*^*00501*000000001*0*P*>~";

    /// Runs the engine and the checker over `input` (with `*`, `:` and `~`)
    /// and returns every diagnostic, `finish` included.
    fn check(spec: &Spec, input: &str) -> Vec<Diagnostic> {
        let mut engine = LoopEngine::new(spec);
        let mut checker = EnvelopeChecker::new(spec);
        let mut out = Vec::new();
        let delims = Delimiters::new(b'*', b':', b'~');
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delims) {
            let events = engine.feed(&segment);
            out.extend_from_slice(checker.on(&segment, events));
        }
        engine.finish();
        out.extend_from_slice(checker.finish());
        out
    }

    fn rendered(spec: &Spec, input: &str) -> Vec<String> {
        check(spec, input).iter().map(ToString::to_string).collect()
    }

    /// A complete interchange around `body`, which sits between `ST*835*0001~`
    /// and the `SE`; `se01` is written as given.
    fn interchange(body: &str, se01: &str) -> String {
        format!(
            "{ISA}GS*HP*SENDER*RECEIVER*20240101*1200*7*X*005010X221A1~ST*835*0001~{body}SE*{se01}*0001~GE*1*7~IEA*1*000000001~"
        )
    }

    #[test]
    fn a_well_formed_interchange_yields_nothing() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~TRN*1*1~", "4");
        assert_eq!(check(&spec, &input), Vec::new());
    }

    #[test]
    fn an_unknown_segment_names_its_id_index_and_path() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~ZZZ*1~", "4");
        let diagnostics = check(&spec, &input);
        assert_eq!(
            diagnostics,
            vec![Diagnostic::new(
                Rule::UnknownSegment {
                    id: b"ZZZ".to_vec()
                },
                Some(4),
                None,
                None,
                vec![
                    LoopRef {
                        name: "interchange".into(),
                        ordinal: 1
                    },
                    LoopRef {
                        name: "group".into(),
                        ordinal: 1
                    },
                    LoopRef {
                        name: "transaction".into(),
                        ordinal: 1
                    },
                ],
                b"ZZZ".to_vec(),
            )]
        );
        assert_eq!(
            diagnostics[0].to_string(),
            "SNIP 1 · segment \"ZZZ\" is not part of the structure: no open loop holds it and it opens no loop · segment #4 · at interchange#1/group#1/transaction#1 · datum \"ZZZ\""
        );
    }

    #[test]
    fn implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end() {
        let spec = Spec::builtin_835();
        assert_eq!(
            rendered(&spec, "ST*835*0001~BPR*I*1*C*CHK~SE*3*0001~"),
            vec![
                "SNIP 1 · loop \"interchange\" opened without its own trigger to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"",
                "SNIP 1 · loop \"group\" opened without its own trigger to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"",
            ]
        );
    }

    #[test]
    fn a_wrong_segment_count_names_the_count_element() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~", "5");
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · SE01 declares \"5\" but the count is 3 · segment #4, element 1 · at interchange#1/group#1/transaction#1 · datum \"5\""
            ]
        );
    }

    #[test]
    fn a_count_that_is_not_a_number_is_a_mismatch_with_the_text_as_datum() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~", "3X");
        let diagnostics = check(&spec, &input);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].rule,
            Rule::ControlCountMismatch {
                segment_id: b"SE".to_vec(),
                element: 1,
                expected: 3,
                found: b"3X".to_vec(),
            }
        );
        assert_eq!(diagnostics[0].datum, b"3X");
    }

    #[test]
    fn leading_zeros_in_a_count_are_accepted() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~", "0003");
        assert_eq!(check(&spec, &input), Vec::new());
    }

    #[test]
    fn a_control_number_that_differs_from_the_opener_is_reported() {
        let spec = Spec::builtin_835();
        let input = format!(
            "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0002~GE*1*8~IEA*1*000000002~"
        );
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\"",
                "SNIP 1 · GE02 \"8\" does not match GS06 \"7\" · segment #4, element 2 · at interchange#1/group#1 · datum \"8\"",
                "SNIP 1 · IEA02 \"000000002\" does not match ISA13 \"000000001\" · segment #5, element 2 · at interchange#1 · datum \"000000002\"",
            ]
        );
    }

    #[test]
    fn group_and_interchange_counts_count_their_children() {
        let spec = Spec::builtin_835();
        let input = format!(
            "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0001~ST*835*0002~SE*2*0002~GE*1*7~IEA*2*000000001~"
        );
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · GE01 declares \"1\" but the count is 2 · segment #6, element 1 · at interchange#1/group#1 · datum \"1\"",
                "SNIP 1 · IEA01 declares \"2\" but the count is 1 · segment #7, element 1 · at interchange#1 · datum \"2\"",
            ]
        );
    }

    #[test]
    fn instances_are_numbered_in_stream_order() {
        let spec = Spec::builtin_835();
        let input = format!(
            "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0001~ST*835*0002~ZZZ~SE*3*0002~GE*2*7~IEA*1*000000001~"
        );
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · segment \"ZZZ\" is not part of the structure: no open loop holds it and it opens no loop · segment #5 · at interchange#1/group#1/transaction#2 · datum \"ZZZ\""
            ]
        );
    }

    #[test]
    fn a_loop_closed_by_an_outer_end_segment_is_unterminated() {
        let spec = Spec::builtin_835();
        let input = format!(
            "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~BPR*I*1*C*CHK~GE*1*7~IEA*1*000000001~"
        );
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · loop \"transaction\" closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
            ]
        );
    }

    #[test]
    fn loops_still_open_at_the_end_of_the_stream_are_unterminated() {
        let spec = Spec::builtin_835();
        let input = format!("{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~");
        assert_eq!(
            rendered(&spec, &input),
            vec![
                "SNIP 1 · loop \"transaction\" closed without its end segment \"SE\" · end of stream · at interchange#1/group#1/transaction#1 · datum \"\"",
                "SNIP 1 · loop \"group\" closed without its end segment \"GE\" · end of stream · at interchange#1/group#1 · datum \"\"",
                "SNIP 1 · loop \"interchange\" closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\"",
            ]
        );
    }

    #[test]
    fn envelope_rules_come_from_the_spec() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "batch":{"trigger":{"segment":"HDR"},"segments":["LN"],"end":"TRL",
                    "control":{"opener_element":1,"closer_element":2,"count_element":1,"count":"segments"}}
            }}"#,
        )
        .unwrap();
        assert_eq!(check(&spec, "HDR*A1~LN*x~LN*y~TRL*4*A1~"), Vec::new());
        assert_eq!(
            rendered(&spec, "HDR*A1~LN*x~TRL*9*B2~"),
            vec![
                "SNIP 1 · TRL01 declares \"9\" but the count is 3 · segment #2, element 1 · at batch#1 · datum \"9\"",
                "SNIP 1 · TRL02 \"B2\" does not match HDR01 \"A1\" · segment #2, element 2 · at batch#1 · datum \"B2\"",
            ]
        );
    }

    #[test]
    fn a_loop_without_control_only_checks_that_its_end_arrives() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{"batch":{"trigger":{"segment":"HDR"},"end":"TRL"}}}"#,
        )
        .unwrap();
        assert_eq!(check(&spec, "HDR*1~TRL*whatever~"), Vec::new());
        assert_eq!(
            rendered(&spec, "HDR*1~"),
            vec![
                "SNIP 1 · loop \"batch\" closed without its end segment \"TRL\" · end of stream · at batch#1 · datum \"\""
            ]
        );
    }

    #[test]
    fn empty_segments_are_not_counted() {
        let spec = Spec::builtin_835();
        let input = interchange("BPR*I*1*C*CHK~~", "3");
        assert_eq!(check(&spec, &input), Vec::new());
    }

    #[test]
    fn finishing_resets_the_checker() {
        let spec = Spec::builtin_835();
        let input = interchange("ZZZ~", "3");
        let first = check(&spec, &input);
        let mut engine = LoopEngine::new(&spec);
        let mut checker = EnvelopeChecker::new(&spec);
        let delims = Delimiters::new(b'*', b':', b'~');
        let run = |engine: &mut LoopEngine<'_>, checker: &mut EnvelopeChecker<'_>| {
            let mut out = Vec::new();
            for segment in Tokenizer::with_delimiters(input.as_bytes(), delims) {
                let events = engine.feed(&segment);
                out.extend_from_slice(checker.on(&segment, events));
            }
            engine.finish();
            out.extend_from_slice(checker.finish());
            out
        };
        assert_eq!(run(&mut engine, &mut checker), first);
        assert_eq!(
            run(&mut engine, &mut checker),
            first,
            "ordinals restart at 1"
        );
    }

    #[test]
    fn counts_parse_digits_only() {
        assert_eq!(parse_count(b"0042"), Some(42));
        assert_eq!(parse_count(b""), None);
        assert_eq!(parse_count(b"4 "), None);
        assert_eq!(parse_count(b"-4"), None);
        assert_eq!(parse_count(b"99999999999999999999999"), None);
    }
}
```

In `crates/edi835_core/src/lib.rs` add `pub mod check;` before `pub mod delimiters;` and `pub use check::EnvelopeChecker;` before the `delimiters` re-export.

- [ ] **Step 5: Run them to verify they fail**

Run: `cargo test -p edi835_core --lib check::`
Expected: compile errors (`cannot find type 'EnvelopeChecker'`, `cannot find function 'parse_count'`).

- [ ] **Step 6: Implement the checker**

Insert between `use crate::spec::{ControlCount, LoopId, Spec};` and `#[cfg(test)]`:

```rust
/// One loop instance the checker is inside of.
#[derive(Debug, Clone)]
struct Open {
    id: LoopId,
    ordinal: usize,
    implicit: bool,
    /// Non-empty segments consumed before the trigger of this instance.
    start: usize,
    /// Child instances opened by their own trigger.
    children: usize,
    /// The trigger's control number, for an envelope opened by its trigger.
    control_number: Option<Vec<u8>>,
    /// `true` once the loop's end segment has been captured.
    ended: bool,
}

/// Turns the engine's events into structural diagnostics, one segment at a time.
#[derive(Debug, Clone)]
pub struct EnvelopeChecker<'s> {
    spec: &'s Spec,
    open: Vec<Open>,
    /// Instances opened so far, per loop index.
    ordinals: Vec<usize>,
    /// Non-empty segments consumed so far.
    seen: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> EnvelopeChecker<'s> {
    /// A checker at the root, with nothing open.
    pub fn new(spec: &'s Spec) -> Self {
        Self {
            spec,
            open: Vec::new(),
            ordinals: vec![0; spec.loops().len()],
            seen: 0,
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics they raise. The slice is valid until the next call.
    pub fn on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic] {
        self.diagnostics.clear();
        for &event in events {
            match event {
                Event::LoopOpened {
                    id,
                    implicit,
                    segment: trigger,
                } => self.opened(id, implicit, trigger, segment),
                Event::Captured { id, .. } => {
                    self.seen += 1;
                    self.captured(id, segment);
                }
                Event::Unmatched { segment: index } => {
                    self.seen += 1;
                    self.report(
                        Rule::UnknownSegment {
                            id: segment.id.to_vec(),
                        },
                        Some(index),
                        None,
                        segment.id.to_vec(),
                    );
                }
                Event::LoopClosed { .. } => self.closed(Some(segment)),
                Event::Empty { .. } => {}
            }
        }
        &self.diagnostics
    }

    /// Closes every loop still open, as the engine's `finish` does, and
    /// returns the diagnostics that raises. The checker is then back at the
    /// root: feeding it again behaves like a fresh checker.
    pub fn finish(&mut self) -> &[Diagnostic] {
        self.diagnostics.clear();
        while !self.open.is_empty() {
            self.closed(None);
        }
        self.seen = 0;
        self.ordinals.iter_mut().for_each(|count| *count = 0);
        &self.diagnostics
    }

    fn opened(&mut self, id: LoopId, implicit: bool, trigger: usize, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(id);
        self.ordinals[id.index()] += 1;
        let ordinal = self.ordinals[id.index()];
        if !implicit && let Some(parent) = self.open.last_mut() {
            parent.children += 1;
        }
        let control_number = match def.control {
            Some(control) if !implicit => Some(value_at(segment, control.opener_element)),
            _ => None,
        };
        self.open.push(Open {
            id,
            ordinal,
            implicit,
            start: self.seen,
            children: 0,
            control_number,
            ended: false,
        });
        if implicit {
            self.report(
                Rule::ImplicitLoop {
                    loop_name: def.name.clone(),
                    caused_by: segment.id.to_vec(),
                },
                Some(trigger),
                None,
                segment.id.to_vec(),
            );
        }
    }

    fn captured(&mut self, id: LoopId, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(id);
        if def.end.as_deref() != Some(segment.id) {
            return;
        }
        let seen = self.seen;
        let Some(top) = self.open.last_mut().filter(|top| top.id == id) else {
            return;
        };
        top.ended = true;
        let Some(control) = def.control else {
            return;
        };
        let counted = match control.count {
            ControlCount::Segments => seen.saturating_sub(top.start),
            ControlCount::Children => top.children,
        };
        let opener_value = top.control_number.clone();

        let found = value_at(segment, control.count_element);
        if parse_count(&found) != Some(counted) {
            self.report(
                Rule::ControlCountMismatch {
                    segment_id: segment.id.to_vec(),
                    element: control.count_element,
                    expected: counted,
                    found: found.clone(),
                },
                Some(segment.index),
                Some(control.count_element),
                found,
            );
        }
        if let Some(opener_value) = opener_value {
            let closer_value = value_at(segment, control.closer_element);
            if closer_value != opener_value {
                self.report(
                    Rule::ControlNumberMismatch {
                        opener: def.trigger.segment.clone(),
                        opener_element: control.opener_element,
                        closer: segment.id.to_vec(),
                        closer_element: control.closer_element,
                        opener_value,
                        closer_value: closer_value.clone(),
                    },
                    Some(segment.index),
                    Some(control.closer_element),
                    closer_value,
                );
            }
        }
    }

    /// Closes the innermost open loop; `at` is the segment whose arrival
    /// closed it, or `None` at the end of the stream.
    fn closed(&mut self, at: Option<&Segment<'_>>) {
        let spec = self.spec;
        let Some(top) = self.open.last() else {
            return;
        };
        let def = spec.get(top.id);
        // An implicit loop never saw its trigger; its missing end is part of
        // the same gap and is already reported by its opening.
        if let Some(end) = &def.end
            && !top.ended
            && !top.implicit
        {
            self.report(
                Rule::UnterminatedLoop {
                    loop_name: def.name.clone(),
                    expected_end: end.clone(),
                },
                at.map(|segment| segment.index),
                None,
                at.map(|segment| segment.id.to_vec()).unwrap_or_default(),
            );
        }
        self.open.pop();
    }

    fn report(
        &mut self,
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        datum: Vec<u8>,
    ) {
        let spec = self.spec;
        let path = self
            .open
            .iter()
            .map(|open| LoopRef {
                name: spec.loop_name(open.id).to_string(),
                ordinal: open.ordinal,
            })
            .collect();
        self.diagnostics
            .push(Diagnostic::new(rule, segment, element, None, path, datum));
    }
}

/// The simple value at a 1-based position; empty when the element is absent
/// or composite.
fn value_at(segment: &Segment<'_>, position: usize) -> Vec<u8> {
    segment
        .element(position)
        .and_then(Element::simple)
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}

/// A count written as ASCII digits (leading zeros allowed); `None` for
/// anything else, including the empty value and overflow.
fn parse_count(value: &[u8]) -> Option<usize> {
    if value.is_empty() {
        return None;
    }
    value.iter().try_fold(0usize, |count, &byte| {
        if byte.is_ascii_digit() {
            count.checked_mul(10)?.checked_add(usize::from(byte - b'0'))
        } else {
            None
        }
    })
}
```

Run: `cargo test -p edi835_core --lib check::`
Expected: 16 passed.

- [ ] **Step 7: Write the integration test over the eleven files**

Append to `crates/edi835_core/tests/common/mod.rs`:

```rust
/// Tokenize `bytes` with `delims`, run the engine and the envelope checker
/// side by side, and return every diagnostic in order, `finish` included.
pub fn diagnostics_of(
    spec: &edi835_core::Spec,
    bytes: &[u8],
    delims: edi835_core::Delimiters,
) -> Vec<edi835_core::Diagnostic> {
    let mut engine = edi835_core::LoopEngine::new(spec);
    let mut checker = edi835_core::EnvelopeChecker::new(spec);
    let mut diagnostics = Vec::new();
    for segment in edi835_core::Tokenizer::with_delimiters(bytes, delims) {
        let events = engine.feed(&segment);
        diagnostics.extend_from_slice(checker.on(&segment, events));
    }
    engine.finish();
    diagnostics.extend_from_slice(checker.finish());
    diagnostics
}
```

Create `crates/edi835_core/tests/check_envelope.rs`:

```rust
//! Structural diagnostics over every real-shaped file: the known anomalies,
//! rendered in full, and nothing else.

mod common;

use std::collections::BTreeMap;

use edi835_core::{Document, SnipLevel, Spec};

#[test]
fn the_known_anomalies_are_reported_exactly_and_nothing_else() {
    let spec = Spec::builtin_835();
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, bytes, delims) in common::all_files() {
        let rendered: Vec<String> = common::diagnostics_of(&spec, &bytes, delims)
            .iter()
            .map(ToString::to_string)
            .collect();
        if !rendered.is_empty() {
            found.insert(name, rendered);
        }
    }
    let transaction = "interchange#1/group#1/transaction#1";
    let unknown = |id: &str, index: usize, path: &str| {
        format!(
            "SNIP 1 · segment \"{id}\" is not part of the structure: no open loop holds it and it opens no loop · segment #{index} · at {path} · datum \"{id}\""
        )
    };
    let se01 = |declared: &str, counted: usize, index: usize| {
        format!(
            "SNIP 1 · SE01 declares \"{declared}\" but the count is {counted} · segment #{index}, element 1 · at {transaction} · datum \"{declared}\""
        )
    };
    let expected = BTreeMap::from([
        (
            "blue_cross_nc_sample.txt".to_string(),
            vec![
                "SNIP 1 · loop \"interchange\" opened without its own trigger to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"".to_string(),
                "SNIP 1 · loop \"group\" opened without its own trigger to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"".to_string(),
                se01("33", 32, 31),
            ],
        ),
        (
            "edi835_test_file.RMT".to_string(),
            vec![se01("1202", 76, 77)],
        ),
        (
            "edi835_test_not_available_claim_id.RMT".to_string(),
            vec![se01("302", 255, 256)],
        ),
        (
            "multi_claim_sample.txt".to_string(),
            vec![
                unknown("N3", 19, &format!("{transaction}/2000#1/2100#1")),
                unknown("N4", 20, &format!("{transaction}/2000#1/2100#1")),
                unknown("N3", 34, &format!("{transaction}/2000#2/2100#2")),
                unknown("N4", 35, &format!("{transaction}/2000#2/2100#2")),
            ],
        ),
        (
            "trizetto_sample.rmt".to_string(),
            vec![
                unknown("XX", 7, &format!("{transaction}/1000A#1")),
                se01("15", 18, 19),
            ],
        ),
    ]);
    assert_eq!(found, expected);
}

#[test]
fn every_diagnostic_is_level_one_and_points_at_a_segment_holding_its_datum() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        for diagnostic in common::diagnostics_of(&spec, &bytes, delims) {
            assert_eq!(diagnostic.level, SnipLevel::L1, "{name}: {diagnostic}");
            let span = diagnostic
                .span(&document)
                .unwrap_or_else(|| panic!("{name}: {diagnostic} names no segment"));
            let body = &document.as_bytes()[span.body];
            assert!(
                body.windows(diagnostic.datum.len())
                    .any(|window| window == diagnostic.datum.as_slice()),
                "{name}: {diagnostic} points at {:?}",
                String::from_utf8_lossy(body)
            );
        }
    }
}
```

Run: `cargo test -p edi835_core --test check_envelope`
Expected: 2 passed. If `the_known_anomalies_are_reported_exactly_and_nothing_else` fails, do not edit the expectation to match: print `common::diagnostics_of` for the file named in the diff, compare it with the table "Facts this plan relies on" at the top of this plan, and report the discrepancy.

- [ ] **Step 8: Record the two excerpt samples**

Append this paragraph to the end of `crates/edi835_core/tests/samples/README.md`, after the paragraph that closes the table (one blank line before it):

```markdown
Two files are excerpts of larger transactions and keep the original `SE01`: `edi835_test_file.RMT` declares 1202 segments and holds 76; `edi835_test_not_available_claim_id.RMT` declares 302 and holds 255. The envelope checker reports both, and the test suite pins those two diagnostics as expected.
```

The sample files themselves are not touched.

- [ ] **Step 9: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0.

```bash
git add crates/edi835_core/src/spec.rs crates/edi835_core/specs/835.json crates/edi835_core/src/check.rs crates/edi835_core/src/lib.rs crates/edi835_core/tests/common/mod.rs crates/edi835_core/tests/check_envelope.rs crates/edi835_core/tests/samples/README.md
git commit -m "check: EnvelopeChecker reports structural diagnostics from the engine's events

Unknown segments, implicit loops, loops closed without their end, and end
segments whose count or control number disagrees with what they close.
Which loops are envelopes and which elements carry the count and control
number is the loop's \"control\" in the spec; the built-in sets it for the
interchange, group and transaction loops."
```

---
## Task 8: Benchmark, crate docs, README status, exit gate

**Implementer tier:** Haiku — three small edits given in full and the final sweep of the gates.

**Files:**
- Modify: `crates/edi835_core/benches/tokenize.rs`
- Modify: `crates/edi835_core/src/lib.rs` (crate docs only)
- Modify: `README.md`

`Cargo.toml` does not change: no dependency is added.

- [ ] **Step 1: Add the `check` group**

In `crates/edi835_core/benches/tokenize.rs`, replace the module doc and the import with:

```rust
//! Throughput of the tokenizer and the document index pass over the three
//! largest fixtures, of the loop engine over the three largest samples in
//! bytes and in events, and of the engine with the envelope checker over the
//! same samples.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use edi835_core::{Document, EnvelopeChecker, LoopEngine, Spec, Tokenizer};
```

add before `fn tokenize_fixtures`:

```rust
/// Runs the engine and the envelope checker over every segment and returns
/// how many diagnostics the checker raised.
fn run_check(spec: &Spec, bytes: &[u8]) -> usize {
    let mut engine = LoopEngine::new(spec);
    let mut checker = EnvelopeChecker::new(spec);
    let mut diagnostics = 0usize;
    for segment in Tokenizer::new(bytes).expect("sample has an ISA") {
        let events = engine.feed(&segment);
        diagnostics += checker.on(&segment, events).len();
    }
    engine.finish();
    diagnostics + checker.finish().len()
}
```

add after `fn engine_events_samples`:

```rust
fn check_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("check");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_check(&spec, black_box(bytes)));
        });
    }
    group.finish();
}
```

and register it:

```rust
criterion_group!(
    benches,
    tokenize_fixtures,
    index_fixtures,
    engine_samples,
    engine_events_samples,
    check_samples
);
```

- [ ] **Step 2: Update the crate docs**

In `crates/edi835_core/src/lib.rs`, replace the `Spec` and `LoopEngine` bullets of the module doc with:

```rust
//! - [`Spec`] is a JSON loop structure (parents, triggers, held segments, end
//!   segments, envelope controls) plus the names and types of each segment's
//!   elements, and can be patched with JSON Merge Patch; [`Spec::builtin_835`]
//!   ships the 835 as such data.
//! - [`LoopEngine`] interprets a segment stream against a spec and emits
//!   [`Event`]s (loops opened and closed, segments captured or unmatched);
//!   [`LoopTree`] collects those events into a tree of loop instances.
//! - [`EnvelopeChecker`] reads the same events and reports structural
//!   [`Diagnostic`]s: unknown segments, implicit or unterminated loops, and
//!   envelope counts or control numbers that do not match.
```

Run: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p edi835_core`
Expected: exits 0 (every intra-doc link resolves).

- [ ] **Step 3: Run the benches and record**

Run: `cargo bench --workspace --no-run --locked && cargo bench --workspace 2>&1 | grep -E '^(engine|check)/|thrpt:'` (about five minutes)
Expected: three `engine/*` and three `check/*` entries; `check/*` throughput a little below `engine/*`, since it runs the engine too and adds a few comparisons per event (it allocates only for an envelope's control number and for each diagnostic). Keep the `check` lines for the commit message.

- [ ] **Step 4: Update the README status**

Replace the `## Status` section of `README.md` with:

```markdown
## Status

**Stage 4a — element definitions and structural diagnostics.** The JSON spec now names
and types every element of the 835's segments and rejects malformed or ambiguous specs
in plain words. Alongside the loop engine, an envelope checker reports unknown segments,
implicit or unterminated loops and envelope counts or control numbers that do not match,
as self-explanatory diagnostics. Typed columnar projection comes next.
```

- [ ] **Step 5: Final sweep and commit**

Run: `cargo build --workspace --all-targets --locked && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo bench --workspace --no-run --locked`
Expected: every command exits 0; `cargo test` reports 228 tests in total (192 in the library).

Run: `grep -nE '//.*(\b(T1[0-9]|P[0-9]+|N[2-7]|D1[01])\b|[Ss]tage [0-9]|#(7|1[7-9])\b)' crates/edi835_core/src/*.rs`
Expected: no output (no decision, principle, stage or issue code in a comment).

```bash
git add crates/edi835_core/benches/tokenize.rs crates/edi835_core/src/lib.rs README.md
git commit -m "bench: envelope checker baseline; crate docs and README status

Baseline (criterion, <machine>):
  <paste the three check thrpt lines>"
```

---

## Status

Task boundaries follow the brief one to one: each task has its own red → green cycle and its own commit. Two adjustments, both inside a task rather than across tasks:

- The `control` object of the spec (with `ControlError`, `SpecError::BadControl` and the built-in's three controls) lives in Task 7, not in Task 3 or Task 5: it has no consumer before the checker, and its tests (`builtin_835_declares_the_envelope_controls`) are only meaningful next to the code that reads it.
- The shape pre-check for `control` is added in Task 7 for the same reason; Task 2 covers the root, `loops`, each loop, each trigger and `where`; Task 3 adds `segments`, each segment, `elements`, each element and each `composite`.

Tests in the whole workspace, measured on a scratch copy where every step of this plan was executed: 159 before; 163 after Task 1, 168 after Task 2, 182 after Task 3, 189 after Task 4, 192 after Task 5, 206 after Task 6, 228 after Tasks 7 and 8.

## Stage 4a exit gate (definition of done)

- [ ] `cargo test --workspace --locked` green: library 192 (`spec` 73, `engine` 18, `tree` 5, `diagnostic` 14, `check` 16, plus the Stage 1–2 modules), `engine_invariants` 5, `engine_golden` 4, `engine_custom_spec` 3, `check_envelope` 2, plus the existing suites; 228 in total.
- [ ] clippy `-D warnings`, fmt, `cargo bench --no-run` and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean.
- [ ] `[dependencies]` of `edi835_core` still exactly `serde` and `serde_json`.
- [ ] No `unwrap`/`expect`/`panic!` in `src/` outside `mod tests`, except the documented `expect` in `Spec::builtin_835`.
- [ ] The nine `.events.txt` goldens regenerated by the test, their diff limited to `open` lines gaining ` #<index>`; the two `.summary.txt` unchanged.
- [ ] Every new `SpecError` variant (`NotAnObject`, `SegmentSchema`, `BadElementDef` with its eight reasons, `EmptySegmentId`, `OverlappingTriggers`, `BadControl` with its three reasons) and every `Rule` variant (nine) has a full-text `Display` test.
- [ ] `src/` names no 835 segment outside `specs/835.json` and tests: `grep -nE '"(ST|SE|GS|GE|ISA|IEA)"|b"(ST|SE|GS|GE|ISA|IEA)"' crates/edi835_core/src/check.rs crates/edi835_core/src/diagnostic.rs` matches only inside `mod tests`.
- [ ] `cargo bench` runs the `check` group; baseline in a commit message.
- [ ] Fixtures and samples unchanged (`git diff master --stat -- crates/edi835_core/tests/fixtures crates/edi835_core/tests/samples` is empty).
- [ ] Issues #7, #17, #18 and #19 are closed by the PR (named in its body, not in code).

## Self-review

**Spec coverage (§7 Stage 4, the 4a subset).**

| Item | Where | Proof |
|---|---|---|
| T11 `Diagnostic` with `rule`, `level`, `segment`, `element`, `component`, `path: Vec<LoopRef>`, `datum`; owned values; `Display` contract; `span(&Document)` | Task 6 | one full-text `Display` test per `Rule` variant (nine), `span_resolves_the_segment_bytes_from_the_document` |
| T12 `LoopOpened { id, implicit, segment }`, `Node::opened_by`, `Event` same size, goldens regenerated once with only `open` lines changed | Task 1 | `an_event_stays_three_words`, `every_opening_names_the_segment_captured_right_after_it`, Step 8 diff command |
| T13 global `segments` section keyed by canonical position; types `AN`, `ID`, `N0`–`N9`, `R` (+`scale`), `DT`, `TM`; `composite`; opaque when absent; merge patch touches one element | Tasks 3, 5 | `segments_are_keyed_by_id_and_elements_by_position`, `a_segment_a_loop_lists_without_a_definition_stays_opaque`, `a_patch_retouches_one_element_and_keeps_the_rest`, `to_json_round_trips_the_segments_section` |
| T13 / #18 objects required, said in plain words, patches included | Tasks 2, 3, 7 | `a_spec_that_is_not_an_object_says_so_in_plain_words`, `every_object_of_the_*_schema_is_checked_with_its_path`, `a_patched_spec_goes_through_the_same_shape_check` |
| T13 / #7 empty ids in `segments`, `end` and the new section | Task 4 | `empty_segment_ids_are_rejected_with_the_loop_and_the_key`, `an_empty_segment_id_in_the_segments_section_is_rejected` |
| T13 / #17 overlapping siblings (no shared position differs and neither condition set strictly contains the other) rejected naming both; catch-all and superset siblings allowed; built-in loads | Task 4 | `siblings_testing_different_positions_overlap_and_are_rejected`, `a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads`, `a_strict_superset_of_conditions_does_not_overlap`, `siblings_that_differ_at_a_shared_position_do_not_overlap` |
| T13 built-in `segments` for the 29 ids | Task 5 | `builtin_835_defines_every_segment_its_loops_name` |
| T17 level 1: `SE01` counts ST..SE; `ST02`=`SE02`; `GS06`=`GE02`; `ISA13`=`IEA02`; `GE01` = number of ST; `IEA01` = number of GS; `UnknownSegment`; `ImplicitLoop` with the causing segment; `UnterminatedLoop` | Task 7 | `a_wrong_segment_count_names_the_count_element`, `a_control_number_that_differs_from_the_opener_is_reported` (all three pairs), `group_and_interchange_counts_count_their_children`, `an_unknown_segment_names_its_id_index_and_path`, `implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end`, both `*_unterminated` tests |
| Envelope rules are data, not code | Task 7 | `envelope_rules_come_from_the_spec` (an invented `HDR`/`TRL` envelope) |
| Known anomalies rendered (trizetto, blue_cross, multi_claim) | Task 7 | `the_known_anomalies_are_reported_exactly_and_nothing_else` |
| Bench | Task 8 | `check` group over the three largest samples |

Not in 4a by design (plan 4b): `TableDef`, `ColumnSource`, `Spec::tables`, `BadColumn`, columns, `Projector`, `Processor`, SNIP 2 emission, the `REF` column patch, table goldens.

**Placeholder scan.** `grep -nE 'TBD|TODO|FIXME|similar to Task|add validation|write tests for'` over this plan: no match. The only angle-bracket fields are `<machine>` and `<paste the three check thrpt lines>` in the Task 8 commit message, filled from the measured output exactly as in the Stage 3 plan; and `<loop>`/`<index>`/`<element>`/`<component>`/`<i>` inside format descriptions.

**Type consistency across tasks.** `Event::LoopOpened { id, implicit, segment }` (Task 1) is destructured with the same field names in `tree.rs`, `engine_golden.rs`, `engine_invariants.rs` (Task 1) and `check.rs` (Task 7). `ElementType` (Task 3) is used by `Rule::TypeMismatch` (Task 6). `SpecError::EmptySegmentId { loop_name: Option<String>, key }` (Task 4) is the shape every later test uses. `Control`/`ControlCount` (Task 7) are read by `EnvelopeChecker::captured` only. `Diagnostic::new(rule, segment, element, component, path, datum)` (Task 6) is the only constructor the checker uses (Task 7). `LoopRef { name, ordinal }` (Task 6) is built in `EnvelopeChecker::report` (Task 7). Every snippet was compiled and its tests run, task by task, on a scratch copy of the repository; the counts in Status are from those runs.

**Review Focus, each pinned to a test.**

1. Implicit openings name the descendant's trigger → `missing_ancestors_open_implicitly` and `every_opening_names_the_segment_captured_right_after_it` (Task 1).
2. Non-objects rejected before serde with the path as written, patches included → `a_patched_spec_goes_through_the_same_shape_check` (Task 2).
3. Siblings one segment can satisfy with neither more specific are rejected; catch-all, superset and `PR`/`PE` siblings load → `siblings_testing_different_positions_overlap_and_are_rejected`, `a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads`, `a_strict_superset_of_conditions_does_not_overlap` (Task 4).
4. Counts include unmatched, exclude empty, accept leading zeros, report text → `empty_segments_are_not_counted`, `leading_zeros_in_a_count_are_accepted`, `a_count_that_is_not_a_number_is_a_mismatch_with_the_text_as_datum` (Task 7).
5. Implicit envelopes never report their missing end; explicit ones do → `implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end`, `loops_still_open_at_the_end_of_the_stream_are_unterminated` (Task 7).
