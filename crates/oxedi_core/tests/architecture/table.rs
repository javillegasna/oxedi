//! Which module of the core may use which. A module may use only the modules in its row, and
//! the rows may not form a cycle. A use the design needs and the rows do not allow goes in
//! `EXCEPTIONS`, with the reason.

/// A module and the modules its files may use.
pub struct Row {
    pub module: &'static str,
    pub uses: &'static [&'static str],
}

/// A dependency the rows do not allow and the design needs.
pub struct Exception {
    pub from: &'static str,
    pub to: &'static str,
    pub reason: &'static str,
}

pub const fn row(module: &'static str, uses: &'static [&'static str]) -> Row {
    Row { module, uses }
}

pub const TABLE: &[Row] = &[
    row(
        "check",
        &[
            "delimiters",
            "diagnostic",
            "engine",
            "segment",
            "spec",
            "trivia",
            "value",
        ],
    ),
    row("column", &["message", "value"]),
    row("delimiters", &["frame", "message", "trivia"]),
    row("diagnostic", &["document", "message", "value"]),
    row("document", &["delimiters", "element", "frame", "segment"]),
    row("element", &["frame"]),
    row("engine", &["segment", "spec"]),
    row("frame", &["trivia"]),
    row("mask", &["vocabulary"]),
    row("message", &[]),
    row(
        "process",
        &[
            "check",
            "column",
            "delimiters",
            "diagnostic",
            "document",
            "engine",
            "mask",
            "project",
            "segment",
            "spec",
        ],
    ),
    row(
        "project",
        &[
            "column",
            "delimiters",
            "diagnostic",
            "document",
            "element",
            "engine",
            "mask",
            "segment",
            "spec",
            "value",
            "vocabulary",
        ],
    ),
    row("segment", &["delimiters", "element", "frame"]),
    row(
        "spec",
        &["element", "message", "segment", "value", "vocabulary"],
    ),
    row("summary", &["column", "spec", "value"]),
    row("tokenizer", &["delimiters", "frame", "segment"]),
    row("tree", &["engine", "segment", "spec"]),
    row("trivia", &[]),
    row("value", &[]),
    row("vocabulary", &[]),
    row(
        "write",
        &[
            "column",
            "delimiters",
            "diagnostic",
            "document",
            "message",
            "process",
            "segment",
            "spec",
            "value",
            "vocabulary",
        ],
    ),
];

pub const EXCEPTIONS: &[Exception] = &[];
