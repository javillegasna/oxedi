# Test data of the DuckDB extension

Synthetic inputs for error paths that no sample or fixture exercises. All are
derived from `crates/oxedi_core/tests/fixtures/multi_claim_sample.txt`
(synthetic) or written by hand; neither holds real data. Do not edit them: the
SQLLogicTests and the oracle test pin their exact bytes.

- `latin1_patient.835`: a copy of `multi_claim_sample.txt` whose first patient
  last name is `M\xfcLLER` (Latin-1, not UTF-8) and with an extra segment
  `Z\xfc*1~` after it. They check that a non-UTF-8 cell fails a VARCHAR column
  (in `claims` and in the `diagnostics` `datum`) with the message that suggests
  `binary := true`, and that `binary := true` returns the bytes unchanged.
- `not_an_interchange.835`: one line of text with no ISA segment. It checks
  that a file the core cannot parse fails the query, and that with
  `ignore_errors := true` it becomes one `diagnostics` row and the scan goes on.
- `not_text.835`: a gzip-like header (`1f 8b 08 00 ...`), not text and not an
  interchange. With `ignore_errors := true` its `diagnostics` row holds the
  bytes found instead of `ISA` as escaped ASCII text in VARCHAR mode and raw
  with `binary := true`.
