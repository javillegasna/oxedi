# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 4a — element definitions and structural diagnostics.** The JSON spec now names
and types every element of the 835's segments and rejects malformed or ambiguous specs
in plain words. Alongside the loop engine, an envelope checker reports unknown segments,
implicit or unterminated loops and envelope counts or control numbers that do not match,
as self-explanatory diagnostics. Typed columnar projection comes next.

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
