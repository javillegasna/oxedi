# Stage 5b — `edi-835-parser` compatibility · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove on real (anonymized) payer files that a data spec reproduces the output of the one hand-written 835 library with real users: `oxedi835.edi_835_parser.parse(path).to_dataframe()` equals `edi_835_parser.parse(path).to_dataframe()` cell for cell (rows, columns, order, dtypes) on the six samples, with the same counts, sums, payer and payee; show with the same files what that frame drops (`to_dataframe(extended=True)`, `x_` columns only); give its users a one-line `import` change today (`parse` is a drop-in replacement: path or directory, same signature) plus additions to read from memory (`parse_bytes`, `parse_file_obj`, `parse_many`) and native counterparts (`Result.count_claims()` …, `Table.to_polars()`/`to_pandas()`) for tomorrow; and leave two oracles: the parity tests in CI and `scripts/compat_oracle.py` for the originals outside the repo.

**Architecture:** One JSON patch, `crates/edi835_core/specs/edi_835_parser.json`, applied over the built-in spec, removes the five native tables and declares seventeen `rows*` tables: one per loop level the library reads (`rows_interchanges`, `rows_payments`, `rows_organizations`, `rows_claims`, `rows` anchored in `2110`) and one long table per repeated segment (`rows_adjustments`, `rows_references`, `rows_remarks`, the claim and service `DTM`/`AMT`/`NM1`/`REF` lists, and for `extended` the full `CAS` groups and `PLB`). The binding embeds the file (`oxedi835._core.EDI_835_PARSER_PATCH`, `include_str!` from the core crate, so there is one copy). A pure-Python package `oxedi835/edi_835_parser/` parses each file once with that spec, splits the tables per transaction, and exposes read-only objects shaped like the library's (`TransactionSet`, `Claim`, `Service`, `Organization`, segment dataclasses); `to_dataframe()` is the library's own loop (`serialize_service` + the `adj_<n>`/`ref_<n>`/`rem_<n>` numbering) over those objects, so pandas infers the same dtypes from the same Python values. The native counterparts live in the binding (Rust) over the built-in tables. The core's code does not change.

**Tech Stack:** Rust edition 2024 (binding only: PyO3 0.29, unchanged deps). Python ≥ 3.11. Runtime extras: `pandas` + `pyarrow` (`oxedi835[edi-835-parser]`, `oxedi835[pandas]`), `polars` (`oxedi835[polars]`). Development: `edi-835-parser==1.8.0`, `pandas>=2.0.3,<3`, `duckdb>=1.1`.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 5b · Compatibilidad con `edi-835-parser`": T25 (spec + long tables + pivot), T26 (strict parity by default, `extended=True` with `x_` columns), T27 (full surface in `oxedi835.edi_835_parser`, extra `oxedi835[edi-835-parser]`), T28 (native counterparts without pandas), T29 (two oracles), the Entregable, Gate and "Fuera de alcance" paragraphs; §6.2 D12 (resolved by this stage); argued from N1, N3, P10.

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/` on branch `stage-5b-edi835parser`. Python commands use the repo's `.venv` (`make venv` creates it; Task 5 adds the new dev packages to that target — until then install them once with `uv pip install --python .venv/bin/python pandas "edi-835-parser==1.8.0" duckdb`).

## Global Constraints

- **Owner rulings (2026-10-04), binding on every task:**
  - **O1 yes:** the built-in `payments` table in `specs/835.json` gains the nine payer/payee columns the plan lists (Task 1, with the `src/spec.rs` test vector and the regenerated project goldens inspected line by line); native `Result.payer`/`payee` return full organizations.
  - **Code tables:** `_codes.py` copies the library's description tables verbatim, confined to `oxedi835/edi_835_parser/`. Its header carries the library's MIT notice exactly as its `LICENSE` reads (copyright line included) plus the project name and author (keiron-stoddart / Senscio Systems); a root `THIRD_PARTY_NOTICES` file lists it with the full MIT text; a test asserts both notices are present. Never in the core or the native API.
  - **Format limits L0–L2** (a column cannot read an enclosing loop; absent vs written-empty; first-match only) stay computed in Python in this stage, each with its one-line justification; they are tracked by a board issue for a format extension before the writer.
  - **`rem_<n>`** (`LQ`) is the third dynamic family, alongside `adj_<n>` and `ref_<n>`.
  - **Performance:** Task 5 measures the compat frame against the library on a release build (`maturin develop --release`) on the three largest samples and records the numbers; an issue is opened only if we are still slower.
  - **`parse` is a strict drop-in replacement** (see Status).

- **Core crate.** `[dependencies]` of `edi835_core` stays exactly `serde` and `serde_json`. No `.rs` file under `crates/edi835_core/src/` changes, with one exception that needs the owner's approval before Task 4 starts (ruling **O1**, see Task 4 Step 1): nine payer/payee columns added to the built-in `payments` table, which changes one expected vector in the unit test `builtin_835_declares_five_tables_and_how_they_nest` of `src/spec.rs`. New core files are data and tests only: the patch, its goldens, one test in `tests/project_golden.rs`.
- **Base wheel has no Python dependencies.** `pandas`, `pyarrow`, `polars` are imported lazily inside the functions that need them; a missing one raises `ImportError` naming the method, the module and the extra (`pip install "oxedi835[polars]"`). The compat package imports `pandas`/`pyarrow` only inside `to_dataframe`/`load`. Task 5 adds a test that reads the installed metadata and fails if any requirement lacks an `extra ==` marker.
- **`pandas<3` in the `edi-835-parser` extra.** `edi-835-parser` 1.8.0 itself requires `pandas>=2.0.3,<3`; pandas 3 infers a `str` dtype where pandas 2 infers `object`, so parity is defined and tested under pandas 2.
- **Where the 835 lives.** Every segment id, qualifier and element position that selects a value is in `edi_835_parser.json`. The Python layer converts types (`bytes`→`str`, `Decimal`→`float`, `date`→`datetime`, the library's `int()`), pivots long tables into numbered columns, gathers parents through the automatic `claim`/`payment`/`service` reference columns, maps codes to the library's descriptions, and applies the four rules listed in "Per-column parity" below, each with its one-line reason in the code's docstring. Nothing re-tokenizes bytes; the only reads of `Document` are presence checks on a segment the spec already located (by its `segment` column or a `segment_index` column).
- **The `N104` shim lives in the tests only** (`crates/oxedi835_py/tests/n104_shim.py`) and is applied by the parity tests and by `scripts/compat_oracle.py`. It is never imported by the package.
- **Privacy.** The originals live outside the repo. Nothing the plan's code prints or writes about them goes beyond counts, shapes, column names, dtypes and verdicts; `compat_oracle.py` names files by their position in the sorted listing. The six anonymized samples may appear in test expectations (counts and shapes only, as below).
- **Gates on every commit:** `make gates` (fmt-check, clippy `-D warnings`, `cargo test --workspace --locked`, `cargo bench --no-run`, rustdoc `-D warnings`) and `make py-test`. Goldens are regenerated only with `UPDATE_GOLDEN=1 cargo test -p edi835_core --test project_golden` and inspected before committing. Fixtures, samples and the existing goldens are never edited by hand.
- No `unwrap`/`expect`/`panic!`/fallible indexing in `crates/oxedi835_py/src/`. Comments and docstrings describe implementation only: no stage, principle (N1, P10), decision (T25, D12), issue numbers or history. `git add` names files; never `-A` or `.`.
- **P10 at the boundary.** Every new exception names the method, what it needed (table, column, count) and what it found. One exact-text test per message.

## Review Focus

1. **Parity is data.** Every frame value is read by a column of `edi_835_parser.json`; the Python code holds no segment id, qualifier or element position except the four presence/last-segment rules of the parity table, each documented at its function. Tests: `test_the_frame_equals_edi_835_parser_cell_for_cell` (six samples, `check_exact=True`), `test_a_directory_reads_the_same_files_in_the_same_order`.
2. **Every input route gives the same frame, and `parse` stays a drop-in replacement.** `parse(path)` takes only a path or a directory, like the library; bytes, `memoryview` and binary file objects go through `parse_bytes`/`parse_file_obj`/`parse_many`; all end in one `oxedi835.parse` call. Add a test that `inspect.signature(compat.parse)` has the library's parameters (`path`, `debug`) and that `compat.parse(b"...")` fails the same way the library's does for a non-path. Tests: the parity test parametrized `by_path`/`by_bytes`/`by_file`, `test_path_bytes_memoryview_and_file_give_the_same_frame`, `test_parse_many_equals_the_directory`.
3. **Strict means strict.** Same rows, columns, order, dtypes and index as the library, including the library's quirks that the samples exercise (empty `NM104` → `" Name"`, the numbered columns sorted as strings). Tests: the parity tests of Task 2; `test_a_claim_without_services_is_left_out_and_recovered` checks the library drops a claim without services and so do we.
4. **`extended=True` only adds.** The strict columns come first in the strict order, every added column starts with `x_`, and the service rows equal the strict frame. Tests: `test_extended_keeps_the_frame_and_adds_only_x_columns`, `test_extended_recovers_what_the_frame_leaves_out`.
5. **Objects match the library's fields.** `payer`, `payee`, `financial_information`, `interchange`, every claim's entities/dates/references/amount and every service's adjustments/dates/remarks/allowed amount. Tests: `test_payer_payee_and_segments_equal_edi_835_parser`, `test_claim_and_service_objects_equal_edi_835_parser`.
6. **Native counterparts and lazy extras.** `Result.count_claims/count_patients/sum_payments/payer/payee` equal the library's on the six samples (`Decimal` vs `float` within a cent); `to_polars`/`to_pandas` raise an `ImportError` naming the extra. Tests of Task 4.
7. **Errors explain themselves (P10).** Exact-text tests for every new message (Task 4) and for the compat layer's `ValueError`s (Task 2).

---

## File Structure

```
crates/edi835_core/
├── specs/edi_835_parser.json                   # NEW: the compat patch (seventeen rows* tables)
├── specs/835.json                              # Task 4, only with ruling O1: nine payer/payee columns in `payments`
├── src/spec.rs                                 # Task 4, only with ruling O1: one expected vector in a unit test
└── tests/
    ├── project_golden.rs                       # + test edi_835_parser_tables_match_the_golden_files
    └── golden/project/edi_835_parser/          # NEW: 9 × <file>.tables.txt + 2 × <file>.tables.summary.txt
crates/oxedi835_py/
├── pyproject.toml                              # extras edi-835-parser, pandas, polars; test extra grows
├── src/lib.rs                                  # + EDI_835_PARSER_PATCH constant
├── src/parse.rs                                # + Result.count_claims/count_patients/sum_payments/payer/payee
├── src/native.rs                               # NEW: the native counterparts over core Tables
├── src/tables.rs                               # + Tables/Table.to_polars/to_pandas; PyTables::tables()
├── python/oxedi835/edi_835_parser/
│   ├── __init__.py                             # parse, TransactionSets, TransactionSet, spec
│   ├── _tables.py                              # spec(), Rows, load(): one parse, tables split per transaction
│   ├── _convert.py                             # library-faithful conversions and the presence rule
│   ├── _codes.py                               # the library's code descriptions
│   ├── _views.py                               # read-only objects shaped like the library's
│   ├── _sets.py                                # parse, TransactionSets, TransactionSet, serialize_service
│   └── _extended.py                            # extended=True rows and x_ columns
└── tests/
    ├── n104_shim.py                            # NEW: lets edi-835-parser read alphanumeric N104
    ├── test_spec.py                            # + the patch loads
    ├── test_edi_835_parser.py                  # NEW: parity (Task 2) + extended (Task 3)
    ├── test_native.py                          # NEW: Task 4
    ├── test_duckdb.py                          # NEW: Task 5
    └── test_packaging.py                       # NEW: Task 5
