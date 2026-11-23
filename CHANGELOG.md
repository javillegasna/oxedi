# Changelog

All notable changes to oxedi (named oxedi835 up to 0.2.1) are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses semantic
versioning; while the version is `0.x`, a minor release may break the API.

This file covers the Python package; the DuckDB extension has its own changelog in
[`crates/oxedi_duckdb/CHANGELOG.md`](crates/oxedi_duckdb/CHANGELOG.md).

## [Unreleased]

### Added
- Rust: `spec_load::Package` reads a package, a JSON file `{"package": {...}, "spec": {...}}` whose header holds `format` (an integer; a reader takes its own and every lower number and refuses a higher one naming both), `id` (`PackageId`: author, `Layer`, `Release`, transaction set, guide), `layers` (`Layers`) and `bases`; `SpecError::BadHeader` names what is wrong.
- Rust: `LoadedSpec` (`model`, `spec`) keeps a spec's typed model beside the spec compiled from it; `LoadedSpec::merge_patch` merges a patch over the model and compiles again, every failure a `SpecError::Patch`. `SpecModel::from_json` reads a spec's text into its model, `SpecModel::compile` compiles it, and `Builtin::model` reads a built-in's model from the JSON the built-in ships. `Builtin::loaded` keeps a built-in's model, read on demand, beside its spec.
- `oxedi.Version` (`V5010`, `V4010`, with `text` and `str()`): the X12 release of a built-in spec.
- `Spec.DEFAULT_VERSION`: the `Version` of the default built-in spec, the default of `Spec.builtin`.
- `Envelope.DEFAULT_QUALIFIER`, `Envelope.DEFAULT_USAGE_INDICATOR` and `Envelope.DEFAULT_CONTROL_NUMBER`:
  the defaults of `sender_qualifier`, `receiver_qualifier`, `usage_indicator` and `control_number`.
- A spec declares `summaries` (named `count`, `distinct`, `sum` and `row` figures over its tables)
  and a `default_table`. The built-in 835 declares `count_claims`, `count_patients`,
  `sum_payments`, `payer` and `payee`, and `claims` as its default table; `Result.count_claims()`,
  `count_patients()`, `sum_payments()`, `payer` and `payee` read those summaries.
- `Segment.element(position, component=None)` and `Segment.has(position, component=None)`: read an
  element or one of its components by 1-based position, `None` or `False` where the segment has
  no such place.
- `Spec.column_source(table, column)` → `ElementSource` (`segment`, `element`, `component`): where a
  table column reads its value, as the spec resolved it; `KeyError` for an unknown table or column,
  `ValueError` for a column that reads a segment index or a repeated group.
- `Spec.element_sources(table)` and `Spec.group_sources(table)`: by name, every column of a table that
  reads one fixed element (`ElementSource`) or an element of each repeated group (`GroupSource`:
  `first`, `step`, `offset`, `component`, `holds(position)`); `Spec.tables()`, the table names;
  `Spec.trigger_segment(name)`, the segment that opens a loop. `KeyError` for an unknown table or
  loop. Rust: `Repeat::holds(offset, position)`.
- Rust: `engine::Instance` and `engine::InstanceId`: one open occurrence of a loop, numbered by the
  engine in opening order, with its loop, its 1-based ordinal in the stream and its parent.
  `LoopEngine::path` returns the open instances (`&[Instance]`) instead of their loop ids.
- Rust: `Event::closes(loop_id)` tells whether an event closes the loop.
- Rust: `Event::instance()` returns the instance an event is about, `None` for an empty segment or an
  unmatched one at the top level.
- Rust: `engine::named_lineage(spec, event, open, events)` gives the loop name and ordinal of each
  instance around an event, outermost first.
- Rust: `Spec::occurrence_of(loop_id, segment)` returns the `OccurrenceId` a segment takes in a loop,
  found through an index built when the spec loads; `LoopDef::occurrence_of` is removed.
- Rust: `Event::Captured` carries `occurrence: Option<OccurrenceId>`, the occurrence of the loop the
  segment matched; `StructureChecker::on` and `Projector::on` read it from the event.
- Rust: `engine::lineage(instance, open, events)` returns an instance and the ones that enclose it,
  outermost first, from the open path and the events of one step.
- Rust: `engine::loop_paths(spec, segments, wanted)` returns the loop instances that enclose each
  wanted segment, outermost first, in one pass that stops at the highest index.

