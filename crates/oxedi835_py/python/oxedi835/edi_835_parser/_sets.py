"""``parse``, ``TransactionSets`` and ``TransactionSet``."""

from __future__ import annotations

import datetime
import functools
import os
import warnings
from typing import Iterator, List, Optional

from . import _codes
from ._convert import date, integer, money, text
from ._tables import load
from ._views import (
    Address, Claim, FinancialInformation, Interchange, Location, Organization,
    OrganizationSegment, at_segment, mapped,
)

_SUFFIXES = (".txt", ".835", ".DAT")


class TransactionSet:
    """One transaction (ST to SE) of one file."""

    def __init__(self, document, tables, file_path):
        self._d, self._t, self.file_path = document, tables, file_path

    @functools.cached_property
    def interchange(self):
        t = self._t["rows_interchanges"]
        if not len(t):
            return None
        day, clock = t["transmission_date"][0], t["transmission_time"][0]
        sent = None if day is None or clock is None else datetime.datetime.combine(day, clock)
        qualifier = text(t["authorization_information_qualifier"][0])
        return Interchange(
            authorization_information_qualifier=None if qualifier == "00" else qualifier,
            sender=text(t["sender"][0]).strip(),
            receiver=text(t["receiver"][0]).strip(),
            transmission_date=sent,
        )

    @functools.cached_property
    def financial_information(self):
        t = self._t["rows_payments"]
        return FinancialInformation(
            amount_paid=money(t["amount_paid"][0]),
            payment_method=mapped(_codes.PAYMENT_METHODS, text(t["payment_method"][0])),
            routing_number=integer(t["routing_number"][0]),
            transaction_date=date(t["transaction_date"][0], self._d, t["bpr_segment"][0], 16),
        )

    @functools.cached_property
    def organizations(self) -> List[Organization]:
        t = self._t["rows_organizations"]
        out = []
        for i in range(len(t)):
            location = None
            if any(t[k][i] is not None for k in ("city", "state", "zip_code")):
                location = Location(text(t["city"][i]), text(t["state"][i]), text(t["zip_code"][i]))
            address = None if t["address"][i] is None else Address(text(t["address"][i]))
            out.append(Organization(
                OrganizationSegment(mapped(_codes.ORGANIZATION_TYPES, text(t["type"][i])),
                                    text(t["name"][i]), text(t["identification_code"][i])),
                location, address, t["segment"][i]))
        return out

    @functools.cached_property
    def claims(self) -> List[Claim]:
        return [Claim(self._t, self._d, i) for i in range(len(self._t["rows_claims"]))]

    def _organization(self, role):
        found = at_segment(self.organizations, self._t["rows_payments"][f"{role}_segment"][0])
        if found is None:
            raise ValueError(
                f"{self.file_path}: the transaction at segment "
                f"{self._t['rows_payments']['segment'][0]} has no {role} loop (N1)"
            )
        return found

    @property
    def payer(self) -> Organization:
        return self._organization("payer")

    @property
    def payee(self) -> Organization:
        return self._organization("payee")

    def __repr__(self):
        return "\n".join(str(item) for item in (
            ("interchange", self.interchange), ("financial_information", self.financial_information),
            ("claims", self.claims), ("organizations", self.organizations), ("file_path", self.file_path)))

    @staticmethod
    def serialize_service(financial_information, payer, claim, service) -> dict:
        """The library's columns for one service, in its order."""
        start = service.service_period_start or claim.claim_statement_period_start
        end = service.service_period_end or claim.claim_statement_period_end
        rendering = claim.rendering_provider
        status = claim.claim.status
        return {
            "marker": claim.claim.marker,
            "patient": claim.patient.name if claim.patient else None,
            "code": service.service.code,
            "modifier": service.service.modifier,
            "qualifier": service.service.qualifier,
            "allowed_units": service.service.allowed_units,
            "billed_units": service.service.billed_units,
            "transaction_date": financial_information.transaction_date,
            "icn": claim.claim.icn,
            "charge_amount": service.service.charge_amount,
            "allowed_amount": service.allowed_amount,
            "paid_amount": service.service.paid_amount,
            "payer": payer.organization.name,
            "start_date": start.date if start else None,
            "end_date": end.date if end else None,
            "rendering_provider": rendering.name if rendering else None,
            "payer_classification": str(status.payer_classification),
            "was_forwarded": status.was_forwarded,
        }

    @classmethod
    def service_record(cls, financial_information, payer, claim, service) -> dict:
        """One row of ``to_dataframe``: the serialized service and its
        adjustments, references and remarks numbered from 0."""
        datum = cls.serialize_service(financial_information, payer, claim, service)
        for n, adjustment in enumerate(service.adjustments):
            datum[f"adj_{n}_group"] = adjustment.group_code.code
            datum[f"adj_{n}_code"] = adjustment.reason_code.code
            datum[f"adj_{n}_amount"] = adjustment.amount
        for n, reference in enumerate(service.references):
            datum[f"ref_{n}_qual"] = reference.qualifier.code
            datum[f"ref_{n}_value"] = reference.value
        for n, remark in enumerate(service.remarks):
            datum[f"rem_{n}_qual"] = remark.qualifier.code
            datum[f"rem_{n}_code"] = remark.code.code
        return datum

    def to_dataframe(self):
        """One row per service, as edi-835-parser builds it."""
        import pandas as pd

        financial_information, payer = self.financial_information, self.payer
        return pd.DataFrame([
            self.service_record(financial_information, payer, claim, service)
            for claim in self.claims
            for service in claim.services
        ])


