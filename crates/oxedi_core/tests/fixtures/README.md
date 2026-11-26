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

## Edge-case fixtures for money

Two more hand-written files, valid end to end like the balanced ones, for the cases where money
moves in ways the balanced fixtures do not show. Same rule: **never edit them**, and every name,
identifier and amount is invented. Both hold the same remittance; the 5010 file adds the payer
technical contact (`PER*BL`) and, unlike the balanced 5010 fixture, carries no `ST03`.

| File | Bytes | Segments | Version | Repetition | Line endings | Claims | Services | CAS rows | PLB groups |
|------|-------|----------|---------|------------|--------------|--------|----------|----------|------------|
| `edge_cases_5010_sample.txt` | 1 493 | 50 | 00501 / 005010X221A1 | `^` | LF after every `~` | 4 | 2 | 5 | 8 |
| `edge_cases_4010_sample.txt` | 1 408 | 49 | 00401 / 004010X091A1 | — | none | 4 | 2 | 5 | 8 |

What they cover:

- **Interest.** A claim paid on time (`MRC1001`) and one paid late (`MRC1002`, received
  2023-11-20, `DTM*050`) that carries `AMT*I*7.25` in loop 2100. The interest is not in the
  claim's payment or its `CAS`: the claim balances on its own (400 − 300 = 100 `CO` 45), and the
  money goes through `PLB` as `L6:MRC1002*-7.25`, negative, so it adds to the payment.
- **Reversal and correction.** An earlier payment of claim `MRC0950` reversed (`CLP02` 22,
  charge −300, payment −240, `CAS*CO*45*-60`, the original payer claim control number
  `BHP23950`), then the corrected claim (`CLP02` 1, charge 300, payment 270, `CAS*CO*45*30`, a new
  payer control number and `REF*F8` pointing at the original). Both balance: −300 − (−240) = −60
  and 300 − 270 = 30.
- **A `PLB` over two segments.** Eight adjustments, more than the six one `PLB` holds: six in the
  first segment, two in the second, same provider and fiscal period. Codes `L6`, `WO` (twice),
  `FB`, `CS` (twice, one negative), `72` and `J1`; total 65.50.
- **Bank details.** `BPR` paid by `ACH` in `CCP` format with every bank field filled: sender DFI
  qualifier and number, account qualifier and number, originating company identifier (equal to
  `TRN03`), receiver DFI qualifier and number, account qualifier and number. No built-in table
  reads them, so a written file leaves them out and still reads to the same tables.
- **Transaction balance.** `BPR02` (444.50) equals the claims' payments (180 + 300 − 240 + 270
  = 510) minus the `PLB` amounts (−7.25 + 35 + 12.50 − 4 + 8 + 3 − 1.75 + 20 = 65.50).

`pyx12` finds nothing in the 4010 file and one thing in the 5010 file: the second `PLB` segment.
Its 5010 map wraps `PLB` in a loop that may occur once, and pyx12 counts a second `PLB` as a
second occurrence of that loop, although the map lets `PLB` itself repeat; the 4010 map's
loop may repeat.

## Findings on patient-data places

Two synthetic files for the patient-data mask: copies of the balanced fixtures with one
finding at each place a mask flags, so a masked parse has diagnostics whose datum would
otherwise quote patient data. Same rule as above: **never edit them**. Every name, identifier
and amount is invented. `pyx12` finds the same five in both files, with two differences: in the
5010 file also its `ST03`, which its 5010 map marks Not Used, and in the 4010 file it reports the
six-digit date twice, as too short and as an invalid date.

| File | Bytes | Version | Line endings | Diagnostics |
|------|-------|---------|--------------|-------------|
| `masked_findings_5010_sample.txt` | 1 855 | 00501 / 005010X221A1 | LF after every `~` | 5 |
| `masked_findings_4010_sample.txt` | 1 726 | 00401 / 004010X091A1 | none | 5 |

Each finding is a length or format rule, the same five in both files, all in the first claim:

- `CLP01` of `CLM1001` is 69 characters (the spec allows 1 to 38).
- `NM109` of the `NM1*QC` patient segment is a 91-character member identifier (2 to 80).
- `REF02` of `REF*1L` is a 66-character group number (1 to 50).
- `DTM02` of the claim's `DTM*232` is not a valid date: `20241341` in the 5010 file and the
  six-digit `241341` in the 4010 file.
- `REF02` of the service's `REF*6R` is a 59-character line item control number (1 to 50).

A masked parse replaces each of the five datums with a 16-character token.

## A payer technical contact under 4010

One synthetic 4010 file for the payer technical contact: a copy of `balanced_4010_sample.txt`
with its `PER*CX*CLAIMS DEPT*TE*8005551234` replaced by `PER*BL*EDI SUPPORT*TE*8005559876`.
**Never edit it.** Every name and number is invented.

| File | Bytes | Version | Line endings | Diagnostics |
|------|-------|---------|--------------|-------------|
| `payer_technical_contact_4010_sample.txt` | 1 470 | 00401 / 004010X091A1 | none | 1 |

It has 61 segments. The one diagnostic is PER01 `BL`, which the 4010 list of that element (`CX`)
leaves out. The reader still fills the three payer technical contact columns of `payments` from
that `PER`, and the writer writes them back there, so the written file reads back with the same
diagnostic (a strict write refuses it with that one finding).
