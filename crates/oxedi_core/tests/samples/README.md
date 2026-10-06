# Samples

Real payer 835 files, **anonymized** with `scripts/anonymize_835.py` before entering the
repository. Every direct identifier (names, member and account ids, NPIs, tax ids, bank
routing and account numbers, addresses, phones, trace numbers) was replaced by a
deterministic fake value; amounts, codes, dates, structure, delimiters, line endings and
the ISA width are untouched. The originals and the re-identification key live outside the
repository and must never be committed.

Unlike `tests/fixtures/` (small synthetic files from the POC), these are full interchanges
with hundreds to thousands of claims, so they are the oracle for anything that must hold on
production-shaped input. Counts measured 2026-10-02 on the anonymized output.

| File | Bytes | Segments | Version | Repetition | Line endings | Claims (CLP) |
|------|-------|----------|---------|------------|--------------|--------------|
| `edi835_test_davisvision.RMT` | 883 | 33 | 00401 | — | none | 1 |
| `edi835_test_eyemed.RMT` | 26 328 | 1 206 | 00401 | — | none | 82 |
| `edi835_test_file.RMT` | 1 953 | 80 | 00401 | — | none | 4 |
| `edi835_test_not_available_claim_id.RMT` | 5 709 | 259 | 00401 | — | none | 18 |
| `edi835_test_united.rmt` | 629 300 | 30 302 | 00501 | `^` | LF after every `~` except the last | 1 332 |
| `edi835_test_versant.RMT` | 204 584 | 10 177 | 00401 | — | none | 648 |

All six use `*` as element separator, `:` as component separator and `~` as terminator.
`united` is the only 5010 file; its segment count equals its `~` count because it has no
trailing trivia.

Two files are excerpts of larger transactions and keep the original `SE01`: `edi835_test_file.RMT` declares 1202 segments and holds 76; `edi835_test_not_available_claim_id.RMT` declares 302 and holds 255. The envelope checker reports both, and the test suite pins those two diagnostics as expected.
