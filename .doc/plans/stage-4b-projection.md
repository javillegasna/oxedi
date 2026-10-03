# Stage 4b — Proyección columnar + validación de elementos · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One pass over an 835 gives typed, Arrow-layout tables (payments, claims, services, adjustments, provider adjustments) and every structural and element diagnostic, all declared as data in the spec; the envelope diagnostics also name the missing trigger and the segment that opened the loop they speak about.

**Architecture:** `spec` gains a `tables` section (one table per loop instance, or per occurrence of a segment, or per element group of a repeating segment; columns read an element, a component, a group element or a segment index) validated at load with `BadTable`/`TableSchema` and linked into a chain of tables above each table. The new `column` module holds Arrow-layout buffers (`Bitmap`, `Column`, `ColumnData`, `Table`, `Tables`) and the X12 value parsers. The new `project` module holds `Projector`, a consumer of the engine's events beside the `EnvelopeChecker`: it checks every element the spec defines as each segment is captured (SNIP 2) and fills the tables from the values it parsed. The new `process` module holds `Processor`, which feeds the engine, the checker and the projector together; `Processor::run` does it over a `Document`. Nothing in `column.rs`, `project.rs` or `process.rs` names an 835 segment or loop outside its tests.

**Tech Stack:** Rust edition 2024 (stable, 1.88+ for `let` chains), `serde` 1 and `serde_json` 1 (no new dependencies: no `arrow`, `chrono` or `rust_decimal`); `proptest` and `criterion` already present.