scripts/compat_oracle.py                        # NEW: oracle over a directory outside the repo
scripts/smoke_wheel.sh                          # installs the new dev packages
Makefile                                        # venv installs them; target compat-oracle
.github/workflows/ci.yml                        # python job installs them
README.md                                       # "Coming from edi-835-parser"
```

Dependencies inside the package: `_sets` → `_views`, `_tables`, `_convert`, `_codes`; `_views` → `_convert`, `_codes`; `_extended` → `_sets`, `_convert`; `_tables` → `oxedi835` (`Spec`, `parse_file`, `_core.EDI_835_PARSER_PATCH`). Only `_tables.load` (pyarrow) and the `to_dataframe` methods (pandas) import third-party modules.

## Python dev loop

```bash
make venv        # once: .venv with maturin, pytest, polars, pyarrow (+ pandas, edi-835-parser, duckdb after Task 5)
make py-test     # maturin develop into .venv, then pytest crates/oxedi835_py/tests
make gates       # the Rust gates CI runs, plus rustdoc
```

## Facts this plan relies on (verified on a scratch copy before writing it)

The parity core was executed on a scratch copy outside the repository (`/tmp/oxe5b`, deleted afterwards): the patch below loaded with `Spec.builtin().patch(...)` into the current binding (a debug build of `master` + the §7 commit), the Python package below, and the tests of Tasks 2 and 3 (58 tests, all passing), run with `edi-835-parser` 1.8.0, pandas 2.3.3, numpy 2.5.3, pyarrow 25.0.1, polars 1.44.2, duckdb 1.5.6 on Python 3.13.13. The only line that differs from what ran is the import of `EDI_835_PARSER_PATCH` (the scratch run set that attribute on `oxedi835._core` by hand, since the binding was not rebuilt). Task 1's Rust test, Task 4's Rust code and Task 5's packaging were not executed; their prose says so where it matters.

| Fact | Value |
|---|---|
| The library without the shim | `ValueError` on 5 of 6 samples (`int()` on `N104`); only `united` parses |
| Library frames (shape, rows × columns) | davisvision 2 × 23, eyemed 414 × 21, file 23 × 21, not_available_claim_id 26 × 23, united 6192 × 26, versant 1778 × 28; `RangeIndex` each |
| Dynamic column families | `adj_<n>_{amount,code,group}`, `ref_<n>_{qual,value}` **and `rem_<n>_{code,qual}`** (LQ remarks: davisvision, not_available_claim_id, versant; §7 lists only the first two) |
| Library dtypes | `float64` for amounts, `int64` for units, `datetime64[ns]` for dates, `bool` for `was_forwarded`; a column that is entirely `None` stays `object` (`allowed_amount` in davisvision/versant, `end_date` in eyemed/file); numbered columns missing on some rows hold `NaN` (`float`) inside `object` |
| `count_claims` / `count_patients` / `sum_payments` | davisvision 1/1/0.0, eyemed 82/82/8982.0, file 4/4/8982.0, not_available 18/5/3715.0, united 1332/1212/173305.0, versant 648/417/123950.65 |
| `assert_frame_equal(check_exact=True)`, compat vs library | **equal on all six samples through each input route (path, bytes, open binary file)**, and path = bytes = `memoryview` = file object frame for frame; `parse_many` over the six (one as an open file, five as bytes, in `os.listdir` order) equals the library on the directory; equal on the six as one directory (copied as `.txt`: 8435 × 30, index restarting per file); equal on the inline file with a claim without services |
| Objects | `payer`, `payee` (type, name, id, address, city/state/zip), `financial_information` (amount, method, routing, date), `interchange` (qualifier, sender, receiver, date-time), every claim (CLP fields, entities, dates, references, last AMT) and every service (adjustments, dates, remarks, allowed amount): equal on all six |
| Rows per compat table, united | rows 6192, rows_adjustment_groups 2370, rows_adjustments 2370, rows_claim_adjustments 0, rows_claim_amounts 1247, rows_claim_dates 1332, rows_claim_entities 3107, rows_claim_references 40, rows_claims 1332, rows_interchanges 1, rows_organizations 2, rows_payments 1, rows_provider_adjustments 0, rows_references 5830, rows_remarks 0, rows_service_amounts 2540, rows_service_dates 6192; 0 diagnostics |
| Rows per compat table, versant | rows 1778, rows_adjustment_groups 780, rows_adjustments 780, rows_claim_adjustments 3, rows_claim_amounts 643, rows_claim_dates 648, rows_claim_entities 837, rows_claim_references 1296, rows_claims 648, rows_interchanges 1, rows_organizations 2, rows_payments 1, rows_provider_adjustments 3, rows_references 0, rows_remarks 1098, rows_service_amounts 0, rows_service_dates 1778; 0 diagnostics |
| `rows` header (emedny) | `row: int64 \| segment: int64 \| interchange: int64 \| payment: int64 \| claim: int64 \| allowed_units: binary \| billed_units: binary \| charge_amount: decimal128(38, 2) \| code: binary \| modifier: binary \| paid_amount: decimal128(38, 2) \| qualifier: binary \| service_date_segment: int64 \| service_period_end_segment: int64 \| service_period_start_segment: int64`; row 0 starts `0 \| 21 \| 0 \| 0 \| 0 \| 1 \| ∅ \| 6.00 \| V2020 \| RB` |
| What `extended=True` recovers (claims without services, claim `CAS` groups, `PLB` groups, claim `REF`, claim `AMT`) | davisvision 0/0/1/2/1, eyemed 0/0/0/0/81, file 0/0/0/0/3, not_available 0/0/0/36/18, united 0/0/0/40/1247, versant 0/3/3/1296/643. No sample or fixture has a claim without services, and no sample has a second `CAS` group or a second `AMT` in a service: the inline file of Task 3 covers the first case |
| Fixtures (not a gate, `compat_oracle.py` over `tests/fixtures`) | emedny and united_healthcare_legacy equal; multi_claim and trizetto differ in `code`/`qualifier` only (their ISA16 is `>` but `SVC01` uses `:`; the library guesses the separator per element, oxedi835 honours ISA16); blue_cross and the README skipped (no ISA) |
| Native counterparts over the built-in tables | `claims` rows = `count_claims`, distinct non-null `claims.patient_id` = `count_patients` (no null ids in any sample), `Decimal` sum of `payments.total_payment_amount` within a cent of `sum_payments` (0.00, 8982.00, 8982.00, 3715.00, 173305.00, 123950.65): all six equal. Payer/payee: equal on all six once `payments` carries the nine columns of ruling O1 (checked with a patch) |
| DuckDB 1.5.6 over the built-in tables of united, from inside a function | `select count(*), count(distinct patient_id), sum(payment_amount) from claims` → `(1332, 1212, Decimal('173305.00'))`; `select count(*) from services s join claims c on s.claim = c.row` → `(6192,)`; `describe claims` gives `BIGINT`, `DECIMAL(38,2)`, `BLOB` |
| sdist layout | `maturin sdist` keeps the workspace layout (`crates/edi835_core/specs/edi_835_parser.json` next to `crates/oxedi835_py/src/lib.rs`), so `include_str!("../../edi835_core/specs/edi_835_parser.json")` resolves from the sdist too |
| Two copies of `edi835_test_file.RMT` back to back | parse as two interchanges: `payments` has 2 rows (Task 4's payer error test) |

### Per-column parity

Source names are `table.column ← X12`. "Spec" means the value comes from a compat-table column and Python only converts its type. R1–R4 are the four Python rules; L1–L3 the format limits that force them. The numbered columns are pivots of one long table each, with the ordinal being the row's position among the rows of the same `service` (the long tables are in file order).

| Column | Source in the spec | Python | Why |
|---|---|---|---|
| `marker` | `rows_claims.marker ← CLP01` | `str` | spec |
| `patient` | `rows_claims.patient_segment ← index of NM1*QC`; entity from `rows_claim_entities` (`NM103`, `NM104`) | `f"{first} {last}".title()`; R1 on `NM104` | the format chooses the segment; title-casing is the library's formatting |
| `code` / `qualifier` | `rows.code ← SVC01-2` / `rows.qualifier ← SVC01-1` | `str` | spec |
| `modifier` | `rows.modifier ← SVC01-3` | R1: written empty → `""`, absent → `None` | L1 |
| `allowed_units` | `rows.allowed_units ← SVC05`, retyped `AN` by the patch | library `int()` (text kept when it fails); R1: absent → `0` if `paid_amount == 0` else `1`, written empty → `None` | L1; L3 (a decimal column cannot give back `"1.0"` vs `"1"`) |
| `billed_units` | `rows.billed_units ← SVC07`, retyped `AN` | `int()`; R1: absent → `allowed_units`, written empty → `None` | L1, L3 |
| `transaction_date` | `rows_payments.transaction_date ← BPR16` (+ `bpr_segment`) | `date` → `datetime`; R2 | spec; R2 for malformed dates |
| `icn` | `rows_claims.icn ← CLP07` | `str` | spec |
| `charge_amount` / `paid_amount` | `rows.charge_amount ← SVC02` / `rows.paid_amount ← SVC03` | `float(Decimal)` (correctly rounded, equal to the library's `float(text)`) | spec |
| `allowed_amount` | `rows_service_amounts` (`AMT01`, `AMT02` of every `AMT` in `2110`) | R3: the service's last `AMT`; its amount only if its qualifier is `B6` | L2 |
| `payer` | `rows_payments.payer_segment ← index of the 1000A N1` → `rows_organizations.name ← N102` | gather by segment index | spec |
| `start_date` | `rows.service_period_start_segment ← DTM*150`, else `rows.service_date_segment ← DTM*472`, else `rows_claims.statement_period_start_segment ← DTM*232`; value `rows_service_dates.date` / `rows_claim_dates.date ← DTM02` | the library's fallback order; R2 | spec chooses each candidate; the order is the library's rule |
| `end_date` | same with `DTM*151`, `DTM*472`, `DTM*233` | same | same |
| `rendering_provider` | `rows_claims.rendering_provider_segment ← index of NM1*82` → entity | title-case; R1 | spec |
| `payer_classification` / `was_forwarded` | `rows_claims.status ← CLP02` | R4: the library's status registry | code table |
| `adj_<n>_{group,code,amount}` | `rows_adjustments ← CAS01, CAS02, CAS03` (CAS in `2110`, **no repeat**: the library reads only the first group of each `CAS`) | pivot | spec (the full groups are `rows_adjustment_groups`, used by `extended`) |
| `ref_<n>_{qual,value}` | `rows_references ← REF01, REF02` (REF in `2110`) | pivot | spec |
| `rem_<n>_{qual,code}` | `rows_remarks ← LQ01, LQ02` (LQ in `2110`) | pivot | spec |
| claim-level values in a service row | the automatic `claim` and `payment` columns of `rows` | gather by row number | L0 |

Format limits found (the `tables` section as of `master`):
- **L0 · No ancestor reads.** A column reads its anchor loop instance or a loop inside it, never an enclosing one, so `rows` (anchored in `2110`) cannot read `CLP`, `BPR` or `N1`. Resolved by the format's own link: one table per loop level, gathered in Python by the automatic `claim`/`payment` reference columns. Rejected (a), "ancestor reads" in the projector: it needs per-open-instance capture state, not a small change.
- **L1 · Null merges "absent" and "written empty".** The library tells them apart by `len(split)`: `NM1*82*2*ACME*****XX*1` gives `" Acme"` (empty `NM104`), `NM1*82*2*ACME` gives `"None Acme"`; `SVC05` absent defaults to 0/1, written empty is `None`. On `united` 5833 rows of `rendering_provider` depend on it. Resolved (b) in Python: `_convert.written` checks the element count of the segment the spec located (the `segment` column, or a `segment_index` column). Rejected (a), an `"empty": "text"` column flag: it covers text only, not the numeric defaults.
- **L2 · No "last segment".** Columns take the first match; the library keeps the *last* `AMT` of a service (and of a claim) and then tests its qualifier. Resolved (b): the long table `rows_service_amounts` and `_views.Service.amount` (its last row), `allowed_amount` compares the library's own description (`"allowed - actual"`). No sample has two `AMT` in one service, so first-`B6` would also pass the gate; R3 keeps the originals honest.
- **L3 · Typed columns lose the text.** `int("1.0")` fails in the library and keeps the text; a `decimal128(38, 2)` column cannot tell `"1.0"` from `"1"`. Resolved with data: the patch retypes `SVC05`/`SVC07` as `AN` for the compat spec only.

Python rules: **R1** presence (`_convert.written`), **R2** dates the library cannot parse (`_convert.date`: a null date cell whose element is written is read from the element text with the library's parser, so an empty `BPR16` gives `""` as in the library; fixture `multi_claim`), **R3** last `AMT`, **R4** code tables (`_convert.STATUSES`, `_codes`: the library's descriptions, MIT-licensed, copied verbatim).

Documented divergences (none occurs in the six samples): the component and element separators (the library guesses per element, oxedi835 reads ISA); a second `NM1*QC`, `NM1*82` or `DTM` of the same qualifier (the library asserts, we take the first); a file with several `ST` (the library keeps the last `BPR` and fails on `payer`, we give one `TransactionSet` per `ST`); an unknown `CLP02` (the library raises `TypeError`, we give `"unknown"`/`False`); a claim without `NM1*QC` (the library raises, `patient` is `None`); trailing blanks before `~` (the library strips each segment, we keep the bytes).

---

## Task 1: The compat patch, its constant in the binding, its goldens

**Implementer tier:** Opus — the JSON is given, but the implementer must read the regenerated goldens and judge them (every table present, the `rows` header as in Facts, counts as in Facts for united/versant).

**Files:**
- Create: `crates/edi835_core/specs/edi_835_parser.json`, `crates/edi835_core/tests/golden/project/edi_835_parser/` (eleven files written by the test)
- Modify: `crates/edi835_core/tests/project_golden.rs`, `crates/oxedi835_py/src/lib.rs`, `crates/oxedi835_py/tests/test_spec.py`

**Interfaces:**
- Consumes: `Spec::builtin_835`, `Spec::merge_patch`, `Processor::run`, `common::{all_files, SUMMARY_ONLY, compare_goldens}`; Python `Spec.builtin().patch(str)`.
- Produces: the file `specs/edi_835_parser.json`; Python `oxedi835._core.EDI_835_PARSER_PATCH: str` (the file's text, embedded at build time).

- [ ] **Step 1: Failing Python test**

Append to `crates/oxedi835_py/tests/test_spec.py`:

```python
COMPAT_TABLES = [
    "rows", "rows_adjustment_groups", "rows_adjustments", "rows_claim_adjustments",
    "rows_claim_amounts", "rows_claim_dates", "rows_claim_entities", "rows_claim_references",
    "rows_claims", "rows_interchanges", "rows_organizations", "rows_payments",
    "rows_provider_adjustments", "rows_references", "rows_remarks", "rows_service_amounts",
    "rows_service_dates",
]


def test_the_edi_835_parser_patch_loads_with_its_tables():
    from oxedi835._core import EDI_835_PARSER_PATCH

    spec = Spec.builtin().patch(EDI_835_PARSER_PATCH)
    assert repr(spec) == "Spec(name='edi_835_parser', loops=8, tables=17)"
    result = oxedi835.parse(read(LARGEST), spec=spec)
    assert result.tables.keys() == COMPAT_TABLES
    assert (len(result.tables["rows"]), len(result.tables["rows_claims"]), len(result.diagnostics)) == (6192, 1332, 0)
