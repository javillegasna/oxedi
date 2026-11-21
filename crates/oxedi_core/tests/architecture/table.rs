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
