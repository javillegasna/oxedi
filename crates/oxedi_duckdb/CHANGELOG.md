# Changelog

All notable changes to the oxedi DuckDB extension are documented here. This file covers the
extension only; the Python package's changes are in [`CHANGELOG.md`](../../CHANGELOG.md) at the
repository root. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the extension has its own version.

## [Unreleased]

## [0.2.0] - 2026-10-06

### Added
- The copy format `edi835`: `COPY (SELECT {'payments': ..., 'claims': ...}) TO 'out.835' (FORMAT
  edi835, sender_id ..., receiver_id ..., date ..., time ...)` writes the tables of the spec
  back into an 835 with the core writer, byte for byte equal to `oxedi.write` with the same
  envelope. Its options are those of `oxedi.Envelope` and `version`. It is strict: any finding
  fails the `COPY` with the listing `oxedi.WriteError` shows and nothing is written. The `time`
  option takes a `TIME`, a `TIME_NS` or text, and a time column takes `TIME` or `TIME_NS` in
  whole seconds.

## [0.1.2] - 2026-10-06

### Fixed
- A test compared the file names a glob returns with forward slashes; on Windows the operating
  system writes them with backslashes. The test now normalizes the separators, so the community
  build passes on Windows. 0.1.1 was never published because of it.

## [0.1.1] - 2026-10-06

### Fixed
- The test of a file that cannot be opened no longer pins the operating system's error text, so
  the extension's tests pass on Windows and DuckDB's community build publishes it on every
  platform. 0.1.0 was never published because that test failed on Windows.

### Added
- The diagnostics table carries six new level-2 rules from the loop structure:
  `RequiredOccurrenceMissing`, `OccurrenceOverMax`, `LoopOverMax`, `OutOfOrder`,
  `UnknownOccurrence` and `RequiredLoopMissing`; `CodeNotInList` may check an element against
  the code list of the occurrence its segment takes.
- The diagnostics table carries the level-3 rule `BalanceMismatch`: a balancing rule of the spec
  (service, claim and transaction balancing in the built-in 835) that does not add up.

### Changed
- A text cell whose element is present but empty is now `''`; an absent element stays `NULL`
  (both used to be `NULL`). Numeric and date columns keep `NULL` for both.

## [0.1.0] - 2026-10-05

### Added
- The DuckDB extension `oxedi`, installed with `INSTALL oxedi FROM community; LOAD oxedi;`:
  `read_835(...)` returns the payments, claims, services, adjustments, provider adjustments and
  diagnostics tables in SQL.

[Unreleased]: https://github.com/javillegasna/oxedi/compare/duckdb-v0.2.0...HEAD
[0.2.0]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.2.0
[0.1.2]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.2
[0.1.1]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.1
[0.1.0]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.0