```

(Add `import oxedi835` and `from conftest import LARGEST, read` at the top if the file lacks them.)

Run: `make py-test`
Expected: FAIL with `ImportError: cannot import name 'EDI_835_PARSER_PATCH'`.

- [ ] **Step 2: The patch**

Create `crates/edi835_core/specs/edi_835_parser.json`:

```json
{
  "name": "edi_835_parser",
  "segments": {
    "SVC": { "elements": {
      "5": {"name": "units_of_service_paid_count", "type": "AN", "min": 1, "max": 15},
      "7": {"name": "original_units_of_service_count", "type": "AN", "min": 1, "max": 15}
    } }
  },
  "tables": {
    "payments": null,
    "claims": null,
    "services": null,
    "adjustments": null,
    "provider_adjustments": null,
    "rows_interchanges": { "loops": ["interchange"], "ref": "interchange", "columns": {
      "authorization_information_qualifier": {"segment": "ISA", "element": 1},
      "sender": {"segment": "ISA", "element": 6},
      "receiver": {"segment": "ISA", "element": 8},
      "transmission_date": {"segment": "ISA", "element": 9},
      "transmission_time": {"segment": "ISA", "element": 10}
    } },
    "rows_payments": { "loops": ["transaction"], "ref": "payment", "columns": {
      "amount_paid": {"segment": "BPR", "element": 2},
      "bpr_segment": {"segment": "BPR", "segment_index": true},
      "payer_segment": {"loop": "1000A", "segment": "N1", "segment_index": true},
      "payee_segment": {"loop": "1000B", "segment": "N1", "segment_index": true},
      "payment_method": {"segment": "BPR", "element": 4},
      "routing_number": {"segment": "BPR", "element": 13},
      "transaction_date": {"segment": "BPR", "element": 16}
    } },
    "rows_organizations": { "loops": ["1000A", "1000B"], "ref": "organization", "columns": {
      "type": {"segment": "N1", "element": 1},
      "name": {"segment": "N1", "element": 2},
      "identification_code": {"segment": "N1", "element": 4},
      "address": {"segment": "N3", "element": 1},
      "city": {"segment": "N4", "element": 1},
      "state": {"segment": "N4", "element": 2},
      "zip_code": {"segment": "N4", "element": 3}
    } },
    "rows_provider_adjustments": { "loops": ["transaction"], "segment": "PLB", "repeat": {"from": 3, "step": 2}, "columns": {
      "provider_id": {"element": 1},
      "fiscal_period_date": {"element": 2},
      "reason_code": {"group_element": 0, "component": 1},
      "reference_id": {"group_element": 0, "component": 2},
      "amount": {"group_element": 1}
    } },
    "rows_claims": { "loops": ["2100"], "ref": "claim", "columns": {
      "marker": {"segment": "CLP", "element": 1},
      "status": {"segment": "CLP", "element": 2},
      "charge_amount": {"segment": "CLP", "element": 3},
      "paid_amount": {"segment": "CLP", "element": 4},
      "claim_type": {"segment": "CLP", "element": 6},
      "icn": {"segment": "CLP", "element": 7},
      "patient_segment": {"segment": "NM1", "where": {"1": "QC"}, "segment_index": true},
      "rendering_provider_segment": {"segment": "NM1", "where": {"1": "82"}, "segment_index": true},
      "statement_period_start_segment": {"segment": "DTM", "where": {"1": "232"}, "segment_index": true},
      "statement_period_end_segment": {"segment": "DTM", "where": {"1": "233"}, "segment_index": true}
    } },
    "rows_claim_entities": { "loops": ["2100"], "segment": "NM1", "columns": {
      "entity": {"element": 1},
      "type": {"element": 2},
      "last_name": {"element": 3},
      "first_name": {"element": 4},
      "identification_code_qualifier": {"element": 8},
      "identification_code": {"element": 9}
    } },
    "rows_claim_references": { "loops": ["2100"], "segment": "REF", "columns": {
      "qualifier": {"element": 1}, "value": {"element": 2}
    } },
    "rows_claim_dates": { "loops": ["2100"], "segment": "DTM", "columns": {
      "qualifier": {"element": 1}, "date": {"element": 2}
    } },
    "rows_claim_amounts": { "loops": ["2100"], "segment": "AMT", "columns": {
      "qualifier": {"element": 1}, "amount": {"element": 2}
    } },
    "rows_claim_adjustments": { "loops": ["2100"], "segment": "CAS", "repeat": {"from": 2, "step": 3}, "columns": {
      "group": {"element": 1},
      "code": {"group_element": 0},
      "amount": {"group_element": 1},
      "quantity": {"group_element": 2}
    } },
    "rows": { "loops": ["2110"], "ref": "service", "columns": {
      "qualifier": {"segment": "SVC", "element": 1, "component": 1},
      "code": {"segment": "SVC", "element": 1, "component": 2},
      "modifier": {"segment": "SVC", "element": 1, "component": 3},
      "charge_amount": {"segment": "SVC", "element": 2},
      "paid_amount": {"segment": "SVC", "element": 3},
      "allowed_units": {"segment": "SVC", "element": 5},
      "billed_units": {"segment": "SVC", "element": 7},
      "service_date_segment": {"segment": "DTM", "where": {"1": "472"}, "segment_index": true},
      "service_period_start_segment": {"segment": "DTM", "where": {"1": "150"}, "segment_index": true},
      "service_period_end_segment": {"segment": "DTM", "where": {"1": "151"}, "segment_index": true}
    } },
    "rows_service_dates": { "loops": ["2110"], "segment": "DTM", "columns": {
      "qualifier": {"element": 1}, "date": {"element": 2}
    } },
    "rows_service_amounts": { "loops": ["2110"], "segment": "AMT", "columns": {
      "qualifier": {"element": 1}, "amount": {"element": 2}
    } },
    "rows_adjustments": { "loops": ["2110"], "segment": "CAS", "columns": {
      "group": {"element": 1}, "code": {"element": 2}, "amount": {"element": 3}
    } },
    "rows_adjustment_groups": { "loops": ["2110"], "segment": "CAS", "repeat": {"from": 2, "step": 3}, "columns": {
      "group": {"element": 1},
      "code": {"group_element": 0},
      "amount": {"group_element": 1},
      "quantity": {"group_element": 2}
    } },
    "rows_references": { "loops": ["2110"], "segment": "REF", "columns": {
      "qual": {"element": 1}, "value": {"element": 2}
    } },
    "rows_remarks": { "loops": ["2110"], "segment": "LQ", "columns": {
      "qual": {"element": 1}, "code": {"element": 2}
    } }
  }
}
```

`"payments": null` and the four other `null`s delete the native tables (JSON Merge Patch), so the compat parse builds only what the compat layer reads, and the `ref` names `payment`, `claim`, `service` are free for the `rows*` tables. The `segments.SVC` member retypes `SVC05` and `SVC07` as text for this spec only (limit L3 in Facts). Columns ending in `_segment` are `segment_index` sources: they point at the segment the qualifier selects, and the Python layer reads that segment's row of a long table.

- [ ] **Step 3: The constant**

In `crates/oxedi835_py/src/lib.rs`, inside `core_module`, before `Ok(())`:

```rust
    m.add(
        "EDI_835_PARSER_PATCH",
        include_str!("../../edi835_core/specs/edi_835_parser.json"),
    )?;
```

and extend the module doc comment's first paragraph with: "It also carries the text of the core's `edi_835_parser.json` patch, so the Python layer that reproduces edi-835-parser applies the same file the core's goldens test."

Run: `make py-test`
Expected: PASS, 113 tests.

- [ ] **Step 4: Failing golden test**

In `crates/edi835_core/tests/project_golden.rs`: in `tables_and_diagnostics_match_the_golden_files` change `common::compare_goldens(&golden_dir(), &outputs, &[])` to `common::compare_goldens(&golden_dir(), &outputs, &["edi_835_parser"])`; extend the module doc with "The tables of the `edi_835_parser.json` patch have their own goldens in `tests/golden/project/edi_835_parser/`, in the same two formats."; append:

```rust
#[test]
fn edi_835_parser_tables_match_the_golden_files() {
    let spec = Spec::builtin_835()
        .merge_patch(include_str!("../specs/edi_835_parser.json"))
        .unwrap();
    let dir = golden_dir().join("edi_835_parser");
    let mut outputs = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, _) = Processor::run(&spec, &document);
        outputs.push(if common::SUMMARY_ONLY.contains(&name.as_str()) {
            (
                dir.join(format!("{name}.tables.summary.txt")),
                row_counts(&tables),
            )
        } else {
            (dir.join(format!("{name}.tables.txt")), all_rows(&tables))
        });
    }
    let failures = common::compare_goldens(&dir, &outputs, &[]);
    assert!(failures.is_empty(), "{failures:#?}");
}
```

Run: `cargo test -p edi835_core --test project_golden`
Expected: FAIL, panic `…/golden/project/edi_835_parser/<file>.tables.txt: No such file or directory …; run with UPDATE_GOLDEN=1 to create it`.

- [ ] **Step 5: Generate and inspect**

Run: `UPDATE_GOLDEN=1 cargo test -p edi835_core --test project_golden && cargo test -p edi835_core --test project_golden`
Expected: both pass; eleven new files. Inspect: `edi835_test_united.rmt.tables.summary.txt` and `edi835_test_versant.RMT.tables.summary.txt` list the seventeen tables with the row counts of the Facts table; `emedny_sample.txt.tables.txt` has `## rows (rows: 10)` with the header given in Facts; `git status` shows no change under `tests/golden/project/*.txt` (the native goldens are untouched).

- [ ] **Step 6: Gates and commit**

Run: `make gates && make py-test`
Expected: green; cargo tests = previous count + 1; pytest 113.

```bash
git add crates/edi835_core/specs/edi_835_parser.json crates/edi835_core/tests/project_golden.rs crates/edi835_core/tests/golden/project/edi_835_parser crates/oxedi835_py/src/lib.rs crates/oxedi835_py/tests/test_spec.py
git commit -m "spec: edi_835_parser patch with the tables that reproduce edi-835-parser's frame; goldens; embedded in the binding"
```

---

## Task 2: `oxedi835.edi_835_parser` — parse, objects, strict frame, parity tests

**Implementer tier:** Opus — the code is given and ran on the scratch copy, but parity is the stage's proof: the implementer must keep every value flowing from a spec column, recognise the four rules R1–R4 and not "simplify" them away (each one is load-bearing on the samples or on the inline file of Task 3), and judge any difference against the library if pandas or the library resolve to other versions.

**Files:**
- Create: `crates/oxedi835_py/python/oxedi835/edi_835_parser/{__init__,_tables,_convert,_codes,_views,_sets}.py`, `crates/oxedi835_py/tests/n104_shim.py`, `crates/oxedi835_py/tests/test_edi_835_parser.py`

**Interfaces:**
- Consumes: `oxedi835.Spec`, `oxedi835.parse_file`, `oxedi835._core.EDI_835_PARSER_PATCH`, `Result.tables` (Arrow by `pyarrow.table`), `Result.document` (`Segment.elements` for the presence rule).
- Produces (Python): `oxedi835.edi_835_parser.parse(path, debug: bool = False) -> TransactionSets`, a drop-in replacement with the library's exact signature and behaviour: a file path or a directory (its `.txt`/`.835`/`.DAT` filter and `os.listdir` order), nothing else; reading from memory is only through the additions `parse_bytes(data, file_path: str | None = None)`, `parse_file_obj(file, file_path: str | None = None)`, `parse_many(items)` (bytes-like or file objects, in order); `file_path` is what each `TransactionSet.file_path` reports (default `"<bytes>"`; the path itself for path inputs). Every route ends in one `oxedi835.parse(data, spec=spec())`, which runs with the GIL released; a path is read once in binary mode. `spec() -> Spec`; `TransactionSets` (`__iter__`, `__len__`, `__repr__`, `to_dataframe()`, `sort_columns(df)` static, `sum_payments() -> float`, `count_claims() -> int`, `count_patients() -> int`); `TransactionSet` (`interchange`, `financial_information`, `organizations`, `claims`, `payer`, `payee`, `file_path`, `to_dataframe()`, `serialize_service(...)` static, `service_record(...)` classmethod); `Claim` (`claim`, `entities`, `services`, `references`, `dates`, `amount`, `patient`, `rendering_provider`, `claim_statement_period_start/end`); `Service` (`service`, `dates`, `references`, `remarks`, `amount`, `adjustments`, `allowed_amount`, `service_date`, `service_period_start/end`); frozen dataclasses `Interchange`, `FinancialInformation`, `Organization`, `OrganizationSegment`, `Address`, `Location`, `Entity`, `Reference`, `Date`, `Amount`, `Remark`, `ServiceAdjustment`, `ClaimSegment`, `ServiceSegment`, `Code`, `Status`, enum `PayerClassification`.
- One `TransactionSet` per `ST`…`SE` (the library makes one per file; identical on single-transaction files, and it is what makes `payer`/`transaction_date` right on files with several).

- [ ] **Step 1: The shim and the failing tests**

Create `crates/oxedi835_py/tests/n104_shim.py`:

```python
"""Lets edi-835-parser read payer ids that are not numeric.

The library converts ``N104`` with ``int()``, which fails on the alphanumeric
ids that X12 allows (qualifier ``XV``). ``apply`` replaces the segment
constructor with one that keeps ``N104`` as text; nothing else changes.
"""

from edi_835_parser.segments import organization
from edi_835_parser.segments.utilities import split_segment


def _init(self, segment):
    self.segment = segment
    elements = split_segment(segment)
    self.identifier = elements[0]
    self.type = elements[1]
    self.name = elements[2]
    self.identification_code = elements[4] if len(elements) >= 5 else None


def apply():
    organization.Organization.__init__ = _init
```

Create `crates/oxedi835_py/tests/test_edi_835_parser.py`:

```python
import os
import shutil
import warnings

import edi_835_parser
import n104_shim
import pandas as pd
import pytest

from conftest import SAMPLES, path_of
from oxedi835 import edi_835_parser as compat

n104_shim.apply()

SHAPES = {
    "edi835_test_davisvision.RMT": (2, 23),
    "edi835_test_eyemed.RMT": (414, 21),
    "edi835_test_file.RMT": (23, 21),
    "edi835_test_not_available_claim_id.RMT": (26, 23),
    "edi835_test_united.rmt": (6192, 26),
    "edi835_test_versant.RMT": (1778, 28),
}
# claims, patients, BPR02
COUNTS = {
    "edi835_test_davisvision.RMT": (1, 1, 0.0),
    "edi835_test_eyemed.RMT": (82, 82, 8982.0),
    "edi835_test_file.RMT": (4, 4, 8982.0),
    "edi835_test_not_available_claim_id.RMT": (18, 5, 3715.0),
    "edi835_test_united.rmt": (1332, 1212, 173305.0),
    "edi835_test_versant.RMT": (648, 417, 123950.65),
}


def old(path):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return edi_835_parser.parse(str(path))


def organization(org):
    return (
        org.organization.type,
        org.organization.name,
        org.organization.identification_code,
        org.address.address if org.address else None,
        (org.location.city, org.location.state, org.location.zip_code) if org.location else None,
    )


@pytest.fixture(params=SAMPLES)
def sample(request):
    return request.param


def by_path(path):
    return compat.parse(path)


def by_bytes(path):
    return compat.parse_bytes(path.read_bytes())


def by_file(path):
    with open(path, "rb") as handle:
        return compat.parse_file_obj(handle)


@pytest.mark.parametrize("route", [by_path, by_bytes, by_file])
def test_the_frame_equals_edi_835_parser_cell_for_cell(sample, route):
    expected = old(path_of(sample)).to_dataframe()
    actual = route(path_of(sample)).to_dataframe()
    pd.testing.assert_frame_equal(actual, expected, check_exact=True)
    assert actual.shape == SHAPES[sample]


def test_path_bytes_memoryview_and_file_give_the_same_frame(sample):
    path = path_of(sample)
    frames = [route(path).to_dataframe() for route in (by_path, by_bytes, by_file)]
    frames.append(compat.parse_bytes(memoryview(bytearray(path.read_bytes()))).to_dataframe())
    with open(path, "rb") as handle:
        frames.append(compat.parse_file_obj(handle, file_path="x.835").to_dataframe())
    for frame in frames[1:]:
        pd.testing.assert_frame_equal(frame, frames[0], check_exact=True)


def test_file_path_is_kept_or_defaults_to_bytes():
    data = path_of(SAMPLES[0]).read_bytes()
    assert [t.file_path for t in compat.parse_bytes(data)] == ["<bytes>"]
    assert [t.file_path for t in compat.parse_bytes(data, file_path="a.835")] == ["a.835"]
    assert [t.file_path for t in compat.parse(path_of(SAMPLES[0]))] == [str(path_of(SAMPLES[0]))]


def test_parse_many_equals_the_directory(tmp_path):
    for name in SAMPLES:
        shutil.copy(path_of(name), tmp_path / f"{name}.txt")
    order = [tmp_path / n for n in os.listdir(tmp_path)]
    expected = old(tmp_path).to_dataframe()
    with open(order[0], "rb") as first:
        actual = compat.parse_many([first, *(p.read_bytes() for p in order[1:])]).to_dataframe()
    pd.testing.assert_frame_equal(actual, expected, check_exact=True)


def test_counts_and_sums_equal_edi_835_parser(sample):
    expected, actual = old(path_of(sample)), compat.parse(path_of(sample))
    assert (actual.count_claims(), actual.count_patients(), actual.sum_payments()) == (
        expected.count_claims(), expected.count_patients(), expected.sum_payments())
    assert actual.count_claims() == COUNTS[sample][0]
    assert actual.count_patients() == COUNTS[sample][1]
    assert actual.sum_payments() == pytest.approx(COUNTS[sample][2], abs=0.005)


def test_payer_payee_and_segments_equal_edi_835_parser(sample):
    (expected,), (actual,) = list(old(path_of(sample))), list(compat.parse(path_of(sample)))
    assert organization(actual.payer) == organization(expected.payer)
    assert organization(actual.payee) == organization(expected.payee)
    for field in ("amount_paid", "payment_method", "routing_number", "transaction_date"):
        assert getattr(actual.financial_information, field) == getattr(expected.financial_information, field)
    for field in ("authorization_information_qualifier", "sender", "receiver", "transmission_date"):
        assert getattr(actual.interchange, field) == getattr(expected.interchange, field)


def test_claim_and_service_objects_equal_edi_835_parser(sample):
    (expected,), (actual,) = list(old(path_of(sample))), list(compat.parse(path_of(sample)))
    assert len(actual.claims) == len(expected.claims)
    for a, e in zip(actual.claims, expected.claims):
        assert (a.claim.marker, a.claim.icn, a.claim.charge_amount, a.claim.paid_amount, a.claim.claim_type) == (
            e.claim.marker, e.claim.icn, e.claim.charge_amount, e.claim.paid_amount, e.claim.claim_type)
        assert [(x.entity, x.type, x.name, x.identification_code_qualifier, x.identification_code)
                for x in a.entities] == [
            (x.entity, x.type, x.name, x.identification_code_qualifier, x.identification_code)
            for x in e.entities]
        assert (str(a.claim.status.payer_classification), a.claim.status.was_forwarded, a.claim.status.description) == (
            str(e.claim.status.payer_classification), e.claim.status.was_forwarded, e.claim.status.description)
        assert [(d.qualifier, d.date) for d in a.dates] == [(d.qualifier, d.date) for d in e.dates]
        assert [str(r) for r in a.references] == [str(r) for r in e.references]
        amounts = [(x.amount.qualifier, x.amount.amount) if x.amount else None for x in (a, e)]
        assert amounts[0] == amounts[1]
        assert len(a.services) == len(e.services)
        for s, t in zip(a.services, e.services):
            assert [(str(x.group_code), str(x.reason_code), x.amount) for x in s.adjustments] == [
                (str(x.group_code), str(x.reason_code), x.amount) for x in t.adjustments]
            assert [(d.qualifier, d.date) for d in s.dates] == [(d.qualifier, d.date) for d in t.dates]
            assert [(str(r.qualifier), str(r.code)) for r in s.remarks] == [(str(r.qualifier), str(r.code)) for r in t.remarks]
            assert [str(r) for r in s.references] == [str(r) for r in t.references]
            assert s.allowed_amount == t.allowed_amount


def test_a_directory_reads_the_same_files_in_the_same_order(tmp_path):
    for name in SAMPLES:
        shutil.copy(path_of(name), tmp_path / f"{name}.txt")
    (tmp_path / "ignored.RMT").write_bytes(path_of(SAMPLES[0]).read_bytes())
    expected, actual = old(tmp_path), compat.parse(tmp_path)
    assert len(actual) == len(expected) == 6
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    assert actual.sum_payments() == expected.sum_payments()
```

