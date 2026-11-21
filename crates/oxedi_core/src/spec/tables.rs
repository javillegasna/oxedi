//! Table definitions: repeats, picks and column sources.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, Serializer};

#[cfg(doc)]
use super::Spec;
use super::UnknownColumn;
use super::loops::LoopId;
use crate::vocabulary::Phi;

/// How a table's segment repeats a group of elements: one row per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// 1-based position of the first group's first element.
    pub from: usize,
    /// Elements per group.
    pub step: usize,
}

/// Which of the segments that match a column's source gives its value.
///
/// The segments considered are, in file order: for a source in the anchor
/// loop or a loop inside it, those captured while the row's anchor instance
/// is open (in that instance, or in any instance of the inner loop inside
/// it); for a source in a loop above the anchor, those the enclosing
/// instance of that loop captured before the row's anchor instance opened
/// (an instance captures nothing while a loop inside it is open, so these
/// are all it captures until the row is appended). A table anchored on a
/// segment reads that segment only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pick {
    /// The first segment that matches; written `"first"`, and the default.
    #[default]
    First,
    /// The last segment that matches; written `"last"`.
    Last,
    /// The n-th segment that matches, counting from 1; written as the
    /// number. The value is null when fewer segments match.
    Nth(usize),
}

/// How a spec writes a pick: a word or a position.
enum Written {
    Word(&'static str),
    Position(usize),
}

impl Pick {
    /// The named picks, in the order the refusal lists their words.
    pub(crate) const NAMED: [Pick; 2] = [Pick::First, Pick::Last];
    /// What a spec may write for a pick; the shape check and the refusal both
    /// say it.
    pub(crate) const EXPECTED: &'static str = "\"first\", \"last\" or a 1-based position";

    /// The one place a pick's word or position is written.
    fn written(self) -> Written {
        match self {
            Pick::First => Written::Word("first"),
            Pick::Last => Written::Word("last"),
            Pick::Nth(nth) => Written::Position(nth),
        }
    }

    /// The named pick a spec's word names, or `None`.
    pub(crate) fn from_word(word: &str) -> Option<Pick> {
        Pick::NAMED
            .into_iter()
            .find(|pick| matches!(pick.written(), Written::Word(named) if named == word))
    }
}

impl fmt::Display for Pick {
    /// The pick as a spec writes it: `"first"`, `"last"` or the position.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.written() {
            Written::Word(word) => write!(f, "\"{word}\""),
            Written::Position(nth) => write!(f, "{nth}"),
        }
    }
}

impl Serialize for Pick {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.written() {
            Written::Word(word) => serializer.serialize_str(word),
            Written::Position(nth) => {
                serializer.serialize_u64(u64::try_from(nth).map_err(|_| {
                    ser::Error::custom(format!("pick position {nth} does not fit in 64 bits"))
                })?)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Pick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PickVisitor)
    }
}

/// Reads a word or a whole number from 1 into a `Pick`; any other JSON value
/// is refused with the same text.
struct PickVisitor;

impl PickVisitor {
    fn refuse<E: de::Error>(written: impl fmt::Display) -> E {
        E::custom(format!(
            "\"pick\" must be {}; found {written}",
            Pick::EXPECTED
        ))
    }
}

