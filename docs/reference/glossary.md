# Glossary

The terms these pages use, in plain language. For a walk through a real file, read
[The 835 in five minutes](../start/the-835.md).

## The file and its parts

### Interchange

The outer envelope of a file, from `ISA` to `IEA`. It names the sender and the receiver, the
date and time, and carries a control number. It holds one or more functional groups.
`oxedi.write` produces one interchange.

### Functional group

A group of transactions inside an interchange, from `GS` to `GE`. `GS08` names the version of
the format, such as `005010X221A1`.

### Transaction set

One 835, from `ST` to `SE`. Also called a transaction. In an 835 it is one payment, so it is one
row of `payments`. [`oxedi.stream`](../python/large-files.md) yields one batch per transaction.

### Segment

One record of the file: an id such as `CLP`, then its elements, then the segment terminator
(`~` by default). Every byte of a file belongs to exactly one segment, which is how oxedi
writes a file back unchanged.

### Element

One value inside a segment. Elements are separated by the element separator (`*` by default)
and numbered from 1, so `CLP02` is the second element of `CLP`.

### Component

One part of a composite element. Components are separated by the component separator (`:` by
default). `SVC01-2` is the second component of the first element of `SVC`, as in
`SVC*HC:99213`.

### Delimiters

The characters that split a file: element separator, component separator, segment terminator,
and the optional repetition separator and release character. A file declares them in its `ISA`
segment, and oxedi reads them from there.

### Qualifier

An element that says what the next value means. `N1*PR` is the payer and `N1*PE` the payee.
`DTM*232` is a statement start date. The docs write a segment with its qualifier this way.

### Loop

A group of segments that repeats together, named by the implementation guide: `1000A` (the
payer), `1000B` (the payee), `2000` (a group of claims), `2100` (a claim), `2110` (a service
line). A diagnostic's path names the open loops, with the instance number: `2100#2` is the
second claim.

### Implementation guide

The published rules for using the 835 in health care: which segments, loops, elements and codes
a file has. The 5010 guide is `005010X221A1` and the 4010 guide `004010X091A1`.

### Spec

oxedi's description of the format, in JSON: loops, segments, elements, code lists, balancing
rules and the tables to fill. oxedi ships a 5010 and a 4010 spec, and you can patch them. See
[Customizing the spec](../customize/spec.md).

The built-in specs are written as **packages**, one per layer (envelope, dictionary, transaction
set, implementation guide, views); a stack of packages makes one spec. See
[Packages and layers](../customize/packages.md).

### Control number

A number that must match between a header and its trailer: `ISA13` and `IEA02`, `GS06` and
`GE02`, `ST02` and `SE02`. The trailers also carry counts (`SE01`, `GE01`, `IEA01`), so a
receiver can check that nothing is missing.

### 5010 and 4010

The two versions of the 835 that oxedi reads and writes. 5010 is the default. A file declares
its version in `ISA12` and `GS08`.

## Payments and claims

### ERA

Electronic remittance advice: another name for the 835.

### Payer

Who pays: usually a health plan. It sends the 835. Its loop is `1000A`, opened by `N1*PR`.

### Payee

Who is paid: usually the provider that sent the claims. It receives the 835. Its loop is
`1000B`, opened by `N1*PE`.

### Check and EFT

The two usual ways the money moves: a paper check, or an electronic funds transfer (EFT). The
money travels apart from the 835. `BPR04` holds the payment method as a code, such as `CHK` or
`ACH`.

### Trace number

`TRN02`, the check or EFT trace number. It lets the payee match the 835 to the money it
received. It is the `trace_number` column of `payments`.

### Claim

A request for payment the provider sent for some care. In the 835, each claim is a `CLP`
segment and its loop `2100`, and one row of `claims`.

### Claim status

`CLP02`, a code for how the payer processed the claim. The 5010 spec lists ten codes. The
[Polars recipe](../recipes/polars.md) counts status `4` as a denial.

### Service line

One billed procedure or service of a claim: an `SVC` segment and its loop `2110`, and one row
of `services`.

### Adjustment

A reason the paid amount differs from the billed one, with its amount: a `CAS` segment on a
claim or on a service line. Each reason in a `CAS` is one row of `adjustments`.

