# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 2 — lossless document.** Bytes → lazy `Segment` stream (Stage 1) or a `Document`
that holds the whole file, borrowed or owned, and yields the same segments on demand.
No 835 knowledge yet (that is Stage 3 data).
