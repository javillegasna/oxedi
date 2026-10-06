# Changelog

All notable changes to the oxedi DuckDB extension are documented here. This file covers the
extension only; the Python package's changes are in [`CHANGELOG.md`](../../CHANGELOG.md) at the
repository root. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the extension has its own version.

## [Unreleased]

## [0.1.0] - 2026-10-05

### Added
- The DuckDB extension `oxedi`, installed with `INSTALL oxedi FROM community; LOAD oxedi;`:
  `read_835(...)` returns the payments, claims, services, adjustments, provider adjustments and
  diagnostics tables in SQL.

[Unreleased]: https://github.com/javillegasna/oxedi/compare/duckdb-v0.1.0...HEAD
[0.1.0]: https://github.com/javillegasna/oxedi/releases/tag/duckdb-v0.1.0
