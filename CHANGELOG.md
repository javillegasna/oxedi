# Changelog

All notable changes to oxedi835 are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses semantic
versioning; while the version is `0.x`, a minor release may break the API.

## [Unreleased]

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

[Unreleased]: https://github.com/javillegasna/oxedi835/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0
[0.2.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.2.0rc1
[0.1.0]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0
[0.1.0rc1]: https://github.com/javillegasna/oxedi835/releases/tag/v0.1.0rc1
