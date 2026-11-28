# Diagnostics

A file that is valid EDI but has questionable data still parses. Each problem comes back as a
diagnostic instead of an exception. This page shows how to read them and what each kind means.

## List the diagnostics of a file

The example file is clean. This example edits two values first: it breaks a service date and
changes an adjustment amount so a claim no longer balances.

```python
import oxedi
from pathlib import Path

data = Path("remittance.835").read_bytes()
edited = data.replace(b"CAS*CO*96*40~", b"CAS*CO*96*45~").replace(b"DTM*472*20240107", b"DTM*472*2024017")
result = oxedi.parse(edited)

for diagnostic in sorted(result.diagnostics, key=lambda d: d.level):
    print(diagnostic)
```

```text
SNIP 2 · element DTM02 (date) is not a valid DT (date CCYYMMDD or YYMMDD) · segment #53, element 2 · at interchange#1/group#1/transaction#1/2000#2/2100#3/2110#4 · datum "2024017"
SNIP 3 · balancing rule "service_balance" fails in loop "2110" opened at segment #41: SVC02 of 2110 "service_payment_information" - SVC03 of 2110 "service_payment_information" is 250.00, but sum of CAS03, CAS06, CAS09, CAS12, CAS15, CAS18 of 2110 "service_adjustment" adds up to 255.00 (off by -5.00); read from segments #41, #43, #44 · segment #41, element 2 · at interchange#1/group#1/transaction#1/2000#1/2100#2/2110#3 · datum "700"
SNIP 3 · balancing rule "claim_balance" fails in loop "2100" opened at segment #36: CLP03 of 2100 "claim_payment_information" - CLP04 of 2100 "claim_payment_information" is 300.00, but sum of CAS03, CAS06, CAS09, CAS12, CAS15, CAS18 of 2100 "claim_adjustment" + sum of CAS03, CAS06, CAS09, CAS12, CAS15, CAS18 of 2110 "service_adjustment" adds up to 305.00 (off by -5.00); read from segments #36, #37, #43, #44 · segment #36, element 3 · at interchange#1/group#1/transaction#1/2000#1/2100#2 · datum "700"
```

- `result.diagnostics` holds them in file order. Sort by `level` to read the lowest first.
- Each batch of [`stream`](large-files.md) carries its own `diagnostics`.
- The parse still returns every table. The bad date reads as null in `service_date`.

## Read one diagnostic

A diagnostic names the rule that failed, where it failed and the offending value. Its fields:

