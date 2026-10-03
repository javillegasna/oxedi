# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 1 — framing + tokenizer.** Bytes → lazy, lossless `Segment` stream, delimiters
read from the ISA, symmetric writer. No 835 knowledge yet (that is Stage 3 data).
