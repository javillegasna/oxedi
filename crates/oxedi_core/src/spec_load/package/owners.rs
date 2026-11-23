//! Which layers may write which keys of a spec.

use serde_json::{Map, Value};

use super::error::StackError;
use super::header::Package;
use super::layer::{Layer, Layers};

/// How a key's name is matched.
#[derive(Clone, Copy)]
enum Name {
    /// The key with this name.
    Is(&'static str),
    /// Any key: the names of loops and occurrences are the spec's own.
    Any,
}

/// Who may write a key.
#[derive(Clone, Copy)]
enum Owner {
    /// No layer: the name of a stack comes from its top package's id.
    Identity,
    /// The whole value belongs to these layers.
    Layers(&'static [Layer]),
    /// The value is an object whose keys have their own owners.
    Keys(&'static [Key]),
}

/// One key of a spec object and its owner.
#[derive(Clone, Copy)]
struct Key {
    name: Name,
    owner: Owner,
}

impl Key {
    const fn layers(name: &'static str, owners: &'static [Layer]) -> Key {
        Key {
            name: Name::Is(name),
            owner: Owner::Layers(owners),
        }
    }

    fn matches(&self, key: &str) -> bool {
        match self.name {
            Name::Is(name) => name == key,
            Name::Any => true,
        }
    }
}

// The envelope is the complete base of every stack, so it owns every key the
// dictionary, the transaction set and the guide own.
const TRANSACTION_SET: &[Layer] = &[Layer::Envelope, Layer::TransactionSet];
const GUIDE: &[Layer] = &[Layer::Envelope, Layer::Guide];
const DICTIONARY: &[Layer] = &[Layer::Envelope, Layer::Dictionary];
const VIEWS: &[Layer] = &[Layer::Views];

const SPEC_KEYS: &[Key] = &[
    Key {
        name: Name::Is("name"),
        owner: Owner::Identity,
    },
    Key::layers("version", GUIDE),
    Key::layers("balancing", GUIDE),
    Key {
        name: Name::Is("loops"),
        owner: Owner::Keys(LOOPS),
    },
    Key::layers("segments", DICTIONARY),
    Key::layers("tables", VIEWS),
    Key::layers("summaries", VIEWS),
    Key::layers("default_table", VIEWS),
];

const LOOPS: &[Key] = &[Key {
    name: Name::Any,
    owner: Owner::Keys(LOOP_KEYS),
}];

const LOOP_KEYS: &[Key] = &[
    Key::layers("parent", TRANSACTION_SET),
    Key::layers("trigger", TRANSACTION_SET),
    Key::layers("end", TRANSACTION_SET),
    Key::layers("control", TRANSACTION_SET),
    Key::layers("usage", GUIDE),
    Key::layers("max", GUIDE),
    Key {
        name: Name::Is("occurrences"),
        owner: Owner::Keys(OCCURRENCES),
    },
];

const OCCURRENCES: &[Key] = &[Key {
    name: Name::Any,
    owner: Owner::Keys(OCCURRENCE_KEYS),
}];

const OCCURRENCE_KEYS: &[Key] = &[
    Key::layers("segment", TRANSACTION_SET),
    Key::layers("pos", TRANSACTION_SET),
    Key::layers("usage", GUIDE),
    Key::layers("max", GUIDE),
    Key::layers("qualifier", GUIDE),
    Key::layers("codes", GUIDE),
];

/// The first key `package` writes that none of its layers owns. A key the
/// table does not know is left to the strict read of the model.
pub(super) fn check_owners(package: &Package) -> Result<(), Box<StackError>> {
    let layers = package.header().layers;
    let Value::Object(body) = package.body() else {
        return Ok(());
    };
    walk(body, SPEC_KEYS, "", layers).map_err(|fault| {
        Box::new(match fault {
            Fault::Name => StackError::NameFromIdentity {
                package: package.id().clone(),
            },
            Fault::Key { path, owners } => StackError::KeyNotOwned {
                package: package.id().clone(),
                path,
                owners: owners.to_vec(),
                layers,
            },
        })
    })
}

/// Why a key is refused.
enum Fault {
    Name,
    Key {
        path: String,
        owners: &'static [Layer],
    },
}

// Checks each key of `object` against `table`; `path` is the object's own.
fn walk(
    object: &Map<String, Value>,
    table: &'static [Key],
    path: &str,
    layers: Layers,
) -> Result<(), Fault> {
    for (name, value) in object {
        if let Some(key) = table.iter().find(|key| key.matches(name)) {
            check_member(key.owner, &join(path, name), value, layers)?;
        }
    }
    Ok(())
}

fn join(path: &str, name: &str) -> String {
    match path {
        "" => name.to_owned(),
        path => format!("{path}.{name}"),
    }
}

// Checks one member: a section is walked, anything else is owned whole.
fn check_member(owner: Owner, at: &str, value: &Value, layers: Layers) -> Result<(), Fault> {
    match (owner, value) {
        (Owner::Keys(inner), Value::Object(members)) => walk(members, inner, at, layers),
        // A section that is not an object is left to the strict read.
        (Owner::Keys(_), other) if !other.is_null() => Ok(()),
        // A deletion removes every key beneath it.
        (owner, _) => require_all(owner, at, layers),
    }
}

// Whether `layers` own `owner` whole: every leaf beneath it, for a section.
fn require_all(owner: Owner, at: &str, layers: Layers) -> Result<(), Fault> {
    match owner {
        Owner::Identity => Err(Fault::Name),
        Owner::Layers(owners) if holds_any(layers, owners) => Ok(()),
        Owner::Layers(owners) => Err(Fault::Key {
            path: at.to_owned(),
            owners,
        }),
        Owner::Keys(table) => table
            .iter()
            .try_for_each(|key| require_all(key.owner, at, layers)),
    }
}

// A patch owns every key but the name.
fn holds_any(layers: Layers, owners: &[Layer]) -> bool {
    layers.contains(Layer::Patch) || owners.iter().any(|&owner| layers.contains(owner))
}
