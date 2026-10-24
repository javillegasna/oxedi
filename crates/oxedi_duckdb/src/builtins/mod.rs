//! The built-in 835 specs `read_835` chooses from, each with the columns of
//! every table it projects.

use oxedi_core::{Builtin, ColumnType, Segment, Spec, UnknownVersion, spec_schema};

/// The columns of one projected table, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTable {
    /// The table's name.
    pub name: String,
    /// Each column's name and core type.
    pub columns: Vec<(String, ColumnType)>,
    /// Per column, in the same order: the table a reference column points
    /// at, `None` for every other column.
    pub parents: Vec<Option<String>>,
}

/// One built-in spec of the core and the schema of its tables.
#[derive(Debug)]
pub struct BuiltinSchema {
    /// The core's built-in.
    builtin: &'static Builtin,
    /// Every table the spec projects, in the spec's order.
    pub tables: Vec<BuiltinTable>,
}

impl BuiltinSchema {
    fn new(builtin: &'static Builtin) -> BuiltinSchema {
        BuiltinSchema {
            builtin,
            tables: schema_of(&builtin.spec),
        }
    }

    /// The version the spec covers, as `version :=` names it.
    pub fn version(&self) -> &'static str {
        self.builtin.version
    }

    /// The spec.
    pub fn spec(&self) -> &Spec {
        &self.builtin.spec
    }

    /// The schema of the table `name`, if the spec projects it.
    pub fn table(&self, name: &str) -> Option<&BuiltinTable> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// The names of every table the spec projects.
    pub fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|table| table.name.clone()).collect()
    }
}

/// The tables a spec projects, as the core describes them.
fn schema_of(spec: &Spec) -> Vec<BuiltinTable> {
    spec_schema(spec)
        .into_iter()
        .map(|table| {
            let (columns, parents) = table
                .columns
                .into_iter()
                .map(|column| ((column.name, column.kind), column.parent))
                .unzip();
            BuiltinTable {
                name: table.name,
                columns,
                parents,
            }
        })
        .collect()
}

/// The schemas of the core's built-in specs, in the core's order: the first
/// is the default.
#[derive(Debug)]
pub struct Builtins {
    schemas: Vec<BuiltinSchema>,
}

impl Builtins {
    /// Describes every built-in the core ships.
    pub fn load() -> Builtins {
        Builtins {
            schemas: Builtin::all().iter().map(BuiltinSchema::new).collect(),
        }
    }

    /// Every built-in, the default first.
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = &BuiltinSchema> {
        self.schemas.iter()
    }

    /// The schema of the core's built-in `builtin`; the default's when the
    /// core's registry has no such entry.
    fn of(&self, builtin: &Builtin) -> &BuiltinSchema {
        self.schemas
            .iter()
            .find(|schema| {
                schema.builtin.transaction == builtin.transaction
                    && schema.builtin.version == builtin.version
            })
            .unwrap_or_else(|| self.default())
    }

    /// The spec used when a file declares no known version.
    pub fn default(&self) -> &BuiltinSchema {
        // `load` maps the registry in order, whose first entry is the fallback.
        &self.schemas[0]
    }

    /// The built-in of `version`.
    pub fn by_version(&self, version: &str) -> Result<&BuiltinSchema, UnknownVersion> {
        Builtin::find(Builtin::fallback().transaction, version).map(|builtin| self.of(builtin))
    }

    /// The built-in of `version`, the default for `None`.
    pub fn pick(&self, version: Option<&str>) -> Result<&BuiltinSchema, UnknownVersion> {
        version.map_or_else(|| Ok(self.default()), |version| self.by_version(version))
    }

    /// The built-in of the version the segments declare, else the default.
    pub fn select<'s>(&self, segments: impl IntoIterator<Item = Segment<'s>>) -> &BuiltinSchema {
        self.of(Builtin::select(segments))
    }
}

#[cfg(test)]
mod tests;
