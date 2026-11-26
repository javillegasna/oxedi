//! Which module of the core may use which. A module may use only the modules in its row, and
//! the rows may not form a cycle. A use the design needs and the rows do not allow goes in
//! `EXCEPTIONS`, with the reason. `SHORTCUTS` names the items a module writes on a type, so a
//! call to one counts as a use of that module.

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
    row(
        "conformance",
        &[
            "column",
            "delimiters",
            "diagnostic",
            "document",
            "process",
            "spec",
            "spec_load",
            "validate",
            "vocabulary",
        ],
    ),
    row("delimiters", &["frame", "message", "trivia"]),
    row("diagnostic", &["document", "message", "value"]),
    row("document", &["delimiters", "element", "frame", "segment"]),
    row("element", &["frame"]),
    row("engine", &["segment", "spec"]),
    row("frame", &["trivia"]),
    row("mask", &["vocabulary"]),
    row("message", &[]),
    row(
        "problem",
        &[
            "delimiters",
            "diagnostic",
            "message",
            "spec_load",
            "value",
            "vocabulary",
            "write",
        ],
    ),
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
    row(
        "spec_load",
        &[
            "delimiters",
            "message",
            "segment",
            "spec",
            "value",
            "vocabulary",
        ],
    ),
    row("summary", &["column", "spec", "value"]),
    row("tokenizer", &["delimiters", "frame", "segment"]),
    row("tree", &["engine", "segment", "spec"]),
    row("trivia", &[]),
    row("validate", &["message", "spec_load", "write"]),
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

/// An item on a type, by the module whose files write it.
pub struct Shortcut {
    /// The call as written: the type, `::` and the item.
    pub call: &'static str,
    /// The module whose files write the item.
    pub module: &'static str,
    /// The module whose files define the type: there `Self::` can be the type, so a call
    /// through `Self::` counts too.
    pub type_module: &'static str,
}

pub const fn shortcut(
    call: &'static str,
    module: &'static str,
    type_module: &'static str,
) -> Shortcut {
    Shortcut {
        call,
        module,
        type_module,
    }
}

impl Shortcut {
    /// The ways a file of `from` writes a call to the item: the type and the item, and in the
    /// type's own module `Self::` and the item too.
    pub fn written_in(&self, from: &str) -> Vec<String> {
        let mut forms = vec![self.call.to_owned()];
        let own_type = from == self.type_module;
        if let Some((_, item)) = self.call.split_once("::").filter(|_| own_type) {
            forms.push(format!("Self::{item}"));
        }
        forms
    }
}

/// Items on a type, by the module whose files write them: a call to one is a use of that
/// module, though the call's path names only the type. In the files of the type's module, a
/// call through `Self::` counts too.
pub const SHORTCUTS: &[Shortcut] = &[
    shortcut("Spec::from_json", "spec_load", "spec"),
    shortcut("Spec::builtin_835", "spec_load", "spec"),
    shortcut("Spec::builtin_835_4010", "spec_load", "spec"),
];