Run: `make py-test`
Expected: FAIL at collection, `ModuleNotFoundError: No module named 'oxedi835.edi_835_parser'` (with `edi-835-parser` and `pandas` installed in `.venv` as said at the top).

- [ ] **Step 2: The package**

Create `crates/oxedi835_py/python/oxedi835/edi_835_parser/__init__.py`:

```python
"""edi-835-parser's API over oxedi835."""

from ._sets import (
    TransactionSet, TransactionSets, parse, parse_bytes, parse_file_obj, parse_many,
)
from ._tables import spec

__all__ = [
    "TransactionSet", "TransactionSets", "parse", "parse_bytes", "parse_file_obj",
    "parse_many", "spec",
]
```

`_tables.py` — one parse with the compat spec, columns as Python lists, split per transaction:

```python
"""The compat spec, and its tables as Python columns split per transaction."""

from __future__ import annotations

import functools
from collections import defaultdict

from .. import Spec, parse
from .._core import EDI_835_PARSER_PATCH


@functools.cache
def spec() -> Spec:
    """The built-in spec with the edi-835-parser tables in place of the native ones."""
    return Spec.builtin().patch(EDI_835_PARSER_PATCH)


class Rows:
    """One table's rows, as columns of Python values."""

    def __init__(self, columns, keep):
        self.columns = {key: [v for v, k in zip(values, keep) if k] for key, values in columns.items()}
        self._groups = {}

    def __len__(self):
        return len(self.columns["row"])

    def __getitem__(self, column):
        return self.columns[column]

    def under(self, parent, key):
        """Positions of the rows whose ``parent`` column is ``key``, in file order."""
        if parent not in self._groups:
            groups = defaultdict(list)
            for at, value in enumerate(self.columns[parent]):
                groups[value].append(at)
            self._groups[parent] = groups
        return self._groups[parent].get(key, [])


def load(data):
    """Parses ``data`` (bytes or any buffer) with the compat spec, with the GIL
    released, and returns the document and, per transaction, the tables
    limited to its rows."""
    import pyarrow as pa

    result = parse(data, spec=spec())
    tables = {name: pa.table(result.tables[name]).to_pydict() for name in result.tables}
    payments = tables["rows_payments"]
    parts = []
    for payment, interchange in zip(payments["row"], payments["interchange"]):
        part = {}
        for name, columns in tables.items():
            if name == "rows_interchanges":
                keep = [row == interchange for row in columns["row"]]
            elif name == "rows_payments":
                keep = [row == payment for row in columns["row"]]
            else:
                keep = [owner == payment for owner in columns["payment"]]
            part[name] = Rows(columns, keep)
        parts.append(part)
    return result.document, parts
```

`_convert.py` — the conversions and rules R1, R2, R4 (status registry):

```python
"""Value conversions that reproduce edi-835-parser's element parsers."""

from __future__ import annotations

import datetime

# Claim status codes: (payer classification, forwarded to another payer).
STATUSES = {
    "1": ("processed as primary", "primary", False),
    "2": ("processed as secondary", "secondary", False),
    "3": ("processed as tertiary", "tertiary", False),
    "4": ("denial", "unspecified", False),
    "19": ("processed as primary, forwarded to additional payer(s)", "primary", True),
    "20": ("processed as secondary, forwarded to additional payer(s)", "secondary", True),
    "21": ("processed as tertiary, forwarded to additional payer(s)", "tertiary", True),
    "22": ("reversal of previous payment", "unspecified", False),
}
UNKNOWN_STATUS = ("uncategorized", "unknown", False)


def text(value):
    """Text cell as ``str``; null stays ``None``."""
    return None if value is None else value.decode("utf-8")


def money(value):
    """Decimal cell as ``float``, as the library's ``float(text)``."""
    return None if value is None else float(value)


def moment(value):
    """Date cell as a midnight ``datetime``, as the library's date parser."""
    return None if value is None else datetime.datetime(value.year, value.month, value.day)


def library_date(raw):
    """edi-835-parser's date parser on the element text: ``YYMMDDHHMM`` and
    ``CCYYMMDD`` become a ``datetime``, anything else stays text."""
    if len(raw) == 10:
        year, month, day, hour, minute = (int(raw[i:i + 2]) for i in range(0, 10, 2))
        return datetime.datetime(2000 + year, month, day, hour, minute)
    if len(raw) == 8:
        return datetime.datetime(int(raw[:4]), int(raw[4:6]), int(raw[6:]))
    return raw


def date(value, document, index, element):
    """Date cell as ``datetime``; a null cell whose element is written is
    read from the element text as the library reads it."""
    if value is not None or index is None:
        return moment(value)
    elements = document[index].elements
    if element > len(elements):
        return None
    raw = elements[element - 1]
    return library_date(raw.decode("utf-8") if isinstance(raw, bytes) else "")


def integer(value):
    """Text cell as ``int`` when it reads as one, else the text."""
    if value is None:
        return None
    value = text(value)
    try:
        return int(value)
    except ValueError:
        return value


def has(document, index, element, component=None):
    """Whether segment ``index`` holds the element (and component), even empty."""
    if index is None:
        return False
    elements = document[index].elements
    if element > len(elements):
        return False
    if component is None:
        return True
    value = elements[element - 1]
    return component <= (len(value) if isinstance(value, list) else 1)


def written(value, document, index, element, component=None, empty="", absent=None):
    """``value``, or for a null cell ``empty`` when the element is written
    empty and ``absent`` when the segment stops before it."""
    if value is not None:
        return value
    return empty if has(document, index, element, component) else absent


def name(first, last):
    """``"first last"`` title-cased, with ``None`` spelled out as the library does."""
    return f"{first} {last}".title()
```

`_codes.py` — the library's description tables, verbatim:

```python
"""Code descriptions edi-835-parser attaches to the codes it reads.

The tables are copied verbatim from edi-835-parser 1.8.0 (MIT License), so the
objects of this package carry the same descriptions.
"""

ADJUSTMENT_GROUPS = {
    "CR": "corrections and reversals",
    "OA": "other adjustment",
    "PR": "patient responsibility",
    "CO": "contractual obligation",
    "PI": "payor initiated reduction",
}
DATE_QUALIFIERS = {
    "050": "received",
    "150": "service period start",
    "151": "service period end",
    "472": "service",
    "232": "claim statement period start",
    "233": "claim statement period end",
}
ENTITY_CODES = {"QC": "patient", "74": "insured", "82": "rendering provider", "85": "billing provider"}
ENTITY_TYPES = {"1": "person", "2": "entity"}
IDENTIFICATION_QUALIFIERS = {
    "MI": "member identification number",
    "C": "insured's changed unique identification number",
    "PC": "provider commercial number",
    "XX": "national provider id",
}
AMOUNT_QUALIFIERS = {"B6": "allowed - actual", "AU": "coverage amount"}
ORGANIZATION_TYPES = {"PE": "payee", "PR": "payer"}
PAYMENT_METHODS = {"ACH": "automatic deposit", "CHK": "check", "NON": "no payment"}
ADJUSTMENT_REASONS = {
    '45': 'Charge exceeds fee schedule maximum allowable or contracted/legislated fee arrangement.',
    '243': 'Services not authorized by network/primary care providers.',
    '29': 'The time limit for filing has expired.',
    '251': 'The attachment/other documentation that was received was incomplete or deficient.',
    '2': 'Coinsurnace Amount.',
    '96': 'Non-covered charge(s). See remark code.',
    '3': 'Co-payment Amount.',
    '16': 'Claim/service lacks information or has submission/billing error(s).',
    'B15': 'This service/procedure requires that a qualifying service/procedure be received and covered. The qualifying other service/procedure has not been received/adjudicated.',
    'A1': 'Claim/Service denied. See remark code.',
    '1': 'Deductible Amount',
    '4': 'The procedure code is inconsistent with the modifier used. Usage: Refer to the 835 Healthcare Policy Identification Segment (loop 2110 Service Payment Information REF), if present.',
    '18': "Exact duplicate claim/service (Use only with Group Code OA except where state workers' compensation regulations requires CO)",
    '23': 'The impact of prior payer(s) adjudication including payments and/or adjustments. (Use only with Group Code OA)',
    '26': 'Expenses incurred prior to coverage.',
    '27': 'Expenses incurred after coverage terminated.',
    '97': 'The benefit for this service is included in the payment/allowance for another service/procedure that has already been adjudicated. Usage: Refer to the 835 Healthcare Policy Identification Segment (loop 2110 Service Payment Information REF), if present.',
    '109': 'Claim/service not covered by this payer/contractor. You must send the claim/service to the correct payer/contractor.',
    '151': 'Payment adjusted because the payer deems the information submitted does not support this many/frequency of services.',
    '234': 'This procedure is not paid separately. At least one Remark Code must be provided (may be comprised of either the NCPDP Reject Reason Code, or Remittance Advice Remark Code that is not an ALERT.)',
    '272': 'Coverage/program guidelines were not met.',
}
REMARK_CODES = {
    'N630': 'Referral not authorized by attending physician.',
    'N650': 'This policy was not in effect for this date of loss. No coverage is available.',
    'M53': 'Missing/incomplete/invalid days or units of service.',
    'M15': 'Separately billed services/tests have been bundled as they are considered components of the same procedure. Separate payment is not allowed.',
    'M80': 'Not covered when performed during the same session/date as a previously processed service for the patient.',
    'M86': 'Service denied because payment already made for same/similar procedure within set time frame.',
    'MA130': 'Your claim contains incomplete and/or invalid information, and no appeal rights are afforded because the claim is unprocessable. Please submit a new claim with the complete/correct information.',
    'N122': 'Add-on code cannot be billed by itself.',
    'N20': 'Service not payable with other service rendered on the same date.',
    'N6': 'Under FEHB law (U.S.C. 8904(b)), we cannot pay more for covered care than the amount Medicare would have allowed if the patient were enrolled in Medicare Part A and/or Medicare Part B.',
    'N640': 'Exceeds number/frequency approved/allowed within time period.',
    'N674': 'Not covered unless a pre-requisite procedure/service has been provided.',
    'N702': 'Decision based on review of previously adjudicated claims or for claims in process for the same/similar type of services.',
    'N781': 'Alert: Patient is a Medicaid/ Qualified Medicare Beneficiary. Review your records for any wrongfully collected deductible. This amount may be billed to a subsequent payer.',
    'N782': 'Alert: Patient is a Medicaid/ Qualified Medicare Beneficiary. Review your records for any wrongfully collected coinsurance. This amount may be billed to a subsequent payer.',
    'N807': 'Payment adjustment based on the Merit-based Incentive Payment System (MIPS).',
}
REFERENCE_QUALIFIERS = {
    "6R": "provider control number",
    "0K": "policy form identifying number",
    "PQ": "payee identification",
    "TJ": "federal taxpayer identification number",
    "LU": "location number",
}
REMARK_QUALIFIERS = {"HE": "claim payment"}
```

`_views.py` — the objects; `at_segment` is how a pointer column of the spec picks one row of a long table; `Service.amount` (last row) is rule R3:

