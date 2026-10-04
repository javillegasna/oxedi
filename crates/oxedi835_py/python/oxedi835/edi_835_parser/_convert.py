"""Value conversions that reproduce edi-835-parser's element parsers."""

from __future__ import annotations

import datetime
import locale

# The library reads each file with ``open()`` in text mode, which decodes with
# the locale's preferred encoding; element text is decoded with the same one.
ENCODING = locale.getpreferredencoding(False)


def readable(data):
    """Decodes ``data`` as the library's ``open()`` does, so a byte that the
    encoding cannot read fails here, before parsing, as it does in the library."""
    return str(data, ENCODING)


def text(value):
    """Text cell as ``str``; null stays ``None``."""
    return None if value is None else value.decode(ENCODING)


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
    """Date cell as ``datetime``. A null cell whose element is written holds
    text the date type rejects (an empty date, say), which the library keeps
    as text, so it is read from the element with the library's parser."""
    if value is not None or index is None:
        return moment(value)
    elements = document[index].elements
    if element > len(elements):
        return None
    raw = elements[element - 1]
    return library_date(raw.decode(ENCODING) if isinstance(raw, bytes) else "")


def integer(value):
    """Text cell as ``int`` when it reads as one, else the text, as the
    library's ``int()`` with its fallback; a decimal column cannot give this
    back, since it reads ``"1.0"`` and ``"1"`` as the same number."""
    if value is None:
        return None
    value = text(value)
    try:
        return int(value)
    except ValueError:
        return value


def has(document, index, element, component=None):
    """Whether segment ``index`` holds the element (and component), even
    empty: a null cell cannot tell an element written empty from one the
    segment stops before, and the library gives the two different values."""
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
    empty and ``absent`` when the segment stops before it (see ``has``)."""
    if value is not None:
        return value
    return empty if has(document, index, element, component) else absent


def name(first, last):
    """``"first last"`` title-cased, with ``None`` spelled out as the library does."""
    return f"{first} {last}".title()