**Spec:** `.doc/architectural-commitment.md` — §7 "Stage 4 · Proyección a dominio + validación": T14 (Arrow-layout columns without the crate), T15 (declarative `tables`), T16 (one pass, two consumers: `Processor`), T17 level 2 (and level 3 as an optional last task), the Entregable, Gate and "Fuera de alcance" paragraphs; argued from N1, N3, N4, N7, P1, P2, P4, P7, P9, P10. Three owner rulings of 2026-10-03 (`.doc/analysis/stage-4a-ledger.md`): the `R` scale cap and `max: 0` (#27), diagnostics that name the missing trigger and the opener (#25), and raw-text validation of elements declared without components (no `ISA16` special case).

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/`.

## Global Constraints

- Edition 2024, stable toolchain. `[dependencies]` of `edi835_core` stays exactly `serde` and `serde_json`; nothing that does I/O, threads or a runtime. `Cargo.toml` does not change in this plan.
- `git add` names the task's files; never `git add -A` or `git add .`.
- Every commit passes the four gates: `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, `cargo bench --workspace --no-run --locked`. The final sweep adds `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`.
- No `unwrap`, `expect`, `panic!` or fallible indexing on input in `src/` outside `#[cfg(test)]`. One documented exception: `Spec::builtin_835()`. Indexing with a `LoopId` or with a table index that the spec created (`spec.tables()[index]`, `self.tables[index]`) is allowed. Arithmetic on input-derived numbers uses `checked_*` / `saturating_*`; the date and time helpers work on at most four digits at a time, so their arithmetic is bounded by construction.
- Comments and doc comments describe implementation only: no stage numbers, principle codes (N1, P10), decision codes (T15), issue numbers or history.
- Every error and diagnostic names the rule that failed, where (table, column, loop and key as written for specs; segment index, element/component position and loop path for files) and the offending datum. Each new `SpecError` variant, each `TableDefError`/`ElementDefError` reason, each `CellError`/`RowError` variant and each changed `Rule` variant has a full-text `Display` test; `source()` returns the serde error or the inner error where there is one and `None` otherwise.
- `src/column.rs`, `src/project.rs` and `src/process.rs` name no 835 segment or loop outside `mod tests`.
- Memory: `Projector::on` allocates only when it appends a row (the row's cell vector and the growth of the table buffers) or emits a diagnostic. A row being collected, the checked values of the current segment and the text of a composite read whole live in buffers that are cleared and reused. The module doc of `project.rs` says so.
- Golden files are regenerated with `UPDATE_GOLDEN=1`, inspected with the commands given in Task 7, and committed with the code; never edited by hand. The nine `.events.txt` and two `.summary.txt` goldens do not change in this plan. Fixtures and samples are never modified.
- `tests/common/mod.rs` helpers (`all_files`, `load_fixture`, `load_sample`, `run_engine`, `run_engine_keeping`, `events_of`, `diagnostics_of`) are reused; Task 6 adds `table_header` and `table_rows`, Task 7 moves `describe_diff` there and adds `compare_goldens` and `SUMMARY_ONLY`.

## Review Focus

1. **A row's number is taken when its anchor instance opens and the row is appended when it closes; every row names the open row of each table above it, null when none is open; numbers are global, so draining part way keeps them meaningful.** Tests: `every_row_names_the_open_row_of_each_table_above_it`, `row_numbers_keep_counting_across_drains`, `finishing_appends_the_rows_still_open` (Task 5); `tables_drained_after_each_transaction_add_up_to_one_run` (Task 6); `draining_after_each_transaction_adds_up_to_one_run_over_the_document` (Task 6) and `every_row_points_at_its_anchor_and_at_the_rows_that_enclose_it` over the eleven files (Task 7).
2. **An element declared without components is validated and projected as its whole raw text, separator included; components are read only where the definition declares them.** Tests: `an_element_defined_without_components_is_read_as_one_text`, `components_are_checked_where_the_definition_declares_them`, `a_composite_with_more_components_than_declared_names_the_first_extra_one` (Task 5); `ISA16` raises nothing on any of the eleven files (Task 7 goldens).
3. **Each element is parsed once: the check parses it, the column takes that value; a value that is missing or not its type is a diagnostic and a null, a value of the wrong length is a diagnostic and kept; numeric lengths count digits only.** Tests: `a_value_that_is_not_its_type_is_reported_and_null`, `a_decimal_with_more_places_than_its_scale_is_a_type_mismatch`, `an_invalid_date_names_the_element_and_the_text`, `lengths_count_bytes_for_text_and_digits_for_numbers`, `valid_values_never_raise_a_diagnostic` (Task 5).
4. **A bad table definition is rejected naming the table, the column when there is one, and the reason with its datum; nested anchors, shared anchors and anchors under unrelated tables are refused because they would break row numbering or the parent chain.** Tests: `bad_tables_are_rejected_with_the_table_the_column_and_the_reason`, `table_errors_display_the_table_the_column_and_every_reason`, `deleting_a_loop_a_table_anchors_in_names_the_table` (Tasks 3, 4).
5. **Buffers follow Arrow: LSB-first validity, `i32` offsets starting at 0 and repeated under a null, zero in a fixed-width slot under a null, every column of a table the same length.** Tests: `a_bitmap_packs_bits_least_significant_first`, `a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls`, `rows_read_back_with_their_validity` (Task 2); `columns_have_equal_lengths_and_arrow_buffers` over the eleven files (Task 7).
6. **The envelope diagnostics name the missing trigger as the spec writes it and the segment that opened the loop.** Tests: `implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end`, `a_control_number_that_differs_from_the_opener_is_reported`, `loops_still_open_at_the_end_of_the_stream_are_unterminated` (Task 1) and blue_cross in `the_known_anomalies_are_reported_exactly_and_nothing_else` (Task 1).

---

## File Structure

```
crates/edi835_core/
├── specs/
│   └── 835.json               # + "tables": payments, claims, services, adjustments, provider_adjustments
├── src/
│   ├── lib.rs                 # + pub mod column, project, process; re-exports; crate docs
│   ├── spec.rs                # R scale cap, max 0; tables section (TableDef, ColumnSource, Repeat), BadTable, TableSchema
│   ├── diagnostic.rs          # ImplicitLoop.expected_trigger; UnterminatedLoop/ControlNumberMismatch.opened_at
│   ├── check.rs               # fills the new Rule fields
│   ├── column.rs              # NEW: Bitmap, Column, ColumnData, Cell, Table, Tables, parsers
│   ├── project.rs             # NEW: Projector (SNIP 2 + tables)
│   └── process.rs             # NEW: Processor, Output
├── benches/
│   └── tokenize.rs            # + "process" and "process_rows" groups
└── tests/
    ├── common/mod.rs          # + table_header, table_rows, describe_diff, compare_goldens, SUMMARY_ONLY
    ├── check_envelope.rs      # blue_cross lines name the missing trigger
    ├── engine_golden.rs       # uses the shared golden helpers; goldens unchanged
    ├── project_files.rs       # NEW: incremental == run, row counts, parents vs tree, Arrow buffers, patch
    ├── project_golden.rs      # NEW: tables and diagnostics goldens
    └── golden/project/        # NEW: <file>.tables.txt (9), <file>.tables.summary.txt (2), <file>.diagnostics.txt (11)
```

`column` depends on `spec` (for `ElementType`); `project` depends on `column`, `spec`, `engine`, `segment`, `element`, `delimiters` and `diagnostic`; `process` depends on `engine`, `check`, `project`, `document` and `column`. `engine` and `check` do not know `project` or `process` exist.

## Facts this plan relies on (verified against the files before writing it)

Every step below was executed on a scratch copy of the repository. With the built-in spec after Task 4, `Processor::run` gives these row counts and level-2 findings. The level-1 findings are the ones `check_envelope.rs` already pins (Task 1 changes only the blue_cross text).

| File | payments / claims / services / adjustments / provider_adjustments | Level-2 findings (all defects of the file, none from a built-in too strict) |
|---|---|---|
| emedny_sample.txt | 1 / 3 / 10 / 4 / 0 | none |
| united_healthcare_legacy_sample.txt | 1 / 2 / 5 / 7 / 0 | none |
| multi_claim_sample.txt | 1 / 2 / 5 / 10 / 0 | 16: `ISA06` 14 bytes (15 required); `BPR10` 9 bytes (10); `BPR17` holds the date `20190316` (one position late, so `BPR16` is empty); `PER03` holds a phone number (the contact name is missing); `DTM01` `50` twice (3 required); `SVC01` written `HC:99213` while `ISA16` is `>`, so it is one 8-byte qualifier and the code is missing, five services × 2 |
| trizetto_sample.rmt | 1 / 1 / 1 / 2 / 0 | 5: `ISA06` and `ISA08` 13 bytes; `NM108` holds `666666666A` (one element missing before it); `SVC01` with `:` while `ISA16` is `>` (× 2) |
| blue_cross_nc_sample.txt | 1 / 1 / 3 / 3 / 0 | 3: `TRN03` `560894904` 9 bytes (10); `PER03` holds a phone number; `SVC02` empty in the third service |
| edi835_test_davisvision.RMT | 1 / 1 / 2 / 2 / 1 | none |
| edi835_test_eyemed.RMT | 1 / 82 / 414 / 105 / 0 | none |
| edi835_test_file.RMT | 1 / 4 / 23 / 9 / 0 | none |
| edi835_test_not_available_claim_id.RMT | 1 / 18 / 26 / 24 / 0 | none; 18 `REF*CE` in 2100 (used by the patch test) |
| edi835_test_united.rmt | 1 / 1332 / 6192 / 2370 / 0 | none |
| edi835_test_versant.RMT | 1 / 648 / 1778 / 783 / 3 | none |

The six anonymized payer files raise no level-2 finding with the built-in definitions, so no built-in definition is relaxed. `ISA16` (a lone component separator) parses as a composite of two empty components; read as one text it is the one-byte separator and passes `AN` 1/1. Baseline on the scratch copy (release, criterion): `check` 83–91 MiB/s, `process` 28–30 MiB/s and 485–690 K rows/s on the three largest samples.

---

## Task 1: Settle two contracts first — the `R` scale cap and diagnostics that name the trigger and the opener

**Implementer tier:** Sonnet — four files and a `Display` contract change; every line is given, but the edits touch existing tests in three places and must land exactly as written.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`
- Modify: `crates/edi835_core/src/diagnostic.rs`
- Modify: `crates/edi835_core/src/check.rs`
- Modify: `crates/edi835_core/tests/check_envelope.rs`

**Interfaces:**
- Consumes: `ElementDefError`, `compile_elements`, `render_trigger` (`spec.rs`); `Rule` (`diagnostic.rs`); `EnvelopeChecker` (`check.rs`) as they are today.
- Produces:
  - `ElementType::MAX_SCALE: u8 = 18`. `ElementDefError::ScaleAboveMaximum { scale: u8 }` — `"scale" 19 is above the maximum of 18`; `ElementDefError::ZeroMax` — `"max" is 0; an element holds at least one character`. Both arrive inside `SpecError::BadElementDef { segment, position, reason }`, so the message names the segment, the position and the value.
  - `pub(crate) fn render_trigger(&Trigger) -> String` in `spec.rs` (was private).
  - `Rule::ImplicitLoop { loop_name, expected_trigger: String, caused_by }` — `loop "group" opened without its own trigger ("GS" with no conditions) to hold segment "ST"`.
  - `Rule::UnterminatedLoop { loop_name, expected_end, opened_at: Option<usize> }` — `loop "transaction" opened at segment #2 closed without its end segment "SE"`; with `None` the `opened at …` part is left out.
  - `Rule::ControlNumberMismatch { …, opened_at: Option<usize> }` — `SE02 "0002" does not match ST02 "0001" of segment #2`; with `None` the ` of segment …` suffix is left out.
  - The checker fills `opened_at` with the trigger index of an instance opened by its own trigger, `None` for an implicit one (which never reports these two rules anyway).

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `fn control_error`:

```rust
    #[test]
    fn an_r_scale_above_18_or_a_zero_max_is_rejected_with_the_value() {
        let cases = [
            (
                r#"{"3":{"name":"a","type":"R","scale":19}}"#,
                ElementDefError::ScaleAboveMaximum { scale: 19 },
            ),
            (
                r#"{"3":{"name":"a","type":"R","scale":200}}"#,
                ElementDefError::ScaleAboveMaximum { scale: 200 },
            ),
            (
                r#"{"3":{"name":"a","type":"AN","max":0}}"#,
                ElementDefError::ZeroMax,
            ),
            (
                r#"{"3":{"name":"a","type":"R","min":0,"max":0}}"#,
                ElementDefError::ZeroMax,
            ),
        ];
        for (elements, expected_reason) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == "3" && *reason == expected_reason),
                "{elements}: {err:?}"
            );
        }
        let at_the_cap = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"R","scale":18,"max":1}}}}}"#,
        )
        .unwrap();
        assert_eq!(
            at_the_cap.segment(b"AA").unwrap().elements[&1].kind,
            ElementType::R { scale: 18 }
        );
    }

    #[test]
    fn scale_and_max_reasons_display_segment_position_and_value() {
        let at = |reason| SpecError::BadElementDef {
            segment: "CLP".into(),
            position: "12".into(),
            reason,
        };
        assert_eq!(
            at(ElementDefError::ScaleAboveMaximum { scale: 19 }).to_string(),
            "segment \"CLP\" element \"12\": \"scale\" 19 is above the maximum of 18"
        );
        assert_eq!(
            at(ElementDefError::ZeroMax).to_string(),
            "segment \"CLP\" element \"12\": \"max\" is 0; an element holds at least one character"
        );
    }
```

In `crates/edi835_core/src/diagnostic.rs`, inside `mod tests`, replace `implicit_loop_displays_the_loop_and_the_segment_that_needed_it`, `unterminated_loop_displays_the_expected_end_and_the_closing_segment`, `a_finding_at_the_end_of_the_stream_says_so` and `control_number_mismatch_displays_both_elements_and_values` with these four tests (two are renamed):

```rust
    #[test]
    fn implicit_loop_displays_the_loop_and_the_segment_that_needed_it() {
        let diagnostic = Diagnostic::new(
            Rule::ImplicitLoop {
                loop_name: "group".into(),
                expected_trigger: "\"GS\" with no conditions".into(),
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
            "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\""
        );
    }

    #[test]
    fn unterminated_loop_displays_the_opener_the_expected_end_and_the_closing_segment() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "transaction".into(),
                expected_end: b"SE".to_vec(),
                opened_at: Some(2),
            },
            Some(4),
            None,
            None,
            path(TRANSACTION),
            b"GE".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
        );
        let implicit = Rule::UnterminatedLoop {
            loop_name: "transaction".into(),
            expected_end: b"SE".to_vec(),
            opened_at: None,
        };
        assert_eq!(
            implicit.to_string(),
            "loop \"transaction\" closed without its end segment \"SE\""
        );
    }

    #[test]
    fn a_finding_at_the_end_of_the_stream_says_so() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "interchange".into(),
                expected_end: b"IEA".to_vec(),
                opened_at: Some(0),
            },
            None,
            None,
            None,
            path(&[("interchange", 1)]),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"interchange\" opened at segment #0 closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\""
        );
    }

    #[test]
    fn control_number_mismatch_displays_both_elements_values_and_the_opener() {
        let diagnostic = Diagnostic::new(
            Rule::ControlNumberMismatch {
                opener: b"ST".to_vec(),
                opener_element: 2,
                closer: b"SE".to_vec(),
                closer_element: 2,
                opener_value: b"0001".to_vec(),
                closer_value: b"0002".to_vec(),
                opened_at: Some(2),
            },
            Some(4),
            Some(2),
            None,
            path(TRANSACTION),
            b"0002".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" of segment #2 · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\""
        );
        let implicit = Rule::ControlNumberMismatch {
            opener: b"ST".to_vec(),
            opener_element: 2,
            closer: b"SE".to_vec(),
            closer_element: 2,
            opener_value: b"0001".to_vec(),
            closer_value: b"0002".to_vec(),
            opened_at: None,
        };
        assert_eq!(
            implicit.to_string(),
            "SE02 \"0002\" does not match ST02 \"0001\""
        );
    }
```

In `crates/edi835_core/src/check.rs`, inside `mod tests`, replace these six tests with the versions below (only the expected strings change):

```rust
    #[test]
    fn implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end() {
        let spec = Spec::builtin_835();
        assert_eq!(
            rendered(&spec, "ST*835*0001~BPR*I*1*C*CHK~SE*3*0001~"),
            vec![
                "SNIP 1 · loop \"interchange\" opened without its own trigger (\"ISA\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"",
                "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"",
            ]
        );
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
                "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" of segment #2 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\"",
                "SNIP 1 · GE02 \"8\" does not match GS06 \"7\" of segment #1 · segment #4, element 2 · at interchange#1/group#1 · datum \"8\"",
                "SNIP 1 · IEA02 \"000000002\" does not match ISA13 \"000000001\" of segment #0 · segment #5, element 2 · at interchange#1 · datum \"000000002\"",
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
                "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
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
                "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · end of stream · at interchange#1/group#1/transaction#1 · datum \"\"",
                "SNIP 1 · loop \"group\" opened at segment #1 closed without its end segment \"GE\" · end of stream · at interchange#1/group#1 · datum \"\"",
                "SNIP 1 · loop \"interchange\" opened at segment #0 closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\"",
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
                "SNIP 1 · TRL02 \"B2\" does not match HDR01 \"A1\" of segment #0 · segment #2, element 2 · at batch#1 · datum \"B2\"",
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
                "SNIP 1 · loop \"batch\" opened at segment #0 closed without its end segment \"TRL\" · end of stream · at batch#1 · datum \"\""
            ]
        );
    }
```

In `crates/edi835_core/tests/check_envelope.rs`, in `the_known_anomalies_are_reported_exactly_and_nothing_else`, replace the two blue_cross `ImplicitLoop` lines with:

```rust
                "SNIP 1 · loop \"interchange\" opened without its own trigger (\"ISA\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"".to_string(),
                "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"".to_string(),
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib`
Expected: compile errors — `no variant named 'ScaleAboveMaximum'`, `no variant named 'ZeroMax'`, `variant 'Rule::ImplicitLoop' has no field named 'expected_trigger'`, `has no field named 'opened_at'`.

- [ ] **Step 3: The `R` scale cap and `max: 0` in the spec**

In `crates/edi835_core/src/spec.rs`, in `enum ElementDefError`, add after `NestedComposite,`:

```rust
    /// `scale` is above [`ElementType::MAX_SCALE`].
    ScaleAboveMaximum {
        /// The scale as written.
        scale: u8,
    },
    /// `max` is 0, so no value could ever be valid.
    ZeroMax,
```

In `impl fmt::Display for ElementDefError`, add after the `ElementDefError::NestedComposite` arm:

```rust
            ElementDefError::ScaleAboveMaximum { scale } => write!(
                f,
                "\"scale\" {scale} is above the maximum of {}",
                ElementType::MAX_SCALE
            ),
            ElementDefError::ZeroMax => {
                write!(f, "\"max\" is 0; an element holds at least one character")
            }
```

At the top of `impl ElementType`, before `fn parse`, add:

```rust
    /// Largest `scale` an `R` element may declare. An `R` value is held as an
    /// `i128` scaled by `10^scale` in a column of precision 38, so a scale of
    /// 18 still leaves 20 digits for the integer part.
    pub const MAX_SCALE: u8 = 18;
```

In `fn compile_elements`, right after `let kind = ElementType::parse(&def.kind, def.scale).map_err(fail)?;`, add:

```rust
        if let ElementType::R { scale } = kind
            && scale > ElementType::MAX_SCALE
        {
            return Err(fail(ElementDefError::ScaleAboveMaximum { scale }));
        }
        if def.max == Some(0) {
            return Err(fail(ElementDefError::ZeroMax));
        }
```

Make `render_trigger` visible to the checker: change `fn render_trigger(trigger: &Trigger) -> String {` to

```rust
pub(crate) fn render_trigger(trigger: &Trigger) -> String {
```

- [ ] **Step 4: The new `Rule` fields and their text**

In `crates/edi835_core/src/diagnostic.rs`, replace the `ImplicitLoop` and `UnterminatedLoop` variants of `enum Rule` with:

```rust
    /// A loop was opened without its own trigger, to hold a descendant.
    ImplicitLoop {
        /// The loop that was opened.
        loop_name: String,
        /// The loop's own trigger as the spec writes it, e.g.
        /// `"GS" with no conditions` or `"N1" where {1: "PR"}`.
        expected_trigger: String,
        /// Id of the segment whose loop needed it.
        caused_by: Vec<u8>,
    },
    /// A loop that declares an end segment closed without capturing it.
    UnterminatedLoop {
        /// The loop.
        loop_name: String,
        /// The end segment the spec declares for it.
        expected_end: Vec<u8>,
        /// Index of the segment that opened the instance; `None` for an
        /// instance opened implicitly.
        opened_at: Option<usize>,
    },
```

In `ControlNumberMismatch`, add after `closer_value: Vec<u8>,`:

```rust
        /// Index of the opening segment; `None` for an instance opened implicitly.
        opened_at: Option<usize>,
```

In `impl fmt::Display for Rule`, replace the `Rule::ImplicitLoop` and `Rule::UnterminatedLoop` arms with:

```rust
            Rule::ImplicitLoop {
                loop_name,
                expected_trigger,
                caused_by,
            } => write!(
                f,
                "loop {loop_name:?} opened without its own trigger ({expected_trigger}) to hold segment {}",
                Quoted(caused_by)
            ),
            Rule::UnterminatedLoop {
                loop_name,
                expected_end,
                opened_at,
            } => {
                write!(f, "loop {loop_name:?} ")?;
                if let Some(opened_at) = opened_at {
                    write!(f, "opened at segment #{opened_at} ")?;
                }
                write!(f, "closed without its end segment {}", Quoted(expected_end))
            }
```

and replace the `Rule::ControlNumberMismatch` arm with:

```rust
            Rule::ControlNumberMismatch {
                opener,
                opener_element,
                closer,
                closer_element,
                opener_value,
                closer_value,
                opened_at,
            } => {
                write!(
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
                )?;
                match opened_at {
                    Some(opened_at) => write!(f, " of segment #{opened_at}"),
                    None => Ok(()),
                }
            }
```

- [ ] **Step 5: The checker fills them**

In `crates/edi835_core/src/check.rs`:

1. Change the spec import to `use crate::spec::{ControlCount, LoopId, Spec, render_trigger};`.
2. In `struct Open`, add after `implicit: bool,`:

```rust
    /// Index of the trigger that opened the instance; `None` when implicit.
    opened_at: Option<usize>,
```

3. In `fn opened`, in the `Open { … }` literal, add after `implicit,`: `opened_at: (!implicit).then_some(trigger),`; and in the `Rule::ImplicitLoop { … }` literal add after `loop_name: def.name.clone(),`: `expected_trigger: render_trigger(&def.trigger),`.
4. In `fn captured`, right after `top.ended = true;`, add `let opened_at = top.opened_at;`, and in the `Rule::ControlNumberMismatch { … }` literal add `opened_at,` after `closer_value: closer_value.to_vec(),`.
5. In `fn closed`, in the `Rule::UnterminatedLoop { … }` literal, add `opened_at: top.opened_at,` after `expected_end: end.clone(),`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib && cargo test -p edi835_core --test check_envelope`
Expected: all pass (library 194, `check_envelope` 2).

- [ ] **Step 7: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 231 tests in the workspace.

```bash
git add crates/edi835_core/src/spec.rs crates/edi835_core/src/diagnostic.rs crates/edi835_core/src/check.rs crates/edi835_core/tests/check_envelope.rs
git commit -m "spec, diagnostic: cap R scale at 18, reject max 0; name the missing trigger and the opener

An implicit loop names the trigger it lacks as the spec writes it.
Unterminated loops and control number mismatches name the segment
that opened the instance."
```

---

## Task 2: The `column` module — Arrow-layout buffers and X12 value parsers

**Implementer tier:** Sonnet — one new file of about 1,300 lines with every line given; the care is in transcribing the bit and offset arithmetic and the date algorithm exactly, and in keeping the proptest strategies as written.

**Files:**
- Create: `crates/edi835_core/src/column.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `ElementType` (`spec.rs`).
- Produces:
  - `pub const DECIMAL_PRECISION: u8 = 38`.
  - `pub enum ColumnType { Binary, Int64 { scale: u8 }, Decimal128 { precision: u8, scale: u8 }, Date32, Time32 }`, `ColumnType::of(Option<ElementType>) -> ColumnType` (`AN`/`ID`/undefined → `Binary`; `N`n → `Int64 { scale: n }`; `R { scale }` → `Decimal128 { precision: 38, scale }`; `DT` → `Date32`; `TM` → `Time32`), `Display` `binary`, `int64`, `int64 (scale 2)`, `decimal128(38, 2)`, `date32`, `time32 (seconds)`.
  - `pub struct Bitmap` — `new`, `push(bool)`, `get(usize) -> Option<bool>`, `len`, `is_empty`, `unset_count`, `as_bytes`; one bit per row, LSB first, set = valid.
  - `pub enum Column { Binary { offsets: Vec<i32>, data: Vec<u8> }, Int64 { values: Vec<i64>, scale: u8 }, Decimal128 { values: Vec<i128>, precision: u8, scale: u8 }, Date32(Vec<i32>), Time32(Vec<i32>) }` — Arrow's physical layouts: offsets start at 0 and repeat under a null; a fixed-width slot holds 0 under a null.
  - `pub enum Cell<'a> { Null, Binary(&'a [u8]), Int64(i64), Decimal128(i128), Date32(i32), Time32(i32) }` (`Copy`).
  - `pub struct ColumnData` (a `Column` plus its `Bitmap`, fields private so the two never disagree) — `new(ColumnType)`, `kind`, `column`, `validity`, `len`, `is_empty`, `null_count`, `get(row) -> Option<Cell<'_>>`, `push_null()`, `push(Cell) -> Result<(), CellError>`, `render(row) -> Option<String>` (bytes as lossy UTF-8, integers as digits, decimals in fixed point with the column's scale, `YYYY-MM-DD`, `HH:MM:SS`, null as `∅`).
  - `pub enum CellError { TypeMismatch { column: ColumnType, cell: &'static str }, BinaryOverflow { bytes: usize } }` — `a date32 column cannot hold a int64 value`; `a binary column holds at most 2147483647 bytes; this value would bring it to <n>`.
  - `pub struct Table` (fields private) — `new(name, columns: impl IntoIterator<Item = (String, ColumnType)>)`, `name`, `columns() -> &[(String, ColumnData)]`, `column(name) -> Option<&ColumnData>`, `len` (rows; every column has it), `is_empty`, `push_row(&[Cell]) -> Result<(), RowError>` (all cells or none), `take_rows() -> Table` (moves the rows out, keeps the empty columns).
  - `pub enum RowError { Arity { table, expected, found }, Cell { table, column, source: CellError } }` — `table "claims" has 5 columns; the row has 4 cells`; `table "claims" column "charge": <cell error>`; `source()` is the `CellError`.
  - `pub struct Tables` — `new(Vec<Table>)` (orders by name), `get(name)`, `iter`, `len`, `is_empty`, `IntoIterator for &Tables`.
  - Parsers, allocation-free and panic-free: `parse_n(&[u8]) -> Option<i64>` (optional `-`, digits only); `parse_r(&[u8], scale: u8) -> Option<i128>` (optional `-`, digits, at most one `.` and at most `scale` decimals, at least one digit, no spaces, no `+`, no exponent, under 38 digits once scaled); `parse_dt(&[u8]) -> Option<i32>` (`CCYYMMDD` or `YYMMDD`, 00–49 → 20xx, 50–99 → 19xx, a real calendar date; days since 1970-01-01); `parse_tm(&[u8]) -> Option<i32>` (`HHMM`, `HHMMSS`, `HHMMSSd`, `HHMMSSdd`; sub-seconds ignored; seconds since midnight).

One `push(Cell)` stands in for a typed `push_*` per type, so the type check lives in one place and a mismatch is an error rather than a silent null; `push_null` stays for the common case.

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/column.rs` with the module doc, the imports and the tests only:

```rust
//! Typed columns laid out the way Apache Arrow lays out its arrays.
//!
//! A [`ColumnData`] is a value buffer plus a validity [`Bitmap`] (one bit per
//! row, least significant bit first, set when the row holds a value). Value
//! buffers follow Arrow's physical layouts: `Binary` keeps `i32` offsets
//! (one more than the rows, starting at 0) into one byte buffer, and the
//! fixed-width types keep one slot per row, zero for a null row. A
//! [`Table`] is a named list of columns that always have the same length,
//! and [`Tables`] is a list of tables ordered by name.
//!
//! The parsers turn X12 element text into column values without allocating:
//! [`parse_n`], [`parse_r`], [`parse_dt`] and [`parse_tm`].

use std::fmt;

use crate::spec::ElementType;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// An `R` value written the way X12 writes it, with exactly `scale` decimals.
    fn format_r(value: i128, scale: u8) -> String {
        let sign = if value < 0 { "-" } else { "" };
        let digits = value.unsigned_abs().to_string();
        let scale = usize::from(scale);
        if scale == 0 {
            return format!("{sign}{digits}");
        }
        let padded = format!("{digits:0>width$}", width = scale + 1);
        let (whole, fraction) = padded.split_at(padded.len() - scale);
        format!("{sign}{whole}.{fraction}")
    }

    #[test]
    fn column_types_follow_the_element_types() {
        assert_eq!(ColumnType::of(None), ColumnType::Binary);
        assert_eq!(ColumnType::of(Some(ElementType::An)), ColumnType::Binary);
        assert_eq!(ColumnType::of(Some(ElementType::Id)), ColumnType::Binary);
        assert_eq!(
            ColumnType::of(Some(ElementType::N(2))),
            ColumnType::Int64 { scale: 2 }
        );
        assert_eq!(
            ColumnType::of(Some(ElementType::R { scale: 4 })),
            ColumnType::Decimal128 {
                precision: 38,
                scale: 4
            }
        );
        assert_eq!(ColumnType::of(Some(ElementType::Dt)), ColumnType::Date32);
        assert_eq!(ColumnType::of(Some(ElementType::Tm)), ColumnType::Time32);
    }

    #[test]
    fn column_types_display_their_arrow_names() {
        assert_eq!(ColumnType::Binary.to_string(), "binary");
        assert_eq!(ColumnType::Int64 { scale: 0 }.to_string(), "int64");
        assert_eq!(
            ColumnType::Int64 { scale: 2 }.to_string(),
            "int64 (scale 2)"
        );
        assert_eq!(
            ColumnType::Decimal128 {
                precision: 38,
                scale: 2
            }
            .to_string(),
            "decimal128(38, 2)"
        );
        assert_eq!(ColumnType::Date32.to_string(), "date32");
        assert_eq!(ColumnType::Time32.to_string(), "time32 (seconds)");
    }

    #[test]
    fn a_bitmap_packs_bits_least_significant_first() {
        let mut bitmap = Bitmap::new();
        for valid in [true, false, true, true, false, false, false, false, true] {
            bitmap.push(valid);
        }
        assert_eq!(bitmap.len(), 9);
        assert_eq!(bitmap.as_bytes(), &[0b0000_1101, 0b0000_0001]);
        assert_eq!(bitmap.get(0), Some(true));
        assert_eq!(bitmap.get(1), Some(false));
        assert_eq!(bitmap.get(8), Some(true));
        assert_eq!(bitmap.get(9), None);
        assert_eq!(bitmap.unset_count(), 5);
        assert!(Bitmap::new().is_empty());
    }

    #[test]
    fn a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls() {
        let mut column = ColumnData::new(ColumnType::Binary);
        column.push(Cell::Binary(b"AB")).unwrap();
        column.push_null();
        column.push(Cell::Binary(b"")).unwrap();
        column.push(Cell::Binary(b"CDE")).unwrap();
        assert_eq!(
            column.column(),
            &Column::Binary {
                offsets: vec![0, 2, 2, 2, 5],
                data: b"ABCDE".to_vec()
            }
        );
        assert_eq!(column.validity().as_bytes(), &[0b1101]);
        assert_eq!(column.get(0), Some(Cell::Binary(b"AB")));
        assert_eq!(column.get(1), Some(Cell::Null));
        assert_eq!(column.get(2), Some(Cell::Binary(b"")));
        assert_eq!(column.get(3), Some(Cell::Binary(b"CDE")));
        assert_eq!(column.get(4), None);
        assert_eq!((column.len(), column.null_count()), (4, 1));
    }

    #[test]
    fn cells_render_as_text_by_column_type() {
        let rendered = |kind: ColumnType, cells: &[Cell<'_>]| -> Vec<String> {
            let mut column = ColumnData::new(kind);
            for &cell in cells {
                column.push(cell).unwrap();
            }
            (0..column.len())
                .map(|row| column.render(row).unwrap())
                .collect()
        };
        assert_eq!(
            rendered(ColumnType::Binary, &[Cell::Binary(b"HC:99213"), Cell::Null]),
            vec!["HC:99213", "∅"]
        );
        assert_eq!(
            rendered(ColumnType::Int64 { scale: 2 }, &[Cell::Int64(-42)]),
            vec!["-42"]
        );
        assert_eq!(
            rendered(
                ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2
                },
                &[
                    Cell::Decimal128(12345),
                    Cell::Decimal128(-5),
                    Cell::Decimal128(0)
                ]
            ),
            vec!["123.45", "-0.05", "0.00"]
        );
        assert_eq!(
            rendered(
                ColumnType::Decimal128 {
                    precision: 38,
                    scale: 0
                },
                &[Cell::Decimal128(7)]
            ),
            vec!["7"]
        );
        assert_eq!(
            rendered(ColumnType::Date32, &[Cell::Date32(0), Cell::Date32(19_782)]),
            vec!["1970-01-01", "2024-02-29"]
        );
        assert_eq!(
            rendered(ColumnType::Time32, &[Cell::Time32(45_045)]),
            vec!["12:30:45"]
        );
        assert_eq!(ColumnData::new(ColumnType::Binary).render(0), None);
    }

    #[test]
    fn fixed_width_columns_hold_zero_under_a_null() {
        let mut column = ColumnData::new(ColumnType::Decimal128 {
            precision: 38,
            scale: 2,
        });
        column.push(Cell::Decimal128(-1250)).unwrap();
        column.push(Cell::Null).unwrap();
        assert_eq!(
            column.column(),
            &Column::Decimal128 {
                values: vec![-1250, 0],
                precision: 38,
                scale: 2
            }
        );
        assert_eq!(column.get(1), Some(Cell::Null));
    }

    #[test]
    fn a_cell_of_another_type_is_refused_and_nothing_is_appended() {
        let mut column = ColumnData::new(ColumnType::Date32);
        assert_eq!(
            column.push(Cell::Int64(3)),
            Err(CellError::TypeMismatch {
                column: ColumnType::Date32,
                cell: "int64"
            })
        );
        assert!(column.is_empty());
    }

    #[test]
    fn cell_errors_display_the_column_and_the_cell() {
        assert_eq!(
            CellError::TypeMismatch {
                column: ColumnType::Date32,
                cell: "int64"
            }
            .to_string(),
            "a date32 column cannot hold a int64 value"
        );
        assert_eq!(
            CellError::BinaryOverflow { bytes: 2147483650 }.to_string(),
            "a binary column holds at most 2147483647 bytes; this value would bring it to 2147483650"
        );
    }

    #[test]
    fn a_row_is_appended_whole_or_not_at_all() {
        let mut table = Table::new(
            "t",
            [
                ("id".to_string(), ColumnType::Binary),
                ("amount".to_string(), ColumnType::Int64 { scale: 0 }),
            ],
        );
        table
            .push_row(&[Cell::Binary(b"A"), Cell::Int64(1)])
            .unwrap();
        let err = table
            .push_row(&[Cell::Binary(b"B"), Cell::Binary(b"x")])
            .unwrap_err();
        assert_eq!(
            err,
            RowError::Cell {
                table: "t".into(),
                column: "amount".into(),
                source: CellError::TypeMismatch {
                    column: ColumnType::Int64 { scale: 0 },
                    cell: "binary"
                }
            }
        );
        assert_eq!(
            table.push_row(&[Cell::Null]),
            Err(RowError::Arity {
                table: "t".into(),
                expected: 2,
                found: 1
            })
        );
        assert_eq!(table.len(), 1);
        for (_, column) in table.columns() {
            assert_eq!(column.len(), 1);
        }
        assert_eq!(table.column("id").unwrap().get(0), Some(Cell::Binary(b"A")));
        assert_eq!(table.column("missing"), None);
    }

    #[test]
    fn row_errors_display_the_table_and_the_column() {
        let arity = RowError::Arity {
            table: "claims".into(),
            expected: 5,
            found: 4,
        };
        assert_eq!(
            arity.to_string(),
            "table \"claims\" has 5 columns; the row has 4 cells"
        );
        assert!(std::error::Error::source(&arity).is_none());
        let cell = RowError::Cell {
            table: "claims".into(),
            column: "charge".into(),
            source: CellError::TypeMismatch {
                column: ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2,
                },
                cell: "binary",
            },
        };
        assert_eq!(
            cell.to_string(),
            "table \"claims\" column \"charge\": a decimal128(38, 2) column cannot hold a binary value"
        );
        assert!(std::error::Error::source(&cell).is_some());
    }

    #[test]
    fn taking_rows_leaves_an_empty_table_with_the_same_columns() {
        let mut table = Table::new("t", [("n".to_string(), ColumnType::Int64 { scale: 2 })]);
        table.push_row(&[Cell::Int64(7)]).unwrap();
        let taken = table.take_rows();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken.column("n").unwrap().get(0), Some(Cell::Int64(7)));
        assert!(table.is_empty());
        assert_eq!(
            table.column("n").unwrap().kind(),
            ColumnType::Int64 { scale: 2 }
        );
    }

    #[test]
    fn tables_are_ordered_by_name() {
        let tables = Tables::new(vec![Table::new("services", []), Table::new("claims", [])]);
        let names: Vec<&str> = tables.iter().map(Table::name).collect();
        assert_eq!(names, vec!["claims", "services"]);
        assert_eq!(tables.get("services").map(Table::name), Some("services"));
        assert_eq!(tables.len(), 2);
    }

    #[test]
    fn n_values_are_digits_with_an_optional_minus() {
        assert_eq!(parse_n(b"0"), Some(0));
        assert_eq!(parse_n(b"007"), Some(7));
        assert_eq!(parse_n(b"-5"), Some(-5));
        assert_eq!(parse_n(b"9223372036854775807"), Some(i64::MAX));
        assert_eq!(parse_n(b"-9223372036854775808"), Some(i64::MIN));
        for text in [
            &b""[..],
            b"-",
            b"+5",
            b"1.0",
            b" 1",
            b"1 ",
            b"1a",
            b"9223372036854775808",
        ] {
            assert_eq!(parse_n(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn r_values_are_scaled_and_never_lose_a_decimal() {
        assert_eq!(parse_r(b"12.34", 2), Some(1234));
        assert_eq!(parse_r(b"12.3", 2), Some(1230));
        assert_eq!(parse_r(b"12", 2), Some(1200));
        assert_eq!(parse_r(b"-0.5", 2), Some(-50));
        assert_eq!(parse_r(b".5", 2), Some(50));
        assert_eq!(parse_r(b"5.", 2), Some(500));
        assert_eq!(parse_r(b"12", 0), Some(12));
        assert_eq!(
            parse_r(b"99999999999999999999.999999999999999999", 18),
            Some(99_999_999_999_999_999_999_999_999_999_999_999_999)
        );
        for (text, scale) in [
            (&b"12.345"[..], 2),
            (b"12.0", 0),
            (b"", 2),
            (b".", 2),
            (b"-", 2),
            (b"1.2.3", 2),
            (b"1e5", 2),
            (b" 1", 2),
            (b"1 ", 2),
            (b"+1", 2),
            (b"1,5", 2),
            (b"100000000000000000000", 18),
        ] {
            assert_eq!(
                parse_r(text, scale),
                None,
                "{:?} at scale {scale}",
                String::from_utf8_lossy(text)
            );
        }
    }

    #[test]
    fn dt_values_are_real_calendar_dates() {
        assert_eq!(parse_dt(b"19700101"), Some(0));
        assert_eq!(parse_dt(b"700101"), Some(0));
        assert_eq!(parse_dt(b"19691231"), Some(-1));
        assert_eq!(parse_dt(b"20240229"), Some(19_782));
        assert_eq!(parse_dt(b"20000229"), Some(11_016));
        assert_eq!(parse_dt(b"491231"), parse_dt(b"20491231"));
        assert_eq!(parse_dt(b"500101"), parse_dt(b"19500101"));
        for text in [
            &b"20230229"[..],
            b"19000229",
            b"20241301",
            b"20240100",
            b"20240431",
            b"2024011",
            b"2024-01-01",
            b"",
            b"240229 ",
        ] {
            assert_eq!(parse_dt(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn tm_values_are_seconds_since_midnight() {
        assert_eq!(parse_tm(b"0000"), Some(0));
        assert_eq!(parse_tm(b"1230"), Some(45_000));
        assert_eq!(parse_tm(b"123045"), Some(45_045));
        assert_eq!(parse_tm(b"1230459"), Some(45_045));
        assert_eq!(parse_tm(b"12304599"), Some(45_045));
        assert_eq!(parse_tm(b"235959"), Some(86_399));
        for text in [
            &b"2400"[..],
            b"1260",
            b"123060",
            b"123",
            b"12304",
            b"123045999",
            b"12a0",
            b"",
            b"12:30",
        ] {
            assert_eq!(parse_tm(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    proptest! {
        #[test]
        fn valid_n_values_round_trip(value in any::<i64>()) {
            prop_assert_eq!(parse_n(value.to_string().as_bytes()), Some(value));
        }

        #[test]
        fn valid_r_values_round_trip(
            scale in 0u8..=18,
            value in -(10i128.pow(30))..10i128.pow(30),
        ) {
            prop_assert_eq!(parse_r(format_r(value, scale).as_bytes(), scale), Some(value));
        }

        #[test]
        fn valid_dates_round_trip(year in 1i32..=9999, month in 1i32..=12, day in 1i32..=31) {
            prop_assume!(day <= days_in_month(year, month));
            let text = format!("{year:04}{month:02}{day:02}");
            let days = parse_dt(text.as_bytes());
            prop_assert!(days.is_some(), "{}", text);
            prop_assert_eq!(days.map(civil_from_days), Some((year, month, day)));
        }

        #[test]
        fn valid_times_round_trip(seconds in 0i32..86_400) {
            let text = format!(
                "{:02}{:02}{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            );
            prop_assert_eq!(parse_tm(text.as_bytes()), Some(seconds));
        }

        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..24), scale in 0u8..=40) {
            let _ = parse_n(&bytes);
            let _ = parse_r(&bytes, scale);
            let _ = parse_dt(&bytes);
            let _ = parse_tm(&bytes);
        }

        #[test]
        fn rows_read_back_with_their_validity(
            rows in proptest::collection::vec(
                (
                    proptest::option::of(proptest::collection::vec(any::<u8>(), 0..8)),
                    proptest::option::of(any::<i64>()),
                    proptest::option::of(any::<i128>()),
                    proptest::option::of(any::<i32>()),
                ),
                0..40,
            )
        ) {
            let mut table = Table::new(
                "t",
                [
                    ("b".to_string(), ColumnType::Binary),
                    ("n".to_string(), ColumnType::Int64 { scale: 0 }),
                    ("r".to_string(), ColumnType::Decimal128 { precision: 38, scale: 2 }),
                    ("d".to_string(), ColumnType::Date32),
                    ("t".to_string(), ColumnType::Time32),
                ],
            );
            for (b, n, r, d) in &rows {
                let cells = [
                    b.as_deref().map_or(Cell::Null, Cell::Binary),
                    n.map_or(Cell::Null, Cell::Int64),
                    r.map_or(Cell::Null, Cell::Decimal128),
                    d.map_or(Cell::Null, Cell::Date32),
                    d.map_or(Cell::Null, Cell::Time32),
                ];
                prop_assert!(table.push_row(&cells).is_ok());
            }
            prop_assert_eq!(table.len(), rows.len());
            for (i, (b, n, r, d)) in rows.iter().enumerate() {
                let cell = |name: &str| table.column(name).and_then(|column| column.get(i));
                prop_assert_eq!(cell("b"), Some(b.as_deref().map_or(Cell::Null, Cell::Binary)));
                prop_assert_eq!(cell("n"), Some(n.map_or(Cell::Null, Cell::Int64)));
                prop_assert_eq!(cell("r"), Some(r.map_or(Cell::Null, Cell::Decimal128)));
                prop_assert_eq!(cell("d"), Some(d.map_or(Cell::Null, Cell::Date32)));
                prop_assert_eq!(cell("t"), Some(d.map_or(Cell::Null, Cell::Time32)));
                prop_assert_eq!(
                    table.column("b").and_then(|column| column.validity().get(i)),
                    Some(b.is_some())
                );
            }
            for (_, column) in table.columns() {
                prop_assert_eq!(column.len(), rows.len());
                prop_assert_eq!(column.validity().as_bytes().len(), rows.len().div_ceil(8));
            }
        }
    }
}
```

In `crates/edi835_core/src/lib.rs`, add `pub mod column;` after `pub mod check;`, and after `pub use check::EnvelopeChecker;` add:

```rust
pub use column::{
    Bitmap, Cell, CellError, Column, ColumnData, ColumnType, RowError, Table, Tables, parse_dt,
    parse_n, parse_r, parse_tm,
};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib column::`
Expected: compile errors (`cannot find type 'ColumnType'`, `'Bitmap'`, `'ColumnData'`, `cannot find function 'parse_n'`, …); the `pub use` line fails too.

- [ ] **Step 3: Implement the module**

Insert between `use crate::spec::ElementType;` and `#[cfg(test)]`:

```rust
/// Precision of every `Decimal128` column: the most digits an `i128` holds in full.
pub const DECIMAL_PRECISION: u8 = 38;

/// The physical type of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// Raw bytes, as they appear in the file.
    Binary,
    /// A 64-bit integer; `scale` implied decimal places (the `n` of `Nn`).
    Int64 {
        /// Implied decimal places.
        scale: u8,
    },
    /// A 128-bit integer scaled by `10^scale`.
    Decimal128 {
        /// Total significant digits.
        precision: u8,
        /// Decimal places.
        scale: u8,
    },
    /// Days since 1970-01-01.
    Date32,
    /// Seconds since midnight.
    Time32,
}

impl ColumnType {
    /// The column type that holds values of an element type; an element
    /// with no definition is held as raw bytes.
    pub fn of(kind: Option<ElementType>) -> ColumnType {
        match kind {
            None | Some(ElementType::An | ElementType::Id) => ColumnType::Binary,
            Some(ElementType::N(scale)) => ColumnType::Int64 { scale },
            Some(ElementType::R { scale }) => ColumnType::Decimal128 {
                precision: DECIMAL_PRECISION,
                scale,
            },
            Some(ElementType::Dt) => ColumnType::Date32,
            Some(ElementType::Tm) => ColumnType::Time32,
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnType::Binary => write!(f, "binary"),
            ColumnType::Int64 { scale: 0 } => write!(f, "int64"),
            ColumnType::Int64 { scale } => write!(f, "int64 (scale {scale})"),
            ColumnType::Decimal128 { precision, scale } => {
                write!(f, "decimal128({precision}, {scale})")
            }
            ColumnType::Date32 => write!(f, "date32"),
            ColumnType::Time32 => write!(f, "time32 (seconds)"),
        }
    }
}

/// One validity bit per row, least significant bit first; a set bit means
/// the row holds a value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bitmap {
    bytes: Vec<u8>,
    len: usize,
}

impl Bitmap {
    /// An empty bitmap.
    pub fn new() -> Bitmap {
        Bitmap::default()
    }

    /// Appends one bit.
    pub fn push(&mut self, valid: bool) {
        let bit = self.len % 8;
        if bit == 0 {
            self.bytes.push(0);
        }
        if valid && let Some(last) = self.bytes.last_mut() {
            *last |= 1 << bit;
        }
        self.len += 1;
    }

    /// The bit of row `index`; `None` past the end.
    pub fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }
        self.bytes
            .get(index / 8)
            .map(|byte| (byte >> (index % 8)) & 1 == 1)
    }

    /// Number of bits.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` when the bitmap holds no bits.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of unset bits.
    pub fn unset_count(&self) -> usize {
        let set: usize = self
            .bytes
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum();
        self.len.saturating_sub(set)
    }

    /// The packed bits; bits past `len` in the last byte are zero.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The value buffers of one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    /// Raw bytes: row `i` is `data[offsets[i]..offsets[i + 1]]`.
    Binary {
        /// One more offset than rows; the first is 0.
        offsets: Vec<i32>,
        /// Every row's bytes, back to back.
        data: Vec<u8>,
    },
    /// 64-bit integers with implied decimals.
    Int64 {
        /// One value per row.
        values: Vec<i64>,
        /// Implied decimal places.
        scale: u8,
    },
    /// Scaled 128-bit integers.
    Decimal128 {
        /// One value per row, scaled by `10^scale`.
        values: Vec<i128>,
        /// Total significant digits.
        precision: u8,
        /// Decimal places.
        scale: u8,
    },
    /// Days since 1970-01-01, one per row.
    Date32(Vec<i32>),
    /// Seconds since midnight, one per row.
    Time32(Vec<i32>),
}

impl Column {
    fn new(kind: ColumnType) -> Column {
        match kind {
            ColumnType::Binary => Column::Binary {
                offsets: vec![0],
                data: Vec::new(),
            },
            ColumnType::Int64 { scale } => Column::Int64 {
                values: Vec::new(),
                scale,
            },
            ColumnType::Decimal128 { precision, scale } => Column::Decimal128 {
                values: Vec::new(),
                precision,
                scale,
            },
            ColumnType::Date32 => Column::Date32(Vec::new()),
            ColumnType::Time32 => Column::Time32(Vec::new()),
        }
    }
}

/// One value on its way into, or out of, a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'a> {
    /// No value.
    Null,
    /// Raw bytes.
    Binary(&'a [u8]),
    /// An integer with the column's implied decimals.
    Int64(i64),
    /// An integer scaled by the column's `10^scale`.
    Decimal128(i128),
    /// Days since 1970-01-01.
    Date32(i32),
    /// Seconds since midnight.
    Time32(i32),
}

impl Cell<'_> {
    fn kind_name(&self) -> &'static str {
        match self {
            Cell::Null => "null",
            Cell::Binary(_) => "binary",
            Cell::Int64(_) => "int64",
            Cell::Decimal128(_) => "decimal128",
            Cell::Date32(_) => "date32",
            Cell::Time32(_) => "time32",
        }
    }
}

/// Why a cell could not be pushed into a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellError {
    /// The cell's type is not the column's.
    TypeMismatch {
        /// The column's type.
        column: ColumnType,
        /// The cell's type, e.g. `int64`.
        cell: &'static str,
    },
    /// The bytes would take a binary column past what `i32` offsets address.
    BinaryOverflow {
        /// The column's byte length the value would have produced.
        bytes: usize,
    },
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::TypeMismatch { column, cell } => {
                write!(f, "a {column} column cannot hold a {cell} value")
            }
            CellError::BinaryOverflow { bytes } => write!(
                f,
                "a binary column holds at most {} bytes; this value would bring it to {bytes}",
                i32::MAX
            ),
        }
    }
}

impl std::error::Error for CellError {}

/// One typed column and the validity of each of its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnData {
    column: Column,
    validity: Bitmap,
}

impl ColumnData {
    /// An empty column of the given type.
    pub fn new(kind: ColumnType) -> ColumnData {
        ColumnData {
            column: Column::new(kind),
            validity: Bitmap::new(),
        }
    }

    /// The column's type.
    pub fn kind(&self) -> ColumnType {
        match &self.column {
            Column::Binary { .. } => ColumnType::Binary,
            Column::Int64 { scale, .. } => ColumnType::Int64 { scale: *scale },
            Column::Decimal128 {
                precision, scale, ..
            } => ColumnType::Decimal128 {
                precision: *precision,
                scale: *scale,
            },
            Column::Date32(_) => ColumnType::Date32,
            Column::Time32(_) => ColumnType::Time32,
        }
    }

    /// The value buffers.
    pub fn column(&self) -> &Column {
        &self.column
    }

    /// The validity bitmap.
    pub fn validity(&self) -> &Bitmap {
        &self.validity
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.validity.len()
    }

    /// `true` when the column has no rows.
    pub fn is_empty(&self) -> bool {
        self.validity.is_empty()
    }

    /// Number of null rows.
    pub fn null_count(&self) -> usize {
        self.validity.unset_count()
    }

    /// The cell of row `row`; `None` past the end.
    pub fn get(&self, row: usize) -> Option<Cell<'_>> {
        if !self.validity.get(row)? {
            return Some(Cell::Null);
        }
        Some(match &self.column {
            Column::Binary { offsets, data } => {
                let start = usize::try_from(*offsets.get(row)?).ok()?;
                let end = usize::try_from(*offsets.get(row.checked_add(1)?)?).ok()?;
                Cell::Binary(data.get(start..end)?)
            }
            Column::Int64 { values, .. } => Cell::Int64(*values.get(row)?),
            Column::Decimal128 { values, .. } => Cell::Decimal128(*values.get(row)?),
            Column::Date32(values) => Cell::Date32(*values.get(row)?),
            Column::Time32(values) => Cell::Time32(*values.get(row)?),
        })
    }

    /// Row `row` as text: bytes as UTF-8 (invalid sequences replaced),
    /// integers as digits, decimals in fixed point with the column's scale,
    /// dates as `YYYY-MM-DD`, times as `HH:MM:SS` and a null as `∅`; `None`
    /// past the end.
    pub fn render(&self, row: usize) -> Option<String> {
        Some(match self.get(row)? {
            Cell::Null => "∅".to_string(),
            Cell::Binary(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            Cell::Int64(value) => value.to_string(),
            Cell::Decimal128(value) => {
                let scale = match self.kind() {
                    ColumnType::Decimal128 { scale, .. } => usize::from(scale),
                    _ => 0,
                };
                let sign = if value < 0 { "-" } else { "" };
                let digits = format!("{:0>width$}", value.unsigned_abs(), width = scale + 1);
                let (whole, fraction) = digits.split_at(digits.len().saturating_sub(scale));
                if fraction.is_empty() {
                    format!("{sign}{whole}")
                } else {
                    format!("{sign}{whole}.{fraction}")
                }
            }
            Cell::Date32(days) => {
                let (year, month, day) = civil_from_days(days);
                format!("{year:04}-{month:02}-{day:02}")
            }
            Cell::Time32(seconds) => format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            ),
        })
    }

    /// Appends a null row.
    pub fn push_null(&mut self) {
        match &mut self.column {
            Column::Binary { offsets, .. } => {
                let last = offsets.last().copied().unwrap_or_default();
                offsets.push(last);
            }
            Column::Int64 { values, .. } => values.push(0),
            Column::Decimal128 { values, .. } => values.push(0),
            Column::Date32(values) | Column::Time32(values) => values.push(0),
        }
        self.validity.push(false);
    }

    /// Appends one row. A [`Cell::Null`] fits any column; any other cell
    /// must match the column's type. On error nothing is appended.
    pub fn push(&mut self, cell: Cell<'_>) -> Result<(), CellError> {
        self.check(cell)?;
        self.push_checked(cell);
        Ok(())
    }

    /// Whether `cell` can be appended.
    fn check(&self, cell: Cell<'_>) -> Result<(), CellError> {
        let fits = matches!(
            (&self.column, cell),
            (_, Cell::Null)
                | (Column::Binary { .. }, Cell::Binary(_))
                | (Column::Int64 { .. }, Cell::Int64(_))
                | (Column::Decimal128 { .. }, Cell::Decimal128(_))
                | (Column::Date32(_), Cell::Date32(_))
                | (Column::Time32(_), Cell::Time32(_))
        );
        if !fits {
            return Err(CellError::TypeMismatch {
                column: self.kind(),
                cell: cell.kind_name(),
            });
        }
        if let (Column::Binary { data, .. }, Cell::Binary(bytes)) = (&self.column, cell) {
            let total = data.len().saturating_add(bytes.len());
            if i32::try_from(total).is_err() {
                return Err(CellError::BinaryOverflow { bytes: total });
            }
        }
        Ok(())
    }

    /// Appends a cell that [`ColumnData::check`] accepted.
    fn push_checked(&mut self, cell: Cell<'_>) {
        if cell == Cell::Null {
            self.push_null();
            return;
        }
        match (&mut self.column, cell) {
            (Column::Binary { offsets, data }, Cell::Binary(bytes)) => {
                data.extend_from_slice(bytes);
                offsets.push(i32::try_from(data.len()).unwrap_or(i32::MAX));
            }
            (Column::Int64 { values, .. }, Cell::Int64(value)) => values.push(value),
            (Column::Decimal128 { values, .. }, Cell::Decimal128(value)) => values.push(value),
            (Column::Date32(values), Cell::Date32(value))
            | (Column::Time32(values), Cell::Time32(value)) => values.push(value),
            _ => {
                self.push_null();
                return;
            }
        }
        self.validity.push(true);
    }
}

/// Why a row could not be appended to a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    /// The row has a different number of cells than the table has columns.
    Arity {
        /// The table.
        table: String,
        /// Columns in the table.
        expected: usize,
        /// Cells in the row.
        found: usize,
    },
    /// One cell does not fit its column.
    Cell {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// Why the cell does not fit.
        source: CellError,
    },
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowError::Arity {
                table,
                expected,
                found,
            } => write!(
                f,
                "table {table:?} has {expected} columns; the row has {found} cells"
            ),
            RowError::Cell {
                table,
                column,
                source,
            } => write!(f, "table {table:?} column {column:?}: {source}"),
        }
    }
}

