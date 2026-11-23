//! Which layers may write which keys of a spec.

use serde_json::{Map, Value};

use super::error::StackError;
use super::header::Package;
use super::layer::{Layer, Layers};
use crate::message::render_key;

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
const NARROWABLE: &[Layer] = &[Layer::Envelope, Layer::Dictionary, Layer::Guide];
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
    Key {
        name: Name::Is("segments"),
        owner: Owner::Keys(SEGMENTS),
    },
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

const SEGMENTS: &[Key] = &[Key {
    name: Name::Any,
    owner: Owner::Keys(SEGMENT_KEYS),
}];

const SEGMENT_KEYS: &[Key] = &[Key {
    name: Name::Is("elements"),
    owner: Owner::Keys(ELEMENTS),
}];

const ELEMENTS: &[Key] = &[Key {
    name: Name::Any,
    owner: Owner::Keys(ELEMENT_KEYS),
}];

// The dictionary defines what an element is; a guide may narrow what it
// allows, so the leaves that narrow have both owners. A component follows the
// same split and holds no composite of its own, so only an element adds the
// `composite` row.
macro_rules! element_keys {
    ($($extra:expr),*) => {
        &[
            Key::layers("name", DICTIONARY),
            Key::layers("type", DICTIONARY),
            Key::layers("scale", DICTIONARY),
            Key::layers("codes", NARROWABLE),
            Key::layers("min", NARROWABLE),
            Key::layers("max", NARROWABLE),
            Key::layers("required", NARROWABLE),
            $($extra),*
        ]
    };
}

const ELEMENT_KEYS: &[Key] = element_keys!(Key {
    name: Name::Is("composite"),
    owner: Owner::Keys(COMPONENTS),
});

const COMPONENTS: &[Key] = &[Key {
    name: Name::Any,
    owner: Owner::Keys(COMPONENT_KEYS),
}];

const COMPONENT_KEYS: &[Key] = element_keys!();

/// The first key `package` writes that none of its layers owns. A key the
/// table does not know is left to the strict read of the model.
pub(super) fn check_owners(package: &Package) -> Result<(), Box<StackError>> {
    let layers = package.header().layers;
    let Value::Object(body) = package.body() else {
        return Ok(());
    };
    walk(body, SPEC_KEYS, &mut Vec::new(), layers).map_err(|fault| {
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

/// The path as the error shows it: each name written by `render_key`, a dot
/// between them, so a name holding a separator cannot make it ambiguous.
fn render(path: &[&str]) -> String {
    let names: Vec<_> = path.iter().map(|name| render_key(name)).collect();
    names.join(".")
}

// Checks each key of `object` against `table`; `path` holds the names from the
// spec's root down to `object`, and only an error renders it.
fn walk<'v>(
    object: &'v Map<String, Value>,
    table: &'static [Key],
    path: &mut Vec<&'v str>,
    layers: Layers,
) -> Result<(), Fault> {
    for (name, value) in object {
        if let Some(key) = table.iter().find(|key| key.matches(name)) {
            path.push(name);
            check_member(key.owner, path, value, layers)?;
            path.pop();
        }
    }
    Ok(())
}

// Checks one member: a section is walked, anything else is owned whole.
fn check_member<'v>(
    owner: Owner,
    path: &mut Vec<&'v str>,
    value: &'v Value,
    layers: Layers,
) -> Result<(), Fault> {
    match (owner, value) {
        // An empty object adds the section itself, which takes every owner of
        // the keys beneath it, as a deletion does.
        (Owner::Keys(_), Value::Object(members)) if members.is_empty() => {
            require_all(owner, path, layers)
        }
        (Owner::Keys(inner), Value::Object(members)) => walk(members, inner, path, layers),
        // A section that is not an object is left to the strict read.
        (Owner::Keys(_), other) if !other.is_null() => Ok(()),
        // A deletion removes every key beneath it.
        (owner, _) => require_all(owner, path, layers),
    }
}

// Whether `layers` own `owner` whole: every leaf beneath it, for a section.
fn require_all(owner: Owner, path: &[&str], layers: Layers) -> Result<(), Fault> {
    match owner {
        Owner::Identity => Err(Fault::Name),
        Owner::Layers(owners) if holds_any(layers, owners) => Ok(()),
        Owner::Layers(owners) => Err(Fault::Key {
            path: render(path),
            owners,
        }),
        Owner::Keys(table) => table
            .iter()
            .try_for_each(|key| require_all(key.owner, path, layers)),
    }
}

// A patch owns every key but the name.
fn holds_any(layers: Layers, owners: &[Layer]) -> bool {
    layers.contains(Layer::Patch) || owners.iter().any(|&owner| layers.contains(owner))
}
