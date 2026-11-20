# When the writer refuses a file

The writer is strict: it writes a file only when the file would be valid. This page shows what
it reports when it refuses, how to write anyway, and how it treats money and empty text.

The examples start from the tables and envelope of [Writing 835 files](writing.md):

```python
import datetime
from decimal import Decimal

import polars as pl
import oxedi

result = oxedi.parse_file("remittance.835")
frames = result.tables.to_polars()
envelope = oxedi.Envelope(
    sender_id="ACMEPAYER",
    receiver_id="SUNRISECLINIC",
    date=datetime.date(2024, 1, 10),
    time=datetime.time(9, 0),
    usage_indicator="T",
)
```

## See why a file was refused

Raise the payment of the first claim by 10.00, so the money no longer balances, and write:

```python
frames["claims"] = frames["claims"].with_columns(
    payment_amount=pl.when(pl.col("row") == 0)
    .then(pl.lit(Decimal("210.00")))
    .otherwise(pl.col("payment_amount"))
)

try:
    data = oxedi.write(frames, envelope, spec=result.spec)
except oxedi.WriteError as error:
    for finding in error.findings:
        print(finding.table, finding.row, finding.column)
```

```text
claims 0 charge_amount
payments 0 total_payment_amount
```

- `oxedi.WriteError` is a `ValueError`. Its message lists every finding. Nothing is written.
- `error.findings` holds each one as an `oxedi.WriteFinding`. A finding names the table, row
  and column, or the envelope field, with the diagnostic behind it.
- Here the claim no longer balances, and neither does the payment's total.

??? note "Details: the full message"
    ```text
    the tables do not make a valid file (2 findings); nothing was written
    1. table "claims" row 0 column "charge_amount": SNIP 3 · balancing rule "claim_balance" fails in loop "2100" opened at segment #15: CLP03 of 2100 "claim_payment_information" - CLP04 of 2100 "claim_payment_information" is 90.00, but sum of CAS03, CAS06, CAS09, CAS12, CAS15, CAS18 of 2100 "claim_adjustment" + sum of CAS03, CAS06, CAS09, CAS12, CAS15, CAS18 of 2110 "service_adjustment" adds up to 100.00 (off by -10.00); read from segments #15, #24, #25, #30, #31 · segment #15, element 3 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum "300"
    2. table "payments" row 0 column "total_payment_amount": SNIP 3 · balancing rule "transaction_balance" fails in loop "transaction" opened at segment #2: BPR02 of transaction "financial_information" is 690.00, but sum of CLP04 of 2100 "claim_payment_information" - sum of PLB04, PLB06, PLB08, PLB10, PLB12, PLB14 of transaction "provider_adjustment" adds up to 700.00 (off by -10.00); read from segments #3, #15, #34, #45, #52, #56 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum "690"
    ```

## What counts as a finding

The written file is read back with the spec. Every diagnostic of that read is a finding, and so
is anything the writer cannot place. For example:

- A required element or occurrence without a value.
- A value outside its code list or its length.
- A row whose parent does not exist, or that comes out of order.
- A value holding a delimiter.
- Money that does not balance.

!!! note "A value holding a delimiter"
    This one is reported first. The file then splits where the data does not, so it cannot be
    read back. The other findings show once that value is fixed.

    With a release character in the delimiters, the writer puts it before a value's element
    separator, component separator, segment terminator or release character instead of
    reporting it. The interchange header's values and a value holding the repetition separator
    stay findings. See [Fill in the envelope](writing.md#fill-in-the-envelope).

## Write anyway

With `allow_findings=True`, `write` returns `(data, findings)` instead of raising. Use it, for
example, to produce invalid files for tests:

```python
data, findings = oxedi.write(frames, envelope, spec=result.spec, allow_findings=True)
print(len(findings))
```

```text
2
```

## Money is never changed

The writer checks the balancing rules. It never uses them to fill in a total:

- `BPR02` against the claims' payments and the `PLB` adjustments.
- Each claim's and each service's charge minus payment against their `CAS` adjustments.

!!! note "Interest"
    Interest a payer owes the provider for a late claim is not part of the claim's payment:
    `CLP04` and the `CAS` adjustments show the adjudication only. The claim reports its interest
    in `AMT*I`, which takes no part in balancing, and the money is paid in `PLB` with reason code
    `L6`. Interest the patient owes is a `CAS` adjustment, and so is already in the claim's sum.
    X12 states this in its public
    [interpretation 1705](https://x12.org/resources/requests-for-interpretation/rfi-1705-5010-835-interest-payment).
    The `PLB` amount is negative, so it adds to `BPR02`, because `BPR02` is the claims' payments
    minus the `PLB` amounts. UnitedHealthcare's public
    [provider-level adjustments guide](https://www.uhcprovider.com/content/dam/provider/docs/public/resources/edi/EDI-835-Provider-Level-Adjustments.pdf)
    describes the same sign convention.

    The balancing rules follow that layout. A file that puts interest into `CLP04` without a
    matching `CAS` does not balance, and the writer refuses it, as it should, unless you set
    `allow_findings`.

## Null and empty text

In text columns:

- `null` is an element the segment does not have.
- `""` is an element it has but leaves empty.

A null cell in the middle of a segment is still written as an empty element, so it reads back
as `""`. Only trailing nulls read back as `null`. The tables of a parse write back to the same
tables.
