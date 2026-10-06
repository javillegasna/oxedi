# Changelog

All notable changes to the oxedi DuckDB extension are documented here. This file covers the
extension only; the Python package's changes are in [`CHANGELOG.md`](../../CHANGELOG.md) at the
repository root. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the extension has its own version.

## [Unreleased]

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

### Changed
- A text cell whose element is present but empty is now `''`; an absent element stays `NULL`
  (both used to be `NULL`). Numeric and date columns keep `NULL` for both.

## [0.1.0] - 2026-10-05

### Added
- The DuckDB extension `oxedi`, installed with `INSTALL oxedi FROM community; LOAD oxedi;`:
  `read_835(...)` returns the payments, claims, services, adjustments, provider adjustments and
  diagnostics tables in SQL.

[Unreleased]: https://github.com/javillegasna/oxedi/compare/duckdb-v0.1.2...HEAD
[0.1.2]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.2
[0.1.1]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.1
[0.1.0]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.0
