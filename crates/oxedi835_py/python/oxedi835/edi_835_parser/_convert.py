"""Value conversions that reproduce edi-835-parser's element parsers."""

from __future__ import annotations

import datetime

# Claim status codes: (payer classification, forwarded to another payer).
STATUSES = {
    "1": ("processed as primary", "primary", False),
    "2": ("processed as secondary", "secondary", False),
    "3": ("processed as tertiary", "tertiary", False),
    "4": ("denial", "unspecified", False),
    "19": ("processed as primary, forwarded to additional payer(s)", "primary", True),
    "20": ("processed as secondary, forwarded to additional payer(s)", "secondary", True),
    "21": ("processed as tertiary, forwarded to additional payer(s)", "tertiary", True),
    "22": ("reversal of previous payment", "unspecified", False),
}
UNKNOWN_STATUS = ("uncategorized", "unknown", False)


def text(value):
    """Text cell as ``str``; null stays ``None``."""
    return None if value is None else value.decode("utf-8")


def money(value):
    """Decimal cell as ``float``, as the library's ``float(text)``."""
    return None if value is None else float(value)


def moment(value):
    """Date cell as a midnight ``datetime``, as the library's date parser."""
    return None if value is None else datetime.datetime(value.year, value.month, value.day)


def library_date(raw):
    """edi-835-parser's date parser on the element text: ``YYMMDDHHMM`` and
    ``CCYYMMDD`` become a ``datetime``, anything else stays text."""
    if len(raw) == 10:
        year, month, day, hour, minute = (int(raw[i:i + 2]) for i in range(0, 10, 2))
        return datetime.datetime(2000 + year, month, day, hour, minute)
    if len(raw) == 8:
        return datetime.datetime(int(raw[:4]), int(raw[4:6]), int(raw[6:]))
    return raw


def date(value, document, index, element):
    """Date cell as ``datetime``; a null cell whose element is written is
    read from the element text as the library reads it."""
    if value is not None or index is None:
        return moment(value)
    elements = document[index].elements
    if element > len(elements):
        return None
    raw = elements[element - 1]
    return library_date(raw.decode("utf-8") if isinstance(raw, bytes) else "")


def integer(value):
    """Text cell as ``int`` when it reads as one, else the text."""
    if value is None:
        return None
    value = text(value)
    try:
        return int(value)
    except ValueError:
        return value


def has(document, index, element, component=None):
    """Whether segment ``index`` holds the element (and component), even empty."""
    if index is None:
        return False
    elements = document[index].elements
    if element > len(elements):
        return False
    if component is None:
        return True
    value = elements[element - 1]
    return component <= (len(value) if isinstance(value, list) else 1)


def written(value, document, index, element, component=None, empty="", absent=None):
    """``value``, or for a null cell ``empty`` when the element is written
    empty and ``absent`` when the segment stops before it."""
    if value is not None:
        return value
    return empty if has(document, index, element, component) else absent


def name(first, last):
    """``"first last"`` title-cased, with ``None`` spelled out as the library does."""
    return f"{first} {last}".title()
