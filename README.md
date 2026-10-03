# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 4 — projection and validation.** The JSON spec names and types every element of
the 835's segments and declares the tables to project. One pass over a file feeds the
loop engine, an envelope checker and a projector: the checker reports unknown segments,
implicit or unterminated loops and envelope counts or control numbers that do not
match; the projector checks every element (required, type, length, components) and
fills typed, Arrow-layout tables of payments, claims, services, adjustments and provider
adjustments, each row pointing at the rows that enclose it. Every finding is a
self-explanatory diagnostic. The Python binding comes next.

## Extending the 835 spec

Patches use JSON Merge Patch (RFC 7386). A patch replaces arrays wholesale: to add a
segment to a loop, list the loop's full `segments`; objects merge key by key, so
changing a `trigger` or `end` does not touch `segments`.

```json
{
  "loops": {
    "1000A": { "segments": ["N3", "N4", "REF", "PER", "XX"] }
  }
}
```