### Changed
- A spec whose occurrence lists a code under `codes` that its element's own `codes` leaves out is refused. Rust: `OccurrenceError::CodeOutsideElement` also replaces `QualifierCodeOutsideElement`, with the same text for a qualifier.
- The built-in spec files are packages: `835.json` is the 5010 spec under a header (`oxedi/views/005010/835/X221A1`, every layer) and `835.4010.json` an edition over it (`oxedi/guide/004010/835/X091A1`); the built-ins are the two spec bodies merged by value in order and read once; a stack of packages is composed with a check of its header, its layers and the keys each layer owns (`SpecError::Package`, `SpecError::Stack`), and tests prove both give one model.
- Rust: loading a spec has its own module, `oxedi_core::spec_load`, beside the compiled schema in `oxedi_core::spec`. `SpecError`, `LoadedSpec`, `Builtin`, `Version`, `UnknownVersion`, `ElementPosition`, `merge_patch`, every `*Model` (`SpecModel`, `LoopModel`, …) and the load errors (`VersionError`, `ControlError`, `ElementDefError`, `OccurrenceError`, `TableDefError`, `AnchorChains`, `AnchorPositions`, `LoopSegments`, `BalanceError`, `SummaryDefError`) move from `oxedi_core::spec::` to `oxedi_core::spec_load::`; their root paths (`oxedi_core::SpecError`, …) are unchanged.
- Rust: `Spec::merge_patch` and `Spec::to_json` are removed, and a compiled `Spec` keeps no JSON: patch and write a spec through `LoadedSpec` (`LoadedSpec::merge_patch`, `LoadedSpec::to_json`), a built-in through `Builtin::loaded`.
- `Spec.to_json()` returns the spec's model as canonical JSON for every spec, loaded, patched or built in: no key equal to its default, no `null`, keys in name order; `Spec.patch` merges over that model.
- A key holding `.`, `/`, `#` or whitespace is quoted in the path of `SpecError::Format`.
- Rust: `SpecError::Format { path, source }`, the fault of a value the model refuses, at its path from the root.
- A fault anywhere in a spec, its top level included (a key it does not take, a missing key, a value of the wrong JSON type, `null` where a value belongs, a spec or a section that is not an object) is reported by the model read at its path from the root, `.` for the root itself, in Rust and Python alike: ``spec: at segments.CLP.elements.3: unknown field `lenght`, expected one of `name`, `type`, `scale`, `required`, `min`, `max`, `codes`, `composite` ``, ``spec: at segments.CLP.elements.3.min: invalid type: string "1", expected usize``, ``spec: at summaries.s.distinct: missing field `column` ``, ``spec: at loops.a.trigger: invalid type: integer `1`, expected a trigger object``, ``spec: at tables.t.repeat: missing field `step` ``, ``spec: at .: missing field `name` ``, ``spec: at tabels: unknown field `tabels`, expected one of `name`, `version`, `loops`, `tables`, `balancing`, `segments`, `summaries`, `default_table` ``, ``spec: at tables: invalid type: sequence, expected a map``.
- A scale the element's type refuses is reported at the element's path, as `scale` is written beside `type`.
- Rust: `SpecError::NotAnObject`, `SpecError::WrongType`, `SpecError::UnknownKey` and `SpecError::MissingKey` are removed; every such fault is a `SpecError::Format`.
- A spec whose loop or occurrence `usage` is not `required` or `situational`, whose control `count`
  is not `segments` or `children`, or whose `where` or `codes` key is not a 1-based position fails
  at the value's path from the spec's root, in Rust and Python alike, and a word outside that
  vocabulary is now refused before the loop's header rules: `spec: at loops.2100.occurrences.clp.usage: "usage" must be "required" or "situational"; found "x"`.
- Rust: `SpecError::BadPosition`, `SpecError::UnknownLoopUsage`, `OccurrenceError::UnknownUsage`,
  `OccurrenceError::BadCodesKey` and `ControlError::UnknownCount` are removed.
- The error for an interchange whose version carries a repetition separator while the envelope has
  none names the version: `the interchange version "00501" (ISA12) carries a repetition separator
  in ISA11, but the envelope field "delimiters.repetition" is not set; set it to the separator to
  write`. In Rust, `WriteError::NoRepetition` holds that `version`.