- `level`: the [SNIP level](#the-three-levels) of the rule: 1, 2 or 3.
- `kind`: the name of the rule that failed, for example `RequiredElementMissing`.
- `rule`: the rule as a sentence with its values.
- `segment`: the index of the segment at fault, or `None` at the end of the file.
- `element` and `component`: the 1-based position inside the segment, when the diagnostic has one.
- `path`: the loops open at that segment, outermost first, joined by `/`. It is empty at the
  root. `2110#4` is the fourth service line of the file.
- `datum`: the offending value as it appears in the file (bytes).
- `origin` and `code`: who reported it. That is `"oxedi"`, or `"pyx12"` with pyx12's own code
  (see [Validating with pyx12](pyx12.md)).

Filter on `kind` to pick one sort of diagnostic:

```python
bad_dates = [d for d in result.diagnostics if d.kind == "TypeMismatch"]
print(bad_dates[0].segment, bad_dates[0].element, bad_dates[0].datum)
```

```text
53 2 b'2024017'
```

## Find the segment in your file

`segment` indexes the document. `result.document[d.segment]` is the segment, `.raw` its bytes
and `.span` its byte range in your file:

```python
segment = result.document[bad_dates[0].segment]
print(segment.raw, segment.span)
```

```text
b'\nDTM*472*2024017~' (1362, 1379)
```

The bytes include the line break before the segment, which belongs to it.

## The three levels

| Level | Checks | Kinds |
|---|---|---|
| 1, integrity | Envelopes, control numbers and counts, segment structure | `ByteOrderMark`, `UnknownSegment`, `ImplicitLoop`, `UnterminatedLoop`, `ControlCountMismatch`, `ControlElementMissing`, `ControlNumberMismatch` |
| 2, requirements | Required elements, types, lengths, codes and the loop structure | `RequiredElementMissing`, `TypeMismatch`, `LengthOutOfRange`, `CompositeShape`, `CodeNotInList`, `ValueDropped`, `RequiredOccurrenceMissing`, `OccurrenceOverMax`, `LoopOverMax`, `OutOfOrder`, `UnknownOccurrence`, `RequiredLoopMissing` |
| 3, balancing | Amounts that must add up | `BalanceMismatch` |

??? note "Details: what each kind means"
    | Kind | Meaning |
    |---|---|
    | `ByteOrderMark` | The file starts with a UTF-8 byte order mark. Informational: nothing else changes. |
    | `UnknownSegment` | No open loop holds the segment and it opens no loop. |
    | `ImplicitLoop` | A loop was opened without its own first segment, to hold a segment that belongs inside it. |
    | `UnterminatedLoop` | A loop that has an end segment closed without it. |
    | `ControlCountMismatch` | A trailer's count does not match what it counts. |
    | `ControlElementMissing` | A control number or count is missing from its segment. |
    | `ControlNumberMismatch` | A trailer's control number differs from its header's. |
    | `RequiredElementMissing` | A required element or component is absent or empty. |
    | `TypeMismatch` | A value does not parse as its type, such as a date. |
    | `LengthOutOfRange` | A value is shorter or longer than allowed. |
    | `CompositeShape` | A composite element has more components than defined. |
    | `CodeNotInList` | A value is not in its element's code list. |
    | `ValueDropped` | A text value was too large to store in its column. Its cell is null. |
    | `RequiredOccurrenceMissing` | A loop closed without a segment it requires. |
    | `OccurrenceOverMax` | A segment repeats more often in one loop than allowed. |
    | `LoopOverMax` | A loop repeats more often under one parent than allowed. |
    | `OutOfOrder` | A segment comes after one that should follow it. |
    | `UnknownOccurrence` | A segment matches none of the forms its loop allows, for example an unknown qualifier. |
    | `RequiredLoopMissing` | A required child loop is absent. |
    | `BalanceMismatch` | The amounts of a balancing rule do not add up in one loop. |
    | `External` | A finding of another validator, such as pyx12. |

## Report another validator's findings

An adapter to another validator turns each finding into a `Diagnostic`, so the finding sits in
the same list as the parser's own. `parse_document` gives the document and the spec without
building the tables, `Document.loop_paths` gives the loops open at a segment, and
`Diagnostic.external` builds the diagnostic. `Mask.diagnostics` masks such findings as `parse`
masks its own.

```python
document, spec, _ = oxedi.parse_document(data)
(path,) = document.loop_paths(spec, [10])
finding = oxedi.Diagnostic.external("my-validator", "claim looks wrong", 2, segment=10, path=path)
print(finding.origin, finding.level, finding.path)
```

```text
my-validator 2 interchange#1/group#1/transaction#1/2000#1/2100#1
```

## What to do about a diagnostic

A diagnostic describes the file, not a failure of the parse. You decide what to do with it:

- Keep the tables and log the diagnostics, for example to review them later.
- Stop the file and send it back to its sender. [oxedi in a pipeline](../recipes/pipelines.md)
  shows a step that does this.
- Count diagnostics across many files in SQL with
  `read_835(..., table_name := 'diagnostics')`. See
  [a Parquet lakehouse](../recipes/lakehouse.md#use-diagnostics-as-a-quality-gate).

When a payer always sends a value the spec does not allow, adapt the spec instead. This patch
adds `7`, the made-up status of the edited example above, to the codes of the claim status:

```python
broken = data.replace(b"CLP*CLM1002*2*", b"CLP*CLM1002*7*")
codes = ["1", "2", "3", "4", "7", "19", "20", "21", "22", "23", "25"]
spec = oxedi.Spec.builtin().patch({"segments": {"CLP": {"elements": {"2": {"codes": codes}}}}})

print(len(oxedi.parse(broken).diagnostics), len(oxedi.parse(broken, spec=spec).diagnostics))
```

```text
1 0
```

A patch replaces a list whole, so it repeats the codes the spec already had. See
[Customizing the spec](../customize/spec.md).
