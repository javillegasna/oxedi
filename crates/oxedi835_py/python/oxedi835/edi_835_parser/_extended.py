"""The extended frame: edi-835-parser's rows plus what that frame leaves out."""

from __future__ import annotations

import warnings

from ._frame import LIBRARY_COLUMNS, extended, frame as columns_frame

__all__ = ["LIBRARY_COLUMNS", "frame"]


def frame(transaction_sets):
    """The library's columns in its order whichever transaction comes first,
    then ``x_row_kind`` and ``x_claim`` (absent when no row has a claim),
    then the other ``x_`` columns by name.
    Rows without a service value widen some library columns' dtypes."""
    import pandas as pd

    from ._sets import TransactionSets

    rows, columns = extended(transaction_sets)
    if not rows:
        return pd.DataFrame([])
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", FutureWarning)
        data = columns_frame(columns)
    fixed = [c for c in ("x_row_kind", "x_claim") if c in data.columns]
    added = sorted(c for c in data.columns if c.startswith("x_") and c not in fixed)
    strict = [c for c in LIBRARY_COLUMNS if c in data.columns]
    strict += [c for c in data.columns if not c.startswith("x_") and c not in LIBRARY_COLUMNS]
    library = TransactionSets.sort_columns(data[strict])
    return pd.concat([library, data[[*fixed, *added]]], axis=1)