- With a release character in the envelope's delimiters, the table writer writes a value holding the
  element separator, the component separator, the segment terminator or the release character
  escaped instead of reporting it (a value holding the repetition separator stays a finding) (the interchange header's values excepted: they stay
  findings), and checks the release character like the other
  delimiters; a file written that way reads back only with the same release character given to
  the reader.
- The Rust core and DuckDB `COPY` refuse a table the spec does not define with `table "x" is not a
  table of the spec; its tables are ...`. In Rust, `Spec::require_table` returns `UnknownTable`
  (now in `spec`, with the name in `name`), and `WriteError::UnknownTable` and
  `WriteError::UnknownColumn` hold an `UnknownTable` and an `UnknownColumn`. A column the spec
  does not have comes from `spec::require_column` (over a table's schema) or
  `TableDef::require_column` (over its declared columns).
- Python: `oxedi.write` refuses an unknown table with `table "x" is not a table of the spec; its
  tables are ...`, and `Spec.column_source` raises `KeyError` with `table "x" is not a table of the
  spec; its tables are ...` or `table "t" has no column "c" in the spec; its columns are ...`
  after the `Spec.column_source("t", "c"): ` prefix.
- Rust: `first_frame` and `next_frame` take a `SegmentEnd` (the segment terminator and the optional
  release byte) instead of `Delimiters`; `SegmentEnd` is a new public type, and
  `Delimiters::segment_end` builds one from the delimiters.
- Rust: every `Event` variant carries `instance: Instance` in place of `id: LoopId`, and
  `Event::Unmatched` gains `within: Option<Instance>`, the innermost open occurrence.
- Rust: `Projector::on` takes the open path (`LoopEngine::path()`) after the events, and
  `Projector::finish` takes the events `LoopEngine::finish` returned; the projector keeps no loop
  stack of its own.
- Rust: `StructureChecker::on` takes the open path (`LoopEngine::path()`) after the events, and
  `StructureChecker::finish` takes the events `LoopEngine::finish` returned; the checker keeps no
  loop ordinals of its own.
- Rust: `Phi` and `DateMode` moved from `oxedi_core::mask` to the new module
  `oxedi_core::vocabulary`; the root paths `oxedi_core::Phi` and `oxedi_core::DateMode` are unchanged.
- Rust: `Pick` moved from `oxedi_core::spec` to `oxedi_core::vocabulary`; the root path `oxedi_core::Pick` is
  unchanged.
- A spec's `pick` that is not `"first"`, `"last"` or a 1-based position is refused with the same
  words as a Rust read of a pick, which names the value found.
- `oxedi[pyx12]` requires pyx12 4.1 or later. A pyx12 finding keeps the X12 code in `code`, and its
  `rule` now ends with pyx12's own name for the error before the `(reported by pyx12, code N)`
  suffix.
- `oxedi.pyx12.validate` indexes the file and selects the spec without projecting the tables, so it
  is faster; its findings are unchanged.
- A spec patch that removes a table or column a summary reads must also remove the summary (with
  `null`): otherwise `Spec.patch` raises `SpecError`, naming the summary, where in it the table or
  column is named and what the spec declares instead. Before, the patch loaded and the summary
  method failed when called.
- The errors of the summary methods name the summary: `KeyError` when the spec declares no such
  summary or the tables lack what it reads, `TypeError` when the spec declares another kind of
  summary under the method's name, `ValueError` for `payer` and `payee` over other than one
  transaction and for a sum that overflows. Each reads `Result.<method>: <reason>`, for example
  `Result.payer: summary "payer" reads the one row of the table "payments", which has 2 rows`.
- The `test` extra is gone (`pip install "oxedi[test]"` no longer resolves): it was a contributor
  list, not a feature. Contributors install the test tools from the package's dependency group:
  `uv pip install --group crates/oxedi_py/pyproject.toml:test`, or `make venv`.
- Introspecting `Envelope` at runtime shows the defaults of `sender_qualifier`, `receiver_qualifier`,
  `usage_indicator` and `control_number` as `...`; their values are the `Envelope.DEFAULT_*` class
  attributes.
- `Spec.builtin` takes a `Version` (the text, such as `"4010"`, is still accepted); introspecting it at
  runtime shows the default of `version` as `...`, its value is `Spec.DEFAULT_VERSION`. The `ValueError` for a version no built-in covers now reads
  `Spec.builtin(version="6020"): unknown version "6020"; version must be one of "5010", "4010"`.
- `oxedi.pyx12.validate` reads pyx12's JSON error output instead of its internal error tree. Until
  pyx12's JSON output carries them, errors inside the envelope segments (ISA, GS, ST, SE, GE, IEA)
  are not reported: neither their element errors nor their segment errors.
- A spec whose table column has a `where` key that is not a 1-based position, a `pick` other than
  `"first"`, `"last"` or a position from 1, or a `phi` other than `name`, `id` or `date` fails at the
  value's path from the spec's root, in Rust and Python alike, and a column's `phi`, `pick` and
  `where` faults are now reported before the table-level rules:
  `spec: at tables.claims.columns.patient_id.phi: "phi" must be "name", "id" or "date"; found "ssn"`.
- Rust: those faults are `SpecError::Format`; `SpecError::TableSchema`, `TableDefError::BadPosition` and
  `TableDefError::UnknownPhi` are removed.
- Rust: `TableDefError::BadPick` is removed: a pick other than `"first"`, `"last"` or a position from 1
  is a `SpecError::Format` at the pick's path, with the same text.
- A segment element whose "type" is unknown or holds a "scale" without type R, or whose position key is not canonical, fails at its path from the spec's root, in Rust and Python alike: `spec: at segments.CLP.elements.3: type "XX" is not one of AN, ID, N0 to N9, R, DT, TM`, or `spec: at segments.CLP.elements: segment "elements" key "01" is not a position: positions are 1-based integers written in canonical form`. A segment is read whole first, so such a fault anywhere in it is reported before a name, length, code or composite fault, and elements are checked in position order (2 before 10).
- Rust: `SpecError::SegmentSchema`, `ElementDefError::NonCanonicalPosition`, `ElementDefError::UnknownType` and `ElementDefError::ScaleWithoutR` are removed, and so is `From<TypeCodeError> for ElementDefError`.
- A summary that holds no operation, or more than one, fails at its path, in Rust and Python alike: `spec: at summaries.s: holds 2 operations (count, sum); a summary holds exactly one of count, distinct, sum, row`. Reading a `SummaryModel` with serde refuses it with the same text.
- Rust: `SpecError::SummarySchema` is removed; `SummaryDefError::Operations` is reported inside `SpecError::Format`.
- A balancing value whose `"sign"` is not `"+"` or `"-"` fails at its path from the spec's root, in Rust and Python alike: `spec: at balancing.claim_balance.sum[1].sign: "sign" must be "+" or "-"; found "plus"`. A rule is read whole first, so such a fault is reported before every other fault of the rule, an empty rule name included.
- Rust: `SpecError::BalanceSchema` and `BalanceError::UnknownSign` are removed.
- A spec is read whole into its model before any section compiles, in Rust and Python alike: a value the model refuses (a word outside its vocabulary, an element type, a key that is not a position, a summary's operations) is reported before every compile fault, and of several the first in the spec's key order (balancing, default_table, loops, name, segments, summaries, tables, version; entries by name) is reported. An empty loops map, segment id, balancing rule name or summary name is reported only when no value is refused.
- Rust: `SpecError::Schema` is removed; such a fault is `SpecError::Format`.

### Removed
- Rust: `Spec::BUILTIN_835_JSON` and `Spec::BUILTIN_835_4010_PATCH`; use `Builtin::fallback().model()` or `Builtin::find(..)?.loaded()`.
- Rust: `Spec::model`, `Spec::from_model` and `SpecError::ModelMismatch`. Read a spec's model with `SpecModel::from_json` or keep it in a `LoadedSpec`, and compile one with `SpecModel::compile` or `LoadedSpec::from_model`.

### Fixed
- The writer and the reader agree on the repetition separator of a custom spec: at interchange
  version 00402 or later the writer writes the envelope's repetition separator in ISA11, and below
  it the element's code. A spec that lists codes for ISA11 at such a version is reported by the
  read-back (code not in list), so `write()` refuses it.
- A spec whose table has a column for the whole element of a segment and another for a component of
  the same element is refused with the column that already writes it, where the second column's
  value used to be dropped without a finding.
- A column that writes the whole of a composite element no longer leaves the writer refusing the
  element's required components, and no longer adds a single-code component beside it, where that
  value was dropped when the segment was written.

## [0.6.0] - 2026-10-18

### Added
- `oxedi.Mask(key, dates="year")` and the `mask=` argument of `parse`, `parse_file` and `stream`:
  the columns the spec flags as patient data (names and ids as keyed 16-character tokens, dates
  reduced to the year, shifted or kept) are masked in the tables and in the diagnostics that
  quote them. The spec marks them with the new `phi` and `phi_seed` keys. The built-in spec
  flags the patient's name and id, the claim number, the payer control number, the group number,
  the statement and service dates, the line item control number and
  `provider_adjustments.reference_id`; a `PLB` reference that equals a claim number gets the same
  token as `claims.claim_id`. The raw document is never masked. A key under 16 bytes raises
  `ValueError`. A spec that flags a column reading an envelope control number or count (`ST02`,
  `SE01`, `GS06`, ...) is refused.
- `oxedi.pyx12.validate` accepts `mask=` and `spec=`. With `mask=`, a finding about a place a
  column flagged as patient data reads has its value masked as `parse` masks its diagnostics, in
  its datum and in pyx12's message, and the flagged values of its segment that the message quotes
  are masked too; pyx12 still reads the original file. Values of 4 or more characters are replaced
  wherever the message quotes them, shorter ones only where they stand apart. A finding for an
  exception inside pyx12 keeps the exception's type and leaves out its text. `spec=` locates the
  findings' loops and sets the columns masked, so a patched spec masks here what it flags in
  `parse`. The exact rules are in [Sharing data without patient
  information](https://javillegas.dev/oxedi/latest/python/without-patient-data/).
- A viewer in the browser at <https://javillegas.dev/oxedi/viewer/>: drop an 835 file to see its
  summary, tables, diagnostics, text and loops. The parser runs as WebAssembly in the page, and
  the page cannot open network connections. A "Hide patient data" switch masks the patient data
  the spec flags.
- `Spec.schema()`: the schema of every table of a spec (built-in or patched) without a file, as a
  mapping of table name to `pyarrow.Schema` with the Arrow types `parse` gives each table. The
  field metadata of a reference column (`payment`, `claim`, `service`) names the table it points
  at under `oxedi.parent`. It needs pyarrow.
- A documentation site at <https://javillegas.dev/oxedi/> (versioned, with a Python API
  reference, recipes and every example run in CI); the README is now its front page. The
  documentation is organized for readers: a five-minute tutorial, a primer on the 835, a
  reference of every table and column, a glossary, and short guides by task. Old page addresses
  redirect to the new ones.

### Changed
- A spec patch that removes or unflags `claims.patient_id` now needs `"phi_seed"` cleared
  (`"phi_seed": null`) or pointed at another `id` column in the same patch; otherwise it raises
  `SpecError`, with or without a mask.

### Fixed
- The documentation now describes how interest balances: a claim's interest is reported in
  `AMT*I` and paid through `PLB` with reason code `L6`, so a claim that carries interest balances
  under the existing rules.

## [0.5.0] - 2026-10-12

### Added
- `Result.spec` and `Tables.spec`: the spec that projected the tables, the one given to `parse` or
  `stream`, or the built-in selected from the file's version. `oxedi.write(frames, envelope,
  spec=result.spec)` writes your own frames with the spec of the file they came from.

### Fixed
- `oxedi.pyx12.validate` reports every error pyx12's engine records, read from its error tree
  instead of its JSON report: element errors of the envelope segments (`ST`, `SE`, `GS`, `GE`,
  `ISA`, `IEA`) and segment errors pyx12 cannot attach to an envelope node are no longer lost.
  It no longer changes logging state (`logging.disable`, levels, propagation) while it runs. On
  first use it attaches a `logging.NullHandler` to the `pyx12` logger when that logger has no
  handler, so pyx12's records are not printed by default and reach the handlers you configure;
  this also applies to code that runs pyx12 directly in the same process.
- `oxedi.pyx12.validate` places a count or control number that does not match on the trailer
  segment, at its element, with the value as `datum`; a duplicate control number on the header
  at its element. A file pyx12 rejects points at its `ISA` segment with the `ISA` bytes as
  `datum`, and a failure after the last segment names the last segment pyx12 completed.
- `oxedi.pyx12.validate` fills `path` with the loops open at the finding's segment, as `parse`
  names them, and `datum` with the element's bytes, or the segment id for a finding with no
  element, when pyx12 gives no value.
- Writing a decimal that cannot be rescaled up to the column's scale (a negative scale, for
  example) reports that it overflows the scale instead of claiming it has too many decimals, and
  a zero decimal is accepted when it only needs scaling up.

### Changed
- `oxedi.write` no longer writes the transaction set header's implementation convention reference
  (`ST03`): no table column or envelope field carries it, the 5010 835 map pyx12 validates
  against marks it Not Used, and the writer does not invent envelope data. A written 5010 header
  is now `ST*835*0001~`.
- `Segment.raw` and `Segment.id` read from the segment's span without parsing its elements.
- Package metadata on PyPI: a description that names the 835 and the writer, keywords, the
  healthcare-industry, libraries and typed classifiers, and Homepage and Documentation links.
- Documentation reorganized into a shorter README and guides under `docs/`, with a
  `CONTRIBUTING.md` for development.

## [0.5.0rc1] - 2026-10-12

Release candidate of 0.5.0, published to TestPyPI only. Same code as 0.5.0.

## [0.4.0] - 2026-10-06

### Added
- `oxedi.write(tables, envelope, spec=None, allow_findings=False)` writes an 835 from tables:
  the tables of a parse, or a mapping of Arrow, Polars or pandas tables with the spec's columns.
  The spec's projection is inverted (each column becomes its element, a single-code qualifier is
  written on its own), rows nest by their `payment`, `claim` and `service` columns, and the file
  is read back with the spec. 5010 and 4010, by the spec that parsed the tables or the one given.
  Only what the tables hold is written; `Document.write()` still reproduces a file byte for byte.
- `oxedi.Envelope`: the sender and receiver with their qualifiers, date and time, usage indicator,
  first control number, optional group application codes, delimiters and line breaks. The writer
  derives the fixed-width `ISA`, matching control numbers and the counts.
- Strict writing: any finding (a required value missing, a code or length out of range, a broken
  or out-of-order row reference, a value holding a delimiter, a balancing rule that fails) raises
  `oxedi.WriteError` (a `ValueError`) listing every finding, and nothing is written. Its
  `findings` are `oxedi.WriteFinding`s, each naming the table, row and column (or the envelope
  field), its kind and the diagnostic behind it. `allow_findings=True` returns `(bytes,
  findings)` instead. Totals are checked, never computed; the claim rule does not model interest
  (`AMT*I`). A null text cell in the middle of a segment is written as an empty element and reads
  back as `""`.
- Six level-2 (SNIP 2) diagnostics from the loop structure, raised while reading:
  `RequiredOccurrenceMissing` (a loop instance closed without a required occurrence),
  `OccurrenceOverMax` (an occurrence repeats past its maximum in one instance), `LoopOverMax`
  (a loop has more instances than its maximum under one parent), `OutOfOrder` (a segment comes
  after one with a higher position), `UnknownOccurrence` (a segment the loop holds matches none
  of its occurrences, e.g. an unknown qualifier) and `RequiredLoopMissing` (a required child
  loop is absent). Each names the loop and its path, the occurrence and the segment; the
  segment stays in the document.
- Code lists per occurrence: an element is checked against the list of the occurrence its
  segment takes (e.g. the seven NM1 of loop 2100, told apart by NM101); the element's own list
  still applies to a segment that matches no occurrence. `CodeNotInList` names the occurrence
  whose list it used.
- A table column can name an occurrence instead of `segment` + `where`
  (`{"occurrence": "patient_name", "element": 3}`), read a loop above the table's anchor
  (`{"loop": "2100", "occurrence": "claim_payment_information", "element": 1}` in a services
  table gives each service its claim's id), and choose among repeated matches with `pick`:
  `"first"` (the default), `"last"` or a 1-based position. A loop above the anchor offers the
  segments its enclosing instance read before the row's instance opened. Invalid sources are
  rejected when the spec loads, naming the table, the column and the value. Existing `segment`
  + `where` columns work as before; several built-in columns now name occurrences, with
  identical cells.

- Balancing rules in the spec, a new `balancing` section: in every instance of a loop, signed
  amounts (elements of occurrences in that loop or below it) must add up exactly. The built-in
  5010 and 4010 specs declare the guide's three: `service_balance` (SVC02 − SVC03 = the service's
  CAS amounts), `claim_balance` (CLP03 − CLP04 = the CAS amounts of the claim and its services)
  and `transaction_balance` (BPR02 = Σ CLP04 − Σ PLB amounts). Each failure is a new level-3
  (SNIP 3) diagnostic, `BalanceMismatch`, naming the rule, the loop instance, the expected and
  computed amounts and the segments read. An absent optional amount counts as zero; an instance
  with a missing required amount or a value that is not a decimal is left to those findings.

- New columns in the built-in tables, so the tables hold what a valid file needs: `payments`
  gains `payer_technical_contact_name`, `payer_technical_contact_qualifier` and
  `payer_technical_contact_number` (the payer's PER*BL contact, null on 4010 files),
  `payer_id_qualifier` and `payee_id_qualifier` (N103); `claims` gains `header_number` (LX01 of
  the claim's header loop), `patient_id_qualifier` and `rendering_provider_id_qualifier` (NM108)
  and `rendering_provider_entity_type` (NM102). Existing columns and their values are unchanged.

### Changed
- A custom patch that deletes a loop or an occurrence a balancing rule reads, or retypes one of
  its amounts away from `R`, must also remove that rule (`"balancing": {"claim_balance": null}`);
  otherwise the spec fails to load with an error that names the rule and the key.
- In text columns, an element (or component) the segment does not have is `null` and one it
  has but leaves empty is `""`; both used to be `null`. Number and date columns keep `null` for
  both. The `oxedi.edi_835_parser` layer keeps the library's values.
- A spec loop now declares its segments as `occurrences`, an object keyed by occurrence name:
  each occurrence names its `segment`, its position `pos`, and optionally its `usage`
  (`required` or `situational`), its maximum repeat `max`, a `qualifier` (the element and
  codes that tell it apart from other occurrences of the same segment) and code lists of its
  own (`codes`). A loop may also declare its maximum repeat `max` and its `usage`. The
  occurrence the loop opens on must have the lowest `pos`. The `segments` list of a
  loop is removed: a patch of your own that redefines a loop's `segments` must move to
  `occurrences`, where it can add, change or remove one occurrence by name.

## [0.4.0rc1] - 2026-10-06

Release candidate of 0.4.0, published to TestPyPI only. Same code as 0.4.0.

## [0.3.0] - 2026-10-05

The project is renamed to `oxedi`; the code is the same as 0.2.1 apart from the name.

### Changed
- The package is now `oxedi` (`pip install oxedi`, `import oxedi`, native module
  `oxedi._core`) and the version line continues at 0.3.0. The extras keep their names
  (`oxedi[pyx12]`, `oxedi[edi-835-parser]`, `oxedi[polars]`, `oxedi[pandas]`) and the API is
  otherwise unchanged.
- `Diagnostic.origin` for the parser's own findings is now `"oxedi"` (was `"oxedi835"`), also
  in the DuckDB extension's diagnostics.
- The `oxedi835` project on PyPI is retired; releases 0.1.0 to 0.2.1 stay under that name.

## [0.2.1] - 2026-10-04

Type stubs, stricter PLB checks and small fixes.

### Added
- Type stubs for the whole native API and a `py.typed` marker, so type checkers (mypy,
  pyright) and IDEs see every signature, parameter name, return type and attribute. The stub
  is generated from the binding and checked against the runtime in CI.

### Changed
- The 5010 spec checks the provider adjustment reason code in every PLB adjustment composite
  (PLB05-1 to PLB13-1), not only the first.
- `CodeNotInList` messages list the allowed codes when there are five or fewer.
- A spec that lists a version value twice is rejected when it loads.
- `oxedi835.pyx12.validate` ignores `pyx12`'s user and system configuration files.
- `validate` given a file opened in text mode raises a `TypeError` that names binary mode.
- The internal external-diagnostic constructor raises a `ValueError` naming the argument
  for an integer out of range.
- `help()` and `inspect.signature` show the real defaults of `Delimiters(...)`
  (`element=b'*'`, `component=b':'`, `segment=b'~'`) instead of `...`.
- For Rust users of the core: `Rule::CodeNotInList.codes` is the list of codes (it was a
  count); `VersionError` gains `DuplicateValue` and is now `#[non_exhaustive]`.

### Fixed
- Wheels and sdists no longer contain `__pycache__` directories or `.pyc` files.

## [0.2.0] - 2026-10-04

Spec versions, code lists and validation with pyx12.

### Added
- A built-in spec for each 835 version: 5010 (`005010X221A1`, the default) and 4010
  (`004010X091A1`), available as `Spec.builtin(version="5010" | "4010")`. Each spec declares
  the version it covers as data, in a new optional `version` section.
- Code lists in the spec format: an element or component may list its allowed values in
  `codes`. A spec is checked when it loads: an empty list, a duplicate code, a code outside
  the element's length, or codes on a numeric element are rejected with errors that name the
  key path and the value. The built-in specs carry the code lists from the HIPAA maps that
  `pyx12` ships; external code sets (claim adjustment and remark codes, states, currencies)
  are left out.
- A new level-2 diagnostic, `CodeNotInList`, for a value outside its element's code list.
- `oxedi835.pyx12.validate(source)`, behind the extra `pip install "oxedi835[pyx12]"`:
  validates a file with `pyx12` and returns `oxedi835.Diagnostic`s, so its findings mix with
  those from `parse`. Envelope errors and `pyx12` failures are level 1, segment and element
  errors level 2. An exception inside `pyx12` becomes a finding instead of a traceback.
  Nothing is written to disk.
- `Diagnostic.origin` (`"oxedi835"`, or `"pyx12"` for findings from `validate`) and
  `Diagnostic.code` (the external validator's own code).
- `Segment.span`: the segment's byte range in the input, `(start, end)`. Use
  `document[d.segment].span` to find the bytes a diagnostic points at.
- A `Changelog` link on the PyPI page.

### Changed
- `parse`, `parse_file` and `stream` called without `spec=` now pick the built-in spec that
  matches the version the file declares (GS08), and fall back to 5010. An explicit `spec=` is
  used as given.
- The 5010 spec is stricter, completed from `pyx12`'s 5010 map: 12 elements are now required
  (among them BPR16, TRN03, CLP06, CLP07 and SVC03), the interchange loop accepts TA1, and 42
  elements have code lists. 5010 files may show new diagnostics, each naming the element and
  the value. 4010 files are checked against the 4010 spec, which follows `pyx12`'s 4010 map.
- For Rust users of the core: `Rule` is now `#[non_exhaustive]` and gains the variants
  `CodeNotInList` and `External`.

## [0.2.0rc1] - 2026-10-04

Release candidate of 0.2.0, published to TestPyPI only. Same code as 0.2.0.

## [0.1.0] - 2026-10-04

First usable version of oxedi835.

### Added
- Lossless parsing of EDI 835 files: every input byte lands in exactly one segment, so
  `write()` gives the file back unchanged, including files with a UTF-8 byte order mark.
- Typed tables (`payments`, `claims`, `services`, and the others the built-in spec defines)
  that reach Polars, pyarrow, pandas and DuckDB through the Arrow PyCapsule interface without
  copying.
- Diagnostics returned as values instead of exceptions, each naming the rule, the position
  and the offending datum.
- Streaming with `oxedi835.stream`, one transaction (or any loop) at a time, with memory
  bounded by that loop; the GIL is released while parsing.
- Data-driven spec: `Spec.builtin().patch({...})` extends segments and table columns with
  JSON Merge Patch documents.
- `oxedi835.edi_835_parser`, a compatibility layer for `edi-835-parser` 1.8.0 that returns the
  same frames cell for cell (`pip install "oxedi835[edi-835-parser]"`).
- One `abi3` wheel per platform for Python 3.11 and later: Linux x86_64 and aarch64
  (manylinux 2.28), Linux x86_64 (musllinux 1.2), macOS x86_64 and arm64, and Windows x86_64;
  plus the source distribution.
- A README and PyPI page covering installation, usage and local development, and a guide for migrating from `edi-835-parser` (`docs/migrating-from-edi-835-parser.md`).

## [0.1.0rc1] - 2026-10-04

Release candidate of 0.1.0, published to TestPyPI only. Same code as 0.1.0.

[Unreleased]: https://github.com/javillegasna/oxedi/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/javillegasna/oxedi/releases/tag/v0.6.0
[0.5.0]: https://github.com/javillegasna/oxedi/releases/tag/v0.5.0
[0.5.0rc1]: https://github.com/javillegasna/oxedi/releases/tag/v0.5.0rc1
[0.4.0]: https://github.com/javillegasna/oxedi/releases/tag/v0.4.0
[0.4.0rc1]: https://github.com/javillegasna/oxedi/releases/tag/v0.4.0rc1
[0.3.0]: https://github.com/javillegasna/oxedi/releases/tag/v0.3.0
[0.2.1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.1
[0.2.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0
[0.2.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0rc1
[0.1.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0
[0.1.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0rc1
