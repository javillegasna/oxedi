"""The extended frame: edi-835-parser's rows plus what that frame leaves out."""

from __future__ import annotations

import warnings

from ._convert import money, moment, text

LIBRARY_COLUMNS = (
    "marker", "patient", "code", "modifier", "qualifier", "allowed_units", "billed_units",
    "transaction_date", "icn", "charge_amount", "allowed_amount", "paid_amount", "payer",
    "start_date", "end_date", "rendering_provider", "payer_classification", "was_forwarded",
)
GROUP = (("group", "group", text), ("code", "code", text),
         ("amount", "amount", money), ("quantity", "quantity", money))
AMOUNT = (("qual", "qualifier", text), ("amount", "amount", money))
# (table, column prefix, ((suffix, table column, conversion), ...))
CLAIM_LISTS = (
    ("rows_claim_adjustments", "x_claim_adj", GROUP),
    ("rows_claim_references", "x_claim_ref", (("qual", "qualifier", text), ("value", "value", text))),
    ("rows_claim_amounts", "x_claim_amt", AMOUNT),
)
SERVICE_LISTS = (
    ("rows_adjustment_groups", "x_svc_adj", GROUP),
    ("rows_service_amounts", "x_svc_amt", AMOUNT),
)


def _numbered(tables, lists, parent, key):
    """Each list's rows under ``key`` as columns numbered from 0."""
    datum = {}
    for table, prefix, fields in lists:
        rows = tables[table]
        for n, at in enumerate(rows.under(parent, key)):
            for suffix, column, convert in fields:
                datum[f"{prefix}_{n}_{suffix}"] = convert(rows[column][at])
    return datum


def _claim_record(transaction_date, payer, claim):
    """The library's columns for a claim without services: claim-level values
    only, every service value ``None``."""
    start, end = claim.claim_statement_period_start, claim.claim_statement_period_end
    rendering, status = claim.rendering_provider, claim.claim.status
    datum = dict.fromkeys(LIBRARY_COLUMNS)
    datum.update(
        marker=claim.claim.marker,
        patient=claim.patient.name if claim.patient else None,
        transaction_date=transaction_date,
        icn=claim.claim.icn,
        payer=payer.organization.name,
        start_date=start.date if start else None,
        end_date=end.date if end else None,
        rendering_provider=rendering.name if rendering else None,
        payer_classification=str(status.payer_classification),
        was_forwarded=status.was_forwarded,
    )
    return datum


def records(transaction_set):
    """Rows in file order: each claim's services (or the claim alone when it
    has none), then the transaction's provider adjustments."""
    tables = transaction_set._t
    plb = tables["rows_provider_adjustments"]
    if not transaction_set.claims and not len(plb):
        return
    financial_information, payer = transaction_set.financial_information, transaction_set.payer
    transaction_date = financial_information.transaction_date if financial_information else None
    for claim in transaction_set.claims:
        claim_extra = {"x_claim": claim._row, **_numbered(tables, CLAIM_LISTS, "claim", claim._row)}
        if not claim.services:
            yield {**_claim_record(transaction_date, payer, claim), **claim_extra, "x_row_kind": "claim"}
        for service in claim.services:
            datum = transaction_set.service_record(financial_information, payer, claim, service)
            datum.update(claim_extra)
            datum.update(_numbered(tables, SERVICE_LISTS, "service", service._row))
            datum["x_row_kind"] = "service"
            yield datum
    for at in range(len(plb)):
        yield {
            "transaction_date": transaction_date,
            "payer": payer.organization.name,
            "x_row_kind": "provider_adjustment",
            "x_plb_provider_id": text(plb["provider_id"][at]),
            "x_plb_fiscal_period_date": moment(plb["fiscal_period_date"][at]),
            "x_plb_reason_code": text(plb["reason_code"][at]),
            "x_plb_reference_id": text(plb["reference_id"][at]),
            "x_plb_amount": money(plb["amount"][at]),
        }


def frame(transaction_sets):
    """The library's columns in its order whichever transaction comes first,
    then ``x_row_kind`` and ``x_claim``, then the other ``x_`` columns by name.
    Rows without a service value widen some library columns' dtypes."""
    import pandas as pd

    from ._sets import TransactionSets

    with warnings.catch_warnings():
        warnings.simplefilter("ignore", FutureWarning)
        data = pd.DataFrame([datum for ts in transaction_sets for datum in records(ts)])
    if data.empty:
        return data
    fixed = ("x_row_kind", "x_claim")
    added = sorted(c for c in data.columns if c.startswith("x_") and c not in fixed)
    strict = [c for c in LIBRARY_COLUMNS if c in data.columns]
    strict += [c for c in data.columns if not c.startswith("x_") and c not in LIBRARY_COLUMNS]
    library = TransactionSets.sort_columns(data[strict])
    return pd.concat([library, data[[*fixed, *added]]], axis=1)
