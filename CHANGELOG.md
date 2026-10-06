# Changelog

All notable changes to oxedi (named oxedi835 up to 0.2.1) are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses semantic
versioning; while the version is `0.x`, a minor release may break the API.

This file covers the Python package; the DuckDB extension has its own changelog in
[`crates/oxedi_duckdb/CHANGELOG.md`](crates/oxedi_duckdb/CHANGELOG.md).

## [Unreleased]

## [0.4.0rc1] - 2026-10-06

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

[Unreleased]: https://github.com/javillegasna/oxedi/compare/v0.4.0rc1...HEAD
[0.4.0rc1]: https://github.com/javillegasna/oxedi/releases/tag/v0.4.0rc1
[0.3.0]: https://github.com/javillegasna/oxedi/releases/tag/v0.3.0
[0.2.1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.1
[0.2.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0
[0.2.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0rc1
[0.1.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0
[0.1.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0rc1