```python
"""Read-only objects shaped like edi-835-parser's, over the compat tables."""

from __future__ import annotations

import dataclasses
import enum
import functools
from typing import Optional

from . import _codes
from ._convert import (
    STATUSES, UNKNOWN_STATUS, date, integer, money, name, text, written,
)


class PayerClassification(enum.Enum):
    PRIMARY = enum.auto()
    SECONDARY = enum.auto()
    TERTIARY = enum.auto()
    UNSPECIFIED = enum.auto()
    UNKNOWN = enum.auto()

    def __str__(self) -> str:
        return self.name.lower()


@dataclasses.dataclass(frozen=True)
class Code:
    code: Optional[str]
    description: Optional[str]

    def __str__(self) -> str:
        return str({"code": self.code, "description": self.description})


@dataclasses.dataclass(frozen=True)
class Status:
    code: str
    description: str
    payer_classification: PayerClassification
    was_forwarded: bool


def status(code):
    description, classification, forwarded = STATUSES.get(code, UNKNOWN_STATUS)
    return Status(code, description, PayerClassification[classification.upper()], forwarded)


def coded(table, value):
    return Code(value, table.get(value))


def mapped(table, value):
    return table.get(value, value)


@dataclasses.dataclass(frozen=True)
class Interchange:
    authorization_information_qualifier: Optional[str]
    sender: str
    receiver: str
    transmission_date: object


@dataclasses.dataclass(frozen=True)
class FinancialInformation:
    amount_paid: Optional[float]
    payment_method: Optional[str]
    routing_number: object
    transaction_date: object


@dataclasses.dataclass(frozen=True)
class OrganizationSegment:
    type: str
    name: Optional[str]
    identification_code: Optional[str]


@dataclasses.dataclass(frozen=True)
class Address:
    address: Optional[str]


@dataclasses.dataclass(frozen=True)
class Location:
    city: Optional[str]
    state: Optional[str]
    zip_code: Optional[str]


@dataclasses.dataclass(frozen=True)
class Organization:
    organization: OrganizationSegment
    location: Optional[Location]
    address: Optional[Address]
    index: Optional[int] = dataclasses.field(default=None, compare=False, repr=False)


@dataclasses.dataclass(frozen=True)
class Entity:
    entity: str
    type: str
    last_name: Optional[str]
    first_name: Optional[str]
    identification_code_qualifier: Optional[str]
    identification_code: Optional[str]
    index: Optional[int] = dataclasses.field(default=None, compare=False, repr=False)

    @property
    def name(self) -> str:
        return name(self.first_name, self.last_name)


@dataclasses.dataclass(frozen=True)
class Reference:
    qualifier: Code
    value: Optional[str]

    def __str__(self) -> str:
        return f"{self.qualifier}: {self.value}"


@dataclasses.dataclass(frozen=True)
class Date:
    qualifier: str
    date: object
    index: Optional[int] = dataclasses.field(default=None, compare=False, repr=False)


@dataclasses.dataclass(frozen=True)
class Amount:
    qualifier: str
    amount: Optional[float]


@dataclasses.dataclass(frozen=True)
class Remark:
    qualifier: Code
    code: Code


@dataclasses.dataclass(frozen=True)
class ServiceAdjustment:
    group_code: Code
    reason_code: Code
    amount: Optional[float]


@dataclasses.dataclass(frozen=True)
class ClaimSegment:
    marker: str
    status: Status
    charge_amount: Optional[float]
    paid_amount: Optional[float]
    claim_type: Optional[str]
    icn: Optional[str]


@dataclasses.dataclass(frozen=True)
class ServiceSegment:
    code: Optional[str]
    qualifier: Optional[str]
    modifier: Optional[str]
    charge_amount: Optional[float]
    paid_amount: Optional[float]
    allowed_units: object
    billed_units: object


def at_segment(items, index):
    """The item read from segment ``index``; ``None`` when ``index`` is."""
    if index is None:
        return None
    return next((item for item in items if item.index == index), None)


class Service:
    """One service line (loop 2110)."""

    def __init__(self, tables, document, at):
        self._t, self._d, self._at = tables, document, at
        self._row = tables["rows"]["row"][at]

    def _under(self, table):
        return self._t[table], self._t[table].under("service", self._row)

    @functools.cached_property
    def service(self) -> ServiceSegment:
        rows, at, doc = self._t["rows"], self._at, self._d
        segment = rows["segment"][at]
        paid = money(rows["paid_amount"][at])
        allowed = written(integer(rows["allowed_units"][at]), doc, segment, 5,
                          empty=None, absent=0 if paid == 0 else 1)
        billed = written(integer(rows["billed_units"][at]), doc, segment, 7,
                         empty=None, absent=allowed)
        return ServiceSegment(
            code=text(rows["code"][at]),
            qualifier=text(rows["qualifier"][at]),
            modifier=written(text(rows["modifier"][at]), doc, segment, 1, 3),
            charge_amount=money(rows["charge_amount"][at]),
            paid_amount=paid,
            allowed_units=allowed,
            billed_units=billed,
        )

    @functools.cached_property
    def dates(self):
        t, ats = self._under("rows_service_dates")
        return [Date(mapped(_codes.DATE_QUALIFIERS, text(t["qualifier"][i])),
                     date(t["date"][i], self._d, t["segment"][i], 2), t["segment"][i]) for i in ats]

    @functools.cached_property
    def references(self):
        t, ats = self._under("rows_references")
        return [Reference(coded(_codes.REFERENCE_QUALIFIERS, text(t["qual"][i])), text(t["value"][i])) for i in ats]

    @functools.cached_property
    def remarks(self):
        t, ats = self._under("rows_remarks")
        return [Remark(coded(_codes.REMARK_QUALIFIERS, text(t["qual"][i])), coded(_codes.REMARK_CODES, text(t["code"][i]))) for i in ats]

    @functools.cached_property
    def amount(self):
        t, ats = self._under("rows_service_amounts")
        if not ats:
            return None
        i = ats[-1]
        return Amount(mapped(_codes.AMOUNT_QUALIFIERS, text(t["qualifier"][i])), money(t["amount"][i]))

    @functools.cached_property
    def adjustments(self):
        t, ats = self._under("rows_adjustments")
        return [ServiceAdjustment(coded(_codes.ADJUSTMENT_GROUPS, text(t["group"][i])),
                                  coded(_codes.ADJUSTMENT_REASONS, text(t["code"][i])), money(t["amount"][i])) for i in ats]

    @property
    def allowed_amount(self):
        if self.amount and self.amount.qualifier == "allowed - actual":
            return self.amount.amount
        return None

    def _date(self, column):
        return at_segment(self.dates, self._t["rows"][column][self._at])

    @property
    def service_date(self):
        return self._date("service_date_segment")

    @property
    def service_period_start(self):
        return self._date("service_period_start_segment") or self.service_date

    @property
    def service_period_end(self):
        return self._date("service_period_end_segment") or self.service_date


class Claim:
    """One claim (loop 2100) with its services."""

    def __init__(self, tables, document, at):
        self._t, self._d, self._at = tables, document, at
        self._row = tables["rows_claims"]["row"][at]

    def _under(self, table):
        return self._t[table], self._t[table].under("claim", self._row)

    @functools.cached_property
    def claim(self) -> ClaimSegment:
        c, at = self._t["rows_claims"], self._at
        return ClaimSegment(
            marker=text(c["marker"][at]),
            status=status(text(c["status"][at])),
            charge_amount=money(c["charge_amount"][at]),
            paid_amount=money(c["paid_amount"][at]),
            claim_type=text(c["claim_type"][at]),
            icn=text(c["icn"][at]),
        )

    @functools.cached_property
    def entities(self):
        t, ats = self._under("rows_claim_entities")
        out = []
        for i in ats:
            segment = t["segment"][i]
            out.append(Entity(
                entity=mapped(_codes.ENTITY_CODES, text(t["entity"][i])),
                type=mapped(_codes.ENTITY_TYPES, text(t["type"][i])),
                last_name=text(t["last_name"][i]) or "",
                first_name=written(text(t["first_name"][i]), self._d, segment, 4),
                identification_code_qualifier=mapped(_codes.IDENTIFICATION_QUALIFIERS,
                                                     written(text(t["identification_code_qualifier"][i]), self._d, segment, 8)),
                identification_code=written(text(t["identification_code"][i]), self._d, segment, 9),
                index=segment,
            ))
        return out

    @functools.cached_property
    def services(self):
        rows = self._t["rows"]
        return [Service(self._t, self._d, i) for i in rows.under("claim", self._row)]

    @functools.cached_property
    def references(self):
        t, ats = self._under("rows_claim_references")
        return [Reference(coded(_codes.REFERENCE_QUALIFIERS, text(t["qualifier"][i])), text(t["value"][i])) for i in ats]

    @functools.cached_property
    def dates(self):
        t, ats = self._under("rows_claim_dates")
        return [Date(mapped(_codes.DATE_QUALIFIERS, text(t["qualifier"][i])),
                     date(t["date"][i], self._d, t["segment"][i], 2), t["segment"][i]) for i in ats]

    @functools.cached_property
    def amount(self):
        t, ats = self._under("rows_claim_amounts")
        if not ats:
            return None
        i = ats[-1]
        return Amount(mapped(_codes.AMOUNT_QUALIFIERS, text(t["qualifier"][i])), money(t["amount"][i]))

    def _pointer(self, items, column):
        return at_segment(items, self._t["rows_claims"][column][self._at])

    @property
    def patient(self):
        return self._pointer(self.entities, "patient_segment")

    @property
    def rendering_provider(self):
        return self._pointer(self.entities, "rendering_provider_segment")

    @property
    def claim_statement_period_start(self):
        return self._pointer(self.dates, "statement_period_start_segment")

    @property
    def claim_statement_period_end(self):
        return self._pointer(self.dates, "statement_period_end_segment")
```

`_sets.py` — `parse`, the two collections, the library's `serialize_service` and numbering:

```python
"""``parse``, ``TransactionSets`` and ``TransactionSet``."""

from __future__ import annotations

import datetime
import functools
import os
import warnings
from typing import Iterator, List, Optional

from . import _codes
from ._convert import date, integer, money, text
from ._tables import load
from ._views import (
    Address, Claim, FinancialInformation, Interchange, Location, Organization,
    OrganizationSegment, at_segment, mapped,
)

_SUFFIXES = (".txt", ".835", ".DAT")


class TransactionSet:
    """One transaction (ST to SE) of one file."""

    def __init__(self, document, tables, file_path):
        self._d, self._t, self.file_path = document, tables, file_path

    @functools.cached_property
    def interchange(self):
        t = self._t["rows_interchanges"]
        if not len(t):
            return None
        day, clock = t["transmission_date"][0], t["transmission_time"][0]
        sent = None if day is None or clock is None else datetime.datetime.combine(day, clock)
        qualifier = text(t["authorization_information_qualifier"][0])
        return Interchange(
            authorization_information_qualifier=None if qualifier == "00" else qualifier,
            sender=text(t["sender"][0]).strip(),
            receiver=text(t["receiver"][0]).strip(),
            transmission_date=sent,
        )

    @functools.cached_property
    def financial_information(self):
        t = self._t["rows_payments"]
        return FinancialInformation(
            amount_paid=money(t["amount_paid"][0]),
            payment_method=mapped(_codes.PAYMENT_METHODS, text(t["payment_method"][0])),
            routing_number=integer(t["routing_number"][0]),
            transaction_date=date(t["transaction_date"][0], self._d, t["bpr_segment"][0], 16),
        )

    @functools.cached_property
    def organizations(self) -> List[Organization]:
        t = self._t["rows_organizations"]
        out = []
        for i in range(len(t)):
            location = None
            if any(t[k][i] is not None for k in ("city", "state", "zip_code")):
                location = Location(text(t["city"][i]), text(t["state"][i]), text(t["zip_code"][i]))
            address = None if t["address"][i] is None else Address(text(t["address"][i]))
            out.append(Organization(
                OrganizationSegment(mapped(_codes.ORGANIZATION_TYPES, text(t["type"][i])),
                                    text(t["name"][i]), text(t["identification_code"][i])),
                location, address, t["segment"][i]))
        return out

    @functools.cached_property
    def claims(self) -> List[Claim]:
        return [Claim(self._t, self._d, i) for i in range(len(self._t["rows_claims"]))]

    def _organization(self, role):
        found = at_segment(self.organizations, self._t["rows_payments"][f"{role}_segment"][0])
        if found is None:
            raise ValueError(
                f"{self.file_path}: the transaction at segment "
                f"{self._t['rows_payments']['segment'][0]} has no {role} loop (N1)"
            )
        return found

    @property
    def payer(self) -> Organization:
        return self._organization("payer")

    @property
    def payee(self) -> Organization:
        return self._organization("payee")

    def __repr__(self):
        return "\n".join(str(item) for item in (
            ("interchange", self.interchange), ("financial_information", self.financial_information),
            ("claims", self.claims), ("organizations", self.organizations), ("file_path", self.file_path)))

    @staticmethod
    def serialize_service(financial_information, payer, claim, service) -> dict:
        """The library's columns for one service, in its order."""
        start = service.service_period_start or claim.claim_statement_period_start
        end = service.service_period_end or claim.claim_statement_period_end
        rendering = claim.rendering_provider
        status = claim.claim.status
        return {
            "marker": claim.claim.marker,
            "patient": claim.patient.name if claim.patient else None,
            "code": service.service.code,
            "modifier": service.service.modifier,
            "qualifier": service.service.qualifier,
            "allowed_units": service.service.allowed_units,
            "billed_units": service.service.billed_units,
            "transaction_date": financial_information.transaction_date,
            "icn": claim.claim.icn,
            "charge_amount": service.service.charge_amount,
            "allowed_amount": service.allowed_amount,
            "paid_amount": service.service.paid_amount,
            "payer": payer.organization.name,
            "start_date": start.date if start else None,
            "end_date": end.date if end else None,
            "rendering_provider": rendering.name if rendering else None,
            "payer_classification": str(status.payer_classification),
            "was_forwarded": status.was_forwarded,
        }

    @classmethod
    def service_record(cls, financial_information, payer, claim, service) -> dict:
        """One row of ``to_dataframe``: the serialized service and its
        adjustments, references and remarks numbered from 0."""
        datum = cls.serialize_service(financial_information, payer, claim, service)
        for n, adjustment in enumerate(service.adjustments):
            datum[f"adj_{n}_group"] = adjustment.group_code.code
            datum[f"adj_{n}_code"] = adjustment.reason_code.code
            datum[f"adj_{n}_amount"] = adjustment.amount
        for n, reference in enumerate(service.references):
            datum[f"ref_{n}_qual"] = reference.qualifier.code
            datum[f"ref_{n}_value"] = reference.value
        for n, remark in enumerate(service.remarks):
            datum[f"rem_{n}_qual"] = remark.qualifier.code
            datum[f"rem_{n}_code"] = remark.code.code
        return datum

    def to_dataframe(self):
        """One row per service, as edi-835-parser builds it."""
        import pandas as pd

        financial_information, payer = self.financial_information, self.payer
        return pd.DataFrame([
            self.service_record(financial_information, payer, claim, service)
            for claim in self.claims
            for service in claim.services
        ])


class TransactionSets:
    """Every transaction of the files parsed."""

    def __init__(self, transaction_sets):
        self.transaction_sets = list(transaction_sets)

    def __iter__(self) -> Iterator[TransactionSet]:
        yield from self.transaction_sets

    def __len__(self) -> int:
        return len(self.transaction_sets)

    def __repr__(self):
        return "\n".join(str(t) for t in self)

    def to_dataframe(self):
        """Every transaction's rows, with the numbered columns sorted last."""
        import pandas as pd

        frames = [t.to_dataframe() for t in self]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", FutureWarning)
            data = pd.concat(frames) if frames else pd.DataFrame()
        return TransactionSets.sort_columns(data)

    @staticmethod
    def sort_columns(data):
        variable = sorted(c for c in data.columns if any(s in c for s in ("adj", "ref", "rem")))
        static = [c for c in data.columns if c not in variable]
        return data[static + variable]

    def sum_payments(self) -> float:
        return sum((t.financial_information.amount_paid for t in self), 0)

    def count_claims(self) -> int:
        return sum(len(t.claims) for t in self)

    def count_patients(self) -> int:
        return len({
            c.patient.identification_code if c.patient else None for t in self for c in t.claims
        })


def _sets(data, file_path) -> List[TransactionSet]:
    document, parts = load(data)
    return [TransactionSet(document, tables, file_path) for tables in parts]


def parse_bytes(data, file_path: Optional[str] = None) -> TransactionSets:
    """Parses one file held in ``bytes``, ``bytearray`` or ``memoryview``;
    ``file_path`` is what each ``TransactionSet.file_path`` reports."""
    return TransactionSets(_sets(data, "<bytes>" if file_path is None else file_path))


def parse_file_obj(file, file_path: Optional[str] = None) -> TransactionSets:
    """Parses what ``file.read()`` returns (a binary file object)."""
    return parse_bytes(file.read(), file_path)


def parse_many(items) -> TransactionSets:
    """Parses each item (bytes-like or binary file object) in order."""
    sets = []
    for item in items:
        if hasattr(item, "read"):
            sets.extend(parse_file_obj(item).transaction_sets)
        else:
            sets.extend(parse_bytes(item).transaction_sets)
    return TransactionSets(sets)


def _path(path) -> List[TransactionSet]:
    with open(path, "rb") as handle:
        return _sets(handle.read(), str(path))


def parse(path: str, debug: bool = False) -> TransactionSets:
    """Parses a file path, or every ``.txt``, ``.835`` and ``.DAT`` file of a
    directory, with the same signature and behaviour as ``edi_835_parser.parse``.
    Data already in memory goes through ``parse_bytes``, ``parse_file_obj`` or
    ``parse_many``."""
    path = os.path.expanduser(os.fspath(path))
    if not os.path.isdir(path):
        return TransactionSets(_path(path))
    sets = []
    for name in os.listdir(path):
        if not name.endswith(_SUFFIXES):
            continue
        file_path = f"{path}/{name}"
        try:
            sets.extend(_path(file_path))
        except Exception as error:
            if debug:
                raise
            warnings.warn(f"Failed to build a transaction set from {file_path} with error: {error}")
    return TransactionSets(sets)
```