### Group code

`CAS01`, who bears an adjustment: `CO` contractual obligation, `PR` patient responsibility,
`OA` other adjustment, `PI` payer-initiated reduction, and `CR` correction in 4010 files.

### CARC

Claim adjustment reason code: the reason of an adjustment, in `CAS02`, `CAS05` and so on. It is
the `reason_code` column of `adjustments`. The codes come from an external code list whose
texts are licensed, so these pages do not reproduce them. Look a code up in the official list.

### RARC

Remittance advice remark code: a remark that adds detail to an adjustment, also from an
external, licensed code list. In the 835 they sit in `LQ` (on a service line) and in `MIA` and
`MOA` (on a claim). The built-in tables have no column for them. The segments stay in the
document, and a [spec patch](../customize/spec.md) can add a column.

### Provider adjustment

An amount added to or taken from the whole payment, not from one claim: a `PLB` segment. Each
adjustment in a `PLB` is one row of `provider_adjustments`.

### Balancing

The amounts of an 835 must add up. A service line's charge minus its payment equals its
adjustments. A claim's charge minus its payment equals its adjustments and those of its service
lines. `BPR02` equals the claims' payments minus the provider adjustments. A rule that fails is
a level 3 diagnostic.

## Reading and validating

### Diagnostic

One finding about the data of a file: the rule that failed, where, and the value. oxedi returns
diagnostics as data instead of raising. See [Diagnostics](../python/diagnostics.md).

### SNIP level

The level a validation rule belongs to. oxedi reports three:

- **Level 1, integrity:** envelopes, control numbers and counts, segment structure.
- **Level 2, requirements:** required elements, types, lengths, codes and the loop structure.
- **Level 3, balancing:** amounts that must add up.

### Document

The file as oxedi holds it, indexed into segments and kept byte for byte.
`result.document.write()` returns the file unchanged.

### Envelope

What the writer needs besides the tables: sender, receiver, date, time and control number. In
Python it is `oxedi.Envelope`. See [Writing](../python/writing.md#fill-in-the-envelope).

### Lossless

Nothing in the file is dropped or changed. Unknown segments stay in the document, and a file
writes back exactly as it was read.

## Segment ids

| Id | Name | What it holds in an 835 |
|---|---|---|
| `ISA` | Interchange control header | Sender, receiver, date, time, version, control number and the delimiters. |
| `IEA` | Interchange control trailer | The number of groups and the interchange control number. |
| `GS` | Functional group header | Application sender and receiver, date, time, control number, version. |
| `GE` | Functional group trailer | The number of transactions and the group control number. |
| `ST` | Transaction set header | Starts one 835 and its control number. |
| `SE` | Transaction set trailer | The number of segments and the transaction control number. |
| `BPR` | Financial information | The total paid, credit or debit, method, format and date. |
| `TRN` | Reassociation trace number | The trace number and the payer identifier. |
| `REF` | Reference identification | An id, with a qualifier that says which one. |
| `DTM` | Date or time reference | A date, with a qualifier that says which one. |
| `N1` | Party identification | The payer (`N1*PR`) or the payee (`N1*PE`): name and id. |
| `N3` | Address | A street address. |
| `N4` | City, state and ZIP code | City, state and postal code. |
| `PER` | Contact information | A contact name and number. |
| `LX` | Header number | Starts a group of claims (loop `2000`). |
| `CLP` | Claim payment information | A claim: id, status, charge, payment, patient responsibility. |
| `NM1` | Individual or organization name | A person or organization of the claim, such as the patient (`NM1*QC`). |
| `MIA`, `MOA` | Inpatient and outpatient adjudication information | Adjudication amounts and remark codes of a claim. |
| `AMT` | Supplemental amount | An amount, with a qualifier that says which one. |
| `SVC` | Service payment information | A service line: procedure, charge, payment, units. |
| `CAS` | Claim or service adjustment | Up to six adjustments of one group: reason, amount, quantity. |
| `LQ` | Health care remark codes | Remark codes of a service line. |
| `PLB` | Provider adjustment | Up to six adjustments of the whole payment. |
