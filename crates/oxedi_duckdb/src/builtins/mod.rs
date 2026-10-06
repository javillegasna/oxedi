//! The built-in 835 specs `read_835` chooses from, each with the columns of
//! every table it projects.

use edi835_core::{ColumnType, Delimiters, Processor, Segment, Spec};

/// The columns of one projected table, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    /// The table's name.
    pub name: String,
    /// Each column's name and core type.
    pub columns: Vec<(String, ColumnType)>,
}

/// One built-in spec and the schema of its tables.
#[derive(Debug)]
pub struct Builtin {
    /// The version the spec covers, as `version :=` names it.
    pub version: &'static str,
    /// The spec.
    pub spec: Spec,
    /// Every table the spec projects, in the spec's order.
    pub tables: Vec<TableSchema>,
}

impl Builtin {
    fn new(version: &'static str, spec: Spec) -> Builtin {
        let tables = schema_of(&spec);
        Builtin {
            version,
            spec,
            tables,
        }
    }

    /// The schema of the table `name`, if the spec projects it.
    pub fn table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// The names of every table the spec projects.
    pub fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|table| table.name.clone()).collect()
    }
}

/// The tables a spec projects, read from a processor that saw no segment.
fn schema_of(spec: &Spec) -> Vec<TableSchema> {
    let delimiters = Delimiters::new(b'*', b':', b'~');
    Processor::new(spec, &delimiters)
        .take_tables()
        .iter()
        .map(|table| TableSchema {
            name: table.name().to_owned(),
            columns: table
                .columns()
                .iter()
                .map(|(name, data)| (name.clone(), data.kind()))
                .collect(),
        })
        .collect()
}

/// Every built-in spec; the first is the default.
#[derive(Debug)]
pub struct Builtins {
    default: Builtin,
    others: Vec<Builtin>,
}

impl Builtins {
    /// Loads the built-in 5010 (the default) and 4010 specs.
    pub fn load() -> Builtins {
        Builtins {
            default: Builtin::new("5010", Spec::builtin_835()),
            others: vec![Builtin::new("4010", Spec::builtin_835_4010())],
        }
    }

    /// The spec used when a file declares no known version.
    pub fn default(&self) -> &Builtin {
        &self.default
    }

    /// Every built-in, the default first.
    pub fn iter(&self) -> impl Iterator<Item = &Builtin> {
        std::iter::once(&self.default).chain(self.others.iter())
    }

    /// The versions `version :=` accepts.
    pub fn versions(&self) -> Vec<&'static str> {
        self.iter().map(|builtin| builtin.version).collect()
    }

    /// The built-in of `version`.
    pub fn by_version(&self, version: &str) -> Option<&Builtin> {
        self.iter().find(|builtin| builtin.version == version)
    }

    /// The built-in of the version the segments declare, else the default.
    pub fn select<'s>(&self, segments: impl IntoIterator<Item = Segment<'s>>) -> &Builtin {
        let specs: Vec<&Spec> = self.iter().map(|builtin| &builtin.spec).collect();
        let chosen = Spec::select(&specs, &self.default.spec, segments);
        self.iter()
            .find(|builtin| std::ptr::eq(&builtin.spec, chosen))
            .unwrap_or(&self.default)
    }
}

#[cfg(test)]
mod tests;