Run: `make py-test`
Expected: PASS, 158 tests (113 + 45).

- [ ] **Step 3: Gates and commit**

Run: `make gates && make py-test`
Expected: green.

Run: `grep -rnE --exclude=_codes.py '"(CLP|SVC|NM1|DTM|AMT|CAS|REF|LQ|BPR|N1|ISA|QC|82|B6|472|150|151|232|233)"' crates/oxedi835_py/python/oxedi835/edi_835_parser/`
Expected: no output (no segment id or qualifier in the Python layer; qualifiers appear only as keys of the library's description tables in `_codes.py`).

```bash
git add crates/oxedi835_py/python/oxedi835/edi_835_parser crates/oxedi835_py/tests/n104_shim.py crates/oxedi835_py/tests/test_edi_835_parser.py
git commit -m "py: oxedi835.edi_835_parser reproduces edi-835-parser's frame and objects from the compat tables"
```

---

## Task 3: `to_dataframe(extended=True)` — what the frame leaves out

**Implementer tier:** Sonnet — code and tests given; the tables it reads already exist since Task 1.

**Files:**
- Create: `crates/oxedi835_py/python/oxedi835/edi_835_parser/_extended.py`
- Modify: `crates/oxedi835_py/python/oxedi835/edi_835_parser/_sets.py`, `crates/oxedi835_py/tests/test_edi_835_parser.py`

**Interfaces:**
- Produces: `TransactionSets.to_dataframe(extended: bool = False)`, `TransactionSet.to_dataframe(extended: bool = False)`. The extended frame: the strict columns in the strict order (`sort_columns` over the library's columns), then `x_row_kind` (`"service"`, `"claim"` for a claim without services, `"provider_adjustment"` for each `PLB` group), `x_claim` (the claim's row number in `rows_claims`, to deduplicate claim-level values repeated on each service row), then by name: `x_claim_adj_<n>_{amount,code,group,quantity}` (every `CAS` group of the claim), `x_claim_amt_<n>_{amount,qual}`, `x_claim_ref_<n>_{qual,value}`, `x_svc_adj_<n>_…` (every `CAS` group of the service, not only the first of each segment), `x_svc_amt_<n>_{amount,qual}` (every service `AMT`), `x_plb_{amount,fiscal_period_date,provider_id,reason_code,reference_id}`. Rows in file order: each claim's services (or the claim alone), then the transaction's `PLB` groups. dtypes of the strict columns may change where added rows bring `None`; values do not.

- [ ] **Step 1: Failing tests**

Append to `crates/oxedi835_py/tests/test_edi_835_parser.py`:

```python
# claims without services, claim adjustment groups, PLB groups, claim REF, claim AMT
RECOVERED = {
    "edi835_test_davisvision.RMT": (0, 0, 1, 2, 1),
    "edi835_test_eyemed.RMT": (0, 0, 0, 0, 81),
    "edi835_test_file.RMT": (0, 0, 0, 0, 3),
    "edi835_test_not_available_claim_id.RMT": (0, 0, 0, 36, 18),
    "edi835_test_united.rmt": (0, 0, 0, 40, 1247),
    "edi835_test_versant.RMT": (0, 3, 3, 1296, 643),
}


def cells(frame, prefix, suffix):
    columns = [c for c in frame.columns if c.startswith(prefix) and c.endswith(suffix)]
    return int(frame[columns].notna().sum().sum())


def test_extended_keeps_the_frame_and_adds_only_x_columns(sample):
    strict = compat.parse(path_of(sample)).to_dataframe()
    extended = compat.parse(path_of(sample)).to_dataframe(extended=True)
    width = len(strict.columns)
    assert list(extended.columns[:width]) == list(strict.columns)
    assert all(c.startswith("x_") for c in extended.columns[width:])
    services = extended[extended.x_row_kind == "service"]
    pd.testing.assert_frame_equal(
        services[list(strict.columns)].reset_index(drop=True),
        strict.reset_index(drop=True),
        check_dtype=False,
    )


def test_extended_recovers_what_the_frame_leaves_out(sample):
    extended = compat.parse(path_of(sample)).to_dataframe(extended=True)
    per_claim = extended[extended.x_row_kind != "provider_adjustment"].drop_duplicates("x_claim")
    recovered = (
        int((extended.x_row_kind == "claim").sum()),
        cells(per_claim, "x_claim_adj_", "_code"),
        int((extended.x_row_kind == "provider_adjustment").sum()),
        cells(per_claim, "x_claim_ref_", "_qual"),
        cells(per_claim, "x_claim_amt_", "_qual"),
    )
    assert recovered == RECOVERED[sample]


SERVICELESS = (
    "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240103*0501*U*00401*000000001*0*P*:~"
    "GS*HP*SENDER*RECEIVER*20240103*0501*1*X*004010X091A1~"
    "ST*835*0001~"
    "BPR*I*100*C*ACH*CCP*01*999999999*DA*1*1234567890**01*999999999*DA*2*20240103~"
    "TRN*1*12345*1234567890~"
    "N1*PR*PAYER~N3*1 MAIN ST~N4*TOWN*ST*12345~"
    "N1*PE*PAYEE*XX*1234567893~"
    "LX*1~"
    "CLP*A1*1*150*100**12*X1~NM1*QC*1*DOE*JANE****MI*M1~"
    "SVC*HC:99213*150*100**1~DTM*472*20240101~CAS*CO*45*50~"
    "CLP*A2*4*80*0**12*X2~NM1*QC*1*ROE*RICK****MI*M2~CAS*CO*29*80~DTM*232*20240102~"
    "SE*17*0001~GE*1*1~IEA*1*000000001~"
)


def test_a_claim_without_services_is_left_out_and_recovered(tmp_path):
    path = tmp_path / "serviceless.txt"
    path.write_text(SERVICELESS)
    strict = compat.parse(path).to_dataframe()
    pd.testing.assert_frame_equal(strict, old(path).to_dataframe(), check_exact=True)
    assert compat.parse(path).count_claims() == old(path).count_claims() == 2
    extended = compat.parse(path).to_dataframe(extended=True)
    assert list(extended.x_row_kind) == ["service", "claim"]
    claim = extended.iloc[1]
    assert (claim.marker, claim.x_claim_adj_0_group, claim.x_claim_adj_0_code, claim.x_claim_adj_0_amount) == (
        "A2", "CO", "29", 80.0)
    assert pd.isna(claim.code) and claim.start_date == pd.Timestamp("2024-01-02")
```

Run: `make py-test`
Expected: 13 failures, `TypeError: TransactionSets.to_dataframe() got an unexpected keyword argument 'extended'`.

- [ ] **Step 2: Implement**

Create `_extended.py`:

```python
"""The extended frame: edi-835-parser's rows plus what that frame leaves out."""

from __future__ import annotations

import warnings

from ._convert import money, moment, text

LIBRARY_COLUMNS = (
    "marker", "patient", "code", "modifier", "qualifier", "allowed_units", "billed_units",
    "transaction_date", "icn", "charge_amount", "allowed_amount", "paid_amount", "payer",
    "start_date", "end_date", "rendering_provider", "payer_classification", "was_forwarded",
)
# (table, column prefix, parent column, ((suffix, table column, conversion), ...))
CLAIM_LISTS = (
    ("rows_claim_adjustments", "x_claim_adj", (("group", "group", text), ("code", "code", text),
                                               ("amount", "amount", money), ("quantity", "quantity", money))),
    ("rows_claim_references", "x_claim_ref", (("qual", "qualifier", text), ("value", "value", text))),
    ("rows_claim_amounts", "x_claim_amt", (("qual", "qualifier", text), ("amount", "amount", money))),
)
SERVICE_LISTS = (
    ("rows_adjustment_groups", "x_svc_adj", (("group", "group", text), ("code", "code", text),
                                             ("amount", "amount", money), ("quantity", "quantity", money))),
    ("rows_service_amounts", "x_svc_amt", (("qual", "qualifier", text), ("amount", "amount", money))),
)


def _numbered(tables, lists, parent, key):
    datum = {}
    for table, prefix, fields in lists:
        rows = tables[table]
        for n, at in enumerate(rows.under(parent, key)):
            for suffix, column, convert in fields:
                datum[f"{prefix}_{n}_{suffix}"] = convert(rows[column][at])
    return datum


def _claim_record(financial_information, payer, claim):
    start, end = claim.claim_statement_period_start, claim.claim_statement_period_end
    rendering, status = claim.rendering_provider, claim.claim.status
    datum = dict.fromkeys(LIBRARY_COLUMNS)
    datum.update(
        marker=claim.claim.marker,
        patient=claim.patient.name if claim.patient else None,
        transaction_date=financial_information.transaction_date,
        icn=claim.claim.icn,
        payer=payer.organization.name,
        start_date=start.date if start else None,
        end_date=end.date if end else None,
        rendering_provider=rendering.name if rendering else None,
        payer_classification=str(status.payer_classification),
        was_forwarded=status.was_forwarded,
    )
    return datum


def records(transaction_set):
    """Rows in file order: each claim's services (or the claim alone when it
    has none), then the transaction's provider adjustments."""
    tables = transaction_set._t
    financial_information, payer = transaction_set.financial_information, transaction_set.payer
    for claim in transaction_set.claims:
        claim_extra = {"x_claim": claim._row, **_numbered(tables, CLAIM_LISTS, "claim", claim._row)}
        if not claim.services:
            yield {**_claim_record(financial_information, payer, claim), **claim_extra, "x_row_kind": "claim"}
        for service in claim.services:
            datum = transaction_set.service_record(financial_information, payer, claim, service)
            datum.update(claim_extra)
            datum.update(_numbered(tables, SERVICE_LISTS, "service", service._row))
            datum["x_row_kind"] = "service"
            yield datum
    plb = tables["rows_provider_adjustments"]
    for at in range(len(plb)):
        yield {
            "transaction_date": financial_information.transaction_date,
            "payer": payer.organization.name,
            "x_row_kind": "provider_adjustment",
            "x_plb_provider_id": text(plb["provider_id"][at]),
            "x_plb_fiscal_period_date": moment(plb["fiscal_period_date"][at]),
            "x_plb_reason_code": text(plb["reason_code"][at]),
            "x_plb_reference_id": text(plb["reference_id"][at]),
            "x_plb_amount": money(plb["amount"][at]),
        }


def frame(transaction_sets):
    """The library's columns in its order, then ``x_row_kind`` and
    ``x_claim``, then the other ``x_`` columns by name."""
    import pandas as pd

    from ._sets import TransactionSets

    with warnings.catch_warnings():
        warnings.simplefilter("ignore", FutureWarning)
        data = pd.DataFrame([datum for ts in transaction_sets for datum in records(ts)])
    if data.empty:
        return data
    added = sorted(c for c in data.columns if c.startswith("x_") and c not in ("x_row_kind", "x_claim"))
    library = TransactionSets.sort_columns(data[[c for c in data.columns if not c.startswith("x_")]])
    return pd.concat([library, data[["x_row_kind", "x_claim", *added]]], axis=1)
```

In `_sets.py` replace the two `to_dataframe` methods. `TransactionSet`:

```python
    def to_dataframe(self, extended: bool = False):
        """One row per service, as edi-835-parser builds it; with ``extended``,
        also the rows and ``x_`` columns that frame leaves out."""
        import pandas as pd

        if extended:
            from ._extended import frame

            return frame([self])
        financial_information, payer = self.financial_information, self.payer
        return pd.DataFrame([
            self.service_record(financial_information, payer, claim, service)
            for claim in self.claims
            for service in claim.services
        ])
```

`TransactionSets`:

```python
    def to_dataframe(self, extended: bool = False):
        """Every transaction's rows, with the numbered columns sorted last."""
        import pandas as pd

        if extended:
            from ._extended import frame

            return frame(self.transaction_sets)
        frames = [t.to_dataframe() for t in self]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", FutureWarning)
            data = pd.concat(frames) if frames else pd.DataFrame()
        return TransactionSets.sort_columns(data)
```

Run: `make py-test`
Expected: PASS, 171 tests.

- [ ] **Step 3: Gates and commit**

Run: `make gates && make py-test` — green.

```bash
git add crates/oxedi835_py/python/oxedi835/edi_835_parser/_extended.py crates/oxedi835_py/python/oxedi835/edi_835_parser/_sets.py crates/oxedi835_py/tests/test_edi_835_parser.py
git commit -m "py: extended frame adds claims without services, claim adjustments, PLB and unmapped REF/AMT as x_ columns"
```

---

## Task 4: Native counterparts — `Result` counts, sum, payer, payee; `to_polars`/`to_pandas`

**Implementer tier:** Sonnet — Rust in the binding following the shapes below and the PyO3 0.29 spellings of the Stage 5 plan; values were checked on the scratch copy in Python over the same tables, the Rust itself was not compiled there.

**Files:**
- Modify (only with ruling O1): `crates/edi835_core/specs/835.json`, `crates/edi835_core/src/spec.rs` (one expected vector in a unit test), `crates/edi835_core/tests/golden/project/*.tables.txt` (regenerated)
- Create: `crates/oxedi835_py/src/native.rs`, `crates/oxedi835_py/tests/test_native.py`
- Modify: `crates/oxedi835_py/src/lib.rs` (`mod native;`), `crates/oxedi835_py/src/parse.rs`, `crates/oxedi835_py/src/tables.rs`

**Interfaces:**
- Produces (Python, on `oxedi835.Result`): `count_claims() -> int` (rows of `claims`), `count_patients() -> int` (distinct non-null `claims.patient_id`), `sum_payments() -> decimal.Decimal` (sum of `payments.total_payment_amount` at its scale, nulls skipped), `payer -> dict | None` and `payee -> dict | None` (properties; keys `name`, `identification_code`, `address`, `city`, `state`, `zip_code`, values `str | None`; `None` when the file has no such loop).
- Produces (Python, on `Table`): `to_polars() -> polars.DataFrame`, `to_pandas() -> pandas.DataFrame` (through `pyarrow.table(self).to_pandas()`); on `Tables`: the same two returning `dict[str, DataFrame]` in table order.
- Errors (exact texts, one test each): missing table `KeyError('Result.<method> reads the table "<name>"; the tables are: <names, comma-separated>')`; missing column `KeyError('Result.<method> reads the column "<column>" of the table "<table>"; its columns are: <names>')`; wrong type `TypeError('Result.sum_payments reads "total_payment_amount" of the table "payments" as a decimal; it is <type>')`; overflow `ValueError('Result.sum_payments: the sum of "total_payment_amount" overflows a 128-bit decimal at row <row>')`; payer/payee with ≠ 1 transaction `ValueError('Result.<payer|payee> reads one transaction; the table "payments" has <n> rows')`; missing extra `ImportError('<Table|Tables>.<method> needs <module>, which is not installed; install it with: pip install "oxedi835[<extra>]"')` with the original `ImportError` as `__cause__`.

- [ ] **Step 1: Owner ruling O1 (before any code)**

The built-in `payments` table has `payer_name`, `payee_name`, `payee_id` and nothing else about the two organizations, so a native `payer`/`payee` equal to the library's (name, id, address, city/state/zip) needs nine more columns in `835.json`, inside `"payments"` → `"columns"`:

```json
      "payer_id": {"loop": "1000A", "segment": "N1", "element": 4},
      "payer_address": {"loop": "1000A", "segment": "N3", "element": 1},
      "payer_city": {"loop": "1000A", "segment": "N4", "element": 1},
      "payer_state": {"loop": "1000A", "segment": "N4", "element": 2},
      "payer_zip": {"loop": "1000A", "segment": "N4", "element": 3},
      "payee_address": {"loop": "1000B", "segment": "N3", "element": 1},
      "payee_city": {"loop": "1000B", "segment": "N4", "element": 1},
      "payee_state": {"loop": "1000B", "segment": "N4", "element": 2},
      "payee_zip": {"loop": "1000B", "segment": "N4", "element": 3},
```

Ripple: in `src/spec.rs`, test `builtin_835_declares_five_tables_and_how_they_nest`, `vec![4, 21, 13, 5, 9]` becomes `vec![4, 21, 22, 5, 9]`; the `## payments` section of the nine full project goldens gains the nine columns (`UPDATE_GOLDEN=1 cargo test -p edi835_core --test project_golden`, inspect: only `payments` headers and rows change; the two summaries do not); Python goldens follow automatically. Checked on the scratch copy with this patch applied over the built-in: `payments` has 24 columns and `payer`/`payee` equal the library's on all six samples.

**If the owner declines O1:** `payer`/`payee` return only the keys the built-in tables carry (`name` for both, `identification_code` for the payee), `test_native_payer_and_payee_equal_edi_835_parser` compares those keys only, and the README table says so. Everything else in this task is unchanged.

Commit O1 on its own (gates green): `git add crates/edi835_core/specs/835.json crates/edi835_core/src/spec.rs crates/edi835_core/tests/golden/project` and `git commit -m "spec: payments carries the payer and payee id, address and location"`.

- [ ] **Step 2: Failing tests**

Create `crates/oxedi835_py/tests/test_native.py` with these tests (expected values from Facts):

- `test_native_counts_equal_edi_835_parser` (parametrized over `SAMPLES`): `result = oxedi835.parse_file(path_of(sample))`; with the shim applied and `warnings` silenced, `old = edi_835_parser.parse(str(path_of(sample)))`; assert `result.count_claims() == old.count_claims()`, `result.count_patients() == old.count_patients()`, `isinstance(result.sum_payments(), Decimal)` and `abs(float(result.sum_payments()) - old.sum_payments()) < 0.005`; and `str(parse_named(LARGEST).sum_payments()) == "173305.00"`.
- `test_native_payer_and_payee_equal_edi_835_parser` (parametrized): for `role` in `("payer", "payee")`, `getattr(result, role)` equals `{"name": o.organization.name, "identification_code": o.organization.identification_code, "address": o.address.address if o.address else None, "city": …, "state": …, "zip_code": …}` built from the library's `TransactionSet` (`o = getattr(next(iter(old)), role)`; `location` fields `None` when `o.location` is).
- `test_a_missing_table_is_named`: `oxedi835.parse(read("edi835_test_file.RMT"), spec=Spec.builtin().patch({"tables": {"payments": None}})).sum_payments()` raises `KeyError` whose `args[0]` is exactly `'Result.sum_payments reads the table "payments"; the tables are: adjustments, claims, provider_adjustments, services'`.
- `test_a_missing_column_is_named`: with `{"tables": {"claims": {"columns": {"patient_id": None}}}}`, `count_patients()` raises `KeyError` with `args[0] == 'Result.count_patients reads the column "patient_id" of the table "claims"; its columns are: ' + ", ".join(result.tables["claims"].columns)`.
- `test_payer_reads_one_transaction`: `data = read("edi835_test_file.RMT")`; `oxedi835.parse(data + data).payer` raises `ValueError` with text `'Result.payer reads one transaction; the table "payments" has 2 rows'`.
- `test_a_table_reaches_polars_and_pandas`: united `claims`: `to_polars().height == 1332` and `to_polars().schema["charge_amount"] == pl.Decimal(38, 2)`; `to_pandas().shape == (1332, 24)`; `Tables.to_polars()` returns a dict whose keys equal `tables.keys()` and whose `services` frame has 6192 rows.
- `test_a_missing_extra_is_named` (parametrized `("to_polars", "polars", "polars")`, `("to_pandas", "pandas", "pandas")`): `monkeypatch.setitem(sys.modules, module, None)`; calling the method on a `Table` raises `ImportError` with text `f'Table.{method} needs {module}, which is not installed; install it with: pip install "oxedi835[{extra}]"'` and `info.value.__cause__` is an `ImportError`.

(19 tests.) Run: `make py-test` — Expected: the new tests fail with `AttributeError: 'builtins.Result' object has no attribute 'count_claims'` (or `'Table' … 'to_polars'`).

- [ ] **Step 3: `native.rs`**

A module with no Python classes, only functions over `&edi835_core::Tables`, used by `parse.rs`:

```rust
//! Counts, sums and the payer and payee of a parse, read from its tables.

use std::collections::BTreeSet;

use edi835_core::{Cell, ColumnData, ColumnType, Table, Tables};
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;

/// The table `name`, or a `KeyError` that names the method and the tables.
pub fn table<'t>(tables: &'t Tables, method: &str, name: &str) -> PyResult<&'t Table> { … }

/// The column `name` of `table`, or a `KeyError` that names its columns.
pub fn column<'t>(table: &'t Table, method: &str, name: &str) -> PyResult<&'t ColumnData> { … }

/// Rows of `claims`.
pub fn count_claims(tables: &Tables) -> PyResult<usize>;

/// Distinct non-null `claims.patient_id` values (a `BTreeSet<&[u8]>` over `Cell::Binary`).
pub fn count_patients(tables: &Tables) -> PyResult<usize>;

/// The sum of `payments.total_payment_amount` as fixed-point text at the column's scale
/// (`ColumnType::Decimal128 { scale, .. }`; any other type is the `TypeError` above),
/// accumulated with `i128::checked_add` (overflow is the `ValueError` above).
pub fn sum_payments(tables: &Tables) -> PyResult<String>;

/// `value / 10^scale` written with exactly `scale` decimals and a leading `-` when negative
/// (`unsigned_abs`, left-pad with zeros to `scale + 1` digits, insert the point).
fn fixed_point(value: i128, scale: u8) -> String;

/// Which organization: column prefix and the name used in messages.
pub enum Role { Payer, Payee }

/// The one `payments` row's `<role>_name`, `<role>_id` (`payee_id` already exists),
/// `<role>_address`, `<role>_city`, `<role>_state`, `<role>_zip` as UTF-8 text
/// (`String::from_utf8_lossy`), `None` for a null cell; `Ok(None)` when every one is null;
/// the `ValueError` above when `payments` has other than one row.
pub fn organization(tables: &Tables, role: Role) -> PyResult<Option<[(&'static str, Option<String>); 6]>>;
```

`Cell` reads go through `ColumnData::get(row)` (an `Option`, never indexing). `PyTables` gains `pub fn tables(&self) -> &Arc<Tables>` (in `tables.rs`).

- [ ] **Step 4: Methods**

In `parse.rs`, `#[pymethods] impl PyParseResult`, each method borrows `self.tables.bind(py).get().tables()` and calls `native`: `fn count_claims(&self, py) -> PyResult<usize>`, `fn count_patients(&self, py) -> PyResult<usize>`, `fn sum_payments<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>` (`py.import("decimal")?.getattr("Decimal")?.call1((text,))`), `#[getter] fn payer<'py>(…) -> PyResult<Option<Bound<'py, PyDict>>>` and `payee` (a `PyDict` with the six keys). Docstrings: one sentence each, stating the table and column read.

In `tables.rs`, a private `fn extra<'py>(py: Python<'py>, owner: &str, method: &str, module: &str, extra: &str) -> PyResult<Bound<'py, PyModule>>` that maps the `py.import(module)` error to the `ImportError` above and sets the original as its cause (`err.set_cause(py, Some(original))`). `PyTable::to_polars` = `extra(py, "Table", "to_polars", "polars", "polars")?.getattr("DataFrame")?.call1((slf,))`; `PyTable::to_pandas` imports `pandas` (extra `pandas`) and then `pyarrow` (extra `pandas`) and returns `pyarrow.table(slf).to_pandas()`; `PyTables::to_polars`/`to_pandas` build a `PyDict` in `keys()` order, with owner `"Tables"` in the messages.

Run: `make py-test` — Expected: PASS, 190 tests.

- [ ] **Step 5: Gates and commit**

Run: `make gates && make py-test` — green. Run: `grep -nE '\.unwrap\(\)|\.expect\(|panic!|unreachable!' crates/oxedi835_py/src/*.rs` — no output.

```bash
git add crates/oxedi835_py/src/native.rs crates/oxedi835_py/src/lib.rs crates/oxedi835_py/src/parse.rs crates/oxedi835_py/src/tables.rs crates/oxedi835_py/tests/test_native.py
git commit -m "py: Result.count_claims, count_patients, sum_payments, payer, payee; Table(s).to_polars and to_pandas behind their extras"
```

---

## Task 5: Oracles, extras, CI, README

**Implementer tier:** Sonnet — configuration, one script given in full, two short tests and documentation from exact text.

**Files:**
- Create: `scripts/compat_oracle.py`, `crates/oxedi835_py/tests/test_duckdb.py`, `crates/oxedi835_py/tests/test_packaging.py`
- Modify: `crates/oxedi835_py/pyproject.toml`, `Makefile`, `.github/workflows/ci.yml`, `scripts/smoke_wheel.sh`, `README.md`

- [ ] **Step 1: Extras**

In `crates/oxedi835_py/pyproject.toml` replace `[project.optional-dependencies]` with:

```toml
[project.optional-dependencies]
edi-835-parser = ["pandas>=2.0.3,<3", "pyarrow>=14"]
pandas = ["pandas>=2", "pyarrow>=14"]
polars = ["polars>=1.0"]
test = ["pytest>=8", "polars>=1.0", "pyarrow>=14", "pandas>=2.0.3,<3", "edi-835-parser==1.8.0", "duckdb>=1.1"]
```

`[project]` keeps no `dependencies` key.

- [ ] **Step 2: Tests for the packaging and DuckDB**

`crates/oxedi835_py/tests/test_packaging.py`:

```python
from importlib import metadata


def test_the_base_package_requires_nothing():
    requirements = metadata.requires("oxedi835") or []
    assert requirements and all("extra ==" in r for r in requirements)


def test_the_extras_are_the_documented_ones():
    assert sorted(metadata.metadata("oxedi835").get_all("Provides-Extra")) == [
        "edi-835-parser", "pandas", "polars", "test",
    ]
```

`crates/oxedi835_py/tests/test_duckdb.py`:

```python
from decimal import Decimal

import duckdb

from conftest import LARGEST, parse_named


def test_duckdb_queries_the_tables_through_the_capsule():
    result = parse_named(LARGEST)
    claims, services = result.tables["claims"], result.tables["services"]  # noqa: F841
    assert duckdb.sql(
        "select count(*), count(distinct patient_id), sum(payment_amount) from claims"
    ).fetchone() == (1332, 1212, Decimal("173305.00"))
    assert duckdb.sql(
        "select count(*) from services s join claims c on s.claim = c.row"
    ).fetchone() == (6192,)
```

(DuckDB's replacement scan finds the local names `claims` and `services` and reads them through `__arrow_c_stream__`; verified from inside a function on the scratch copy.)

- [ ] **Step 3: Tooling installs the dev packages**

- `Makefile`, target `venv`: `uv pip install --python $(PYTHON) maturin pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb`. New target (and in `.PHONY`):

```make
compat-oracle: ## compare with edi-835-parser on DIR (outside the repo); prints counts and verdicts only
	@test -n "$(DIR)" || (echo "usage: make compat-oracle DIR=/path/outside/the/repo [OUT=report.txt]" && exit 1)
	$(PYTHON) scripts/compat_oracle.py "$(DIR)" $(if $(OUT),--out "$(OUT)")
```

- `.github/workflows/ci.yml`, job `python`, step `Tools`: `uv pip install "maturin>=1.9.4,<2" pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb`.
- `scripts/smoke_wheel.sh`: the `uv pip install` line installs `pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb` next to the wheel (the clean venv runs the parity tests too).

- [ ] **Step 4: `scripts/compat_oracle.py`** (mode 755)

```python
"""Compares oxedi835.edi_835_parser with edi-835-parser on every file of a directory.

Not a gate. Meant for files that must not enter the repository: the report
names files by their position in the sorted listing and prints only shapes,
column names, counts and verdicts, never a value read from a file. The N104
shim of the test suite is applied so the library reads alphanumeric payer
ids. Usage: python scripts/compat_oracle.py DIR [--out REPORT]
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

TESTS = Path(__file__).resolve().parents[1] / "crates" / "oxedi835_py" / "tests"


def compare(path, old, new):
    """One report block for one file: a list of lines."""
    if path.read_bytes()[:3] != b"ISA":
        return ["skipped: does not start with ISA"]
    try:
        expected_sets = old.parse(str(path))
        expected = expected_sets.to_dataframe()
    except Exception as error:
        return [f"edi-835-parser failed: {type(error).__name__}"]
    try:
        actual_sets = new.parse(path)
        actual = actual_sets.to_dataframe()
    except Exception as error:
        return [f"oxedi835 failed: {type(error).__name__}"]
    lines = [f"shape edi-835-parser {expected.shape} oxedi835 {actual.shape}"]
    only_old = [c for c in expected.columns if c not in actual.columns]
    only_new = [c for c in actual.columns if c not in expected.columns]
    if only_old or only_new:
        lines.append(f"columns only in edi-835-parser: {only_old}; only in oxedi835: {only_new}")
    if list(expected.columns) != list(actual.columns) and not (only_old or only_new):
        lines.append("same columns in a different order")
    if expected.shape[0] == actual.shape[0]:
        for column in (c for c in expected.columns if c in actual.columns):
            left, right = expected[column].reset_index(drop=True), actual[column].reset_index(drop=True)
            if str(left.dtype) != str(right.dtype):
                lines.append(f"{column}: dtype {left.dtype} vs {right.dtype}")
            differs = ~((left == right) | (left.isna() & right.isna()))
            if int(differs.sum()):
                lines.append(f"{column}: {int(differs.sum())} cells differ")
    for name in ("count_claims", "count_patients", "sum_payments"):
        a, b = getattr(expected_sets, name)(), getattr(actual_sets, name)()
        if abs(a - b) > 0.005:
            lines.append(f"{name} differs")
    if len(lines) == 1:
        lines.append("equal")
    return lines


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path, help="write the report here instead of stdout")
    args = parser.parse_args(argv)
    try:
        import edi_835_parser as old
    except ImportError:
        sys.exit('edi-835-parser is not installed: uv pip install "edi-835-parser==1.8.0"')
    sys.path.insert(0, str(TESTS))
    import n104_shim

    from oxedi835 import edi_835_parser as new

    n104_shim.apply()
    warnings.simplefilter("ignore")
    files = sorted(p for p in args.directory.iterdir() if p.is_file())
    report = []
    for number, path in enumerate(files, 1):
        report.append(f"file {number}/{len(files)}")
        report.extend(f"  {line}" for line in compare(path, old, new))
    text = "\n".join(report) + "\n"
    if args.out:
        args.out.write_text(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
```

Run: `make compat-oracle DIR=crates/edi835_core/tests/samples`
Expected: `file 1/7` skipped (the README), files 2–7 `equal` with the shapes of Facts. On the fixtures directory: multi_claim and trizetto report `code`/`qualifier` cells differing (separator guessing, documented). Then the owner runs it on the originals: `make compat-oracle DIR=<originals> OUT=/tmp/oracle.txt`; the gate is that every difference in the report is one of the documented divergences.

- [ ] **Step 5: README**

Add before `## Extending the 835 spec`:

````markdown
## Coming from edi-835-parser

Two paths. **Change the import** and keep your code:

```python
# before: from edi_835_parser import parse
from oxedi835.edi_835_parser import parse      # pip install "oxedi835[edi-835-parser]"

frame = parse("remittances/").to_dataframe()    # same rows, columns, order and dtypes
frame = parse(blob).to_dataframe()              # also bytes, memoryview or an open binary file
# parse_bytes(data, file_path=...), parse_file_obj(f), parse_many([...]) name the route
extra = parse("remittances/").to_dataframe(extended=True)  # + claims without services, claim
                                                # adjustments, PLB, unmapped REF/AMT (x_ columns)
```

The frame equals edi-835-parser 1.8.0's cell for cell on our test files; payer ids that
are not numbers (`N104` with qualifier `XV`) work. Differences, all on input that
edi-835-parser rejects or misreads: one `TransactionSet` per `ST` (not per file); the
separators come from the ISA instead of being guessed per element; an unknown claim
status gives `"unknown"` instead of an error.

**Or move to the native API**, which needs no pandas:

| edi-835-parser | oxedi835 |
|---|---|
| `parse(path)` | `oxedi835.parse_file(path)` → `Result` |
| `.to_dataframe()` | `result.tables["services"].to_polars()` / `.to_pandas()`, joined to `claims` on `claim`, to `payments` on `payment` |
| `.count_claims()` | `result.count_claims()` |
| `.count_patients()` | `result.count_patients()` (null ids are not a patient) |
| `.sum_payments()` (float) | `result.sum_payments()` (`Decimal`) |
| `transaction_set.payer` / `.payee` | `result.payer` / `result.payee` (dict: `name`, `identification_code`, `address`, `city`, `state`, `zip_code`) |
| — | SQL: `duckdb.sql("select … from claims")` with `claims = result.tables["claims"]` |
````

(Without ruling O1 the payer/payee row reads `dict: name, and identification_code for the payee`.)

- [ ] **Step 6: Final sweep**

Run: `make gates && make py-test && make smoke`
Expected: green; pytest 193 (190 + 3) twice.

Run: `grep -rnE '(//|#).*(\b(T2[5-9]|P[0-9]+|N[1-7]|D[0-9]+|O1|R[1-4]|L[0-3])\b|[Ss]tage [0-9])' crates/oxedi835_py/src crates/oxedi835_py/python crates/oxedi835_py/tests scripts/compat_oracle.py crates/edi835_core/tests/project_golden.rs`
Expected: no output.

Run: `git diff master --stat -- crates/edi835_core/src crates/edi835_core/Cargo.toml`
Expected: empty, or only `src/spec.rs` (one test line) with ruling O1.

- [ ] **Step 7: Commit**

```bash
git add crates/oxedi835_py/pyproject.toml Makefile .github/workflows/ci.yml scripts/smoke_wheel.sh scripts/compat_oracle.py crates/oxedi835_py/tests/test_duckdb.py crates/oxedi835_py/tests/test_packaging.py README.md
git commit -m "py: extras, parity and DuckDB in CI, compat oracle script, README for edi-835-parser users"
```

---

## Status

Owner ruling (2026-10-04): `parse` is a strict drop-in replacement (path or directory only, same signature `parse(path, debug=False)`); reading from memory is only through the additions `parse_bytes`, `parse_file_obj` and `parse_many`. The plan text was adjusted accordingly after the scratch run; the routing tests now call the explicit methods.

Task boundaries follow the brief's five tasks, merged where a task had no test cycle of its own: the brief's Task 2 (frame) and Task 3 (objects) are one task here because the frame is built from the objects (`serialize_service` over `Claim`/`Service`, as the library does), and `extended` became its own task with its own tests. Decisions where §7 is silent:

- **`rem_<n>_{qual,code}`** is a third dynamic family (LQ remarks); the patch adds `rows_remarks` beside the two long tables §7 names.
- **Seventeen tables, not three.** §7 names `rows`, `rows_adjustments`, `rows_references`; L0 (no ancestor reads) makes one table per loop level necessary, and the object views need the claim and service lists (`NM1`, `DTM`, `REF`, `AMT`) as long tables. The native tables are removed in the compat spec so it builds only what the layer reads.
- **The ordinal "within the service"** is not a column: the format has no per-parent ordinal; the pivot enumerates the rows of each `service` group, which are in file order.
- **Bytes inputs** (owner addition to T27): `parse` dispatches on bytes-like and file objects, `parse_bytes`/`parse_file_obj`/`parse_many` name the routes, `file_path` defaults to `"<bytes>"`; all routes share `_tables.load(data)`, one `oxedi835.parse` call. The library has no equivalent; its tests are ours only.
- **One `TransactionSet` per `ST`**, where the library makes one per file (identical on single-transaction files, correct on the others).
- **The library's failures are not reproduced**: an unknown `CLP02`, a missing `NM1*QC` or a second payer give values (`"unknown"`, `None`, the first) instead of `TypeError`/`AssertionError`; the README lists them.
- **`count_patients` native skips null ids**; the compat one counts `None` once, as the library.
- **The code-description tables are copied** from edi-835-parser (MIT) with a docstring note; open question below.
- **Ruling O1** (payer/payee columns in the built-in `payments`) is the one change to the core's data.

Not executed on the scratch copy: Task 1's Rust test and constant (the patch itself loads through the core, since the binding applies it with `merge_patch`), Task 4's Rust, Task 5's packaging and CI. pytest totals: 113 · 158 · 171 · 190 · 193 after Tasks 1–5 (the first three measured, the last two counted from the listed tests).

## Stage 5b exit gate (definition of done)

- [ ] `make gates` green; cargo tests = the count before the stage + 1.
- [ ] `make py-test` and `make smoke`: 193 passed; the `python` CI job green on 3.11 and 3.13 with `edi-835-parser`, `pandas`, `duckdb` installed.
- [ ] `assert_frame_equal(check_exact=True)` between `edi_835_parser.parse(...).to_dataframe()` (with the `N104` shim) and `oxedi835.edi_835_parser.parse(...).to_dataframe()` on the six samples and on the six as a directory; `count_claims`, `count_patients`, `sum_payments` equal; `payer`/`payee` with the same fields.
- [ ] `extended=True` adds rows and only `x_` columns; the per-file recovered counts of Facts are asserted; the inline file shows a claim without services left out by both libraries and recovered by `extended`.
- [ ] Native `count_claims`, `count_patients`, `sum_payments` (`Decimal`, within a cent) and `payer`/`payee` equal the library's on the six samples.
- [ ] DuckDB test green; the base package declares no requirement outside an extra.
- [ ] `make compat-oracle DIR=<originals>` run by the owner: every difference is a documented divergence (none expected on payer files with a single `ST`).
- [ ] `edi835_core` `[dependencies]` unchanged; `src/` unchanged except the O1 test line; comments free of codes (Task 5 Step 6 grep).
- [ ] Controller: `.doc/roadmap.md` and `.doc/state.md` updated; D12 marked resolved in §6.2; deferred minors to Project #8.

## Self-review

**Spec coverage (§7 Stage 5b).**

| Item | Where | Proof |
|---|---|---|
| T25 spec patch `specs/edi_835_parser.json` with `rows` anchored in `2110`, `rows_adjustments`, `rows_references`, ordinal per service, Python pivots in the library's order | Tasks 1, 2 | goldens; `test_the_frame_equals_edi_835_parser_cell_for_cell`; per-column table |
| T25 every 835 semantic in data; limits recorded with the choice | Facts (L0–L3, R1–R4); Task 2 Step 3 grep | the grep for segment ids/qualifiers in the package |
| T26 strict parity by default (rows, columns, order, dtypes) | Task 2 | `check_exact=True` on six samples and the directory |
| T26 `extended=True`, `x_` prefix, recovered items per file | Task 3 | `test_extended_keeps_the_frame_and_adds_only_x_columns`, `test_extended_recovers_what_the_frame_leaves_out`, `test_a_claim_without_services_is_left_out_and_recovered` |
| T27 bytes inputs: `parse` on path, directory, bytes-like, file object; `parse_bytes`, `parse_file_obj`, `parse_many`; `file_path` default `"<bytes>"`; parity on all three input kinds | Task 2 | `test_the_frame_equals_edi_835_parser_cell_for_cell[by_path/by_bytes/by_file]`, `test_path_bytes_memoryview_and_file_give_the_same_frame`, `test_file_path_is_kept_or_defaults_to_bytes`, `test_parse_many_equals_the_directory` |
| T27 full surface (`parse(path | dir)`, `TransactionSets` methods, `TransactionSet` objects, `Claim`, `Service`) as read-only views | Task 2 | `test_payer_payee_and_segments_equal_edi_835_parser`, `test_claim_and_service_objects_equal_edi_835_parser`, `test_a_directory_reads_the_same_files_in_the_same_order` |
| T27 extra `oxedi835[edi-835-parser]` (pandas + pyarrow); base without dependencies | Task 5 | `test_the_base_package_requires_nothing`, `test_the_extras_are_the_documented_ones` |
| T28 `count_claims`, `count_patients`, `sum_payments -> Decimal`, `payer`, `payee` on `Result`; `to_polars`/`to_pandas` lazy with the extra in the error | Task 4 | `test_native.py` |
| T28 method table in the README | Task 5 | README section |
| T29 (a) `compat_oracle.py` over the originals, no data in the output | Task 5 | script; privacy rules in Global Constraints |
| T29 (b) CI parity with `edi-835-parser` as a dev dependency and the shim | Tasks 2, 5 | CI `Tools` step; `n104_shim.py` |
| T29 goldens of `rows` generated like the others | Task 1 | `edi_835_parser_tables_match_the_golden_files` |
| T29 DuckDB test | Task 5 | `test_duckdb_queries_the_tables_through_the_capsule` |
| Fuera de alcance: other libraries, writing 835, fixing edi-835-parser | — | none appears; the shim is test-only |

**Placeholder scan.** `grep -nE 'TBD|TODO|FIXME|similar to Task|add validation|write tests for'` over this plan: no match. The `…` in Task 4 Step 3 are function bodies described in the doc comment above each signature (prose by design: the lean-planner rule executes only the parity core). Angle brackets appear only in error-message templates and in `DIR=<originals>`.

**Type and name consistency.** `EDI_835_PARSER_PATCH` (Task 1) is what `_tables.spec()` (Task 2) patches with. The spec's pointer columns (`patient_segment`, `rendering_provider_segment`, `statement_period_{start,end}_segment`, `service_date_segment`, `service_period_{start,end}_segment`, `payer_segment`, `payee_segment`, `bpr_segment`) are the names `_views` and `_sets` read. `TransactionSet.service_record` (Task 2) is what `_extended.records` (Task 3) reuses. `Rows.under(parent, key)` serves the pivots of both. `PyTables::tables()` (Task 4) is what every native method borrows. Test names in Review Focus exist in Tasks 2–5.
