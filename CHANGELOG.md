# Changelog

All notable changes to oxedi835 are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses semantic
versioning; while the version is `0.x`, a minor release may break the API.

## [Unreleased]

## [0.1.0rc1] - 2026-10-04

First release candidate of 0.1.0, the first usable version.

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

[Unreleased]: https://github.com/javillegasna/oxedi835/compare/v0.1.0rc1...HEAD
[0.1.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0rc1
