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

## Balanced fixtures for the writer

Two synthetic files written by hand for the writer's round-trip and `pyx12` gate, the first
fixtures that are valid end to end: they balance, carry every occurrence the implementation
guide requires and parse with zero diagnostics. `pyx12` finds nothing in the 4010 file and one
thing in the 5010 file: its `ST03`, which pyx12's 5010 map marks Not Used. Same rule
as above: **never edit them**. Every name, identifier and amount is invented.

| File | Bytes | Segments | Version | Repetition | Line endings | Claims | Services | CAS rows | PLB groups |
|------|-------|----------|---------|------------|--------------|--------|----------|----------|------------|
| `balanced_5010_sample.txt` | 1 597 | 62 | 00501 / 005010X221A1 | `^` | LF after every `~` | 4 | 4 | 15 | 3 |
| `balanced_4010_sample.txt` | 1 470 | 61 | 00401 / 004010X091A1 | — | none | 4 | 4 | 15 | 3 |

Both hold the same remittance; the 5010 file adds `ST03` and the payer technical contact
(`PER*BL`), which 4010 does not define. What they cover:

- Two `LX` groups with two claims each: paid (`CLP02` 1), paid as secondary (2) and denied (4),
  one claim without services.
- Every balancing rule holds: each service's charge minus payment equals its `CAS`; each
  claim's charge minus payment equals its claim-level plus service-level `CAS` (a claim with
  only service adjustments, one with both, one with only a claim-level one, which then pays
  less than its services); `BPR02` (690) equals the claims' payments (720) minus the `PLB`
  amounts (25 − 5 + 10).
- The claim totals are consistent too: a claim with services charges the sum of their
  charges, and every claim's patient responsibility (`CLP05`) is the sum of its `PR`
  adjustments, claim and service level (0 when it has none).
- `CAS` with several reasons in one segment, with a quantity, several groups on one service,
  and a split group: seven `CO` reasons over two `CAS` segments (six, then one).
- A `PLB` with three reason groups, one negative; payer and payee `N1`/`N3`/`N4`, payee `REF*TJ`
  and payer `PER*CX`, which no built-in table carries (the written file leaves them out and
  still validates).
- Claim columns: patient and rendering provider (person and organization), statement dates,
  `AMT*AU`, `REF*1L`; service columns: `DTM*472`, `REF*6R`, `AMT*B6`, units paid and original
  units.