impl std::error::Error for RowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RowError::Arity { .. } => None,
            RowError::Cell { source, .. } => Some(source),
        }
    }
}

/// Named columns of equal length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    name: String,
    columns: Vec<(String, ColumnData)>,
    rows: usize,
}

impl Table {
    /// An empty table with the given columns, in order.
    pub fn new(
        name: impl Into<String>,
        columns: impl IntoIterator<Item = (String, ColumnType)>,
    ) -> Table {
        Table {
            name: name.into(),
            columns: columns
                .into_iter()
                .map(|(name, kind)| (name, ColumnData::new(kind)))
                .collect(),
            rows: 0,
        }
    }

    /// The table's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every column with its name, in order.
    pub fn columns(&self) -> &[(String, ColumnData)] {
        &self.columns
    }

    /// A column by name.
    pub fn column(&self, name: &str) -> Option<&ColumnData> {
        self.columns
            .iter()
            .find(|(column, _)| column == name)
            .map(|(_, data)| data)
    }

    /// Number of rows; every column has this length.
    pub fn len(&self) -> usize {
        self.rows
    }

    /// `true` when the table has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// Appends one row, one cell per column in column order. Either every
    /// cell is appended or, on error, none is.
    pub fn push_row(&mut self, cells: &[Cell<'_>]) -> Result<(), RowError> {
        if cells.len() != self.columns.len() {
            return Err(RowError::Arity {
                table: self.name.clone(),
                expected: self.columns.len(),
                found: cells.len(),
            });
        }
        for ((column, data), &cell) in self.columns.iter().zip(cells) {
            data.check(cell).map_err(|source| RowError::Cell {
                table: self.name.clone(),
                column: column.clone(),
                source,
            })?;
        }
        for ((_, data), &cell) in self.columns.iter_mut().zip(cells) {
            data.push_checked(cell);
        }
        self.rows += 1;
        Ok(())
    }

    /// Moves the rows out into a new table and leaves this one empty, with
    /// the same columns and types.
    pub fn take_rows(&mut self) -> Table {
        let columns = self
            .columns
            .iter_mut()
            .map(|(name, data)| {
                let empty = ColumnData::new(data.kind());
                (name.clone(), std::mem::replace(data, empty))
            })
            .collect();
        Table {
            name: self.name.clone(),
            columns,
            rows: std::mem::take(&mut self.rows),
        }
    }
}

/// Tables ordered by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tables {
    tables: Vec<Table>,
}

impl Tables {
    /// Collects tables, ordering them by name.
    pub fn new(mut tables: Vec<Table>) -> Tables {
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Tables { tables }
    }

    /// A table by name.
    pub fn get(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// Every table, ordered by name.
    pub fn iter(&self) -> std::slice::Iter<'_, Table> {
        self.tables.iter()
    }

    /// Number of tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// `true` when there are no tables.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

impl<'t> IntoIterator for &'t Tables {
    type Item = &'t Table;
    type IntoIter = std::slice::Iter<'t, Table>;