class TransactionSets:
    """Every transaction of the files parsed."""

    def __init__(self, transaction_sets):
        self.transaction_sets = list(transaction_sets)

    def __iter__(self) -> Iterator[TransactionSet]:
        yield from self.transaction_sets

    def __len__(self) -> int:
        return len(self.transaction_sets)

    def __repr__(self):
        return "\n".join(str(t) for t in self)

    def to_dataframe(self):
        """Every transaction's rows, with the numbered columns sorted last."""
        import pandas as pd

        frames = [t.to_dataframe() for t in self]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", FutureWarning)
            data = pd.concat(frames) if frames else pd.DataFrame()
        return TransactionSets.sort_columns(data)

    @staticmethod
    def sort_columns(data):
        variable = sorted(c for c in data.columns if any(s in c for s in ("adj", "ref", "rem")))
        static = [c for c in data.columns if c not in variable]
        return data[static + variable]

    def sum_payments(self) -> float:
        return sum((t.financial_information.amount_paid for t in self), 0)

    def count_claims(self) -> int:
        return sum(len(t.claims) for t in self)

    def count_patients(self) -> int:
        return len({
            c.patient.identification_code if c.patient else None for t in self for c in t.claims
        })


def _sets(data, file_path) -> List[TransactionSet]:
    document, parts = load(data)
    return [TransactionSet(document, tables, file_path) for tables in parts]


def parse_bytes(data, file_path: Optional[str] = None) -> TransactionSets:
    """Parses one file held in ``bytes``, ``bytearray`` or ``memoryview``;
    ``file_path`` is what each ``TransactionSet.file_path`` reports."""
    return TransactionSets(_sets(data, "<bytes>" if file_path is None else file_path))


def parse_file_obj(file, file_path: Optional[str] = None) -> TransactionSets:
    """Parses what ``file.read()`` returns (a binary file object)."""
    return parse_bytes(file.read(), file_path)


def parse_many(items) -> TransactionSets:
    """Parses each item (bytes-like or binary file object) in order."""
    sets = []
    for item in items:
        if hasattr(item, "read"):
            sets.extend(parse_file_obj(item).transaction_sets)
        else:
            sets.extend(parse_bytes(item).transaction_sets)
    return TransactionSets(sets)


def _path(path) -> List[TransactionSet]:
    with open(path, "rb") as handle:
        return _sets(handle.read(), str(path))


def parse(path: str, debug: bool = False) -> TransactionSets:
    """Parses a file path, or every ``.txt``, ``.835`` and ``.DAT`` file of a
    directory, with the same signature and behaviour as ``edi_835_parser.parse``.
    Data already in memory goes through ``parse_bytes``, ``parse_file_obj`` or
    ``parse_many``."""
    path = os.path.expanduser(os.fspath(path))
    if not os.path.isdir(path):
        return TransactionSets(_path(path))
    sets = []
    for name in os.listdir(path):
        if not name.endswith(_SUFFIXES):
            continue
        file_path = f"{path}/{name}"
        try:
            sets.extend(_path(file_path))
        except Exception as error:
            if debug:
                raise
            warnings.warn(f"Failed to build a transaction set from {file_path} with error: {error}")
    return TransactionSets(sets)
