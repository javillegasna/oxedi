//! `edi835_core` — lossless, fast, data-driven EDI 835 parser core.
//!
//! Stage 0: scaffolding only. No parsing logic yet — see
//! `.doc/architectural-commitment.md` for the roadmap.

#[cfg(test)]
mod tests {
    /// Smoke test: proves the unit-test harness compiles and runs.
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