    fn into_iter(self) -> Self::IntoIter {
        self.tables.iter()
    }
}

/// Splits an optional leading minus sign off `text`.
fn sign(text: &[u8]) -> (bool, &[u8]) {
    match text {
        [b'-', rest @ ..] => (true, rest),
        _ => (false, text),
    }
}

/// Appends ASCII digits to `negated`, a value kept negative so the most
/// negative number still fits; `None` on any other byte or on overflow.
fn accumulate(negated: i128, digits: &[u8]) -> Option<i128> {
    digits.iter().try_fold(negated, |value, &byte| {
        if !byte.is_ascii_digit() {
            return None;
        }
        value.checked_mul(10)?.checked_sub(i128::from(byte - b'0'))
    })
}

/// An `N` value: an optional `-` and at least one ASCII digit, nothing
/// else (no `+`, no spaces, no decimal point). The implied decimals are the
/// column's scale, so the integer is returned as written.
pub fn parse_n(text: &[u8]) -> Option<i64> {
    let (negative, digits) = sign(text);
    if digits.is_empty() {
        return None;
    }
    let negated = accumulate(0, digits)?;
    let value = if negative {
        negated
    } else {
        negated.checked_neg()?
    };
    i64::try_from(value).ok()
}

/// An `R` value scaled by `10^scale`: an optional `-`, digits, and at most
/// one `.` followed by no more than `scale` digits; at least one digit in
/// all. `None` for anything else, including spaces, a `+`, an exponent, or
/// a value of more than [`DECIMAL_PRECISION`] digits once scaled.
pub fn parse_r(text: &[u8], scale: u8) -> Option<i128> {
    let (negative, body) = sign(text);
    let (whole, fraction) = match body.iter().position(|&byte| byte == b'.') {
        Some(at) => {
            let (whole, rest) = body.split_at(at);
            (whole, rest.get(1..).unwrap_or_default())
        }
        None => (body, &[][..]),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    let missing = usize::from(scale).checked_sub(fraction.len())?;
    let mut negated = accumulate(accumulate(0, whole)?, fraction)?;
    for _ in 0..missing {
        negated = negated.checked_mul(10)?;
    }
    let limit = 10i128.checked_pow(u32::from(DECIMAL_PRECISION))?;
    if negated <= -limit {
        return None;
    }
    if negative {
        Some(negated)
    } else {
        negated.checked_neg()
    }
}

/// Up to four ASCII digits as a number.
fn small_number(digits: &[u8]) -> Option<i32> {
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    digits.iter().try_fold(0i32, |value, &byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + i32::from(byte - b'0'))
    })
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i32, month: i32, day: i32) -> i32 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let march_based_month = (month + 9) % 12;
    let day_of_year = (153 * march_based_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The proleptic Gregorian date `days` after 1970-01-01 (H. Hinnant's
/// `civil_from_days`), the inverse of [`days_from_civil`].
fn civil_from_days(days: i32) -> (i32, i32, i32) {
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_based_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_based_month + 2) / 5 + 1;
    let month = if march_based_month < 10 {
        march_based_month + 3
    } else {
        march_based_month - 9
    };
    let year = year_of_era + era * 400 + i32::from(month <= 2);
    (year, month, day)
}

/// A `DT` value as days since 1970-01-01: `CCYYMMDD`, or `YYMMDD` with
/// years 00–49 read as 20xx and 50–99 as 19xx. The date must exist.
pub fn parse_dt(text: &[u8]) -> Option<i32> {
    let (year, month_day) = match text.len() {
        8 => (small_number(text.get(..4)?)?, text.get(4..)?),
        6 => {
            let year = small_number(text.get(..2)?)?;
            let century = if year < 50 { 2000 } else { 1900 };
            (century + year, text.get(2..)?)
        }
        _ => return None,
    };
    let month = small_number(month_day.get(..2)?)?;
    let day = small_number(month_day.get(2..)?)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// A `TM` value as seconds since midnight: `HHMM`, `HHMMSS`, or `HHMMSS`
/// followed by one or two decimal-second digits, which are ignored.
pub fn parse_tm(text: &[u8]) -> Option<i32> {
    if !matches!(text.len(), 4 | 6 | 7 | 8) {
        return None;
    }
    let hour = small_number(text.get(..2)?)?;
    let minute = small_number(text.get(2..4)?)?;
    let second = match text.get(4..6) {
        Some(digits) => small_number(digits)?,
        None => 0,
    };
    if text.len() > 6 {
        small_number(text.get(6..)?)?;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(hour * 3600 + minute * 60 + second)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib column::`
Expected: 22 passed (six of them property tests).

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 253 tests in the workspace.

```bash
git add crates/edi835_core/src/column.rs crates/edi835_core/src/lib.rs
git commit -m "column: Arrow-layout columns, tables and X12 value parsers

Bitmap validity (LSB first), i32 binary offsets, Int64 with implied
decimals, Decimal128(38, scale), Date32 and Time32, with no dependency
on the arrow crate. N, R, DT and TM parse without allocating; a row is
appended whole or not at all."
```

---

## Task 3: The `tables` section of the spec

**Implementer tier:** Sonnet — one large file plus `lib.rs`; the code is complete here, but the raw serde types, the per-column deserialization and the linking pass must be wired without drifting from it, and nineteen reasons must keep their exact text.

**Files:**
- Modify: `crates/edi835_core/src/spec.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `Spec::from_value`, `check_shape`, `parse_position`, `Spec::ancestors`, `Spec::loop_id` (all in `spec.rs`).
- Produces:
  - JSON: `"tables": { "<name>": { "loops": ["2100"], "ref": "claim", "segment": "CAS", "repeat": {"from": 2, "step": 3}, "columns": { "<column>": {…} } } }`. `ref`, `segment`, `repeat` and `columns` are optional. A column is an object with exactly one of `"element": n`, `"group_element": k` (0-based offset inside the group) or `"segment_index": true`, plus `"component"`, and — in a table without `segment` only — `"segment"` (required), `"loop"` and `"where"`. In a table with `segment`, columns read the anchor segment itself.
  - `pub const ROW_COLUMN: &str = "row"`, `pub const SEGMENT_COLUMN: &str = "segment"` (automatic columns).
  - `pub struct Repeat { pub from: usize, pub step: usize }`.
  - `pub enum ColumnSource { Element { loop_id: Option<LoopId>, segment: Vec<u8>, conditions: Vec<(usize, Vec<u8>)>, element: usize, component: Option<usize> }, SegmentIndex { loop_id, segment, conditions }, GroupElement { offset: usize, component: Option<usize> } }` — in a table with `segment`, `Element`/`SegmentIndex` carry the anchor segment id, no loop, no conditions.
  - `pub struct TableDef { pub name, pub reference: String, pub loops: Vec<LoopId>, pub segment: Option<Vec<u8>>, pub repeat: Option<Repeat>, pub columns: Vec<(String, ColumnSource)>, pub parent: Option<usize>, pub ancestors: Vec<usize> }` — `reference` is the `ref` (default: the table name); `ancestors` lists the tables above, outermost first, `parent` is its last entry. A table's chain is built from its anchor loops: walking up from each anchor's parent loop (from the anchor itself for a table with `segment`, whose rows live inside that loop), every loop that anchors a table without `segment` adds that table. The longest chain wins; every other anchor's chain must begin it.
  - `Spec::tables() -> &[TableDef]` (name order), `Spec::table(name) -> Option<&TableDef>`, `Spec::element_def(segment, element, component) -> Option<&ElementDef>`.
  - `SpecError::TableSchema { table, column: Option<String>, source: serde_json::Error }` — `table "t" does not match the schema: …` / `table "t" column "c" does not match the schema: …`; `source()` is the serde error.
  - `SpecError::BadTable { table, column: Option<String>, reason: TableDefError }` — `table "claims": <reason>` / `table "claims" column "charge": <reason>`.
  - `pub enum TableDefError` with nineteen reasons: `EmptyName { what }`, `NoLoops`, `UnknownLoop { name }`, `ReservedName { name }`, `RefTaken { name, table }`, `RepeatWithoutSegment`, `ZeroStep`, `ZeroPosition { key }`, `BadPosition { key }`, `OffsetBeyondStep { offset, step }`, `NestedAnchors { outer, inner }`, `SharedAnchor { loop_name, other }`, `UnrelatedAnchors { first, second }`, `NotADescendant { loop_name, anchor }`, `AnchorSegmentOnly { key }`, `NeedsSegment`, `GroupWithoutRepeat`, `SourceCount { found }`, `ComponentOnIndex`. Texts are in the Display test below.
  - An empty segment id in `tables.<t>.segment` or `tables.<t>.columns.<c>.segment` is `SpecError::EmptySegmentId { loop_name: None, key }` with that key.
  - The shape pre-check covers `tables`, each table, `repeat`, `columns`, each column and its `where`.

§7 names the error `BadColumn { table, column, reason }`; this plan calls it `BadTable` with an optional column, since half of the reasons are about the table itself (`NoLoops`, `NestedAnchors`, `SharedAnchor`, …). Three rules keep row numbers and parent indices sound, and are why their reasons exist: a table without `segment` anchors in loops that do not nest (so at most one of its instances is open, and rows close in the order they opened); a loop anchors at most one such table (so "the table above" is never ambiguous); and every anchor's chain of tables above must begin the longest one.

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `fn control_error`:

```rust
    const TABLED: &str = r#"{"name":"t",
        "loops":{
            "A":{"trigger":{"segment":"AA"},"segments":["A1"],"end":"AE"},
            "B":{"parent":"A","trigger":{"segment":"BB"},"segments":["B1","AJ"]},
            "C":{"parent":"B","trigger":{"segment":"CC"},"segments":["C1","AJ"]},
            "D":{"parent":"A","trigger":{"segment":"DD"}}
        },
        "segments":{
            "BB":{"elements":{"1":{"name":"id","type":"AN"},"2":{"name":"amount","type":"R"}}},
            "CC":{"elements":{"1":{"name":"code","type":"AN","composite":{
                "1":{"name":"qualifier","type":"ID"},"2":{"name":"value","type":"AN"}}}}}
        },
        "tables":{
            "heads":{"loops":["A"],"ref":"head","columns":{
                "code":{"segment":"AA","element":1},
                "note":{"loop":"D","segment":"DD","where":{"1":"N"},"element":2}
            }},
            "bodies":{"loops":["B"],"ref":"body","columns":{
                "id":{"segment":"BB","element":1},
                "line_at":{"loop":"C","segment":"C1","segment_index":true}
            }},
            "lines":{"loops":["C"],"ref":"line","columns":{
                "code":{"segment":"CC","element":1,"component":2}
            }},
            "adjustments":{"loops":["B","C"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
                "kind":{"element":1},
                "reason":{"group_element":0},
                "amount":{"group_element":1}
            }}
        }
    }"#;

    fn table_error(tables: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{
                "A":{{"trigger":{{"segment":"AA"}}}},
                "B":{{"parent":"A","trigger":{{"segment":"BB"}}}},
                "C":{{"parent":"B","trigger":{{"segment":"CC"}}}},
                "D":{{"parent":"A","trigger":{{"segment":"DD"}}}}
            }},"tables":{tables}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn tables_are_read_in_name_order_with_their_sources() {
        let spec = Spec::from_json(TABLED).unwrap();
        let names: Vec<&str> = spec.tables().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["adjustments", "bodies", "heads", "lines"]);
        let id = |name: &str| spec.loop_id(name).unwrap();
        let heads = spec.table("heads").unwrap();
        assert_eq!(heads.reference, "head");
        assert_eq!(heads.loops, vec![id("A")]);
        assert_eq!((heads.segment.as_ref(), heads.repeat), (None, None));
        assert_eq!(
            heads.columns,
            vec![
                (
                    "code".to_string(),
                    ColumnSource::Element {
                        loop_id: None,
                        segment: b"AA".to_vec(),
                        conditions: Vec::new(),
                        element: 1,
                        component: None,
                    }
                ),
                (
                    "note".to_string(),
                    ColumnSource::Element {
                        loop_id: Some(id("D")),
                        segment: b"DD".to_vec(),
                        conditions: vec![(1, b"N".to_vec())],
                        element: 2,
                        component: None,
                    }
                ),
            ]
        );
        assert_eq!(
            spec.table("bodies").unwrap().columns[1].1,
            ColumnSource::SegmentIndex {
                loop_id: Some(id("C")),
                segment: b"C1".to_vec(),
                conditions: Vec::new(),
            }
        );
        let adjustments = spec.table("adjustments").unwrap();
        assert_eq!(
            adjustments.reference, "adjustments",
            "ref defaults to the name"
        );
        assert_eq!(adjustments.loops, vec![id("B"), id("C")]);
        assert_eq!(adjustments.segment.as_deref(), Some(&b"AJ"[..]));
        assert_eq!(adjustments.repeat, Some(Repeat { from: 2, step: 2 }));
        assert_eq!(
            adjustments.columns,
            vec![
                (
                    "amount".to_string(),
                    ColumnSource::GroupElement {
                        offset: 1,
                        component: None
                    }
                ),
                (
                    "kind".to_string(),
                    ColumnSource::Element {
                        loop_id: None,
                        segment: b"AJ".to_vec(),
                        conditions: Vec::new(),
                        element: 1,
                        component: None,
                    }
                ),
                (
                    "reason".to_string(),
                    ColumnSource::GroupElement {
                        offset: 0,
                        component: None
                    }
                ),
            ]
        );
        assert_eq!(spec.table("missing"), None);
    }

    #[test]
    fn a_table_hangs_from_the_tables_anchored_above_it() {
        let spec = Spec::from_json(TABLED).unwrap();
        let index = |name: &str| spec.tables().iter().position(|t| t.name == name).unwrap();
        let (adjustments, bodies, heads, lines) = (
            index("adjustments"),
            index("bodies"),
            index("heads"),
            index("lines"),
        );
        let table = |i: usize| &spec.tables()[i];
        assert_eq!(
            (table(heads).parent, table(heads).ancestors.clone()),
            (None, vec![])
        );
        assert_eq!(table(bodies).parent, Some(heads));
        assert_eq!(table(lines).ancestors, vec![heads, bodies]);
        assert_eq!(
            table(adjustments).ancestors,
            vec![heads, bodies, lines],
            "a segment table anchored in B and C hangs from the deepest chain"
        );
        assert_eq!(table(adjustments).parent, Some(lines));
    }

    #[test]
    fn element_definitions_are_found_by_segment_position_and_component() {
        let spec = Spec::from_json(TABLED).unwrap();
        assert_eq!(spec.element_def(b"BB", 2, None).unwrap().name, "amount");
        assert_eq!(spec.element_def(b"CC", 1, Some(2)).unwrap().name, "value");
        assert_eq!(spec.element_def(b"CC", 1, Some(3)), None);
        assert_eq!(spec.element_def(b"BB", 9, None), None);
        assert_eq!(spec.element_def(b"ZZ", 1, None), None);
    }

    #[test]
    fn bad_tables_are_rejected_with_the_table_the_column_and_the_reason() {
        let cases: Vec<(&str, &str, Option<&str>, TableDefError)> = vec![
            (
                r#"{"":{"loops":["A"]}}"#,
                "",
                None,
                TableDefError::EmptyName { what: "table name" },
            ),
            (r#"{"t":{"loops":[]}}"#, "t", None, TableDefError::NoLoops),
            (
                r#"{"t":{"loops":["Z"]}}"#,
                "t",
                None,
                TableDefError::UnknownLoop { name: "Z".into() },
            ),
            (
                r#"{"t":{"loops":["A"],"ref":""}}"#,
                "t",
                None,
                TableDefError::EmptyName { what: "ref" },
            ),
            (
                r#"{"t":{"loops":["A"],"ref":"segment"}}"#,
                "t",
                None,
                TableDefError::ReservedName {
                    name: "segment".into(),
                },
            ),
            (
                r#"{"a":{"loops":["A"],"ref":"x"},"b":{"loops":["B"],"ref":"x"}}"#,
                "b",
                None,
                TableDefError::RefTaken {
                    name: "x".into(),
                    table: "a".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"repeat":{"from":1,"step":2}}}"#,
                "t",
                None,
                TableDefError::RepeatWithoutSegment,
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":0,"step":2}}}"#,
                "t",
                None,
                TableDefError::ZeroPosition { key: "repeat.from" },
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":1,"step":0}}}"#,
                "t",
                None,
                TableDefError::ZeroStep,
            ),
            (
                r#"{"t":{"loops":["C","A"]}}"#,
                "t",
                None,
                TableDefError::NestedAnchors {
                    outer: "A".into(),
                    inner: "C".into(),
                },
            ),
            (
                r#"{"a":{"loops":["B"]},"b":{"loops":["D","B"]}}"#,
                "b",
                None,
                TableDefError::SharedAnchor {
                    loop_name: "B".into(),
                    other: "a".into(),
                },
            ),
            (
                r#"{"b":{"loops":["B"]},"d":{"loops":["D"]},"x":{"loops":["C","D"],"segment":"XX"}}"#,
                "x",
                None,
                TableDefError::UnrelatedAnchors {
                    first: "C".into(),
                    second: "D".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"":{"segment":"AA","element":1}}}}"#,
                "t",
                Some(""),
                TableDefError::EmptyName {
                    what: "column name",
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"row":{"segment":"AA","element":1}}}}"#,
                "t",
                Some("row"),
                TableDefError::ReservedName { name: "row".into() },
            ),
            (
                r#"{"a":{"loops":["A"],"ref":"head"},"b":{"loops":["B"],"columns":{"head":{"segment":"BB","element":1}}}}"#,
                "b",
                Some("head"),
                TableDefError::ReservedName {
                    name: "head".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA"}}}}"#,
                "t",
                Some("c"),
                TableDefError::SourceCount { found: 0 },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"segment_index":true}}}}"#,
                "t",
                Some("c"),
                TableDefError::SourceCount { found: 2 },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","segment_index":true,"component":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::ComponentOnIndex,
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::ZeroPosition { key: "element" },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"component":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::ZeroPosition { key: "component" },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"group_element":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::GroupWithoutRepeat,
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":2,"step":3},"columns":{"c":{"group_element":3}}}}"#,
                "t",
                Some("c"),
                TableDefError::OffsetBeyondStep { offset: 3, step: 3 },
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","columns":{"c":{"loop":"B","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::AnchorSegmentOnly { key: "loop" },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NeedsSegment,
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"loop":"Z","segment":"ZZ","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::UnknownLoop { name: "Z".into() },
            ),
            (
                r#"{"t":{"loops":["B","D"],"columns":{"c":{"loop":"C","segment":"CC","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NotADescendant {
                    loop_name: "C".into(),
                    anchor: "D".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"loop":"A","segment":"AA","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NotADescendant {
                    loop_name: "A".into(),
                    anchor: "A".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","where":{"01":"X"},"element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::BadPosition { key: "01".into() },
            ),
        ];
        for (tables, expected_table, expected_column, expected_reason) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::BadTable { table, column, reason } if table == expected_table && column.as_deref() == expected_column && *reason == expected_reason),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn empty_segment_ids_in_tables_name_their_key() {
        let cases = [
            (r#"{"t":{"loops":["A"],"segment":""}}"#, "tables.t.segment"),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"","element":1}}}}"#,
                "tables.t.columns.c.segment",
            ),
        ];
        for (tables, expected_key) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == expected_key),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn a_table_that_breaks_the_schema_names_the_table_and_the_column() {
        let err = table_error(r#"{"t":{"loops":["A"],"anchor":"x"}}"#);
        assert!(
            matches!(&err, SpecError::TableSchema { table, column: None, .. } if table == "t"),
            "{err:?}"
        );
        assert!(err.to_string().contains("anchor"), "{err}");
        let err = table_error(r#"{"t":{"loops":["A"],"columns":{"c":{"elemnt":1}}}}"#);
        assert!(
            matches!(&err, SpecError::TableSchema { table, column: Some(column), .. } if table == "t" && column == "c"),
            "{err:?}"
        );
        assert!(err.to_string().contains("elemnt"), "{err}");
    }

    #[test]
    fn every_object_of_the_table_schema_is_checked_with_its_path() {
        let cases = [
            (r#"[]"#, "tables", "an array"),
            (r#"{"t":[]}"#, "tables.t", "an array"),
            (
                r#"{"t":{"loops":["A"],"repeat":[2,3]}}"#,
                "tables.t.repeat",
                "an array",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":[]}}"#,
                "tables.t.columns",
                "an array",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":"CLP01"}}}"#,
                "tables.t.columns.c",
                "a string",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"where":["X"]}}}}"#,
                "tables.t.columns.c.where",
                "an array",
            ),
        ];
        for (tables, expected_path, expected_found) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patch_adds_a_column_with_three_lines() {
        let spec = Spec::from_json(TABLED).unwrap();
        let patched = spec
            .merge_patch(
                r#"{"tables":{"bodies":{"columns":{
                    "amount":{"segment":"BB","element":2}
                }}}}"#,
            )
            .unwrap();
        let names: Vec<&str> = patched
            .table("bodies")
            .unwrap()
            .columns
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["amount", "id", "line_at"]);
    }

    #[test]
    fn to_json_round_trips_the_tables_section() {
        let spec = Spec::from_json(TABLED).unwrap();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert_eq!(again.tables(), spec.tables());
        assert_eq!(again.tables().len(), 4);
    }

    #[test]
    fn table_errors_display_the_table_the_column_and_every_reason() {
        let cases = [
            (TableDefError::EmptyName { what: "ref" }, "the ref is empty"),
            (
                TableDefError::NoLoops,
                "\"loops\" is empty; a table anchors in at least one loop",
            ),
            (
                TableDefError::UnknownLoop {
                    name: "2101".into(),
                },
                "loop \"2101\" does not exist",
            ),
            (
                TableDefError::ReservedName { name: "row".into() },
                "the name \"row\" is taken by an automatic column",
            ),
            (
                TableDefError::RefTaken {
                    name: "claim".into(),
                    table: "claims".into(),
                },
                "\"ref\" \"claim\" is already used by table \"claims\"",
            ),
            (
                TableDefError::RepeatWithoutSegment,
                "\"repeat\" requires \"segment\": only a segment's elements repeat",
            ),
            (TableDefError::ZeroStep, "\"repeat.step\" is 0"),
            (
                TableDefError::ZeroPosition { key: "element" },
                "\"element\" must be a 1-based position; found 0",
            ),
            (
                TableDefError::BadPosition { key: "01".into() },
                "\"where\" position \"01\" is not a 1-based integer in canonical form",
            ),
            (
                TableDefError::OffsetBeyondStep { offset: 3, step: 3 },
                "\"group_element\" 3 is outside a group of 3 elements (offsets start at 0)",
            ),
            (
                TableDefError::NestedAnchors {
                    outer: "2100".into(),
                    inner: "2110".into(),
                },
                "anchor loops \"2100\" and \"2110\" nest; a table without \"segment\" anchors in loops that do not",
            ),
            (
                TableDefError::SharedAnchor {
                    loop_name: "2100".into(),
                    other: "claims".into(),
                },
                "loop \"2100\" already anchors table \"claims\"; a loop anchors at most one table without \"segment\"",
            ),
            (
                TableDefError::UnrelatedAnchors {
                    first: "2110".into(),
                    second: "1000A".into(),
                },
                "anchor loops \"2110\" and \"1000A\" sit under tables that are not one chain",
            ),
            (
                TableDefError::NotADescendant {
                    loop_name: "1000A".into(),
                    anchor: "2100".into(),
                },
                "loop \"1000A\" is not inside anchor loop \"2100\"",
            ),
            (
                TableDefError::AnchorSegmentOnly { key: "where" },
                "\"where\" does not apply in a table anchored on a segment: its columns read that segment",
            ),
            (
                TableDefError::NeedsSegment,
                "the column names no \"segment\" to read",
            ),
            (
                TableDefError::GroupWithoutRepeat,
                "\"group_element\" requires the table's \"repeat\"",
            ),
            (
                TableDefError::SourceCount { found: 2 },
                "a column takes exactly one of \"element\", \"group_element\" or \"segment_index\"; found 2",
            ),
            (
                TableDefError::ComponentOnIndex,
                "\"component\" does not apply to \"segment_index\"",
            ),
        ];
        for (reason, expected) in cases {
            let in_column = SpecError::BadTable {
                table: "claims".into(),
                column: Some("charge".into()),
                reason: reason.clone(),
            };
            assert_eq!(
                in_column.to_string(),
                format!("table \"claims\" column \"charge\": {expected}")
            );
            let in_table = SpecError::BadTable {
                table: "claims".into(),
                column: None,
                reason,
            };
            assert_eq!(
                in_table.to_string(),
                format!("table \"claims\": {expected}")
            );
            assert!(std::error::Error::source(&in_table).is_none());
        }
    }

    #[test]
    fn table_schema_errors_display_the_table_the_column_and_the_serde_message() {
        let source = || serde_json::from_value::<RawColumn>(serde_json::json!(1)).unwrap_err();
        let in_column = SpecError::TableSchema {
            table: "claims".into(),
            column: Some("charge".into()),
            source: source(),
        };
        assert_eq!(
            in_column.to_string(),
            "table \"claims\" column \"charge\" does not match the schema: invalid type: integer `1`, expected a column object"
        );
        assert!(std::error::Error::source(&in_column).is_some());
        let in_table = SpecError::TableSchema {
            table: "claims".into(),
            column: None,
            source: serde_json::from_value::<RawTable>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            in_table.to_string(),
            "table \"claims\" does not match the schema: invalid type: integer `1`, expected a table object"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: compile errors (`cannot find type 'ColumnSource'`, `'TableDefError'`, `'Repeat'`, `no method named 'tables'`, `no variant named 'BadTable'`, …).

- [ ] **Step 3: Add the public types**

Insert before `/// Why a spec could not be loaded. Each variant names where in the spec the fault is.`:

```rust
/// Name of the automatic column that numbers a table's rows from 0, across
/// the whole stream.
pub const ROW_COLUMN: &str = "row";

/// Name of the automatic column that holds the index of a row's anchor
/// segment: the segment that opened the anchor loop instance, or the
/// anchored segment itself.
pub const SEGMENT_COLUMN: &str = "segment";

/// How a table's segment repeats a group of elements: one row per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// 1-based position of the first group's first element.
    pub from: usize,
    /// Elements per group.
    pub step: usize,
}

/// Where a column takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnSource {
    /// An element (or one of its components) of the first segment that
    /// matches `segment` and `conditions`, captured in the anchor loop
    /// instance, or in the first instance of `loop_id` inside it. In a table
    /// anchored on a segment, the anchor segment itself.
    Element {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
    /// The index of the first segment that matches, chosen as for `Element`.
    SegmentIndex {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
    },
    /// An element of the row's group, in a table whose segment repeats a
    /// group: position `from + k * step + offset` for group `k`.
    GroupElement {
        /// 0-based position inside the group.
        offset: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
}

/// One table of the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    /// Name used in the JSON, e.g. `claims`.
    pub name: String,
    /// Name of the column that refers to a row of this table from the
    /// tables below it, e.g. `claim`; the table's name unless the spec says.
    pub reference: String,
    /// The loops whose instances (or whose segments) give rows.
    pub loops: Vec<LoopId>,
    /// With a segment, one row per occurrence of it in an anchor loop
    /// instead of one row per loop instance.
    pub segment: Option<Vec<u8>>,
    /// With a repeat, one row per element group of the segment.
    pub repeat: Option<Repeat>,
    /// The declared columns by name, in name order.
    pub columns: Vec<(String, ColumnSource)>,
    /// The nearest table above this one, as an index into [`Spec::tables`].
    pub parent: Option<usize>,
    /// Every table above this one, outermost first; the last is `parent`.
    /// Each one gets an automatic column named after its `reference`.
    pub ancestors: Vec<usize>,
}

/// Why a table definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDefError {
    /// A table name, column name or `ref` is the empty string.
    EmptyName {
        /// Which one: `table name`, `column name` or `ref`.
        what: &'static str,
    },
    /// `loops` is empty.
    NoLoops,
    /// A loop name does not exist.
    UnknownLoop {
        /// The name as written.
        name: String,
    },
    /// A column name or `ref` is taken by an automatic column.
    ReservedName {
        /// The name as written.
        name: String,
    },
    /// Another table already uses this `ref`.
    RefTaken {
        /// The `ref` as written.
        name: String,
        /// The table that uses it first, in name order.
        table: String,
    },
    /// `repeat` was given without `segment`.
    RepeatWithoutSegment,
    /// `repeat.step` is 0.
    ZeroStep,
    /// A 1-based position holds 0.
    ZeroPosition {
        /// The key: `repeat.from`, `element` or `component`.
        key: &'static str,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The key as written.
        key: String,
    },
    /// `group_element` does not fit inside a group.
    OffsetBeyondStep {
        /// The offset as written.
        offset: usize,
        /// The group size.
        step: usize,
    },
    /// Two anchor loops of a table without `segment` nest.
    NestedAnchors {
        /// The enclosing loop.
        outer: String,
        /// The loop inside it.
        inner: String,
    },
    /// The loop already anchors another table without `segment`.
    SharedAnchor {
        /// The loop.
        loop_name: String,
        /// The table that anchors in it first, in name order.
        other: String,
    },
    /// Two anchor loops lead to tables above that are not one chain.
    UnrelatedAnchors {
        /// The anchor loop with the longest chain of tables above it.
        first: String,
        /// The anchor loop whose chain disagrees.
        second: String,
    },
    /// A column's `loop` is not inside every anchor loop.
    NotADescendant {
        /// The column's loop.
        loop_name: String,
        /// The anchor loop it is not inside.
        anchor: String,
    },
    /// A key that picks a segment is used in a table anchored on a segment.
    AnchorSegmentOnly {
        /// The key: `segment`, `loop` or `where`.
        key: &'static str,
    },
    /// A column of a table without `segment` names no segment.
    NeedsSegment,
    /// `group_element` is used in a table without `repeat`.
    GroupWithoutRepeat,
    /// A column does not name exactly one value source.
    SourceCount {
        /// How many of `element`, `group_element` and `segment_index` it names.
        found: usize,
    },
    /// `component` is given with `segment_index`.
    ComponentOnIndex,
}

impl fmt::Display for TableDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableDefError::EmptyName { what } => write!(f, "the {what} is empty"),
            TableDefError::NoLoops => {
                write!(
                    f,
                    "\"loops\" is empty; a table anchors in at least one loop"
                )
            }
            TableDefError::UnknownLoop { name } => write!(f, "loop {name:?} does not exist"),
            TableDefError::ReservedName { name } => {
                write!(f, "the name {name:?} is taken by an automatic column")
            }
            TableDefError::RefTaken { name, table } => {
                write!(f, "\"ref\" {name:?} is already used by table {table:?}")
            }
            TableDefError::RepeatWithoutSegment => write!(
                f,
                "\"repeat\" requires \"segment\": only a segment's elements repeat"
            ),
            TableDefError::ZeroStep => write!(f, "\"repeat.step\" is 0"),
            TableDefError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based position; found 0")
            }
            TableDefError::BadPosition { key } => write!(
                f,
                "\"where\" position {key:?} is not a 1-based integer in canonical form"
            ),
            TableDefError::OffsetBeyondStep { offset, step } => write!(
                f,
                "\"group_element\" {offset} is outside a group of {step} elements (offsets start at 0)"
            ),
            TableDefError::NestedAnchors { outer, inner } => write!(
                f,
                "anchor loops {outer:?} and {inner:?} nest; a table without \"segment\" anchors in loops that do not"
            ),
            TableDefError::SharedAnchor { loop_name, other } => write!(
                f,
                "loop {loop_name:?} already anchors table {other:?}; a loop anchors at most one table without \"segment\""
            ),
            TableDefError::UnrelatedAnchors { first, second } => write!(
                f,
                "anchor loops {first:?} and {second:?} sit under tables that are not one chain"
            ),
            TableDefError::NotADescendant { loop_name, anchor } => {
                write!(f, "loop {loop_name:?} is not inside anchor loop {anchor:?}")
            }
            TableDefError::AnchorSegmentOnly { key } => write!(
                f,
                "{key:?} does not apply in a table anchored on a segment: its columns read that segment"
            ),
            TableDefError::NeedsSegment => write!(f, "the column names no \"segment\" to read"),
            TableDefError::GroupWithoutRepeat => {
                write!(f, "\"group_element\" requires the table's \"repeat\"")
            }
            TableDefError::SourceCount { found } => write!(
                f,
                "a column takes exactly one of \"element\", \"group_element\" or \"segment_index\"; found {found}"
            ),
            TableDefError::ComponentOnIndex => {
                write!(f, "\"component\" does not apply to \"segment_index\"")
            }
        }
    }
}
```

- [ ] **Step 4: Add the error variants**

In `enum SpecError`, add before the `BadControl` variant (the line ``/// A loop's `control` is invalid.``):

```rust
    /// A table or column definition does not match the schema.
    TableSchema {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// A table definition is invalid.
    BadTable {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What is wrong with it.
        reason: TableDefError,
    },
```

In `impl fmt::Display for SpecError`, add before the `SpecError::BadControl` arm:

```rust
            SpecError::TableSchema {
                table,
                column: None,
                source,
            } => write!(f, "table {table:?} does not match the schema: {source}"),
            SpecError::TableSchema {
                table,
                column: Some(column),
                source,
            } => write!(
                f,
                "table {table:?} column {column:?} does not match the schema: {source}"
            ),
            SpecError::BadTable {
                table,
                column: None,
                reason,
            } => write!(f, "table {table:?}: {reason}"),
            SpecError::BadTable {
                table,
                column: Some(column),
                reason,
            } => write!(f, "table {table:?} column {column:?}: {reason}"),
```

In `impl std::error::Error for SpecError`, extend the first arm so `TableSchema` returns its serde error:

```rust
            SpecError::Json(e)
            | SpecError::Schema { source: e, .. }
            | SpecError::SegmentSchema { source: e, .. }
            | SpecError::TableSchema { source: e, .. } => Some(e),
```

- [ ] **Step 5: Deserialize, compile and link the section**

In `struct RawSpec`, add after the `segments` field:

```rust
    #[serde(default)]
    tables: BTreeMap<String, Value>,
```

Insert before `/// A loaded, validated loop structure.`:

```rust
// Columns are kept as raw values so a schema error can name the column.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a table object")]
struct RawTable {
    loops: Vec<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    segment: Option<String>,
    repeat: Option<RawRepeat>,
    #[serde(default)]
    columns: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a repeat object")]
struct RawRepeat {
    from: usize,
    step: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a column object")]
struct RawColumn {
    #[serde(rename = "loop")]
    loop_name: Option<String>,
    segment: Option<String>,
    #[serde(default, rename = "where")]
    conditions: BTreeMap<String, String>,
    element: Option<usize>,
    component: Option<usize>,
    group_element: Option<usize>,
    #[serde(default)]
    segment_index: bool,
}
```

In `pub struct Spec`, add `tables: Vec<TableDef>,` after `segments: BTreeMap<Vec<u8>, SegmentDef>,`.

At the end of `Spec::from_value`, replace the last three statements (`let spec = Spec { … };`, `spec.check_ambiguity()?;` and `Ok(spec)`) with:

```rust
        let mut spec = Spec {
            name: raw.name,
            loops,
            roots,
            segments,
            tables: Vec::new(),
            source,
        };
        spec.check_ambiguity()?;
        spec.tables = compile_tables(&spec, &raw.tables)?;
        Ok(spec)
```

In `impl Spec`, add before ``/// Children of a loop, or the top-level loops for `None`.``:

```rust
    /// The definition of an element, or of one of its components; `None`
    /// when the spec does not define it.
    pub fn element_def(
        &self,
        segment: &[u8],
        element: usize,
        component: Option<usize>,
    ) -> Option<&ElementDef> {
        let def = self.segments.get(segment)?.elements.get(&element)?;
        match component {
            None => Some(def),
            Some(component) => def.composite.get(&component),
        }
    }

    /// Every table, in name order.
    pub fn tables(&self) -> &[TableDef] {
        &self.tables
    }

    /// A table by name.
    pub fn table(&self, name: &str) -> Option<&TableDef> {
        self.tables.iter().find(|table| table.name == name)
    }
```

Insert before ``/// Validates a loop's `control`; `has_end` says whether the loop declares an end segment.``:

```rust
/// Compiles the `tables` section against the loops of `spec`, then links
/// every table to the tables above it.
fn compile_tables(spec: &Spec, raw: &BTreeMap<String, Value>) -> Result<Vec<TableDef>, SpecError> {
    let mut tables = Vec::with_capacity(raw.len());
    for (name, value) in raw {
        let def: RawTable =
            serde_json::from_value(value.clone()).map_err(|e| SpecError::TableSchema {
                table: name.clone(),
                column: None,
                source: e,
            })?;
        tables.push(compile_table(spec, name, &def)?);
    }
    link_tables(spec, &mut tables)?;
    Ok(tables)
}

fn compile_table(spec: &Spec, name: &str, def: &RawTable) -> Result<TableDef, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: name.to_string(),
        column: None,
        reason,
    };
    if name.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "table name" }));
    }
    if def.loops.is_empty() {
        return Err(fail(TableDefError::NoLoops));
    }
    let mut loops = Vec::with_capacity(def.loops.len());
    for loop_name in &def.loops {
        let id = spec.loop_id(loop_name).ok_or_else(|| {
            fail(TableDefError::UnknownLoop {
                name: loop_name.clone(),
            })
        })?;
        loops.push(id);
    }
    let reference = def.reference.clone().unwrap_or_else(|| name.to_string());
    if reference.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "ref" }));
    }
    if reference == ROW_COLUMN || reference == SEGMENT_COLUMN {
        return Err(fail(TableDefError::ReservedName { name: reference }));
    }
    let segment = match def.segment.as_deref() {
        None => None,
        Some("") => {
            return Err(SpecError::EmptySegmentId {
                loop_name: None,
                key: format!("tables.{name}.segment"),
            });
        }
        Some(id) => Some(id.as_bytes().to_vec()),
    };
    let repeat = match &def.repeat {
        None => None,
        Some(_) if segment.is_none() => return Err(fail(TableDefError::RepeatWithoutSegment)),
        Some(raw) if raw.from == 0 => {
            return Err(fail(TableDefError::ZeroPosition { key: "repeat.from" }));
        }
        Some(raw) if raw.step == 0 => return Err(fail(TableDefError::ZeroStep)),
        Some(raw) => Some(Repeat {
            from: raw.from,
            step: raw.step,
        }),
    };
    if segment.is_none() {
        for &a in &loops {
            for &b in &loops {
                if spec.ancestors(b).contains(&a) {
                    return Err(fail(TableDefError::NestedAnchors {
                        outer: spec.loop_name(a).to_string(),
                        inner: spec.loop_name(b).to_string(),
                    }));
                }
            }
        }
    }
    let mut columns = Vec::with_capacity(def.columns.len());
    for (column, value) in &def.columns {
        let raw: RawColumn =
            serde_json::from_value(value.clone()).map_err(|e| SpecError::TableSchema {
                table: name.to_string(),
                column: Some(column.clone()),
                source: e,
            })?;
        let source = compile_column(spec, name, column, &raw, &loops, segment.as_deref(), repeat)?;
        columns.push((column.clone(), source));
    }
    Ok(TableDef {
        name: name.to_string(),
        reference,
        loops,
        segment,
        repeat,
        columns,
        parent: None,
        ancestors: Vec::new(),
    })
}

fn compile_column(
    spec: &Spec,
    table: &str,
    column: &str,
    raw: &RawColumn,
    anchors: &[LoopId],
    anchor_segment: Option<&[u8]>,
    repeat: Option<Repeat>,
) -> Result<ColumnSource, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: table.to_string(),
        column: Some(column.to_string()),
        reason,
    };
    if column.is_empty() {
        return Err(fail(TableDefError::EmptyName {
            what: "column name",
        }));
    }
    if column == ROW_COLUMN || column == SEGMENT_COLUMN {
        return Err(fail(TableDefError::ReservedName {
            name: column.to_string(),
        }));
    }
    let found = usize::from(raw.element.is_some())
        + usize::from(raw.group_element.is_some())
        + usize::from(raw.segment_index);
    if found != 1 {
        return Err(fail(TableDefError::SourceCount { found }));
    }
    if raw.segment_index && raw.component.is_some() {
        return Err(fail(TableDefError::ComponentOnIndex));
    }
    if raw.element == Some(0) {
        return Err(fail(TableDefError::ZeroPosition { key: "element" }));
    }
    if raw.component == Some(0) {
        return Err(fail(TableDefError::ZeroPosition { key: "component" }));
    }
    if let Some(offset) = raw.group_element {
        let Some(repeat) = repeat else {
            return Err(fail(TableDefError::GroupWithoutRepeat));
        };
        if offset >= repeat.step {
            return Err(fail(TableDefError::OffsetBeyondStep {
                offset,
                step: repeat.step,
            }));
        }
        return Ok(ColumnSource::GroupElement {
            offset,
            component: raw.component,
        });
    }
    let (loop_id, segment, conditions) = match anchor_segment {
        Some(anchor) => {
            let keys = [
                ("segment", raw.segment.is_some()),
                ("loop", raw.loop_name.is_some()),
                ("where", !raw.conditions.is_empty()),
            ];
            if let Some(&(key, _)) = keys.iter().find(|(_, present)| *present) {
                return Err(fail(TableDefError::AnchorSegmentOnly { key }));
            }
            (None, anchor.to_vec(), Vec::new())
        }
        None => {
            let segment = match raw.segment.as_deref() {
                None => return Err(fail(TableDefError::NeedsSegment)),
                Some("") => {
                    return Err(SpecError::EmptySegmentId {
                        loop_name: None,
                        key: format!("tables.{table}.columns.{column}.segment"),
                    });
                }
                Some(id) => id.as_bytes().to_vec(),
            };
            let loop_id = match &raw.loop_name {
                None => None,
                Some(name) => {
                    let id = spec
                        .loop_id(name)
                        .ok_or_else(|| fail(TableDefError::UnknownLoop { name: name.clone() }))?;
                    if let Some(&anchor) = anchors
                        .iter()
                        .find(|&&anchor| !spec.ancestors(id).contains(&anchor))
                    {
                        return Err(fail(TableDefError::NotADescendant {
                            loop_name: name.clone(),
                            anchor: spec.loop_name(anchor).to_string(),
                        }));
                    }
                    Some(id)
                }
            };
            let mut conditions = Vec::with_capacity(raw.conditions.len());
            for (key, value) in &raw.conditions {
                let position = parse_position(key)
                    .ok_or_else(|| fail(TableDefError::BadPosition { key: key.clone() }))?;
                conditions.push((position, value.as_bytes().to_vec()));
            }
            conditions.sort();
            (loop_id, segment, conditions)
        }
    };
    Ok(match raw.element {
        Some(element) => ColumnSource::Element {
            loop_id,
            segment,
            conditions,
            element,
            component: raw.component,
        },
        None => ColumnSource::SegmentIndex {
            loop_id,
            segment,
            conditions,
        },
    })
}

/// Rejects a loop anchoring two tables without `segment` and a `ref` used
/// twice, then gives each table its chain of tables above: the tables
/// anchored on the loops above each anchor (from the anchor loop itself for
/// a table anchored on a segment, whose rows live inside that loop). Chains
/// run outermost first, and every anchor's chain must begin the longest one.
fn link_tables(spec: &Spec, tables: &mut [TableDef]) -> Result<(), SpecError> {
    let bad = |table: &TableDef, column: Option<&str>, reason| SpecError::BadTable {
        table: table.name.clone(),
        column: column.map(str::to_string),
        reason,
    };
    let mut anchored: Vec<Option<usize>> = vec![None; spec.loops().len()];
    for (index, table) in tables.iter().enumerate() {
        if table.segment.is_some() {
            continue;
        }
        for &id in &table.loops {
            if let Some(other) = anchored[id.index()] {
                return Err(bad(
                    table,
                    None,
                    TableDefError::SharedAnchor {
                        loop_name: spec.loop_name(id).to_string(),
                        other: tables[other].name.clone(),
                    },
                ));
            }
            anchored[id.index()] = Some(index);
        }
    }
    for (index, table) in tables.iter().enumerate() {
        if let Some(first) = tables[..index]
            .iter()
            .find(|other| other.reference == table.reference)
        {
            return Err(bad(
                table,
                None,
                TableDefError::RefTaken {
                    name: table.reference.clone(),
                    table: first.name.clone(),
                },
            ));
        }
    }
    for index in 0..tables.len() {
        let table = &tables[index];
        let mut longest: Option<(LoopId, Vec<usize>)> = None;
        let mut chains = Vec::with_capacity(table.loops.len());
        for &anchor in &table.loops {
            let mut chain = Vec::new();
            let mut current = if table.segment.is_some() {
                Some(anchor)
            } else {
                spec.get(anchor).parent
            };
            while let Some(id) = current {
                if let Some(owner) = anchored[id.index()] {
                    chain.push(owner);
                }
                current = spec.get(id).parent;
            }
            chain.reverse();
            if longest
                .as_ref()
                .is_none_or(|(_, best)| chain.len() > best.len())
            {
                longest = Some((anchor, chain.clone()));
            }
            chains.push((anchor, chain));
        }
        let Some((first, ancestors)) = longest else {
            continue;
        };
        for (anchor, chain) in &chains {
            if !ancestors.starts_with(chain) {
                return Err(bad(
                    table,
                    None,
                    TableDefError::UnrelatedAnchors {
                        first: spec.loop_name(first).to_string(),
                        second: spec.loop_name(*anchor).to_string(),
                    },
                ));
            }
        }
        for (column, _) in &table.columns {
            if ancestors
                .iter()
                .any(|&above| tables[above].reference == *column)
            {
                return Err(bad(
                    table,
                    Some(column),
                    TableDefError::ReservedName {
                        name: column.clone(),
                    },
                ));
            }
        }
        let table = &mut tables[index];
        table.parent = ancestors.last().copied();
        table.ancestors = ancestors;
    }
    Ok(())
}
```

- [ ] **Step 6: Extend the shape pre-check**

In `fn check_shape`, add before `if let Some(segments) = root.get("segments") {`:

```rust
    if let Some(tables) = root.get("tables") {
        for (name, def) in object_at(tables, "tables")? {
            let at = format!("tables.{name}");
            let def = object_at(def, &at)?;
            if let Some(repeat) = def.get("repeat") {
                object_at(repeat, &format!("{at}.repeat"))?;
            }
            if let Some(columns) = def.get("columns") {
                let at = format!("{at}.columns");
                for (column, def) in object_at(columns, &at)? {
                    let at = format!("{at}.{column}");
                    let def = object_at(def, &at)?;
                    if let Some(conditions) = def.get("where") {
                        object_at(conditions, &format!("{at}.where"))?;
                    }
                }
            }
        }
    }
```

- [ ] **Step 7: Export the types**

In `crates/edi835_core/src/lib.rs`, replace the `pub use spec::{…};` block with:

```rust
pub use spec::{
    ColumnSource, Control, ControlCount, ControlError, ElementDef, ElementDefError, ElementType,
    LoopDef, LoopId, Repeat, SegmentDef, Spec, SpecError, TableDef, TableDefError, Trigger,
    merge_patch,
};
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: 86 passed (11 new).

- [ ] **Step 9: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 264 tests in the workspace.

```bash
git add crates/edi835_core/src/spec.rs crates/edi835_core/src/lib.rs
git commit -m "spec: tables section — anchors, repeating groups, column sources, parent chain

A table anchors in loops, optionally on a segment and an element group
that repeats inside it. Columns read an element, a component, a group
element or a segment index. Each table is linked to the tables above
it at load; every fault names the table, the column and the datum."
```

---

## Task 4: The built-in 835 declares its five tables

**Implementer tier:** Opus — the column lists below come from the 005010X221A1 (835) implementation guide and must be checked against it, not merely transcribed: each column's segment, qualifier (`where`), element and component, and the loop that holds it.

**Files:**
- Modify: `crates/edi835_core/specs/835.json`
- Modify: `crates/edi835_core/src/spec.rs`

**Interfaces:**
- Consumes: the `tables` section (Task 3), the built-in `segments` (all 29 ids).
- Produces: five tables, in name order `adjustments`, `claims`, `payments`, `provider_adjustments`, `services`:
  - `payments` (loop `transaction`, ref `payment`): `handling_code` BPR01, `total_payment_amount` BPR02, `credit_debit_flag` BPR03, `payment_method` BPR04, `payment_format` BPR05, `payment_date` BPR16, `trace_number` TRN02, `payer_identifier` TRN03, `payer_name` N102 of loop 1000A, `payee_name` N102 and `payee_id` N104 of loop 1000B, `receiver_id` REF02 of `REF*EV`, `production_date` DTM02 of `DTM*405`.
  - `claims` (loop `2100`, ref `claim`): CLP01–09 and 11–13 (`claim_id`, `claim_status`, `charge_amount`, `payment_amount`, `patient_responsibility`, `filing_indicator`, `payer_control_number`, `facility_type`, `frequency_code`, `drg_code`, `drg_weight`, `discharge_fraction`), `patient_last_name`/`patient_first_name`/`patient_id` (NM103/04/09 of `NM1*QC`), `rendering_provider_name`/`rendering_provider_id` (NM103/09 of `NM1*82`), `statement_from`/`statement_to` (DTM02 of `DTM*232`/`DTM*233`), `coverage_amount` (AMT02 of `AMT*AU`), `group_number` (REF02 of `REF*1L`).
  - `services` (loop `2110`, ref `service`): `procedure_qualifier` SVC01-1, `procedure_code` SVC01-2, `charge_amount` SVC02, `payment_amount` SVC03, `units_paid` SVC05, `original_units` SVC07, `service_date` (DTM02 of `DTM*472`), `line_item_control_number` (REF02 of `REF*6R`), `allowed_amount` (AMT02 of `AMT*B6`).
  - `adjustments` (loops `2100` and `2110`, segment `CAS`, groups from CAS02 every 3): `group_code` CAS01, `reason_code`, `amount`, `quantity` (group offsets 0, 1, 2). Its chain is payments → claims → services; a claim-level adjustment has a null `service`.
  - `provider_adjustments` (loop `transaction`, segment `PLB`, groups from PLB03 every 2): `provider_id` PLB01, `fiscal_period_date` PLB02, `reason_code` and `reference_id` (components 1 and 2 of the group's adjustment identifier), `amount` (offset 1). Its chain is payments.
- Deleting a loop that a table anchors in is now a spec error that names the table; the existing `patch_null_deletes` test deletes the tables that use loop `2110` along with it.

- [ ] **Step 1: Write the failing tests**

In `crates/edi835_core/src/spec.rs`, inside `mod tests`, add before `fn ancestors_are_listed_root_first`:

```rust
    #[test]
    fn builtin_835_declares_five_tables_and_how_they_nest() {
        let spec = Spec::builtin_835();
        let names: Vec<&str> = spec.tables().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "adjustments",
                "claims",
                "payments",
                "provider_adjustments",
                "services"
            ]
        );
        let above = |name: &str| -> Vec<&str> {
            spec.table(name)
                .unwrap()
                .ancestors
                .iter()
                .map(|&i| spec.tables()[i].name.as_str())
                .collect()
        };
        assert!(above("payments").is_empty());
        assert_eq!(above("claims"), vec!["payments"]);
        assert_eq!(above("services"), vec!["payments", "claims"]);
        assert_eq!(above("adjustments"), vec!["payments", "claims", "services"]);
        assert_eq!(above("provider_adjustments"), vec!["payments"]);
        let references: Vec<&str> = ["payments", "claims", "services"]
            .iter()
            .map(|name| spec.table(name).unwrap().reference.as_str())
            .collect();
        assert_eq!(references, vec!["payment", "claim", "service"]);
        let loops = |name: &str| -> Vec<&str> {
            spec.table(name)
                .unwrap()
                .loops
                .iter()
                .map(|&id| spec.loop_name(id))
                .collect()
        };
        assert_eq!(loops("adjustments"), vec!["2100", "2110"]);
        assert_eq!(loops("provider_adjustments"), vec!["transaction"]);
        let adjustments = spec.table("adjustments").unwrap();
        assert_eq!(adjustments.segment.as_deref(), Some(&b"CAS"[..]));
        assert_eq!(adjustments.repeat, Some(Repeat { from: 2, step: 3 }));
        let plb = spec.table("provider_adjustments").unwrap();
        assert_eq!(plb.segment.as_deref(), Some(&b"PLB"[..]));
        assert_eq!(plb.repeat, Some(Repeat { from: 3, step: 2 }));
        let counts: Vec<usize> = spec.tables().iter().map(|t| t.columns.len()).collect();
        assert_eq!(counts, vec![4, 21, 13, 5, 9]);
    }

    #[test]
    fn builtin_835_columns_read_elements_the_spec_defines_in_loops_that_hold_them() {
        let spec = Spec::builtin_835();
        for table in spec.tables() {
            for (column, source) in &table.columns {
                let at = format!("{}.{column}", table.name);
                match source {
                    ColumnSource::Element {
                        loop_id,
                        segment,
                        element,
                        component,
                        ..
                    } => {
                        assert!(
                            spec.element_def(segment, *element, *component).is_some(),
                            "{at} reads an element the spec does not define"
                        );
                        let readers = loop_id.map_or(table.loops.clone(), |id| vec![id]);
                        for id in readers {
                            let def = spec.get(id);
                            assert!(
                                def.trigger.segment == *segment || def.accepts(segment),
                                "{at}: loop {} does not hold {}",
                                def.name,
                                String::from_utf8_lossy(segment)
                            );
                        }
                    }
                    ColumnSource::SegmentIndex { .. } => {}
                    ColumnSource::GroupElement { offset, component } => {
                        let (Some(segment), Some(repeat)) = (&table.segment, table.repeat) else {
                            panic!("{at}: a group column outside a repeating table");
                        };
                        assert!(
                            spec.element_def(segment, repeat.from + offset, *component)
                                .is_some(),
                            "{at} reads a group element the spec does not define"
                        );
                    }
                }
            }
        }
    }
```

Replace `patch_null_deletes` with these two tests:

```rust
    #[test]
    fn patch_null_deletes() {
        let spec = Spec::builtin_835()
            .merge_patch(
                r#"{"loops":{"2110":null},"tables":{"services":null,"adjustments":{"loops":["2100"]}}}"#,
            )
            .unwrap();
        assert_eq!(spec.loop_id("2110"), None);
        assert!(spec.children(spec.loop_id("2100")).is_empty());
        assert_eq!(spec.table("services"), None);
    }

    #[test]
    fn deleting_a_loop_a_table_anchors_in_names_the_table() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2110":null}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: table \"adjustments\": loop \"2110\" does not exist"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib spec::`
Expected: 2 failed — `builtin_835_declares_five_tables_and_how_they_nest` (the built-in has no tables yet) and `deleting_a_loop_a_table_anchors_in_names_the_table` (the patch still loads); the new `patch_null_deletes` and `builtin_835_columns_read_…` already pass.

- [ ] **Step 3: Write the tables**

In `crates/edi835_core/specs/835.json`, the file ends with the closing of the `segments` section and of the root object. Close `segments` with `},` and add the `tables` section, so the file ends:

```json
  },
  "tables": {
    "payments": { "loops": ["transaction"], "ref": "payment", "columns": {
      "handling_code": {"segment": "BPR", "element": 1},
      "total_payment_amount": {"segment": "BPR", "element": 2},
      "credit_debit_flag": {"segment": "BPR", "element": 3},
      "payment_method": {"segment": "BPR", "element": 4},
      "payment_format": {"segment": "BPR", "element": 5},
      "payment_date": {"segment": "BPR", "element": 16},
      "trace_number": {"segment": "TRN", "element": 2},
      "payer_identifier": {"segment": "TRN", "element": 3},
      "payer_name": {"loop": "1000A", "segment": "N1", "element": 2},
      "payee_name": {"loop": "1000B", "segment": "N1", "element": 2},
      "payee_id": {"loop": "1000B", "segment": "N1", "element": 4},
      "receiver_id": {"segment": "REF", "where": {"1": "EV"}, "element": 2},
      "production_date": {"segment": "DTM", "where": {"1": "405"}, "element": 2}
    } },
    "claims": { "loops": ["2100"], "ref": "claim", "columns": {
      "claim_id": {"segment": "CLP", "element": 1},
      "claim_status": {"segment": "CLP", "element": 2},
      "charge_amount": {"segment": "CLP", "element": 3},
      "payment_amount": {"segment": "CLP", "element": 4},
      "patient_responsibility": {"segment": "CLP", "element": 5},
      "filing_indicator": {"segment": "CLP", "element": 6},
      "payer_control_number": {"segment": "CLP", "element": 7},
      "facility_type": {"segment": "CLP", "element": 8},
      "frequency_code": {"segment": "CLP", "element": 9},
      "drg_code": {"segment": "CLP", "element": 11},
      "drg_weight": {"segment": "CLP", "element": 12},
      "discharge_fraction": {"segment": "CLP", "element": 13},
      "patient_last_name": {"segment": "NM1", "where": {"1": "QC"}, "element": 3},
      "patient_first_name": {"segment": "NM1", "where": {"1": "QC"}, "element": 4},
      "patient_id": {"segment": "NM1", "where": {"1": "QC"}, "element": 9},
      "rendering_provider_name": {"segment": "NM1", "where": {"1": "82"}, "element": 3},
      "rendering_provider_id": {"segment": "NM1", "where": {"1": "82"}, "element": 9},
      "statement_from": {"segment": "DTM", "where": {"1": "232"}, "element": 2},
      "statement_to": {"segment": "DTM", "where": {"1": "233"}, "element": 2},
      "coverage_amount": {"segment": "AMT", "where": {"1": "AU"}, "element": 2},
      "group_number": {"segment": "REF", "where": {"1": "1L"}, "element": 2}
    } },
    "services": { "loops": ["2110"], "ref": "service", "columns": {
      "procedure_qualifier": {"segment": "SVC", "element": 1, "component": 1},
      "procedure_code": {"segment": "SVC", "element": 1, "component": 2},
      "charge_amount": {"segment": "SVC", "element": 2},
      "payment_amount": {"segment": "SVC", "element": 3},
      "units_paid": {"segment": "SVC", "element": 5},
      "original_units": {"segment": "SVC", "element": 7},
      "service_date": {"segment": "DTM", "where": {"1": "472"}, "element": 2},
      "line_item_control_number": {"segment": "REF", "where": {"1": "6R"}, "element": 2},
      "allowed_amount": {"segment": "AMT", "where": {"1": "B6"}, "element": 2}
    } },
    "adjustments": { "loops": ["2100", "2110"], "segment": "CAS", "repeat": {"from": 2, "step": 3}, "columns": {
      "group_code": {"element": 1},
      "reason_code": {"group_element": 0},
      "amount": {"group_element": 1},
      "quantity": {"group_element": 2}
    } },
    "provider_adjustments": { "loops": ["transaction"], "segment": "PLB", "repeat": {"from": 3, "step": 2}, "columns": {
      "provider_id": {"element": 1},
      "fiscal_period_date": {"element": 2},
      "reason_code": {"group_element": 0, "component": 1},
      "reference_id": {"group_element": 0, "component": 2},
      "amount": {"group_element": 1}
    } }
  }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib spec::`
Expected: 89 passed.

Run: `cargo test --workspace --locked`
Expected: all pass: the built-in loads with its tables everywhere it is used, and no other test deletes a loop that a table uses.

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 267 tests in the workspace.

```bash
git add crates/edi835_core/specs/835.json crates/edi835_core/src/spec.rs
git commit -m "spec: the built-in 835 declares payments, claims, services, adjustments and provider_adjustments

Columns follow the 005010X221A1 guide; CAS and PLB explode into one row
per element group. Deleting a loop a table anchors in is refused with
the table named."
```

---

## Task 5: `Projector` — element checks and table rows from the engine's events

**Implementer tier:** Opus — a second state machine that must stay in step with the engine's stack (row numbers taken at open and appended at close, parent rows, first-match filling, element groups) while checking every defined element once and reusing the parsed value for the columns; judgment is needed wherever a test disagrees with the code as written.

**Files:**
- Create: `crates/edi835_core/src/project.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `Spec::tables`, `TableDef`, `ColumnSource`, `Spec::element_def`, `Spec::segment`, `ROW_COLUMN`, `SEGMENT_COLUMN` (Tasks 3–4); `ColumnType`, `Cell`, `Table`, `Tables`, `parse_n`/`parse_r`/`parse_dt`/`parse_tm` (Task 2); `Event` with `LoopOpened.segment`; `Diagnostic::new`, `LoopRef`, the level-2 `Rule` variants (declared in 4a); `Delimiters::component`.
- Produces:
  - `pub struct Projector<'s>` (`Debug`, `Clone`) with `new(spec: &'s Spec, delimiters: &Delimiters) -> Self`, `on(&mut self, &Segment<'_>, &[Event]) -> &[Diagnostic]`, `finish(&mut self) -> &[Diagnostic]`, `take_tables(&mut self) -> Tables`.
  - Every table has, in order, `row` (`int64`, 0-based, counted across the stream), `segment` (`int64`, the anchor segment: the trigger of the anchor instance, or the anchored segment), one `int64` column per table above it named after that table's `ref` (outermost first; null when no instance of it is open), then the declared columns in name order. A declared column's type is `ColumnType::of` the definition it reads (`int64` for `segment_index`).
  - A table without `segment`: row number taken at `LoopOpened` of an anchor loop; each column takes the value of the first captured segment that matches its source while the instance is open (in the anchor loop, or in the first instance of its `loop`); appended at that instance's `LoopClosed`. A table with `segment`: rows appended when the segment is captured in an anchor loop — one per element group whose first element has content when it has `repeat`.
  - Every segment captured into a loop and defined in `segments` is checked as it arrives (unmatched segments are not): an element defined without `composite` as one text (a composite in the file is joined back with the component separator); with `composite`, each declared component (a simple value is component 1). Rules: `RequiredElementMissing` (empty or absent; datum empty), `TypeMismatch` (datum the text), `LengthOutOfRange` (bytes for `AN`/`ID`/`DT`/`TM`, digits for `N`/`R`; checked only when the type parses; the value is kept), `CompositeShape` (more components than the highest declared; the location's component is the first extra one, the datum its text). The diagnostic carries the segment index, the element, the component and the path of open loops with stream-wide ordinals, like the checker's.
  - `finish` closes what is open (appending those rows) and resets the loop ordinals and the row numbers; `take_tables` leaves rows still being collected in place.

`new` takes the delimiters so an element read whole can be joined back with the component separator; nothing else reads them. §7 sketches `new(&Spec)`; a projector fed from a `Tokenizer` passes `tokenizer.delimiters()`, and `Processor::run` passes `document.delimiters()`.

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/project.rs` with the module doc, the imports and the tests only:

```rust
//! Projection of a segment stream into typed tables, with element checks.
//!
//! The projector follows the loops the engine opens and closes and fills the
//! tables of the spec. A table without a `segment` gets one row per instance
//! of its anchor loops: the row number is taken when the instance opens, each
//! column fills from the first captured segment that matches its source while
//! the instance is open, and the row is appended when the instance closes. A
//! table anchored on a segment gets its rows when that segment is captured,
//! one per element group when the segment repeats a group. Every row also
//! carries its number, the index of its anchor segment and, for each table
//! above it, the number of that table's open row (null when none is open).
//! Row numbers count from 0 across the whole stream, so they keep their
//! meaning when the tables are drained part way.
//!
//! Every captured segment the spec defines is checked element by element as
//! it arrives: required elements, types, lengths and composite shapes. An
//! element defined without components is read as one text, with any
//! component separator it contains kept in place; components are read only
//! where the definition declares them. A column reads the value the check
//! already parsed, so no element is parsed twice. A value that is missing or
//! does not parse as its type is null in its column; a value whose length is
//! out of range is reported and kept.
//!
//! Allocation: appending a row collects its cells into a new vector and grows
//! the table's buffers; each diagnostic owns its text. A row being collected
//! copies the bytes of its text columns into a buffer that is reused from one
//! instance to the next, and the per-segment state (the checked values and
//! the text of a composite read as one) lives in buffers that are cleared and
//! reused.

use crate::column::{Cell, ColumnType, Table, Tables, parse_dt, parse_n, parse_r, parse_tm};
use crate::delimiters::Delimiters;
use crate::diagnostic::{Diagnostic, LoopRef, Rule};
use crate::element::Element;
use crate::engine::Event;
use crate::segment::Segment;
use crate::spec::{
    ColumnSource, ElementDef, ElementType, LoopId, ROW_COLUMN, SEGMENT_COLUMN, Spec, TableDef,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoopEngine, Tokenizer};
    use proptest::prelude::*;

    const SPEC: &str = r#"{"name":"t",
        "loops":{
            "head":{"trigger":{"segment":"HD"},"segments":["ZZ"],"end":"TR"},
            "note":{"parent":"head","trigger":{"segment":"NM","where":{"1":"P"}}},
            "claim":{"parent":"head","trigger":{"segment":"CL"},"segments":["DT","AJ","RF"]},
            "line":{"parent":"claim","trigger":{"segment":"LN"},"segments":["DT","AJ"]}
        },
        "segments":{
            "HD":{"elements":{"1":{"name":"batch","type":"AN","required":true,"min":1,"max":5}}},
            "CL":{"elements":{
                "1":{"name":"claim_id","type":"AN","required":true,"min":1,"max":10},
                "2":{"name":"charge","type":"R","required":true,"max":10},
                "3":{"name":"units","type":"N0","max":2},
                "4":{"name":"procedure","type":"AN","composite":{
                    "1":{"name":"qualifier","type":"ID","required":true,"min":2,"max":2},
                    "2":{"name":"code","type":"AN","required":true}
                }}
            }},
            "DT":{"elements":{
                "1":{"name":"qualifier","type":"ID","required":true},
                "2":{"name":"date","type":"DT"},
                "3":{"name":"time","type":"TM"}
            }},
            "AJ":{"elements":{
                "1":{"name":"group","type":"ID","required":true},
                "2":{"name":"reason","type":"ID"},
                "3":{"name":"amount","type":"R"},
                "4":{"name":"reason_2","type":"ID"},
                "5":{"name":"amount_2","type":"R"}
            }}
        },
        "tables":{
            "heads":{"loops":["head"],"ref":"head","columns":{
                "batch":{"segment":"HD","element":1},
                "payer":{"loop":"note","segment":"NM","element":2}
            }},
            "claims":{"loops":["claim"],"ref":"claim","columns":{
                "claim_id":{"segment":"CL","element":1},
                "charge":{"segment":"CL","element":2},
                "units":{"segment":"CL","element":3},
                "procedure":{"segment":"CL","element":4},
                "code":{"segment":"CL","element":4,"component":2},
                "from":{"segment":"DT","where":{"1":"150"},"element":2},
                "reference":{"segment":"RF","element":2},
                "first_line_at":{"loop":"line","segment":"LN","segment_index":true}
            }},
            "lines":{"loops":["line"],"ref":"line","columns":{
                "code":{"segment":"LN","element":1},
                "date":{"segment":"DT","element":2},
                "time":{"segment":"DT","element":3}
            }},
            "adjustments":{"loops":["claim","line"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
                "group":{"element":1},
                "reason":{"group_element":0},
                "amount":{"group_element":1}
            }}
        }
    }"#;

    fn spec() -> Spec {
        Spec::from_json(SPEC).unwrap()
    }

    fn delimiters() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    /// Runs the engine and a projector over `input`, `finish` included.
    fn project(spec: &Spec, input: &str) -> (Tables, Vec<Diagnostic>) {
        let mut engine = LoopEngine::new(spec);
        let mut projector = Projector::new(spec, &delimiters());
        let mut diagnostics = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let events = engine.feed(&segment);
            diagnostics.extend_from_slice(projector.on(&segment, events));
        }
        engine.finish();
        diagnostics.extend_from_slice(projector.finish());
        (projector.take_tables(), diagnostics)
    }

    /// A table as text: the column names, then one line per row.
    fn rows(tables: &Tables, name: &str) -> Vec<String> {
        let table = tables.get(name).unwrap();
        let header = table
            .columns()
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        std::iter::once(header)
            .chain((0..table.len()).map(|row| {
                table
                    .columns()
                    .iter()
                    .map(|(_, column)| column.render(row).unwrap())
                    .collect::<Vec<_>>()
                    .join(" | ")
            }))
            .collect()
    }

    fn rendered(diagnostics: &[Diagnostic]) -> Vec<String> {
        diagnostics.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn columns_fill_from_the_first_matching_segment_and_rows_close_with_their_loop() {
        let spec = spec();
        let (tables, diagnostics) = project(
            &spec,
            "HD*B1~NM*P*ACME~NM*P*OTHER~CL*C1*12.5*3*HC:99213~DT*151*20240101~DT*150*20240105~DT*150*20240106~RF*Q*X1~LN*L1~DT*472*20240107*1230~TR~",
        );
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
        assert_eq!(
            rows(&tables, "heads"),
            vec!["row | segment | batch | payer", "0 | 0 | B1 | ACME"]
        );
        assert_eq!(
            rows(&tables, "claims"),
            vec![
                "row | segment | head | charge | claim_id | code | first_line_at | from | procedure | reference | units",
                "0 | 3 | 0 | 12.50 | C1 | 99213 | 8 | 2024-01-05 | HC:99213 | X1 | 3",
            ]
        );
        assert_eq!(
            rows(&tables, "lines"),
            vec![
                "row | segment | head | claim | code | date | time",
                "0 | 8 | 0 | 0 | L1 | 2024-01-07 | 12:30:00",
            ]
        );
    }

    #[test]
    fn a_column_no_segment_matches_is_null() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~TR~");
        assert_eq!(
            rows(&tables, "claims")[1],
            "0 | 1 | 0 | 1.00 | C1 | ∅ | ∅ | ∅ | ∅ | ∅ | ∅"
        );
        assert_eq!(rows(&tables, "heads")[1], "0 | 0 | B1 | ∅");
    }

    #[test]
    fn every_row_names_the_open_row_of_each_table_above_it() {
        let spec = spec();
        let (tables, _) = project(
            &spec,
            "HD*B1~CL*C1*1~AJ*CO*45*10~LN*L1~AJ*PR*1*2~LN*L2~CL*C2*2~LN*L3~AJ*OA*3*4*5*6~TR~",
        );
        assert_eq!(
            rows(&tables, "lines"),
            vec![
                "row | segment | head | claim | code | date | time",
                "0 | 3 | 0 | 0 | L1 | ∅ | ∅",
                "1 | 5 | 0 | 0 | L2 | ∅ | ∅",
                "2 | 7 | 0 | 1 | L3 | ∅ | ∅",
            ]
        );
        assert_eq!(
            rows(&tables, "adjustments"),
            vec![
                "row | segment | head | claim | line | amount | group | reason",
                "0 | 2 | 0 | 0 | ∅ | 10.00 | CO | 45",
                "1 | 4 | 0 | 0 | 0 | 2.00 | PR | 1",
                "2 | 8 | 0 | 1 | 2 | 4.00 | OA | 3",
                "3 | 8 | 0 | 1 | 2 | 6.00 | OA | 5",
            ]
        );
    }

    #[test]
    fn a_repeated_group_without_its_first_element_gives_no_row() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~AJ*CO*45*10**7~AJ*PR~TR~");
        assert_eq!(
            rows(&tables, "adjustments"),
            vec![
                "row | segment | head | claim | line | amount | group | reason",
                "0 | 2 | 0 | 0 | ∅ | 10.00 | CO | 45",
            ]
        );
    }

    #[test]
    fn row_numbers_keep_counting_across_drains() {
        let spec = spec();
        let mut engine = LoopEngine::new(&spec);
        let mut projector = Projector::new(&spec, &delimiters());
        let mut drains = Vec::new();
        let input = "HD*B1~CL*C1*1~LN*L1~CL*C2*2~LN*L2~TR~";
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let events = engine.feed(&segment).to_vec();
            projector.on(&segment, &events);
            if events.contains(&Event::LoopClosed {
                id: spec.loop_id("claim").unwrap(),
            }) {
                drains.push(projector.take_tables());
            }
        }
        engine.finish();
        projector.finish();
        drains.push(projector.take_tables());
        let lines: Vec<Vec<String>> = drains
            .iter()
            .map(|tables| rows(tables, "lines").split_off(1))
            .collect();
        assert_eq!(
            lines,
            vec![
                vec!["0 | 2 | 0 | 0 | L1 | ∅ | ∅".to_string()],
                vec!["1 | 4 | 0 | 1 | L2 | ∅ | ∅".to_string()],
                vec![],
            ]
        );
        assert_eq!(
            rows(&drains[1], "heads").split_off(1),
            vec!["0 | 0 | B1 | ∅"],
            "the end segment closes the head right after the last claim"
        );
        assert!(drains[2].iter().all(Table::is_empty));
    }

    #[test]
    fn finishing_appends_the_rows_still_open() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~LN*L1~");
        assert_eq!(tables.get("heads").unwrap().len(), 1);
        assert_eq!(tables.get("claims").unwrap().len(), 1);
        assert_eq!(tables.get("lines").unwrap().len(), 1);
    }

    #[test]
    fn a_value_that_is_not_its_type_is_reported_and_null() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*12A~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL02 (charge) is not a valid R (decimal, scale 2) · segment #1, element 2 · at head#1/claim#1 · datum \"12A\""
            ]
        );
        assert_eq!(
            tables
                .get("claims")
                .unwrap()
                .column("charge")
                .unwrap()
                .get(0),
            Some(Cell::Null)
        );
    }

    #[test]
    fn a_decimal_with_more_places_than_its_scale_is_a_type_mismatch() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL*C1*1.234~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL02 (charge) is not a valid R (decimal, scale 2) · segment #1, element 2 · at head#1/claim#1 · datum \"1.234\""
            ]
        );
    }

    #[test]
    fn an_empty_required_element_is_reported_with_its_name() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL**5~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · required element CL01 (claim_id) is missing or empty · segment #1, element 1 · at head#1/claim#1 · datum \"\""
            ]
        );
    }

    #[test]
    fn an_invalid_date_names_the_element_and_the_text() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*1~DT*150*20240230~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element DT02 (date) is not a valid DT (date CCYYMMDD or YYMMDD) · segment #2, element 2 · at head#1/claim#1 · datum \"20240230\""
            ]
        );
        assert_eq!(
            tables.get("claims").unwrap().column("from").unwrap().get(0),
            Some(Cell::Null),
            "the first matching DTM decides the column, even when its value is invalid"
        );
    }

    #[test]
    fn lengths_count_bytes_for_text_and_digits_for_numbers() {
        let spec = spec();
        let (tables, diagnostics) = project(
            &spec,
            "HD*B1~CL*ABCDEFGHIJK*-12345678.90*-12~CL*C2*1*123~TR~",
        );
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL01 (claim_id) has length 11; the spec allows 1 to 10 · segment #1, element 1 · at head#1/claim#1 · datum \"ABCDEFGHIJK\"",
                "SNIP 2 · element CL03 (units) has length 3; the spec allows at most 2 · segment #2, element 3 · at head#1/claim#2 · datum \"123\"",
            ]
        );
        let claims = tables.get("claims").unwrap();
        assert_eq!(
            claims.column("claim_id").unwrap().get(0),
            Some(Cell::Binary(b"ABCDEFGHIJK")),
            "a value out of range is reported and kept"
        );
        assert_eq!(
            claims.column("charge").unwrap().get(0),
            Some(Cell::Decimal128(-1_234_567_890))
        );
        assert_eq!(
            claims.column("units").unwrap().get(0),
            Some(Cell::Int64(-12))
        );
    }

    #[test]
    fn a_composite_with_more_components_than_declared_names_the_first_extra_one() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL*C1*1**HC:1:X~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL04 (procedure) has 3 components; the spec declares 2 · segment #1, element 4, component 3 · at head#1/claim#1 · datum \"X\""
            ]
        );
    }

    #[test]
    fn components_are_checked_where_the_definition_declares_them() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*1**HCPC~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL04-1 (qualifier) has length 4; the spec allows 2 to 2 · segment #1, element 4, component 1 · at head#1/claim#1 · datum \"HCPC\"",
                "SNIP 2 · required element CL04-2 (code) is missing or empty · segment #1, element 4, component 2 · at head#1/claim#1 · datum \"\"",
            ]
        );
        assert_eq!(
            tables
                .get("claims")
                .unwrap()
                .column("procedure")
                .unwrap()
                .get(0),
            Some(Cell::Binary(b"HCPC"))
        );
    }

    #[test]
    fn an_element_defined_without_components_is_read_as_one_text() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*A:B~TR~");
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
        assert_eq!(
            tables.get("heads").unwrap().column("batch").unwrap().get(0),
            Some(Cell::Binary(b"A:B"))
        );
        let (_, diagnostics) = project(&spec, "HD*A:BCDE~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element HD01 (batch) has length 6; the spec allows 1 to 5 · segment #0, element 1 · at head#1 · datum \"A:BCDE\""
            ]
        );
    }

    #[test]
    fn only_captured_segments_with_a_definition_are_checked() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~ZZ*whatever~QQ*!~CL*C1*1~RF~TR~");
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
    }

    #[test]
    fn the_spec_tables_give_the_columns_and_their_types() {
        let spec = spec();
        let (tables, _) = project(&spec, "");
        let kinds: Vec<(&str, ColumnType)> = tables
            .get("lines")
            .unwrap()
            .columns()
            .iter()
            .map(|(name, column)| (name.as_str(), column.kind()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("row", ColumnType::Int64 { scale: 0 }),
                ("segment", ColumnType::Int64 { scale: 0 }),
                ("head", ColumnType::Int64 { scale: 0 }),
                ("claim", ColumnType::Int64 { scale: 0 }),
                ("code", ColumnType::Binary),
                ("date", ColumnType::Date32),
                ("time", ColumnType::Time32),
            ]
        );
        let names: Vec<&str> = tables.iter().map(Table::name).collect();
        assert_eq!(names, vec!["adjustments", "claims", "heads", "lines"]);
    }

    /// An `R` value written the way X12 writes it, with two decimals.
    fn money(cents: i64) -> String {
        let sign = if cents < 0 { "-" } else { "" };
        let cents = cents.unsigned_abs();
        format!("{sign}{}.{:02}", cents / 100, cents % 100)
    }

    proptest! {
        #[test]
        fn valid_values_never_raise_a_diagnostic(
            claim_id in "[A-Z0-9]{1,10}",
            cents in -99_999_999i64..99_999_999,
            units in 0i64..99,
            (year, month, day) in (1900i32..2100, 1i32..=12, 1i32..=28),
            seconds in 0i32..86_400,
        ) {
            let spec = spec();
            let input = format!(
                "HD*B1~CL*{claim_id}*{}*{units}*HC:X1~LN*L1~DT*472*{year:04}{month:02}{day:02}*{:02}{:02}{:02}~TR~",
                money(cents),
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60,
            );
            let (tables, diagnostics) = project(&spec, &input);
            prop_assert_eq!(rendered(&diagnostics), Vec::<String>::new());
            let claims = tables.get("claims").unwrap();
            prop_assert_eq!(claims.column("charge").unwrap().get(0), Some(Cell::Decimal128(i128::from(cents))));
            prop_assert_eq!(claims.column("units").unwrap().get(0), Some(Cell::Int64(units)));
            let lines = tables.get("lines").unwrap();
            prop_assert_eq!(lines.column("time").unwrap().get(0), Some(Cell::Time32(seconds)));
        }

        #[test]
        fn random_elements_never_panic_and_rows_stay_aligned(
            bodies in proptest::collection::vec("[A-Z0-9*:.\\-]{0,24}", 0..8),
        ) {
            let spec = spec();
            let mut input = String::from("HD*B1~");
            for (i, body) in bodies.iter().enumerate() {
                let id = ["CL", "LN", "DT", "AJ", "RF", "NM"][i % 6];
                input.push_str(&format!("{id}*{body}~"));
            }
            input.push_str("TR~");
            let (tables, _) = project(&spec, &input);
            for table in &tables {
                for (_, column) in table.columns() {
                    prop_assert_eq!(column.len(), table.len());
                }
            }
        }
    }
}
```

In `crates/edi835_core/src/lib.rs`, add `pub mod project;` after `pub mod frame;`, and `pub use project::Projector;` after `pub use frame::{Frame, next_frame};`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib project::`
Expected: compile errors (`cannot find struct 'Projector'`); the `pub use` line fails too.

