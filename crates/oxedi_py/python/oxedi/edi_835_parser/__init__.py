"""edi-835-parser's API over oxedi."""

from ._sets import (
    TransactionSet, TransactionSets, parse, parse_bytes, parse_file_obj, parse_many,
)
from ._tables import spec

__all__ = [
    "TransactionSet", "TransactionSets", "parse", "parse_bytes", "parse_file_obj",
    "parse_many", "spec",
]
