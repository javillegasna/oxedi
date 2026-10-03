# Fixtures

Synthetic 835 files inherited byte-for-byte from the `fast_edi835` POC. **Never edit
them**: they are the oracle for the lossless gates. Counts measured 2026-10-02.

| File | ISA | Version | Component | Repetition | `~` | Line endings |
|------|-----|---------|-----------|------------|-----|--------------|
| `emedny_sample.txt` | 106 B | 00501 | `:` | `^` | 69 | none |
| `united_healthcare_legacy_sample.txt` | 106 B | 00501 | `>` | `^` | 65 | none |
| `multi_claim_sample.txt` | **105 B** (ISA06 padded to 14) | 00401 | `>` | — | 51 | LF after every `~` |
| `trizetto_sample.rmt` | **102 B** (ISA06/08 padded to 13) | 00401 | `>` | — | 22 | LF after every `~` |
| `blue_cross_nc_sample.txt` | **none** (starts at `ST`) | — | caller-supplied | — | 32 | none |

Known quirks, kept on purpose:

- `trizetto_sample.rmt` line 7: `N1*PR*INSURANCE COMPANY OF AMERICA~XX*654321~` — a `~` where
  `*` was almost certainly meant. It yields a bogus `XX` segment and the `SE` count does not
  match. It is the standing case for "unknown segment preserved" and "malformed input does
  not abort".
- The two short ISAs are why delimiters are read by counting separators, never by offset.
- `blue_cross_nc_sample.txt` is a fragment without an envelope: `Tokenizer::new` must fail
  with `NotIsa` and `Tokenizer::with_delimiters` must work.
- `multi_claim_sample.txt` carries `N3`/`N4` patient address segments inside loop 2100, which
  the 835 standard does not define there; with the built-in spec they are reported as
  unmatched (4 segments), and a user patch adding them to loop 2100 captures them.