- [ ] **Step 3: Implement the projector**

Insert between the `use crate::spec::{…};` block and `#[cfg(test)]`:

```rust
/// A value an element check parsed, or a column read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parsed {
    /// Absent, empty, or not a valid value of its type.
    Null,
    /// Text, to be copied from the segment.
    Text,
    Int(i64),
    Decimal(i128),
    Date(i32),
    Time(i32),
}

/// One defined element (or component) of the current segment, checked.
#[derive(Debug, Clone, Copy)]
struct Checked {
    element: usize,
    component: Option<usize>,
    /// The column type its definition maps to.
    kind: ColumnType,
    value: Parsed,
}

/// A column value of a row being collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// No segment has matched the column's source yet.
    Unset,
    Null,
    /// A range of the row's byte buffer.
    Bytes(usize, usize),
    Int(i64),
    Decimal(i128),
    Date(i32),
    Time(i32),
}

/// A row being collected.
#[derive(Debug, Clone, Default)]
struct Row {
    ordinal: usize,
    segment: usize,
    parents: Vec<Option<usize>>,
    cells: Vec<Slot>,
    bytes: Vec<u8>,
}

/// What the projector keeps for one table.
#[derive(Debug, Clone)]
struct TableState {
    /// Column types of the declared columns, in order.
    kinds: Vec<ColumnType>,
    /// Number of the next row.
    next: usize,
    /// `true` while an instance of an anchor loop is open (tables without
    /// `segment` only).
    open: bool,
    row: Row,
    table: Table,
}

/// Turns the engine's events and the segments they name into table rows and
/// element diagnostics, one segment at a time.
#[derive(Debug, Clone)]
pub struct Projector<'s> {
    spec: &'s Spec,
    separator: u8,
    tables: Vec<TableState>,
    /// Per loop: the table without `segment` anchored in it.
    anchored: Vec<Option<usize>>,
    /// Per loop: `(table, column)` for the columns that read segments captured in it.
    watchers: Vec<Vec<(usize, usize)>>,
    /// Per loop: the tables anchored on a segment inside it.
    segment_tables: Vec<Vec<usize>>,
    /// Open loop instances with their ordinals, outermost first.
    open: Vec<(LoopId, usize)>,
    /// Instances opened so far, per loop index.
    ordinals: Vec<usize>,
    checked: Vec<Checked>,
    joined: Vec<u8>,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> Projector<'s> {
    /// A projector at the root with empty tables. `delimiters` gives the
    /// component separator, which an element read as one text keeps.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        let loops = spec.loops().len();
        let mut anchored = vec![None; loops];
        let mut watchers = vec![Vec::new(); loops];
        let mut segment_tables = vec![Vec::new(); loops];
        let mut tables = Vec::with_capacity(spec.tables().len());
        for (index, def) in spec.tables().iter().enumerate() {
            let kinds: Vec<ColumnType> = def
                .columns
                .iter()
                .map(|(_, source)| column_type(spec, def, source))
                .collect();
            let index_column = ColumnType::Int64 { scale: 0 };
            let mut columns = vec![
                (ROW_COLUMN.to_string(), index_column),
                (SEGMENT_COLUMN.to_string(), index_column),
            ];
            for &above in &def.ancestors {
                columns.push((spec.tables()[above].reference.clone(), index_column));
            }
            columns.extend(
                def.columns
                    .iter()
                    .map(|(name, _)| name.clone())
                    .zip(kinds.iter().copied()),
            );
            for &id in &def.loops {
                match def.segment {
                    Some(_) => segment_tables[id.index()].push(index),
                    None => anchored[id.index()] = Some(index),
                }
            }
            if def.segment.is_none() {
                for (column, (_, source)) in def.columns.iter().enumerate() {
                    let reader = match source {
                        ColumnSource::Element { loop_id, .. }
                        | ColumnSource::SegmentIndex { loop_id, .. } => *loop_id,
                        ColumnSource::GroupElement { .. } => continue,
                    };
                    match reader {
                        Some(id) => watchers[id.index()].push((index, column)),
                        None => {
                            for &id in &def.loops {
                                watchers[id.index()].push((index, column));
                            }
                        }
                    }
                }
            }
            tables.push(TableState {
                kinds,
                next: 0,
                open: false,
                row: Row::default(),
                table: Table::new(def.name.clone(), columns),
            });
        }
        Self {
            spec,
            separator: delimiters.component,
            tables,
            anchored,
            watchers,
            segment_tables,
            open: Vec::new(),
            ordinals: vec![0; loops],
            checked: Vec::new(),
            joined: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics its elements raise. The slice is valid until the next call.
    pub fn on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic] {
        self.diagnostics.clear();
        for &event in events {
            match event {
                Event::LoopOpened {
                    id,
                    segment: trigger,
                    ..
                } => self.opened(id, trigger),
                Event::Captured { id, .. } => self.captured(id, segment),
                Event::LoopClosed { .. } => self.closed(),
                Event::Unmatched { .. } | Event::Empty { .. } => {}
            }
        }
        &self.diagnostics
    }

    /// Closes every loop still open, as the engine's `finish` does, appending
    /// the rows they were collecting. The projector is then back at the
    /// root and its row numbers restart at 0, so the tables should be taken
    /// before it is fed a new stream.
    pub fn finish(&mut self) -> &[Diagnostic] {
        self.diagnostics.clear();
        while !self.open.is_empty() {
            self.closed();
        }
        self.ordinals.iter_mut().for_each(|count| *count = 0);
        self.tables.iter_mut().for_each(|state| state.next = 0);
        &self.diagnostics
    }

    /// Moves every appended row out, leaving the tables empty. Rows still
    /// being collected stay, and keep their numbers.
    pub fn take_tables(&mut self) -> Tables {
        Tables::new(
            self.tables
                .iter_mut()
                .map(|state| state.table.take_rows())
                .collect(),
        )
    }

    fn opened(&mut self, id: LoopId, trigger: usize) {
        let ordinal = self.ordinals[id.index()].saturating_add(1);
        self.ordinals[id.index()] = ordinal;
        self.open.push((id, ordinal));
        let Some(index) = self.anchored[id.index()] else {
            return;
        };
        let spec = self.spec;
        let def = &spec.tables()[index];
        let mut row = std::mem::take(&mut self.tables[index].row);
        row.parents.clear();
        row.parents
            .extend(def.ancestors.iter().map(|&above| self.open_row(above)));
        row.cells.clear();
        row.cells.resize(def.columns.len(), Slot::Unset);
        row.bytes.clear();
        row.segment = trigger;
        let state = &mut self.tables[index];
        row.ordinal = state.next;
        state.next = state.next.saturating_add(1);
        state.row = row;
        state.open = true;
    }

    fn closed(&mut self) {
        let Some((id, _)) = self.open.pop() else {
            return;
        };
        let Some(index) = self.anchored[id.index()] else {
            return;
        };
        let state = &mut self.tables[index];
        if state.open {
            state.open = false;
            append(&mut state.table, &state.row);
        }
    }

    fn captured(&mut self, id: LoopId, segment: &Segment<'_>) {
        let mut joined = std::mem::take(&mut self.joined);
        self.check(segment, &mut joined);
        self.fill(id, segment, &mut joined);
        self.segment_rows(id, segment, &mut joined);
        self.joined = joined;
    }

    /// The number of the row `table` is collecting, if an instance is open.
    fn open_row(&self, table: usize) -> Option<usize> {
        let state = &self.tables[table];
        state.open.then_some(state.row.ordinal)
    }

    /// Checks every element the spec defines for the segment and keeps the
    /// parsed values for the columns that read them.
    fn check(&mut self, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        self.checked.clear();
        let spec = self.spec;
        let Some(def) = spec.segment(segment.id) else {
            return;
        };
        for (&position, element) in &def.elements {
            if element.composite.is_empty() {
                let text = leaf_text(segment, position, None, self.separator, joined);
                let value = self.check_value(segment, position, None, element, text);
                self.checked.push(Checked {
                    element: position,
                    component: None,
                    kind: ColumnType::of(Some(element.kind)),
                    value,
                });
                continue;
            }
            let parts: &[std::borrow::Cow<'_, [u8]>] = match segment.element(position) {
                Some(Element::Composite(parts)) => parts,
                Some(Element::Simple(value)) => std::slice::from_ref(value),
                None => &[],
            };
            if parts.iter().all(|part| part.is_empty()) {
                if element.required {
                    self.report(
                        Rule::RequiredElementMissing {
                            segment_id: segment.id.to_vec(),
                            element: position,
                            component: None,
                            name: element.name.clone(),
                        },
                        segment.index,
                        position,
                        None,
                        &[],
                    );
                }
                continue;
            }
            let declared = element.composite.keys().copied().max().unwrap_or_default();
            if parts.len() > declared {
                let extra = parts.get(declared).map_or(&[][..], |part| part.as_ref());
                self.report(
                    Rule::CompositeShape {
                        segment_id: segment.id.to_vec(),
                        element: position,
                        name: element.name.clone(),
                        declared,
                        found: parts.len(),
                    },
                    segment.index,
                    position,
                    declared.checked_add(1),
                    extra,
                );
            }
            for (&component, def) in &element.composite {
                let text = component
                    .checked_sub(1)
                    .and_then(|at| parts.get(at))
                    .map_or(&[][..], |part| part.as_ref());
                let value = self.check_value(segment, position, Some(component), def, text);
                self.checked.push(Checked {
                    element: position,
                    component: Some(component),
                    kind: ColumnType::of(Some(def.kind)),
                    value,
                });
            }
        }
    }

    /// Checks one value against its definition, reports what fails, and
    /// returns the parsed value (null when missing or of the wrong type).
    fn check_value(
        &mut self,
        segment: &Segment<'_>,
        element: usize,
        component: Option<usize>,
        def: &ElementDef,
        text: &[u8],
    ) -> Parsed {
        if text.is_empty() {
            if def.required {
                self.report(
                    Rule::RequiredElementMissing {
                        segment_id: segment.id.to_vec(),
                        element,
                        component,
                        name: def.name.clone(),
                    },
                    segment.index,
                    element,
                    component,
                    &[],
                );
            }
            return Parsed::Null;
        }
        let Some(value) = parse(ColumnType::of(Some(def.kind)), text) else {
            self.report(
                Rule::TypeMismatch {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    expected: def.kind,
                },
                segment.index,
                element,
                component,
                text,
            );
            return Parsed::Null;
        };
        // Numeric lengths count digits only, as X12 does: no sign, no point.
        let length = match def.kind {
            ElementType::N(_) | ElementType::R { .. } => {
                text.iter().filter(|byte| byte.is_ascii_digit()).count()
            }
            _ => text.len(),
        };
        if def.min.is_some_and(|min| length < min) || def.max.is_some_and(|max| length > max) {
            self.report(
                Rule::LengthOutOfRange {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    min: def.min,
                    max: def.max,
                    length,
                },
                segment.index,
                element,
                component,
                text,
            );
        }
        value
    }

    /// Fills the open rows whose columns read this segment and have no value yet.
    fn fill(&mut self, id: LoopId, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        let spec = self.spec;
        for &(index, column) in &self.watchers[id.index()] {
            let state = &mut self.tables[index];
            if !state.open || state.row.cells.get(column) != Some(&Slot::Unset) {
                continue;
            }
            let Some((_, source)) = spec.tables()[index].columns.get(column) else {
                continue;
            };
            let slot = match source {
                ColumnSource::Element {
                    segment: wanted,
                    conditions,
                    element,
                    component,
                    ..
                } => {
                    if !matches(segment, wanted, conditions) {
                        continue;
                    }
                    let kind = state
                        .kinds
                        .get(column)
                        .copied()
                        .unwrap_or(ColumnType::Binary);
                    let at = Place {
                        element: *element,
                        component: *component,
                        kind,
                    };
                    read(
                        &self.checked,
                        segment,
                        at,
                        self.separator,
                        joined,
                        &mut state.row.bytes,
                    )
                }
                ColumnSource::SegmentIndex {
                    segment: wanted,
                    conditions,
                    ..
                } => {
                    if !matches(segment, wanted, conditions) {
                        continue;
                    }
                    i64::try_from(segment.index).map_or(Slot::Null, Slot::Int)
                }
                ColumnSource::GroupElement { .. } => continue,
            };
            if let Some(cell) = state.row.cells.get_mut(column) {
                *cell = slot;
            }
        }
    }

    /// Appends the rows of the tables anchored on this segment.
    fn segment_rows(&mut self, id: LoopId, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        let spec = self.spec;
        for slot in 0..self.segment_tables[id.index()].len() {
            let index = self.segment_tables[id.index()][slot];
            let def = &spec.tables()[index];
            if def.segment.as_deref() != Some(segment.id) {
                continue;
            }
            let Some(repeat) = def.repeat else {
                self.segment_row(index, segment, None, joined);
                continue;
            };
            let mut start = Some(repeat.from);
            while let Some(position) = start.filter(|&at| at <= segment.elements.len()) {
                if segment.element(position).is_some_and(has_content) {
                    self.segment_row(index, segment, Some(position), joined);
                }
                start = position.checked_add(repeat.step);
            }
        }
    }

    /// Appends one row of a table anchored on a segment; `group` is the
    /// position of the first element of the row's group.
    fn segment_row(
        &mut self,
        index: usize,
        segment: &Segment<'_>,
        group: Option<usize>,
        joined: &mut Vec<u8>,
    ) {
        let spec = self.spec;
        let def = &spec.tables()[index];
        let mut row = std::mem::take(&mut self.tables[index].row);
        row.parents.clear();
        row.parents
            .extend(def.ancestors.iter().map(|&above| self.open_row(above)));
        row.cells.clear();
        row.bytes.clear();
        for (column, (_, source)) in def.columns.iter().enumerate() {
            let kind = self.tables[index]
                .kinds
                .get(column)
                .copied()
                .unwrap_or(ColumnType::Binary);
            let at = match source {
                ColumnSource::Element {
                    element, component, ..
                } => Some((*element, *component)),
                ColumnSource::GroupElement { offset, component } => group
                    .and_then(|start| start.checked_add(*offset))
                    .map(|element| (element, *component)),
                ColumnSource::SegmentIndex { .. } => {
                    row.cells
                        .push(i64::try_from(segment.index).map_or(Slot::Null, Slot::Int));
                    continue;
                }
            };
            let slot = match at {
                Some((element, component)) => read(
                    &self.checked,
                    segment,
                    Place {
                        element,
                        component,
                        kind,
                    },
                    self.separator,
                    joined,
                    &mut row.bytes,
                ),
                None => Slot::Null,
            };
            row.cells.push(slot);
        }
        let state = &mut self.tables[index];
        row.ordinal = state.next;
        row.segment = segment.index;
        state.next = state.next.saturating_add(1);
        append(&mut state.table, &row);
        state.row = row;
    }

    fn report(
        &mut self,
        rule: Rule,
        segment: usize,
        element: usize,
        component: Option<usize>,
        datum: &[u8],
    ) {
        let spec = self.spec;
        let path = self
            .open
            .iter()
            .map(|&(id, ordinal)| LoopRef {
                name: spec.loop_name(id).to_string(),
                ordinal,
            })
            .collect();
        self.diagnostics.push(Diagnostic::new(
            rule,
            Some(segment),
            Some(element),
            component,
            path,
            datum.to_vec(),
        ));
    }
}

/// The column type of a declared column: its element's type, `Int64` for a
/// segment index, `Binary` for an element the spec does not define.
fn column_type(spec: &Spec, table: &TableDef, source: &ColumnSource) -> ColumnType {
    let def = match source {
        ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } => spec.element_def(segment, *element, *component),
        ColumnSource::SegmentIndex { .. } => return ColumnType::Int64 { scale: 0 },
        ColumnSource::GroupElement { offset, component } => match (&table.segment, table.repeat) {
            (Some(segment), Some(repeat)) => repeat
                .from
                .checked_add(*offset)
                .and_then(|position| spec.element_def(segment, position, *component)),
            _ => None,
        },
    };
    ColumnType::of(def.map(|def| def.kind))
}

/// `true` when the segment has the id and every condition holds.
fn matches(segment: &Segment<'_>, id: &[u8], conditions: &[(usize, Vec<u8>)]) -> bool {
    segment.id == id
        && conditions.iter().all(|(position, value)| {
            segment.element(*position).and_then(Element::simple) == Some(value.as_slice())
        })
}

/// `true` when an element has a non-empty value or component.
fn has_content(element: &Element<'_>) -> bool {
    match element {
        Element::Simple(value) => !value.is_empty(),
        Element::Composite(parts) => parts.iter().any(|part| !part.is_empty()),
    }
}

/// The text of an element, or of one of its components. An element read
/// whole that the file split into components is written back into
/// `joined` with the component separator between them; component 1 of a
/// simple element is the element itself.
fn leaf_text<'a>(
    segment: &'a Segment<'_>,
    element: usize,
    component: Option<usize>,
    separator: u8,
    joined: &'a mut Vec<u8>,
) -> &'a [u8] {
    match (segment.element(element), component) {
        (None, _) | (Some(Element::Simple(_)), Some(2..)) => &[],
        (Some(Element::Simple(value)), _) => value,
        (Some(Element::Composite(parts)), Some(component)) => component
            .checked_sub(1)
            .and_then(|at| parts.get(at))
            .map_or(&[][..], |part| part.as_ref()),
        (Some(Element::Composite(parts)), None) => {
            joined.clear();
            for (at, part) in parts.iter().enumerate() {
                if at > 0 {
                    joined.push(separator);
                }
                joined.extend_from_slice(part);
            }
            joined
        }
    }
}

/// A non-empty text as a value of the column type; `None` when it does not parse.
fn parse(kind: ColumnType, text: &[u8]) -> Option<Parsed> {
    Some(match kind {
        ColumnType::Binary => Parsed::Text,
        ColumnType::Int64 { .. } => Parsed::Int(parse_n(text)?),
        ColumnType::Decimal128 { scale, .. } => Parsed::Decimal(parse_r(text, scale)?),
        ColumnType::Date32 => Parsed::Date(parse_dt(text)?),
        ColumnType::Time32 => Parsed::Time(parse_tm(text)?),
    })
}

/// Where a column reads inside a segment, and as what.
#[derive(Debug, Clone, Copy)]
struct Place {
    element: usize,
    component: Option<usize>,
    kind: ColumnType,
}

/// The column value at `at`: the checked value when the definition there
/// maps to the column's type, otherwise the text parsed as that type (an
/// element the spec does not define, or a group whose definition differs).
/// Text is copied into `bytes`.
fn read(
    checked: &[Checked],
    segment: &Segment<'_>,
    at: Place,
    separator: u8,
    joined: &mut Vec<u8>,
    bytes: &mut Vec<u8>,
) -> Slot {
    let known = checked
        .iter()
        .find(|c| c.element == at.element && c.component == at.component && c.kind == at.kind);
    let value = match known {
        Some(checked) => checked.value,
        None => {
            let text = leaf_text(segment, at.element, at.component, separator, joined);
            if text.is_empty() {
                Parsed::Null
            } else {
                parse(at.kind, text).unwrap_or(Parsed::Null)
            }
        }
    };
    match value {
        Parsed::Null => Slot::Null,
        Parsed::Text => {
            let text = leaf_text(segment, at.element, at.component, separator, joined);
            let start = bytes.len();
            bytes.extend_from_slice(text);
            Slot::Bytes(start, bytes.len())
        }
        Parsed::Int(value) => Slot::Int(value),
        Parsed::Decimal(value) => Slot::Decimal(value),
        Parsed::Date(value) => Slot::Date(value),
        Parsed::Time(value) => Slot::Time(value),
    }
}

/// Appends a collected row: its number, its anchor segment, the rows above
/// it, then its columns; a column no segment matched is null.
fn append(table: &mut Table, row: &Row) {
    let index = |value: usize| i64::try_from(value).map_or(Cell::Null, Cell::Int64);
    let mut cells = Vec::with_capacity(2 + row.parents.len() + row.cells.len());
    cells.push(index(row.ordinal));
    cells.push(index(row.segment));
    cells.extend(
        row.parents
            .iter()
            .map(|parent| parent.map_or(Cell::Null, index)),
    );
    cells.extend(row.cells.iter().map(|slot| match *slot {
        Slot::Unset | Slot::Null => Cell::Null,
        Slot::Bytes(start, end) => row.bytes.get(start..end).map_or(Cell::Null, Cell::Binary),
        Slot::Int(value) => Cell::Int64(value),
        Slot::Decimal(value) => Cell::Decimal128(value),
        Slot::Date(value) => Cell::Date32(value),
        Slot::Time(value) => Cell::Time32(value),
    }));
    if table.push_row(&cells).is_err() {
        // Every cell has its column's type, so only text can fail to fit (a
        // binary column past what i32 offsets address): keep the row, with
        // those cells null, so row numbers stay aligned. Nulls always fit.
        for cell in &mut cells {
            if matches!(cell, Cell::Binary(_)) {
                *cell = Cell::Null;
            }
        }
        let _ = table.push_row(&cells);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib project::`