impl<'de> Visitor<'de> for PickVisitor {
    type Value = Pick;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Pick::EXPECTED)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Pick, E> {
        usize::try_from(v)
            .ok()
            .filter(|nth| *nth > 0)
            .map(Pick::Nth)
            .ok_or_else(|| Self::refuse(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Pick, E> {
        match u64::try_from(v) {
            Ok(positive) => self.visit_u64(positive),
            Err(_) => Err(Self::refuse(v)),
        }
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Pick, E> {
        Err(Self::refuse(v))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Pick, E> {
        Err(Self::refuse(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Pick, E> {
        Err(Self::refuse("null"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Pick, E> {
        Pick::from_word(v).ok_or_else(|| Self::refuse(serde_json::Value::from(v)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<Pick, A::Error> {
        Err(Self::refuse("an array"))
    }

    fn visit_map<A: MapAccess<'de>>(self, _: A) -> Result<Pick, A::Error> {
        Err(Self::refuse("an object"))
    }
}

/// Where a column takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnSource {
    /// An element (or one of its components) of the segment that `pick`
    /// chooses among those that match `segment` and `conditions` (or the
    /// named `occurrence`), read in the anchor loop instance, in the loops
    /// `loop_id` names inside it, or in the instance of `loop_id` above it;
    /// see [`Pick`]. In a table anchored on a segment, the anchor segment
    /// itself.
    Element {
        /// The loop to read from: one inside the anchor loop, or one above
        /// it; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id; for an occurrence, the occurrence's segment.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position;
        /// empty for an occurrence.
        conditions: Vec<(usize, Vec<u8>)>,
        /// The occurrence of the reading loop the segment must match, by
        /// name, instead of `segment` and `conditions`.
        occurrence: Option<String>,
        /// Which matching segment gives the value.
        pick: Pick,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
    /// The index of the segment chosen as for `Element`.
    SegmentIndex {
        /// The loop to read from: one inside the anchor loop, or one above
        /// it; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id; for an occurrence, the occurrence's segment.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position;
        /// empty for an occurrence.
        conditions: Vec<(usize, Vec<u8>)>,
        /// The occurrence of the reading loop the segment must match, by
        /// name, instead of `segment` and `conditions`.
        occurrence: Option<String>,
        /// Which matching segment gives the index.
        pick: Pick,
    },
    /// An element of the row's group, in a table whose segment repeats a
    /// group: position `from + k * step + offset` for group `k`.
    GroupElement {
        /// 0-based position inside the group.
        offset: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
}

/// One table of the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    /// Name used in the JSON, e.g. `claims`.
    pub name: String,
    /// Name of the column that refers to a row of this table from the
    /// tables below it, e.g. `claim`; the table's name unless the spec says.
    pub reference: String,
    /// The loops whose instances (or whose segments) give rows.
    pub loops: Vec<LoopId>,
    /// With a segment, one row per occurrence of it in an anchor loop
    /// instead of one row per loop instance.
    pub segment: Option<Vec<u8>>,
    /// With a repeat, one row per element group of the segment.
    pub repeat: Option<Repeat>,
    /// The declared columns by name, in name order.
    pub columns: Vec<(String, ColumnSource)>,
    /// The nearest table above this one, as an index into [`Spec::tables`].
    pub parent: Option<usize>,
    /// Every table above this one, outermost first; the last is `parent`.
    /// Each one gets an automatic column named after its `reference`.
    pub ancestors: Vec<usize>,
    /// The kind of patient data each column of `columns` holds, parallel to
    /// it; read through [`TableDef::phi`].
    pub(super) phi: Vec<Option<Phi>>,
    /// The index in `columns` of the column whose value seeds the row's date
    /// shift; read through [`TableDef::phi_seed`].
    pub(super) phi_seed: Option<usize>,
}

impl TableDef {
    /// The source of the declared column `name`; `None` for a name the table
    /// does not declare, including the automatic `row`, `segment` and
    /// reference columns.
    pub fn column(&self, name: &str) -> Option<&ColumnSource> {
        self.columns
            .iter()
            .find_map(|(column, source)| (column == name).then_some(source))
    }

    /// The source of the declared column `name`; a name the table does not
    /// declare is an [`UnknownColumn`] that lists the declared columns, in the
    /// order [`Self::columns`] holds them.
    pub fn require_column(&self, name: &str) -> Result<&ColumnSource, UnknownColumn> {
        self.column(name).ok_or_else(|| UnknownColumn {
            table: self.name.clone(),
            column: name.to_owned(),
            columns: self
                .columns
                .iter()
                .map(|(column, _)| column.clone())
                .collect(),
        })
    }

    /// The kind of patient data column `column_index` of [`Self::columns`]
    /// holds, or `None` when it holds none.
    pub fn phi(&self, column_index: usize) -> Option<Phi> {
        self.phi.get(column_index).copied().flatten()
    }

    /// The index in [`Self::columns`] of the column whose value seeds the
    /// date shift of the table's rows.
    pub fn phi_seed(&self) -> Option<usize> {
        self.phi_seed
    }
}
