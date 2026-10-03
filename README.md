# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 3 — declarative loop engine.** A JSON spec describes the loop structure; the
engine turns the segment stream into loop events and a tree, and users extend the
built-in 835 spec with JSON merge patches. Element names and validation come next.