Expected: 18 passed (two of them property tests).

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 285 tests in the workspace.

Run: `grep -nE '"(ST|SE|GS|GE|ISA|IEA|CLP|SVC|CAS|PLB|BPR|NM1|REF|DTM)"|b"(ST|SE|CLP|SVC|CAS|PLB)"' crates/edi835_core/src/project.rs`
Expected: no output (the module's tests use an invented spec).

```bash
git add crates/edi835_core/src/project.rs crates/edi835_core/src/lib.rs
git commit -m "project: Projector checks every defined element and fills the spec's tables

One row per anchor instance (numbered at open, appended at close) or per
anchored segment and element group; each row names the open row of every
table above it. Elements are parsed once: the check's value fills the
column. Required, type, length and composite-shape findings carry the
segment, element, component, path and datum."
```

---

## Task 6: `Processor` — one pass, every consumer

**Implementer tier:** Sonnet — a small module plus an integration test over the eleven files; all code is given, and the only judgment is reading a failure of the incremental test correctly if one appears.

**Files:**
- Create: `crates/edi835_core/src/process.rs`
- Modify: `crates/edi835_core/src/lib.rs`
- Modify: `crates/edi835_core/tests/common/mod.rs`
- Create: `crates/edi835_core/tests/project_files.rs`

**Interfaces:**
- Consumes: `LoopEngine`, `EnvelopeChecker`, `Projector` (Task 5), `Document::segments`, `Document::delimiters`, `Tables`.
- Produces:
  - `pub struct Output` (`Debug`, `Clone`, `Default`, `PartialEq`, `Eq`) with `events(&self) -> &[Event]` and `diagnostics(&self) -> &[Diagnostic]` (the checker's, then the projector's).
  - `pub struct Processor<'s>` (`Debug`, `Clone`): `new(spec: &'s Spec, delimiters: &Delimiters)`, `feed(&mut self, &Segment<'_>) -> &Output`, `finish(&mut self) -> &Output`, `diagnostics(&self) -> &[Diagnostic]` (those of the latest `feed` or `finish`), `take_tables(&mut self) -> Tables`, and `Processor::run(spec: &Spec, document: &Document<'_>) -> (Tables, Vec<Diagnostic>)`, which is `new` + `feed` over every segment + `finish` + `take_tables`.
  - `tests/common`: `table_header(&Table) -> String` (`name: type | …`) and `table_rows(&Table) -> Vec<String>` (cells rendered with `ColumnData::render`, joined by ` | `).

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/process.rs` with the module doc, the imports and the tests only:

```rust
//! One pass over a segment stream with every consumer of the engine's events.
//!
//! The processor feeds each segment to the loop engine and hands the events
//! it returns to the envelope checker and to the projector, so the loop
//! structure, the structural and element diagnostics and the table rows all
//! come from the same walk. [`Processor::run`] does the same over a whole
//! document and is the same code path.

use crate::check::EnvelopeChecker;
use crate::column::Tables;
use crate::delimiters::Delimiters;
use crate::diagnostic::Diagnostic;
use crate::document::Document;
use crate::engine::{Event, LoopEngine};
use crate::project::Projector;
use crate::segment::Segment;
use crate::spec::Spec;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SnipLevel, Tokenizer};

    const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240101*1200*^*00501*000000001*0*P*>~";

    fn delimiters() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    /// A complete interchange holding the given transactions, each written
    /// between its `ST` and its `SE`.
    fn interchange(transactions: &[&str]) -> String {
        let mut text = format!("{ISA}GS*HP*SENDER*RECEIVER*20240101*1200*7*X*005010X221A1~");
        for (i, body) in transactions.iter().enumerate() {
            let count = body.matches('~').count() + 2;
            text.push_str(&format!(
                "ST*835*{:04}~{body}SE*{count}*{:04}~",
                i + 1,
                i + 1
            ));
        }
        text.push_str(&format!("GE*{}*7~IEA*1*000000001~", transactions.len()));
        text
    }

    const CLAIM: &str = "BPR*I*10*C*CHK~TRN*1*1~LX*1~CLP*C1*1*10*10~SVC*HC:99213*10*10~";

    #[test]
    fn one_feed_returns_the_events_and_every_diagnostic_of_its_segment() {
        let spec = Spec::builtin_835();
        let mut processor = Processor::new(&spec, &delimiters());
        let input = "ST*835*0001~CLP*C1*1*12A*0~";
        let segments: Vec<_> = Tokenizer::with_delimiters(input.as_bytes(), delimiters()).collect();
        processor.feed(&segments[0]);
        let output = processor.feed(&segments[1]);
        let names: Vec<String> = output
            .events()
            .iter()
            .map(|event| match *event {
                Event::LoopOpened { id, implicit, .. } => {
                    format!(
                        "open{} {}",
                        if implicit { "!" } else { "" },
                        spec.loop_name(id)
                    )
                }
                Event::Captured { id, segment } => format!("cap {} #{segment}", spec.loop_name(id)),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(names, vec!["open! 2000", "open 2100", "cap 2100 #1"]);
        let levels: Vec<SnipLevel> = output.diagnostics().iter().map(|d| d.level).collect();
        assert_eq!(levels, vec![SnipLevel::L1, SnipLevel::L2]);
        assert_eq!(
            output.diagnostics()[1].to_string(),
            "SNIP 2 · element CLP03 (total_claim_charge_amount) is not a valid R (decimal, scale 2) · segment #1, element 3 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum \"12A\""
        );
        let latest = output.diagnostics().to_vec();
        assert_eq!(processor.diagnostics(), latest);
    }

    #[test]
    fn finishing_reports_what_is_left_open_and_appends_its_rows() {
        let spec = Spec::builtin_835();
        let mut processor = Processor::new(&spec, &delimiters());
        let input = format!("{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~{CLAIM}");
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            processor.feed(&segment);
        }
        let output = processor.finish();
        assert_eq!(output.events().len(), 6, "six loops close");
        assert_eq!(
            output.diagnostics().len(),
            3,
            "three envelopes lack their end"
        );
        let tables = processor.take_tables();
        for name in ["payments", "claims", "services"] {
            assert_eq!(tables.get(name).unwrap().len(), 1, "{name}");
        }
    }

    #[test]
    fn run_over_a_document_is_feeding_every_segment_then_finishing() {
        let spec = Spec::builtin_835();
        let input = interchange(&[CLAIM, "BPR*I*1*C*CHK~TRN*1*2~ZZZ~"]);
        let document = Document::with_delimiters(input.as_bytes(), delimiters());
        let (tables, diagnostics) = Processor::run(&spec, &document);
        let mut processor = Processor::new(&spec, &delimiters());
        let mut by_hand = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            by_hand.extend_from_slice(processor.feed(&segment).diagnostics());
        }
        by_hand.extend_from_slice(processor.finish().diagnostics());
        assert_eq!(diagnostics, by_hand);
        assert_eq!(tables, processor.take_tables());
        assert_eq!(diagnostics.len(), 1, "the unknown ZZZ");
        assert_eq!(tables.get("payments").unwrap().len(), 2);
    }

    #[test]
    fn tables_drained_after_each_transaction_add_up_to_one_run() {
        let spec = Spec::builtin_835();
        let input = interchange(&[CLAIM, CLAIM, CLAIM]);
        let transaction = spec.loop_id("transaction").unwrap();
        let mut processor = Processor::new(&spec, &delimiters());
        let mut drained = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let closes = processor
                .feed(&segment)
                .events()
                .contains(&Event::LoopClosed { id: transaction });
            if closes {
                drained.push(processor.take_tables());
            }
        }
        processor.finish();
        drained.push(processor.take_tables());
        let document = Document::with_delimiters(input.as_bytes(), delimiters());
        let (whole, _) = Processor::run(&spec, &document);
        for table in &whole {
            let rows = |tables: &Tables| -> Vec<String> {
                let part = tables.get(table.name()).unwrap();
                (0..part.len())
                    .map(|row| {
                        part.columns()
                            .iter()
                            .map(|(_, column)| column.render(row).unwrap())
                            .collect::<Vec<_>>()
                            .join("|")
                    })
                    .collect()
            };
            let pieces: Vec<String> = drained.iter().flat_map(rows).collect();
            assert_eq!(pieces, rows(&whole), "{}", table.name());
        }
        assert_eq!(
            drained
                .iter()
                .map(|t| t.get("claims").unwrap().len())
                .collect::<Vec<_>>(),
            vec![1, 1, 1, 0]
        );
    }
}
```

In `crates/edi835_core/src/lib.rs`, add `pub mod process;` after `pub mod frame;`, and `pub use process::{Output, Processor};` before `pub use project::Projector;`.

Append to `crates/edi835_core/tests/common/mod.rs`:

```rust
/// A table's header line: every column as `name: type`, separated by ` | `.
pub fn table_header(table: &edi835_core::Table) -> String {
    table
        .columns()
        .iter()
        .map(|(name, column)| format!("{name}: {}", column.kind()))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Every row of a table as one line, cells rendered by
/// `ColumnData::render` and separated by ` | `.
pub fn table_rows(table: &edi835_core::Table) -> Vec<String> {
    (0..table.len())
        .map(|row| {
            table
                .columns()
                .iter()
                .map(|(_, column)| column.render(row).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect()
}
```

Create `crates/edi835_core/tests/project_files.rs`:

```rust
//! Tables and diagnostics of the processor over every real-shaped file.

mod common;

use edi835_core::{Document, Event, Processor, Spec, Tokenizer};

/// Fed segment by segment from a tokenizer and drained after each
/// transaction, the processor gives the same rows and diagnostics as one run
/// over the whole document.
#[test]
fn draining_after_each_transaction_adds_up_to_one_run_over_the_document() {
    let spec = Spec::builtin_835();
    let transaction = spec.loop_id("transaction").unwrap();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (whole, diagnostics) = Processor::run(&spec, &document);

        let mut processor = Processor::new(&spec, &delims);
        let mut drained = Vec::new();
        let mut incremental = Vec::new();
        for segment in Tokenizer::with_delimiters(&bytes, delims) {
            let output = processor.feed(&segment);
            incremental.extend_from_slice(output.diagnostics());
            if output
                .events()
                .contains(&Event::LoopClosed { id: transaction })
            {
                drained.push(processor.take_tables());
            }
        }
        incremental.extend_from_slice(processor.finish().diagnostics());
        drained.push(processor.take_tables());

        assert_eq!(incremental, diagnostics, "{name}");
        assert_eq!(whole.len(), 5, "{name}");
        for table in &whole {
            let pieces: Vec<String> = drained
                .iter()
                .flat_map(|tables| common::table_rows(tables.get(table.name()).unwrap()))
                .collect();
            assert_eq!(
                pieces,
                common::table_rows(table),
                "{name}: {}",
                table.name()
            );
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib process:: && cargo test -p edi835_core --test project_files`
Expected: compile errors (`cannot find struct 'Processor'`, `'Output'`).

- [ ] **Step 3: Implement the module**

Insert between `use crate::spec::Spec;` and `#[cfg(test)]`:

```rust
/// What one segment, or the end of the stream, produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    events: Vec<Event>,
    diagnostics: Vec<Diagnostic>,
}

impl Output {
    /// The engine's events, in order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The envelope checker's diagnostics, then the projector's.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// The loop engine, the envelope checker and the projector, fed together.
#[derive(Debug, Clone)]
pub struct Processor<'s> {
    engine: LoopEngine<'s>,
    checker: EnvelopeChecker<'s>,
    projector: Projector<'s>,
    output: Output,
}

impl<'s> Processor<'s> {
    /// A processor at the root with empty tables; `delimiters` are those the
    /// segments were read with.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        Self {
            engine: LoopEngine::new(spec),
            checker: EnvelopeChecker::new(spec),
            projector: Projector::new(spec, delimiters),
            output: Output::default(),
        }
    }

    /// Consumes one segment. The output is valid until the next call.
    pub fn feed(&mut self, segment: &Segment<'_>) -> &Output {
        self.output.events.clear();
        self.output.diagnostics.clear();
        let events = self.engine.feed(segment);
        self.output.events.extend_from_slice(events);
        self.output
            .diagnostics
            .extend_from_slice(self.checker.on(segment, events));
        self.output
            .diagnostics
            .extend_from_slice(self.projector.on(segment, events));
        &self.output
    }

    /// Closes every loop still open and appends the rows they were
    /// collecting. The processor is then back at the root, as a new one;
    /// take the tables before feeding it another stream.
    pub fn finish(&mut self) -> &Output {
        self.output.events.clear();
        self.output.diagnostics.clear();
        self.output.events.extend_from_slice(self.engine.finish());
        self.output
            .diagnostics
            .extend_from_slice(self.checker.finish());
        self.output
            .diagnostics
            .extend_from_slice(self.projector.finish());
        &self.output
    }

    /// The diagnostics of the latest `feed` or `finish`.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.output.diagnostics()
    }

    /// Moves every appended row out; see [`Projector::take_tables`].
    pub fn take_tables(&mut self) -> Tables {
        self.projector.take_tables()
    }

    /// Processes a whole document: every segment, then `finish`. Returns
    /// the tables and every diagnostic in stream order.
    pub fn run(spec: &Spec, document: &Document<'_>) -> (Tables, Vec<Diagnostic>) {
        let mut processor = Processor::new(spec, document.delimiters());
        let mut diagnostics = Vec::new();
        for segment in document.segments() {
            diagnostics.extend_from_slice(processor.feed(&segment).diagnostics());
        }
        diagnostics.extend_from_slice(processor.finish().diagnostics());
        (processor.take_tables(), diagnostics)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib process:: && cargo test -p edi835_core --test project_files`
Expected: 4 passed, then 1 passed.

- [ ] **Step 5: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 290 tests in the workspace.

```bash
git add crates/edi835_core/src/process.rs crates/edi835_core/src/lib.rs crates/edi835_core/tests/common/mod.rs crates/edi835_core/tests/project_files.rs
git commit -m "process: Processor feeds the engine, the checker and the projector together

feed returns the segment's events and every diagnostic it raised; run
does the same over a Document. Draining the tables after each
transaction gives the rows and diagnostics of one run, on all eleven
files."
```

---

## Task 7: Real files — table and diagnostic goldens, invariants, extension by data

**Implementer tier:** Opus — twenty-two new golden files must be generated, read and judged against the facts table before they are committed, the shared golden helper changes a working test, and the invariants tie the tables to the loop tree over all eleven files.

**Files:**
- Modify: `crates/edi835_core/tests/common/mod.rs`
- Modify: `crates/edi835_core/tests/engine_golden.rs`
- Create: `crates/edi835_core/tests/project_golden.rs`
- Modify: `crates/edi835_core/tests/project_files.rs`
- Create: `crates/edi835_core/tests/golden/project/*.txt` (22 files, generated by the test)

**Interfaces:**
- Consumes: `Processor::run` (Task 6), `common::table_header`/`table_rows` (Task 6), `LoopTree`, `Node::opened_by`, `Spec::tables`.
- Produces:
  - `tests/common`: `SUMMARY_ONLY` (the two large samples), `describe_diff` (moved from `engine_golden.rs`, unchanged), `compare_goldens(dir, &[(PathBuf, String)]) -> Vec<String>` (compare, or write under `UPDATE_GOLDEN=1`, and report files directly in `dir` that nothing compares).
  - Golden files in `tests/golden/project/`: `<file>.tables.txt` for the nine small files (per table: `## <name> (rows: <n>)`, the header line, one line per row, a blank line), `<file>.tables.summary.txt` for the two large ones (`<name> rows: <n>` per table), `<file>.diagnostics.txt` for all eleven (one `Display` line per diagnostic, empty when there is none). The subdirectory keeps them out of `engine_golden`'s orphan check, which only looks at files directly in `tests/golden`.
  - The `.events.txt` and `.summary.txt` goldens do not change.

The invariant tests pass as soon as they are written: they verify the code of Tasks 5–6 against the real files, as the envelope test of the checker did.

- [ ] **Step 1: Share the golden helpers**

In `crates/edi835_core/tests/common/mod.rs`, replace `use std::path::PathBuf;` with:

```rust
use std::path::{Path, PathBuf};

/// The two large samples, whose goldens keep a summary instead of every line.
pub const SUMMARY_ONLY: &[&str] = &["edi835_test_united.rmt", "edi835_test_versant.RMT"];
```

and append:

```rust
/// Human-readable location of the first difference between two line streams.
pub fn describe_diff(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();

    for (i, (a, e)) in actual_lines.iter().zip(expected_lines.iter()).enumerate() {
        if a != e {
            return format!("line {}: actual {:?}, expected {:?}", i + 1, a, e);
        }
    }

    match actual_lines.len().cmp(&expected_lines.len()) {
        std::cmp::Ordering::Greater => {
            let extra = actual_lines.len() - expected_lines.len();
            let first_extra = actual_lines[expected_lines.len()];
            format!(
                "expected ends at line {}; actual has {} extra line(s), first: {:?}",
                expected_lines.len(),
                extra,
                first_extra
            )
        }
        std::cmp::Ordering::Less => {
            let extra = expected_lines.len() - actual_lines.len();
            let first_extra = expected_lines[actual_lines.len()];
            format!(
                "actual ends at line {}; expected has {} more line(s), first: {:?}",
                actual_lines.len(),
                extra,
                first_extra
            )
        }
        std::cmp::Ordering::Equal => "no difference".to_string(),
    }
}

/// Compares each `(path, actual)` pair with the committed file at `path`, or
/// writes `actual` there when `UPDATE_GOLDEN=1`, then reports every file
/// directly in `dir` that no pair names. Returns one message per failure.
pub fn compare_goldens(dir: &Path, outputs: &[(PathBuf, String)]) -> Vec<String> {
    let update = std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1");
    let mut failures = Vec::new();
    for (path, actual) in outputs {
        if update {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(path, actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with UPDATE_GOLDEN=1 to create it",
                path.display()
            )
        });
        if *actual != expected {
            failures.push(format!(
                "{}: {}",
                path.display(),
                describe_diff(actual, &expected)
            ));
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !outputs.iter().any(|(named, _)| *named == path) {
                failures.push(format!(
                    "orphaned golden file with no test that compares it: {}",
                    path.display()
                ));
            }
        }
    }
    failures
}
```

Replace `crates/edi835_core/tests/engine_golden.rs` with this version, which uses the shared helpers (the stream and summary formats do not change):

```rust
//! Event streams compared against committed golden files. Small files keep
//! the full stream; the two large samples keep a count summary. Regenerate
//! with `UPDATE_GOLDEN=1 cargo test --test engine_golden`, inspect the diff,
//! commit. Goldens of other suites live in subdirectories, which this one
//! leaves alone.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Event, Spec, Tokenizer};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn line(spec: &Spec, event: Event, segment_id: &[u8]) -> String {
    let id = String::from_utf8_lossy(segment_id);
    match event {
        Event::LoopOpened {
            id: l,
            implicit,
            segment,
        } => format!(
            "open{} {} #{segment}",
            if implicit { "!" } else { "" },
            spec.loop_name(l)
        ),
        Event::LoopClosed { id: l } => format!("close {}", spec.loop_name(l)),
        Event::Captured { id: l, segment } => format!("cap {} {id} #{segment}", spec.loop_name(l)),
        Event::Unmatched { segment } => format!("unmatched {id} #{segment}"),
        Event::Empty { segment } => format!("empty #{segment}"),
    }
}

fn full_stream(spec: &Spec, bytes: &[u8], delims: edi835_core::Delimiters) -> String {
    let mut out = String::new();
    let segments: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    let events = common::run_engine(spec, segments.iter().cloned());
    for event in events {
        let segment_id = match event {
            edi835_core::Event::Captured { segment, .. }
            | edi835_core::Event::Unmatched { segment }
            | edi835_core::Event::Empty { segment } => {
                segments.get(segment).map(|s| s.id).unwrap_or(b"")
            }
            _ => b"",
        };
        let _ = writeln!(out, "{}", line(spec, event, segment_id));
    }
    out
}

fn summary(spec: &Spec, bytes: &[u8], delims: edi835_core::Delimiters) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let segments: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    let events = common::run_engine(spec, segments.iter().cloned());
    for event in events {
        let segment_id = match event {
            edi835_core::Event::Captured { segment, .. }
            | edi835_core::Event::Unmatched { segment }
            | edi835_core::Event::Empty { segment } => {
                segments.get(segment).map(|s| s.id).unwrap_or(b"")
            }
            _ => b"",
        };
        let key = line(spec, event, segment_id);
        let key = key
            .rsplit_once(" #")
            .map(|(k, _)| k.to_string())
            .unwrap_or(key);
        *counts.entry(key).or_default() += 1;
    }
    counts.into_iter().fold(String::new(), |mut out, (key, n)| {
        let _ = writeln!(out, "{key} x{n}");
        out
    })
}

#[test]
fn event_streams_match_the_golden_files() {
    let spec = Spec::builtin_835();
    let mut outputs = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        outputs.push(if common::SUMMARY_ONLY.contains(&name.as_str()) {
            (
                golden_dir().join(format!("{name}.summary.txt")),
                summary(&spec, &bytes, delims),
            )
        } else {
            (
                golden_dir().join(format!("{name}.events.txt")),
                full_stream(&spec, &bytes, delims),
            )
        });
    }
    let failures = common::compare_goldens(&golden_dir(), &outputs);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn describe_diff_reports_content_drift() {
    let actual = "line 1\nline 2a\nline 3";
    let expected = "line 1\nline 2b\nline 3";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(diff, r#"line 2: actual "line 2a", expected "line 2b""#);
}

#[test]
fn describe_diff_reports_when_actual_is_longer() {
    let actual = "line 1\nline 2\nline 3 extra";
    let expected = "line 1\nline 2";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"expected ends at line 2; actual has 1 extra line(s), first: "line 3 extra""#
    );
}

#[test]
fn describe_diff_reports_when_expected_is_longer() {
    let actual = "line 1\nline 2";
    let expected = "line 1\nline 2\nline 3 missing";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"actual ends at line 2; expected has 1 more line(s), first: "line 3 missing""#
    );
}
```

Run: `cargo test -p edi835_core --test engine_golden`
Expected: 4 passed; `git status --short crates/edi835_core/tests/golden` prints nothing.

- [ ] **Step 2: Write the golden test**

Create `crates/edi835_core/tests/project_golden.rs`:

```rust
//! Tables and diagnostics compared against committed golden files in
//! `tests/golden/project/`. Small files keep every row of every table; the
//! two large samples keep a row count per table. Every file keeps its whole
//! diagnostic list, one `Display` line each. Regenerate with
//! `UPDATE_GOLDEN=1 cargo test --test project_golden`, inspect the diff,
//! commit.

mod common;

use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Diagnostic, Document, Processor, Spec, Tables};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/project")
}

/// Every table: a title with its row count, the header, one line per row.
fn all_rows(tables: &Tables) -> String {
    let mut out = String::new();
    for table in tables {
        let _ = writeln!(out, "## {} (rows: {})", table.name(), table.len());
        let _ = writeln!(out, "{}", common::table_header(table));
        for row in common::table_rows(table) {
            let _ = writeln!(out, "{row}");
        }
        out.push('\n');
    }
    out
}

/// One line per table with its row count.
fn row_counts(tables: &Tables) -> String {
    tables.iter().fold(String::new(), |mut out, table| {
        let _ = writeln!(out, "{} rows: {}", table.name(), table.len());
        out
    })
}

fn diagnostic_lines(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .fold(String::new(), |mut out, diagnostic| {
            let _ = writeln!(out, "{diagnostic}");
            out
        })
}

#[test]
fn tables_and_diagnostics_match_the_golden_files() {
    let spec = Spec::builtin_835();
    let mut outputs = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, diagnostics) = Processor::run(&spec, &document);
        outputs.push(if common::SUMMARY_ONLY.contains(&name.as_str()) {
            (
                golden_dir().join(format!("{name}.tables.summary.txt")),
                row_counts(&tables),
            )
        } else {
            (
                golden_dir().join(format!("{name}.tables.txt")),
                all_rows(&tables),
            )
        });
        outputs.push((
            golden_dir().join(format!("{name}.diagnostics.txt")),
            diagnostic_lines(&diagnostics),
        ));
    }
    let failures = common::compare_goldens(&golden_dir(), &outputs);
    assert!(failures.is_empty(), "{failures:#?}");
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p edi835_core --test project_golden`
Expected: FAIL — `…/tests/golden/project/emedny_sample.txt.tables.txt: No such file or directory (os error 2); run with UPDATE_GOLDEN=1 to create it`.

- [ ] **Step 4: Generate the goldens and inspect them**

Run: `UPDATE_GOLDEN=1 cargo test -p edi835_core --test project_golden`
Then run:

```bash
git status --short crates/edi835_core/tests/golden
ls crates/edi835_core/tests/golden/project | wc -l
for f in crates/edi835_core/tests/golden/project/*.diagnostics.txt; do printf '%5d %s\n' "$(wc -l < "$f")" "$(basename "$f")"; done
grep -c 'SNIP 2' crates/edi835_core/tests/golden/project/*.diagnostics.txt
```

Expected: `git status` lists only `crates/edi835_core/tests/golden/project/` as untracked (no `.events.txt` or `.summary.txt` changes); 22 files; diagnostic line counts 6 blue_cross, 0 davisvision, 0 eyemed, 1 edi835_test_file, 1 not_available_claim_id, 0 united, 0 versant, 0 emedny, 20 multi_claim, 7 trizetto, 0 united_healthcare_legacy; `SNIP 2` counts 3 blue_cross, 16 multi_claim, 5 trizetto, 0 elsewhere — the facts table at the top of this plan. Read every non-empty diagnostics file against that table; their content must be exactly:

`blue_cross_nc_sample.txt.diagnostics.txt`:

```text
SNIP 1 · loop "interchange" opened without its own trigger ("ISA" with no conditions) to hold segment "ST" · segment #0 · at interchange#1 · datum "ST"
SNIP 1 · loop "group" opened without its own trigger ("GS" with no conditions) to hold segment "ST" · segment #0 · at interchange#1/group#1 · datum "ST"
SNIP 2 · element TRN03 (payer_identifier) has length 9; the spec allows 10 to 10 · segment #2, element 3 · at interchange#1/group#1/transaction#1 · datum "560894904"
SNIP 2 · element PER03 (communication_number_qualifier) has length 10; the spec allows 2 to 2 · segment #8, element 3 · at interchange#1/group#1/transaction#1/1000A#1 · datum "8005554844"
SNIP 2 · required element SVC02 (line_item_charge_amount) is missing or empty · segment #27, element 2 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#3 · datum ""
SNIP 1 · SE01 declares "33" but the count is 32 · segment #31, element 1 · at interchange#1/group#1/transaction#1 · datum "33"
```

`multi_claim_sample.txt.diagnostics.txt`:

```text
SNIP 2 · element ISA06 (interchange_sender_id) has length 14; the spec allows 15 to 15 · segment #0, element 6 · at interchange#1 · datum "RUSHMORE      "
SNIP 2 · element BPR10 (originating_company_identifier) has length 9; the spec allows 10 to 10 · segment #3, element 10 · at interchange#1/group#1/transaction#1 · datum "111333555"
SNIP 2 · element BPR17 (business_function_code) has length 8; the spec allows 1 to 3 · segment #3, element 17 · at interchange#1/group#1/transaction#1 · datum "20190316"
SNIP 2 · element PER03 (communication_number_qualifier) has length 10; the spec allows 2 to 2 · segment #10, element 3 · at interchange#1/group#1/transaction#1/1000A#1 · datum "8002144844"
SNIP 1 · segment "N3" is not part of the structure: no open loop holds it and it opens no loop · segment #19 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum "N3"
SNIP 1 · segment "N4" is not part of the structure: no open loop holds it and it opens no loop · segment #20 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum "N4"
SNIP 2 · element DTM01 (date_time_qualifier) has length 2; the spec allows 3 to 3 · segment #24, element 1 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum "50"
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #25, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#1 · datum "HC:99213"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #25, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#1 · datum ""
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #27, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#2 · datum "HC:99214"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #27, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#2 · datum ""
SNIP 1 · segment "N3" is not part of the structure: no open loop holds it and it opens no loop · segment #34 · at interchange#1/group#1/transaction#1/2000#2/2100#2 · datum "N3"
SNIP 1 · segment "N4" is not part of the structure: no open loop holds it and it opens no loop · segment #35 · at interchange#1/group#1/transaction#1/2000#2/2100#2 · datum "N4"
SNIP 2 · element DTM01 (date_time_qualifier) has length 2; the spec allows 3 to 3 · segment #39, element 1 · at interchange#1/group#1/transaction#1/2000#2/2100#2 · datum "50"
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #40, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#3 · datum "HC:99213"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #40, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#3 · datum ""
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #43, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#4 · datum "HC:99214"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #43, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#4 · datum ""
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #46, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#5 · datum "HC:99215"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #46, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#2/2100#2/2110#5 · datum ""
```

`trizetto_sample.rmt.diagnostics.txt`:

```text
SNIP 2 · element ISA06 (interchange_sender_id) has length 13; the spec allows 15 to 15 · segment #0, element 6 · at interchange#1 · datum "SENDER       "
SNIP 2 · element ISA08 (interchange_receiver_id) has length 13; the spec allows 15 to 15 · segment #0, element 8 · at interchange#1 · datum "RECEIVER     "
SNIP 1 · segment "XX" is not part of the structure: no open loop holds it and it opens no loop · segment #7 · at interchange#1/group#1/transaction#1/1000A#1 · datum "XX"
SNIP 2 · element NM108 (identification_code_qualifier) has length 10; the spec allows 1 to 2 · segment #14, element 8 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum "666666666A"
SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) has length 8; the spec allows 2 to 2 · segment #17, element 1, component 1 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#1 · datum "HC:99213"
SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty · segment #17, element 1, component 2 · at interchange#1/group#1/transaction#1/2000#1/2100#1/2110#1 · datum ""
SNIP 1 · SE01 declares "15" but the count is 18 · segment #19, element 1 · at interchange#1/group#1/transaction#1 · datum "15"
```

`edi835_test_file.RMT.diagnostics.txt` and `edi835_test_not_available_claim_id.RMT.diagnostics.txt`:

```text
SNIP 1 · SE01 declares "1202" but the count is 76 · segment #77, element 1 · at interchange#1/group#1/transaction#1 · datum "1202"
SNIP 1 · SE01 declares "302" but the count is 255 · segment #256, element 1 · at interchange#1/group#1/transaction#1 · datum "302"
```

The two summaries:

```text
# edi835_test_united.rmt.tables.summary.txt
adjustments rows: 2370
claims rows: 1332
payments rows: 1
provider_adjustments rows: 0
services rows: 6192
# edi835_test_versant.RMT.tables.summary.txt
adjustments rows: 783
claims rows: 648
payments rows: 1
provider_adjustments rows: 3
services rows: 1778
```

Spot check one full table file: `emedny_sample.txt.tables.txt` must read exactly:

```text
## adjustments (rows: 4)
row: int64 | segment: int64 | payment: int64 | claim: int64 | service: int64 | amount: decimal128(38, 2) | group_code: binary | quantity: decimal128(38, 2) | reason_code: binary
0 | 41 | 0 | 1 | 4 | 12.00 | CO | ∅ | 29
1 | 44 | 0 | 1 | 5 | 22.00 | CO | ∅ | 29
2 | 60 | 0 | 2 | 8 | 2.75 | CO | ∅ | 251
3 | 64 | 0 | 2 | 9 | 20.00 | CO | ∅ | 251

## claims (rows: 3)
row: int64 | segment: int64 | payment: int64 | charge_amount: decimal128(38, 2) | claim_id: binary | claim_status: binary | coverage_amount: decimal128(38, 2) | discharge_fraction: decimal128(38, 4) | drg_code: binary | drg_weight: decimal128(38, 4) | facility_type: binary | filing_indicator: binary | frequency_code: binary | group_number: binary | patient_first_name: binary | patient_id: binary | patient_last_name: binary | patient_responsibility: decimal128(38, 2) | payer_control_number: binary | payment_amount: decimal128(38, 2) | rendering_provider_id: binary | rendering_provider_name: binary | statement_from: date32 | statement_to: date32
0 | 14 | 0 | 34.25 | PATIENT ACCOUNT NUMBER | 1 | 34.25 | ∅ | ∅ | ∅ | 11 | MC | ∅ | ∅ | SUBMITTED FIRST | LL99999L | SUBMITTED LAST | ∅ | 1000210000000030 | 34.25 | ∅ | ∅ | 2010-01-01 | 2010-01-01
1 | 33 | 0 | 34.00 | PATIENT ACCOUNT NUMBER | 2 | ∅ | ∅ | ∅ | ∅ | 11 | MC | ∅ | ∅ | SUBMITTED FIRST | LL88888L | SUBMITTED LAST | ∅ | 1000220000000020 | 0.00 | ∅ | ∅ | 2010-01-01 | 2010-01-01
2 | 45 | 0 | 34.25 | PATIENT ACCOUNT NUMBER | 2 | 11.50 | ∅ | ∅ | ∅ | 11 | MC | ∅ | ∅ | SUBMITTED FIRST | LL77777L | SUBMITTED LAST | ∅ | 1000230000000020 | 11.50 | ∅ | ∅ | 2010-01-01 | 2010-01-01

## payments (rows: 1)
row: int64 | segment: int64 | credit_debit_flag: binary | handling_code: binary | payee_id: binary | payee_name: binary | payer_identifier: binary | payer_name: binary | payment_date: date32 | payment_format: binary | payment_method: binary | production_date: date32 | receiver_id: binary | total_payment_amount: decimal128(38, 2) | trace_number: binary
0 | 2 | C | I | 9999999995 | MAJOR MEDICAL PROVIDER | 1000000000 | NYSDOH | 2010-01-01 | CCP | ACH | 2010-01-01 | ETIN | 45.75 | 10100000000

## provider_adjustments (rows: 0)
row: int64 | segment: int64 | payment: int64 | amount: decimal128(38, 2) | fiscal_period_date: date32 | provider_id: binary | reason_code: binary | reference_id: binary

## services (rows: 10)
row: int64 | segment: int64 | payment: int64 | claim: int64 | allowed_amount: decimal128(38, 2) | charge_amount: decimal128(38, 2) | line_item_control_number: binary | original_units: decimal128(38, 2) | payment_amount: decimal128(38, 2) | procedure_code: binary | procedure_qualifier: binary | service_date: date32 | units_paid: decimal128(38, 2)
0 | 21 | 0 | 0 | 6.00 | 6.00 | ∅ | ∅ | 6.00 | V2020 | HC | 2010-01-01 | 1.00
1 | 24 | 0 | 0 | 2.75 | 2.75 | ∅ | ∅ | 2.75 | V2700 | HC | 2010-01-01 | 1.00
2 | 27 | 0 | 0 | 5.50 | 5.50 | ∅ | ∅ | 5.50 | V2103 | HC | 2010-01-01 | 1.00
3 | 30 | 0 | 0 | 20.00 | 20.00 | ∅ | ∅ | 20.00 | S0580 | HC | 2010-01-01 | 2.00
4 | 39 | 0 | 1 | ∅ | 12.00 | ∅ | ∅ | 0.00 | V2020 | HC | 2010-01-01 | 0.00
5 | 42 | 0 | 1 | ∅ | 22.00 | ∅ | ∅ | 0.00 | V2103 | HC | 2010-01-01 | 0.00
6 | 52 | 0 | 2 | 6.00 | 6.00 | ∅ | ∅ | 6.00 | V2020 | HC | 2010-01-01 | 1.00
7 | 55 | 0 | 2 | 5.50 | 5.50 | ∅ | ∅ | 5.50 | V2103 | HC | 2013-09-17 | 1.00
8 | 58 | 0 | 2 | ∅ | 2.75 | ∅ | ∅ | 0.00 | V2700 | HC | 2010-01-01 | 0.00
9 | 62 | 0 | 2 | ∅ | 20.00 | ∅ | ∅ | 0.00 | S0580 | HC | 2010-01-01 | 0.00
```

In every `.tables.txt`, check by eye that each `claims` row's `payment` is a valid `payments` row, each `services` row's `claim` points at a claim whose `segment` precedes the service's, and the `adjustments` rows of a service carry that service's row in `service` while claim-level ones show `∅` there. Then:

Run: `cargo test -p edi835_core --test project_golden`
Expected: 1 passed.

- [ ] **Step 5: Add the invariants and the extension test**

In `crates/edi835_core/tests/project_files.rs`, replace the `use edi835_core::{…};` line with:

```rust
use std::collections::BTreeMap;

use edi835_core::{
    Cell, Column, Document, Element, Event, LoopId, LoopTree, Processor, Segment, SnipLevel, Spec,
    Table, Tokenizer,
};
```

and append:

```rust
/// The cell of a row-index column as a number; `None` when null.
fn index_at(table: &Table, column: &str, row: usize) -> Option<usize> {
    match table.column(column).and_then(|data| data.get(row)) {
        Some(Cell::Int64(value)) => usize::try_from(value).ok(),
        _ => None,
    }
}

/// `true` when an element has a non-empty value or component.
fn has_content(element: &Element<'_>) -> bool {
    match element {
        Element::Simple(value) => !value.is_empty(),
        Element::Composite(parts) => parts.iter().any(|part| !part.is_empty()),
    }
}

/// Element groups of a segment that start with content, from `from` every `step`.
fn groups(segment: &Segment<'_>, from: usize, step: usize) -> usize {
    (from..=segment.elements.len())
        .step_by(step)
        .filter(|&position| segment.element(position).is_some_and(has_content))
        .count()
}

/// One payment per transaction, one claim per CLP, one service per SVC, one
/// adjustment per CAS group with a reason code and one provider adjustment
/// per PLB group with an adjustment identifier.
#[test]
fn row_counts_follow_the_segments_of_each_file() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let segments: Vec<Segment<'_>> = Tokenizer::with_delimiters(&bytes, delims).collect();
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, _) = Processor::run(&spec, &document);
        let rows = |table: &str| tables.get(table).unwrap().len();
        let of = |id: &'static [u8]| segments.iter().filter(move |segment| segment.id == id);
        assert_eq!(rows("payments"), of(b"ST").count(), "{name}: payments");
        assert_eq!(rows("claims"), of(b"CLP").count(), "{name}: claims");
        assert_eq!(rows("services"), of(b"SVC").count(), "{name}: services");
        assert_eq!(
            rows("adjustments"),
            of(b"CAS").map(|cas| groups(cas, 2, 3)).sum::<usize>(),
            "{name}: adjustments"
        );
        assert_eq!(
            rows("provider_adjustments"),
            of(b"PLB").map(|plb| groups(plb, 3, 2)).sum::<usize>(),
            "{name}: provider_adjustments"
        );
    }
}

/// Every row's `segment` is captured by an instance of an anchor loop (its
/// trigger, for a table without `segment`), and every row index of a table
/// above points at the row of the instance that encloses that one, or is
/// null when no instance of it does.
#[test]
fn every_row_points_at_its_anchor_and_at_the_rows_that_enclose_it() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let segments: Vec<Segment<'_>> = Tokenizer::with_delimiters(&bytes, delims).collect();
        let tree = LoopTree::build(&spec, segments.iter().cloned());
        let nodes = tree.nodes();
        let mut owner = vec![None; segments.len()];
        for (index, node) in nodes.iter().enumerate() {
            for &segment in &node.segments {
                owner[segment] = Some(index);
            }
        }
        let enclosing = |node: usize, loops: &[LoopId]| {
            let mut at = Some(node);
            while let Some(index) = at {
                if nodes[index].loop_id.is_some_and(|id| loops.contains(&id)) {
                    return Some(index);
                }
                at = nodes[index].parent.map(|parent| parent.index());
            }
            None
        };
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, _) = Processor::run(&spec, &document);
        for def in spec.tables() {
            let table = tables.get(&def.name).unwrap();
            for row in 0..table.len() {
                assert_eq!(
                    index_at(table, "row", row),
                    Some(row),
                    "{name}: {}",
                    def.name
                );
                let at = index_at(table, "segment", row).unwrap();
                let node =
                    owner[at].unwrap_or_else(|| panic!("{name}: segment #{at} is not captured"));
                let anchor = enclosing(node, &def.loops).unwrap();
                assert_eq!(
                    anchor, node,
                    "{name}: {} row {row} sits in its anchor",
                    def.name
                );
                if def.segment.is_none() {
                    assert_eq!(
                        nodes[node].opened_by,
                        Some(at),
                        "{name}: {} row {row}",
                        def.name
                    );
                }
                for &above in &def.ancestors {
                    let parent = &spec.tables()[above];
                    let expected = enclosing(node, &parent.loops).and_then(|n| nodes[n].opened_by);
                    let found = index_at(table, &parent.reference, row).map(|r| {
                        let parent_rows = tables.get(&parent.name).unwrap();
                        assert!(
                            r < parent_rows.len(),
                            "{name}: {} row {row} out of range",
                            def.name
                        );
                        index_at(parent_rows, "segment", r).unwrap()
                    });
                    assert_eq!(
                        found, expected,
                        "{name}: {}.{} row {row}",
                        def.name, parent.reference
                    );
                }
            }
        }
    }
}

/// Every column of a table has the table's length; validity bitmaps and
/// binary offsets are laid out as Arrow expects.
#[test]
fn columns_have_equal_lengths_and_arrow_buffers() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, _) = Processor::run(&spec, &document);
        for table in &tables {
            let rows = table.len();
            for (column, data) in table.columns() {
                let at = format!("{name}: {}.{column}", table.name());
                assert_eq!(data.len(), rows, "{at}");
                assert_eq!(data.validity().len(), rows, "{at}");
                assert_eq!(data.validity().as_bytes().len(), rows.div_ceil(8), "{at}");
                match data.column() {
                    Column::Binary {
                        offsets,
                        data: bytes,
                    } => {
                        assert_eq!(offsets.len(), rows + 1, "{at}");
                        assert_eq!(offsets.first(), Some(&0), "{at}");
                        assert_eq!(
                            offsets.last().map(|&end| end as usize),
                            Some(bytes.len()),
                            "{at}"
                        );
                        for row in 0..rows {
                            assert!(offsets[row] <= offsets[row + 1], "{at}");
                            if data.validity().get(row) == Some(false) {
                                assert_eq!(offsets[row], offsets[row + 1], "{at}: null row {row}");
                            }
                        }
                    }
                    Column::Int64 { values, .. } => assert_eq!(values.len(), rows, "{at}"),
                    Column::Decimal128 { values, .. } => assert_eq!(values.len(), rows, "{at}"),
                    Column::Date32(values) | Column::Time32(values) => {
                        assert_eq!(values.len(), rows, "{at}")
                    }
                }
            }
        }
    }
}

/// The element findings over the eleven files are all defects of the
/// synthetic fixtures; the six anonymized payer files raise none.
#[test]
fn only_the_synthetic_fixtures_raise_element_findings() {
    let spec = Spec::builtin_835();
    let mut found = BTreeMap::new();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (_, diagnostics) = Processor::run(&spec, &document);
        let level_two = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.level == SnipLevel::L2)
            .count();
        if level_two > 0 {
            found.insert(name, level_two);
        }
    }
    assert_eq!(
        found,
        BTreeMap::from([
            ("blue_cross_nc_sample.txt".to_string(), 3),
            ("multi_claim_sample.txt".to_string(), 16),
            ("trizetto_sample.rmt".to_string(), 5),
        ])
    );
}

/// A payer's proprietary reference becomes a column with a three-line patch.
#[test]
fn a_three_line_patch_adds_a_column_the_table_then_shows() {
    let bytes = common::load_sample("edi835_test_not_available_claim_id.RMT");
    let builtin = Spec::builtin_835();
    let patched = builtin
        .merge_patch(
            r#"{"tables":{"claims":{"columns":{
                "contract_class":{"segment":"REF","where":{"1":"CE"},"element":2}
            }}}}"#,
        )
        .unwrap();
    let document = Document::parse(&bytes[..]).unwrap();
    let (before, _) = Processor::run(&builtin, &document);
    let (after, _) = Processor::run(&patched, &document);
    assert!(
        before
            .get("claims")
            .unwrap()
            .column("contract_class")
            .is_none()
    );
    let claims = after.get("claims").unwrap();
    let column = claims.column("contract_class").unwrap();
    let expected: Vec<String> = Tokenizer::new(&bytes)
        .unwrap()
        .filter(|segment| {
            segment.id == b"REF" && segment.element(1).and_then(Element::simple) == Some(b"CE")
        })
        .map(|segment| {
            String::from_utf8_lossy(segment.element(2).and_then(Element::simple).unwrap())
                .into_owned()
        })
        .collect();
    assert_eq!(expected.len(), 18);
    let values: Vec<String> = (0..claims.len())
        .map(|row| column.render(row).unwrap())
        .collect();
    assert_eq!(values, expected);
    for (name, data) in before.get("claims").unwrap().columns() {
        assert_eq!(claims.column(name), Some(data), "{name} is unchanged");
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p edi835_core --test project_files`
Expected: 6 passed.

- [ ] **Step 7: Gates and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo test --workspace --locked && cargo bench --workspace --no-run --locked`
Expected: all exit 0; 296 tests in the workspace.

```bash
git add crates/edi835_core/tests/common/mod.rs crates/edi835_core/tests/engine_golden.rs crates/edi835_core/tests/project_golden.rs crates/edi835_core/tests/project_files.rs crates/edi835_core/tests/golden/project
git commit -m "tests: table and diagnostic goldens and table invariants over the eleven files

Rows follow the segments (one claim per CLP, one adjustment per CAS
group, ...), every parent index points at the enclosing instance, and
the buffers are Arrow-shaped. The anonymized payer files raise no
element finding; the synthetic fixtures' defects are pinned. A
three-line patch adds a REF column to claims."
```

---

## Task 8: Benchmark, crate docs, README status, exit gate

**Implementer tier:** Haiku — three small edits given in full and the final sweep of the gates.

**Files:**
- Modify: `crates/edi835_core/benches/tokenize.rs`
- Modify: `crates/edi835_core/src/lib.rs`
- Modify: `README.md`

- [ ] **Step 1: Add the `process` and `process_rows` groups**

In `crates/edi835_core/benches/tokenize.rs`, replace the module doc with:

```rust
//! Throughput of the tokenizer and the document index pass over the three
//! largest fixtures, of the loop engine over the three largest samples in
//! bytes and in events, of the engine with the envelope checker over the
//! same samples, and of the whole processor (engine, checker, projector)
//! over them in bytes and in table rows.
```

Change the import to `use edi835_core::{Document, EnvelopeChecker, LoopEngine, Processor, Spec, Tokenizer};`. Add before `/// Runs the engine over every segment and returns how many events it emitted.`:

```rust
/// Indexes the bytes, runs the processor over the document and returns how
/// many table rows it produced.
fn run_process(spec: &Spec, bytes: &[u8]) -> usize {
    let document = Document::parse(bytes).expect("sample has an ISA");
    let (tables, _) = Processor::run(spec, &document);
    tables.iter().map(|table| table.len()).sum()
}
```

Replace the `criterion_group!(…);` invocation with these two functions and the new invocation (keep `criterion_main!(benches);` after it):

```rust
fn process_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("process");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_process(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

fn process_rows_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("process_rows");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        let rows = run_process(&spec, &bytes);
        group.throughput(Throughput::Elements(rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_process(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    tokenize_fixtures,
    index_fixtures,
    engine_samples,
    engine_events_samples,
    check_samples,
    process_samples,
    process_rows_samples
);
```

- [ ] **Step 2: Update the crate docs**

In `crates/edi835_core/src/lib.rs`, replace the crate doc (every `//!` line) with:

```rust
//! Lossless, fast, data-driven EDI 835 parser core.
//!
//! Bytes in, structure out, in layers that each point back at the input:
//!
//! - [`Tokenizer`] reads the delimiters from the ISA and yields generic
//!   [`Segment`]s lazily; [`Segment::write_to`] writes them back.
//! - [`Document`] indexes every segment of a buffer once so any segment can be
//!   reached by index or byte span.
//! - [`Spec`] is a JSON loop structure (parents, triggers, held segments, end
//!   segments, envelope controls), the names and types of each segment's
//!   elements, and the tables to project, and can be patched with JSON Merge
//!   Patch; [`Spec::builtin_835`] ships the 835 as such data.
//! - [`LoopEngine`] interprets a segment stream against a spec and emits
//!   [`Event`]s (loops opened and closed, segments captured or unmatched);
//!   [`LoopTree`] collects those events into a tree of loop instances.
//! - [`EnvelopeChecker`] reads the same events and reports structural
//!   [`Diagnostic`]s: unknown segments, implicit or unterminated loops, and
//!   envelope counts or control numbers that do not match.
//! - [`Projector`] reads the same events too: it checks every element the
//!   spec defines (required, type, length, components) and fills the spec's
//!   tables, typed [`Table`]s of [`ColumnData`] laid out as Apache Arrow
//!   lays out its arrays, with each row pointing at the rows that enclose it.
//! - [`Processor`] feeds one segment at a time to the engine, the checker and
//!   the projector; [`Processor::run`] does it over a whole [`Document`].
//!
//! The structure is data: no code in this crate is specific to the 835 beyond
//! the built-in spec it loads.
```

- [ ] **Step 3: Run the benches and record**

Run: `cargo bench --workspace --no-run --locked && cargo bench --workspace 2>&1 | grep -E '^(engine|check|process|process_rows)/|thrpt:'` (about six minutes)
Expected: three entries each for `engine/*`, `check/*`, `process/*` and `process_rows/*`. `process/*` runs well below `check/*` (on the scratch copy 28–30 MiB/s against 83–91 MiB/s): it indexes the document, checks every defined element and appends rows. Keep the `process` and `process_rows` lines for the commit message.

- [ ] **Step 4: Update the README status**

Replace the `## Status` section of `README.md` with:

```markdown
## Status

Owner ruling (2026-10-03): `payments` projects BPR05 (`payment_format`) instead of BPR06; the header and row shown for the emedny golden above were adjusted by hand to that ruling, so the implementer must trust the regenerated file over this sample if they differ in that column.

**Stage 4 — projection and validation.** The JSON spec names and types every element of
the 835's segments and declares the tables to project. One pass over a file feeds the
loop engine, an envelope checker and a projector: the checker reports unknown segments,
implicit or unterminated loops and envelope counts or control numbers that do not
match; the projector checks every element (required, type, length, components) and
fills typed, Arrow-layout tables of payments, claims, services, adjustments and provider
adjustments, each row pointing at the rows that enclose it. Every finding is a
self-explanatory diagnostic. The Python binding comes next.
```

- [ ] **Step 5: Final sweep and commit**

Run: `cargo build --workspace --all-targets --locked && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo bench --workspace --no-run --locked && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
Expected: every command exits 0; `cargo test` reports 296 tests in total (252 in the library).

Run: `grep -nE '//.*(\b(T1[0-9]|P[0-9]+|N[2-7]|D1[0-3])\b|[Ss]tage [0-9]|#(2[0-9]|7|1[7-9])\b)' crates/edi835_core/src/*.rs`
Expected: no output (no decision, principle, stage or issue code in a comment).

```bash
git add crates/edi835_core/benches/tokenize.rs crates/edi835_core/src/lib.rs README.md
git commit -m "bench: processor baseline in bytes and rows; crate docs and README status

Baseline (criterion, <machine>):
  <paste the three process and three process_rows thrpt lines>"
```

---

## Task 9 (optional): SNIP 3 balancing — not included

Measured on the scratch copy with the tables of Task 7: `SVC02 − SVC03 = Σ adjustments.amount by service` holds on every service of the eleven files; `CLP03 − CLP04 = Σ adjustments.amount by claim` holds on every claim except multi_claim's two and trizetto's one (synthetic fixtures); `BPR02 = Σ claims.payment_amount − Σ provider_adjustments.amount by payment` holds on eight files and fails on trizetto and on the two excerpt samples (`edi835_test_file`, `edi835_test_not_available_claim_id`, whose `SE01` already shows they are cut from larger files). The results are explainable, so the checks themselves are sound.

It stays out of this plan for three reasons, recorded for D11:
1. The `BPR02` balance needs two group sums with opposite signs on one side; the declared form `"rhs": { "sum": …, "by": … }` holds one. Covering it needs a small expression form (a signed list of sums), which is the "lenguaje de reglas" §7 sends to D11.
2. A balance needs complete groups. `take_tables` can drain a claim before its adjustments only if a caller drains mid-transaction; evaluating per drain is safe for drains on transaction close and wrong otherwise, so the evaluation point is a design decision of its own.
3. The scratch estimate (spec section with its own validation errors and `Display` tests, scale alignment between decimal columns, a `Rule::OutOfBalance` variant, evaluation) is about 200 lines of non-test code, over the 150-line bound set for this optional task.

---

## Status

Task boundaries follow the stage's scope one to one: each task has its own red → green cycle and its own commit. Adjustments, each inside a task rather than across tasks:

- `ColumnData::render` lives in the `column` module (Task 2), not in test helpers: the golden files, the projector's unit tests and the processor's incremental test all render cells the same way, and a Python caller will want the same text.
- Task 4 replaces the existing `patch_null_deletes` test: deleting loop `2110` from the built-in now breaks the `services` and `adjustments` tables, which is the behaviour this stage wants (`deleting_a_loop_a_table_anchors_in_names_the_table`), so that test deletes the tables along with the loop.
- `tests/project_files.rs` is created in Task 6 with the incremental test and extended in Task 7; Task 7 also moves `describe_diff` into `tests/common` and gives `engine_golden.rs` the shared `compare_goldens`, whose orphan check looks only at files directly in its directory. The new goldens live in `tests/golden/project/`, so neither test sees the other's files as orphans. The event goldens do not change.
- `Projector::new` and `Processor::new` take `&Delimiters` beside the spec (§7 sketches `new(&Spec)`): reading an element declared without components as its whole text needs the component separator to join what the tokenizer split.
- SNIP 3 (Task 9) is not included; the measurements and the reasons are in its note, for D11.

Tests in the whole workspace, measured on a scratch copy where every step of this plan was executed and every gate run at every commit: 229 before; 231 after Task 1, 253 after Task 2, 264 after Task 3, 267 after Task 4, 285 after Task 5, 290 after Task 6, 296 after Tasks 7 and 8.

## Stage 4b exit gate (definition of done)

- [ ] `cargo test --workspace --locked` green: library 252 (`spec` 89, `column` 22, `project` 18, `process` 4, `diagnostic` 14, `check` 16, plus the Stage 1–3 modules), `project_files` 6, `project_golden` 1, `engine_golden` 4, `check_envelope` 2, plus the existing suites; 296 in total.
- [ ] clippy `-D warnings`, fmt, `cargo bench --no-run` and `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean.
- [ ] `[dependencies]` of `edi835_core` still exactly `serde` and `serde_json`.
- [ ] No `unwrap`/`expect`/`panic!` in `src/` outside `mod tests`, except the documented `expect` in `Spec::builtin_835`.
- [ ] `src/column.rs`, `src/project.rs` and `src/process.rs` name no 835 segment: `grep -nE '"(ST|SE|GS|GE|ISA|IEA|CLP|SVC|CAS|PLB|BPR|NM1|REF|DTM)"|b"(ST|SE|CLP|SVC|CAS|PLB)"' crates/edi835_core/src/column.rs crates/edi835_core/src/project.rs crates/edi835_core/src/process.rs` matches only inside `mod tests`.
- [ ] 22 goldens in `tests/golden/project/` generated by the test and inspected as in Task 7 Step 4; `git diff master --stat -- crates/edi835_core/tests/golden/*.txt` is empty (the event goldens did not change).
- [ ] Every new `SpecError` variant (`TableSchema`, `BadTable` with its nineteen `TableDefError` reasons), the two new `ElementDefError` reasons, `CellError` (two), `RowError` (two) and the three changed `Rule` variants (both forms of `opened_at`) have full-text `Display` tests.
- [ ] `cargo bench` runs the `process` and `process_rows` groups; baseline in a commit message.
- [ ] Fixtures and samples unchanged (`git diff master --stat -- crates/edi835_core/tests/fixtures crates/edi835_core/tests/samples` is empty).
- [ ] Issues #25 and #27 are closed by the PR (named in its body, not in code).

## Self-review

**Spec coverage (§7 Stage 4, the 4b subset, and the three rulings).**

| Item | Where | Proof |
|---|---|---|
| T14 Arrow-layout columns without the crate: LSB-first validity; `Binary` with `i32` offsets over raw bytes for `AN`/`ID`; `Int64` with the implied scale as metadata for `N`n; `Decimal128` (`i128`, precision 38, column scale) for `R`; `Date32`; `Time32` in seconds; a value that fails its type or exceeds the scale is a diagnostic and a null | Tasks 2, 5 | `a_bitmap_packs_bits_least_significant_first`, `a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls`, `column_types_follow_the_element_types`, `a_value_that_is_not_its_type_is_reported_and_null`, `a_decimal_with_more_places_than_its_scale_is_a_type_mismatch`, `columns_have_equal_lengths_and_arrow_buffers` |
| T14 `Table { name, columns }` with equal lengths checked; `Tables` | Task 2 | `a_row_is_appended_whole_or_not_at_all`, `rows_read_back_with_their_validity` |
| T15 `tables` section: anchors in loops, optional `segment`, optional `repeat`; sources `element` (segment, `where`, `loop`, `component`), `segment_index`, group elements; automatic `segment` and parent-index columns; global ordinals; types from `segments`, `Binary` without a definition; built-in `payments`, `claims`, `services`, `adjustments`, `provider_adjustments`; a three-line patch adds a column | Tasks 3, 4, 5, 7 | `tables_are_read_in_name_order_with_their_sources`, `a_table_hangs_from_the_tables_anchored_above_it`, `builtin_835_declares_five_tables_and_how_they_nest`, `the_spec_tables_give_the_columns_and_their_types`, `a_patch_adds_a_column_with_three_lines`, `a_three_line_patch_adds_a_column_the_table_then_shows` |
| T15 / P10 bad tables rejected with table, column and reason | Task 3 | `bad_tables_are_rejected_with_the_table_the_column_and_the_reason`, `table_errors_display_the_table_the_column_and_every_reason`, `table_schema_errors_display_the_table_the_column_and_the_serde_message`, `every_object_of_the_table_schema_is_checked_with_its_path`, `empty_segment_ids_in_tables_name_their_key` |
| T16 one walk, two consumers: `Processor` with `feed` → `&Output` (events and diagnostics), `finish`, `take_tables`, `diagnostics()`, `run(&Spec, &Document)`; type and requirement checks in the same access that fills the column | Tasks 5, 6 | `one_feed_returns_the_events_and_every_diagnostic_of_its_segment`, `run_over_a_document_is_feeding_every_segment_then_finishing`, `read` reusing `Checked` in `project.rs` |
| T16 / P9 incremental == document | Task 6 | `tables_drained_after_each_transaction_add_up_to_one_run`, `draining_after_each_transaction_adds_up_to_one_run_over_the_document` |
| T17 level 2: `RequiredElementMissing`, `TypeMismatch`, `LengthOutOfRange`, `CompositeShape`, with segment, element, component, path and datum | Task 5 | `an_empty_required_element_is_reported_with_its_name`, `an_invalid_date_names_the_element_and_the_text`, `lengths_count_bytes_for_text_and_digits_for_numbers`, `a_composite_with_more_components_than_declared_names_the_first_extra_one`, `components_are_checked_where_the_definition_declares_them` |
| T17 level 3 (optional) | Task 9 | not included; measured and reasoned for D11 |
| Gate: goldens of tables (summary for the two large samples) and of diagnostics, same switch, orphan detection | Task 7 | `tables_and_diagnostics_match_the_golden_files`, `compare_goldens` |
| Gate: invariants over the eleven files (row counts, parent indices vs the tree, anchor segments captured by the anchor loop, equal lengths, coherent bitmaps and offsets) | Task 7 | `row_counts_follow_the_segments_of_each_file`, `every_row_points_at_its_anchor_and_at_the_rows_that_enclose_it`, `columns_have_equal_lengths_and_arrow_buffers` |
| Gate: property tests (valid values round-trip, random bytes never panic, random rows read back) | Tasks 2, 5 | `valid_n_values_round_trip`, `valid_r_values_round_trip`, `valid_dates_round_trip`, `valid_times_round_trip`, `random_bytes_never_panic`, `rows_read_back_with_their_validity`, `valid_values_never_raise_a_diagnostic`, `random_elements_never_panic_and_rows_stay_aligned` |
| Gate: costura engine → projector and end-to-end over the eleven files | Tasks 5, 6, 7 | every `project::tests` test runs the real engine; `project_files.rs` runs `Processor::run` |
| Gate: bench in bytes/s and rows/s | Task 8 | `process`, `process_rows` |
| Ruling 1 (#27): `scale > 18` and `max: 0` rejected naming segment, position and value; `R` → `Decimal128(38, scale)` | Tasks 1, 2 | `an_r_scale_above_18_or_a_zero_max_is_rejected_with_the_value`, `scale_and_max_reasons_display_segment_position_and_value`, `column_types_follow_the_element_types` |
| Ruling 2 (#25): `ImplicitLoop.expected_trigger` (rendered as the spec writes it), `opened_at` on `UnterminatedLoop` and `ControlNumberMismatch` | Task 1 | the four `diagnostic` tests and six `check` tests of Task 1; blue_cross in `check_envelope.rs` |
| Ruling 3: raw text for elements without `composite`, components only where declared, no `ISA16` case | Task 5 | `an_element_defined_without_components_is_read_as_one_text`; `ISA16` raises nothing on the eleven files (Task 7 goldens) |

**Placeholder scan.** `grep -nE 'TBD|TODO|FIXME|similar to Task|add validation|write tests for'` over this plan: no match. The only angle-bracket fields are `<machine>` and `<paste … thrpt lines>` in the Task 8 commit message, filled from the measured output as in the Stage 3 and 4a plans; and `<name>`, `<file>`, `<n>`, `<column>`, `<reason>`, `<t>`, `<c>` inside format descriptions.

**Type consistency across tasks.** `ColumnType::of(Option<ElementType>)` (Task 2) is the only mapping from element types to columns; the projector uses it for both the column types and the checked values (Task 5), so a checked value is reused only when its `ColumnType` equals the column's. `TableDef { name, reference, loops, segment, repeat, columns, parent, ancestors }` and `ColumnSource::{Element, SegmentIndex, GroupElement}` (Task 3) are read with those field names in `project.rs` (Task 5) and in `project_files.rs` (Task 7). `ROW_COLUMN`/`SEGMENT_COLUMN` (Task 3) name the automatic columns in `Projector::new` (Task 5) and in the invariants (Task 7). `Projector::new(&Spec, &Delimiters)` (Task 5) is what `Processor::new` passes (Task 6). `Output::events()`/`diagnostics()` (Task 6) are what the incremental tests read (Tasks 6, 7). `common::table_rows`/`table_header` (Task 6) render the goldens (Task 7). `render_trigger` becomes `pub(crate)` in Task 1 and is the only source of `expected_trigger`. Every snippet was compiled and its tests run, task by task, on a scratch copy of the repository, with clippy, fmt and `bench --no-run` green at every commit and `cargo doc` clean at the end; the counts in Status are from those runs.

**Review Focus, each pinned to a test.**

1. Row numbers at open, append at close, parents as open rows, global numbering across drains → `every_row_names_the_open_row_of_each_table_above_it`, `row_numbers_keep_counting_across_drains` (Task 5); `draining_after_each_transaction_adds_up_to_one_run_over_the_document` (Task 6); `every_row_points_at_its_anchor_and_at_the_rows_that_enclose_it` (Task 7).
2. Raw text for elements without components; components only where declared → `an_element_defined_without_components_is_read_as_one_text`, `components_are_checked_where_the_definition_declares_them` (Task 5).
3. One parse per element; null on type failure, kept on length failure; digits for numeric lengths → `a_value_that_is_not_its_type_is_reported_and_null`, `lengths_count_bytes_for_text_and_digits_for_numbers` (Task 5).
4. Bad tables named with table, column and datum; nesting, sharing and unrelated anchors refused → `bad_tables_are_rejected_with_the_table_the_column_and_the_reason` (Task 3), `deleting_a_loop_a_table_anchors_in_names_the_table` (Task 4).
5. Arrow buffers → `a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls`, `rows_read_back_with_their_validity` (Task 2), `columns_have_equal_lengths_and_arrow_buffers` (Task 7).
6. Envelope diagnostics name the missing trigger and the opener → `implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end`, `a_control_number_that_differs_from_the_opener_is_reported`, `loops_still_open_at_the_end_of_the_stream_are_unterminated` (Task 1).
