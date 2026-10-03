//! Loop specifications: the data that tells the engine how a file is structured.
//!
//! A spec is plain JSON: a map of loops, each naming its parent, the segment
//! (and optional element conditions) that opens it, the segments it may hold
//! and an optional segment that closes it. An optional `segments` section
//! names and types the elements of each segment id, wherever the segment
//! appears. Loading compiles that into index-based definitions so the engine
//! never compares strings, and keeps the JSON value so patches can be applied
//! on top. Patches follow RFC 7386: objects merge key by key, while arrays and
//! scalars replace wholesale; see [`Spec::merge_patch`] for what that means
//! when extending a loop's segment list.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::element::Element;
use crate::segment::Segment;

/// Index of a loop definition inside a [`Spec`]. A `LoopId` is only
/// meaningful for the `Spec` that produced it: using it with another spec,
/// including one produced by [`Spec::merge_patch`], indexes a different loop
/// or panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopId(usize);

impl LoopId {
    /// Position of the loop in [`Spec::loops`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// What opens a loop: a segment id plus optional element conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Segment id, e.g. `CLP`.
    pub segment: Vec<u8>,
    /// `(1-based element position, required value)`, sorted by position.
    pub conditions: Vec<(usize, Vec<u8>)>,
}

impl Trigger {
    /// `true` when the segment has this id and every condition holds.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment.id == self.segment.as_slice()
            && self.conditions.iter().all(|(position, value)| {
                segment.element(*position).and_then(Element::simple) == Some(value.as_slice())
            })
    }
}

/// One loop of the structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopDef {
    /// Name used in the JSON, e.g. `2100`.
    pub name: String,
    /// `None` for a top-level loop.
    pub parent: Option<LoopId>,
    /// What opens the loop. The trigger segment is captured by the loop.
    pub trigger: Trigger,
    /// Segments the loop holds after its trigger.
    pub segments: Vec<Vec<u8>>,
    /// Segment that is captured and then closes the loop, e.g. `SE`.
    pub end: Option<Vec<u8>>,
    /// Loops whose parent is this one, in spec order.
    pub children: Vec<LoopId>,
}

impl LoopDef {
    /// `true` when the loop holds `id` (as a listed segment or as its end).
    pub fn accepts(&self, id: &[u8]) -> bool {
        self.segments.iter().any(|segment| segment == id) || self.end.as_deref() == Some(id)
    }
}

/// The data type of an element, as the X12 standard names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// `AN`: a string.
    An,
    /// `ID`: a code from a list.
    Id,
    /// `N0` to `N9`: an integer with that many implied decimal places.
    N(u8),
    /// `R`: a decimal number with an explicit point, kept at `scale` places.
    R {
        /// Decimal places; 2 unless the spec says otherwise.
        scale: u8,
    },
    /// `DT`: a date, `CCYYMMDD` or `YYMMDD`.
    Dt,
    /// `TM`: a time, `HHMM` optionally followed by seconds and decimal seconds.
    Tm,
}

impl ElementType {
    /// Reads the type code of an element definition; `scale` is the
    /// definition's `scale` key, which only `R` accepts.
    fn parse(code: &str, scale: Option<u8>) -> Result<ElementType, ElementDefError> {
        let kind = match code.as_bytes() {
            b"AN" => ElementType::An,
            b"ID" => ElementType::Id,
            b"R" => ElementType::R {
                scale: scale.unwrap_or(2),
            },
            b"DT" => ElementType::Dt,
            b"TM" => ElementType::Tm,
            [b'N', digit @ b'0'..=b'9'] => ElementType::N(digit - b'0'),
            _ => {
                return Err(ElementDefError::UnknownType {
                    found: code.to_string(),
                });
            }
        };
        if scale.is_some() && !matches!(kind, ElementType::R { .. }) {
            return Err(ElementDefError::ScaleWithoutR {
                kind: code.to_string(),
            });
        }
        Ok(kind)
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementType::An => write!(f, "AN (string)"),
            ElementType::Id => write!(f, "ID (code)"),
            ElementType::N(places) => {
                write!(f, "N{places} (integer with {places} implied decimals)")
            }
            ElementType::R { scale } => write!(f, "R (decimal, scale {scale})"),
            ElementType::Dt => write!(f, "DT (date CCYYMMDD or YYMMDD)"),
            ElementType::Tm => write!(f, "TM (time HHMM, HHMMSS or HHMMSSD..)"),
        }
    }
}

/// One element of a segment, or one component of a composite element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDef {
    /// Name used for the element downstream, e.g. `claim_submitter_id`.
    pub name: String,
    /// Data type.
    pub kind: ElementType,
    /// `true` when the element must be present and non-empty.
    pub required: bool,
    /// Minimum length, when the spec sets one.
    pub min: Option<usize>,
    /// Maximum length, when the spec sets one.
    pub max: Option<usize>,
    /// Components by 1-based position; empty for a simple element.
    pub composite: BTreeMap<usize, ElementDef>,
}

/// The elements of one segment id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// Elements by 1-based position. Positions with no entry are opaque.
    pub elements: BTreeMap<usize, ElementDef>,
}

/// Why an element definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementDefError {
    /// The position key is not a 1-based integer in canonical form.
    NonCanonicalPosition,
    /// `name` is the empty string.
    EmptyName,
    /// Another element at the same level already has this name.
    DuplicateName {
        /// The repeated name.
        name: String,
        /// Position key of the element that used it first, as written.
        first: String,
    },
    /// `type` is not one of the known codes.
    UnknownType {
        /// The code as written.
        found: String,
    },
    /// `scale` was given for a type other than `R`.
    ScaleWithoutR {
        /// The type code as written.
        kind: String,
    },
    /// `min` is greater than `max`.
    MinAboveMax {
        /// The minimum as written.
        min: usize,
        /// The maximum as written.
        max: usize,
    },
    /// `composite` was given for a type other than `AN`.
    CompositeOnNonAn {
        /// The type code as written.
        kind: String,
    },
    /// A component declares a `composite` of its own.
    NestedComposite,
}

impl fmt::Display for ElementDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementDefError::NonCanonicalPosition => write!(
                f,
                "positions are 1-based integers written in canonical form"
            ),
            ElementDefError::EmptyName => write!(f, "\"name\" is empty"),
            ElementDefError::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by position {first:?}")
            }
            ElementDefError::UnknownType { found } => write!(
                f,
                "type {found:?} is not one of AN, ID, N0 to N9, R, DT, TM"
            ),
            ElementDefError::ScaleWithoutR { kind } => {
                write!(f, "\"scale\" applies only to type R; found type {kind:?}")
            }
            ElementDefError::MinAboveMax { min, max } => {
                write!(f, "\"min\" {min} is greater than \"max\" {max}")
            }
            ElementDefError::CompositeOnNonAn { kind } => {
                write!(f, "\"composite\" requires type AN; found type {kind:?}")
            }
            ElementDefError::NestedComposite => {
                write!(f, "a component cannot declare its own \"composite\"")
            }
        }
    }
}

/// Why a spec could not be loaded. Each variant names where in the spec the fault is.
#[derive(Debug)]
pub enum SpecError {
    /// The text is not valid JSON.
    Json(serde_json::Error),
    /// The JSON is valid but does not match the spec schema.
    Schema {
        /// The loop whose definition is malformed; `None` when the top level is.
        loop_name: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// Applying a patch produced an error.
    Patch {
        /// The error the patched spec (or the patch text) produced.
        source: Box<SpecError>,
    },
    /// A loop names a parent that does not exist.
    UnknownParent {
        /// The loop with the bad reference.
        loop_name: String,
        /// The missing parent.
        parent: String,
    },
    /// Following `parent` links never reaches a top-level loop.
    Cycle {
        /// The loops on the cycle in walk order, starting at the one that repeats.
        members: Vec<String>,
    },
    /// The `loops` map is empty.
    NoLoops {
        /// The spec's `name`.
        spec_name: String,
    },
    /// A segment id is the empty string.
    EmptySegmentId {
        /// The loop holding it; `None` for a key of the `segments` section.
        loop_name: Option<String>,
        /// Where it sits, as written: `trigger.segment`, `segments[<i>]`,
        /// `end`, or `segments.""` for the section.
        key: String,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The loop with the bad key.
        loop_name: String,
        /// The key as written.
        position: String,
    },
    /// A value the schema requires to be an object is something else.
    NotAnObject {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.2100.trigger`); empty for the top level.
        path: String,
        /// What was found instead: `an array`, `a string`, `a number`,
        /// `a boolean` or `null`.
        found: &'static str,
    },
    /// A segment definition does not match the schema.
    SegmentSchema {
        /// The segment id as written.
        segment: String,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// An element definition is invalid.
    BadElementDef {
        /// The segment id as written.
        segment: String,
        /// The element's key as written; a component is `<element>.composite.<component>`.
        position: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// Two loops with the same parent have identical triggers.
    AmbiguousTrigger {
        /// First loop, in spec order.
        first: String,
        /// Second loop, in spec order.
        second: String,
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// The shared trigger segment id.
        segment: String,
        /// The shared trigger conditions, sorted by position.
        conditions: Vec<(usize, String)>,
    },
    /// Two loops with the same parent trigger on the same segment id, no
    /// position tested by both requires different values, and neither set of
    /// conditions contains the other: one segment can satisfy both and
    /// neither trigger is more specific.
    OverlappingTriggers {
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// First loop, in spec order.
        a: String,
        /// Second loop, in spec order.
        b: String,
        /// The first loop's trigger, e.g. `"N1" where {1: "PR"}`.
        conditions_a: String,
        /// The second loop's trigger, written the same way.
        conditions_b: String,
    },
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Json(e) => write!(f, "invalid spec JSON: {e}"),
            SpecError::Schema {
                loop_name: Some(loop_name),
                source,
            } => write!(f, "loop {loop_name:?} does not match the schema: {source}"),
            SpecError::Schema {
                loop_name: None,
                source,
            } => write!(f, "spec does not match the schema: {source}"),
            SpecError::NotAnObject { path, found } if path.is_empty() => {
                write!(f, "the spec must be a JSON object; found {found}")
            }
            SpecError::NotAnObject { path, found } => write!(
                f,
                "spec: the value at {path} must be a JSON object; found {found}"
            ),
            SpecError::Patch { source } => write!(f, "applying patch: {source}"),
            SpecError::UnknownParent { loop_name, parent } => {
                write!(f, "loop {loop_name:?} names unknown parent {parent:?}")
            }
            SpecError::Cycle { members } => {
                write!(f, "loops form a parent cycle: ")?;
                for member in members {
                    write!(f, "{member} -> ")?;
                }
                match members.first() {
                    Some(first) => write!(f, "{first}"),
                    None => Ok(()),
                }
            }
            SpecError::NoLoops { spec_name } => {
                write!(f, "spec {spec_name:?} declares no loops")
            }
            SpecError::EmptySegmentId {
                loop_name: Some(loop_name),
                key,
            } => write!(f, "loop {loop_name:?} has an empty segment id at {key}"),
            SpecError::EmptySegmentId {
                loop_name: None,
                key,
            } => write!(f, "the spec has an empty segment id at {key}"),
            SpecError::BadPosition {
                loop_name,
                position,
            } => write!(
                f,
                "loop {loop_name:?} has an invalid \"where\" position {position:?}: \
                 positions are 1-based integers written in canonical form"
            ),
            SpecError::SegmentSchema { segment, source } => {
                write!(f, "segment {segment:?} does not match the schema: {source}")
            }
            SpecError::BadElementDef {
                segment,
                position,
                reason,
            } => write!(f, "segment {segment:?} element {position:?}: {reason}"),
            SpecError::AmbiguousTrigger {
                first,
                second,
                parent,
                segment,
                conditions,
            } => {
                write!(f, "loops {first:?} and {second:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(f, " share the identical trigger {segment:?}")?;
                if !conditions.is_empty() {
                    write!(f, " where {{")?;
                    for (i, (position, value)) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{position}: {value:?}")?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            SpecError::OverlappingTriggers {
                parent,
                a,
                b,
                conditions_a,
                conditions_b,
            } => {
                write!(f, "loops {a:?} and {b:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(
                    f,
                    " can open on the same segment: {a:?} on {conditions_a}, {b:?} on \
                     {conditions_b}, no position they both test requires different values, \
                     and neither trigger is more specific than the other"
                )
            }
        }
    }
}

impl std::error::Error for SpecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpecError::Json(e)
            | SpecError::Schema { source: e, .. }
            | SpecError::SegmentSchema { source: e, .. } => Some(e),
            SpecError::Patch { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

// Loops are kept as raw values so each one is deserialized on its own and a
// schema error can name the loop it came from.
#[derive(Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a spec object with \"name\" and \"loops\""
)]
struct RawSpec {
    name: String,
    loops: BTreeMap<String, Value>,
    #[serde(default)]
    segments: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a loop object")]
struct RawLoop {
    parent: Option<String>,
    trigger: RawTrigger,
    #[serde(default)]
    segments: Vec<String>,
    end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a trigger object")]
struct RawTrigger {
    segment: String,
    #[serde(default, rename = "where")]
    conditions: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a segment object")]
struct RawSegment {
    #[serde(default)]
    elements: BTreeMap<String, RawElement>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "an element object")]
struct RawElement {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    required: bool,
    min: Option<usize>,
    max: Option<usize>,
    scale: Option<u8>,
    #[serde(default)]
    composite: BTreeMap<String, RawElement>,
}

/// A loaded, validated loop structure.
#[derive(Debug, Clone)]
pub struct Spec {
    name: String,
    loops: Vec<LoopDef>,
    roots: Vec<LoopId>,
    segments: BTreeMap<Vec<u8>, SegmentDef>,
    source: Value,
}

impl Spec {
    /// The built-in 835 structure as shipped, in the same JSON a user would write.
    pub const BUILTIN_835_JSON: &'static str = include_str!("../specs/835.json");

    /// The built-in 835 structure.
    pub fn builtin_835() -> Spec {
        Spec::from_json(Self::BUILTIN_835_JSON)
            .expect("the built-in 835 spec is valid; spec::tests::builtin_835_loads checks it")
    }

    /// Loads and validates a spec from JSON text.
    pub fn from_json(json: &str) -> Result<Spec, SpecError> {
        let source: Value = serde_json::from_str(json).map_err(SpecError::Json)?;
        Spec::from_value(source)
    }

    /// Applies a JSON Merge Patch (RFC 7386) to this spec's JSON and loads the
    /// result: objects merge recursively, arrays and scalars are replaced,
    /// `null` deletes. The result is validated like any spec; every failure,
    /// including unparsable patch text, is wrapped in [`SpecError::Patch`].
    ///
    /// Because `segments` is an array, a patch that touches it replaces the whole
    /// list rather than appending. To add a segment to a loop, the patch must
    /// list the loop's complete segment list, including the built-in entries that
    /// would otherwise be lost. Patches that only change `trigger`, `parent`, or
    /// `end` leave `segments` untouched, since objects merge key by key. The
    /// current segment list is visible through [`Self::to_json()`] or [`Self::get()`].
    ///
    /// # Example
    ///
    /// ```
    /// # use edi835_core::Spec;
    /// let spec = Spec::builtin_835();
    /// let patched = spec.merge_patch(
    ///     r#"{"loops":{"1000A":{"segments":["N3","N4","REF","PER","XX"]}}}"#
    /// ).unwrap();
    /// let loop_1000a = patched.get(patched.loop_id("1000A").unwrap());
    /// assert_eq!(loop_1000a.segments.len(), 5);
    /// assert!(loop_1000a.segments.contains(&b"XX".to_vec()));
    /// ```
    pub fn merge_patch(&self, patch_json: &str) -> Result<Spec, SpecError> {
        let patched = serde_json::from_str::<Value>(patch_json)
            .map_err(SpecError::Json)
            .and_then(|patch| {
                let mut source = self.source.clone();
                merge_patch(&mut source, &patch);
                Spec::from_value(source)
            });
        patched.map_err(|e| SpecError::Patch {
            source: Box::new(e),
        })
    }

    /// The spec as JSON text, including any patches applied to it.
    pub fn to_json(&self) -> String {
        // A `Value` always serializes; a failure here would be a bug in serde_json.
        serde_json::to_string_pretty(&self.source).unwrap_or_default()
    }

    /// The spec's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every loop, in spec order. Spec order is alphabetical by loop name
    /// (the JSON map is read into a `BTreeMap`), and it breaks ties between
    /// equally specific triggers.
    pub fn loops(&self) -> &[LoopDef] {
        &self.loops
    }

    /// The loop behind an id.
    pub fn get(&self, id: LoopId) -> &LoopDef {
        &self.loops[id.0]
    }

    /// The id of a loop by name.
    pub fn loop_id(&self, name: &str) -> Option<LoopId> {
        self.loops
            .iter()
            .position(|def| def.name == name)
            .map(LoopId)
    }

    /// The name of a loop by id.
    pub fn loop_name(&self, id: LoopId) -> &str {
        &self.loops[id.0].name
    }

    /// Top-level loops (no parent), in spec order.
    pub fn roots(&self) -> &[LoopId] {
        &self.roots
    }

    /// The element definitions of a segment id, if the spec has any.
    pub fn segment(&self, id: &[u8]) -> Option<&SegmentDef> {
        self.segments.get(id)
    }

    /// Every defined segment id with its definition, ordered by id.
    pub fn segments(&self) -> impl Iterator<Item = (&[u8], &SegmentDef)> {
        self.segments.iter().map(|(id, def)| (id.as_slice(), def))
    }

    /// Children of a loop, or the top-level loops for `None`.
    pub fn children(&self, parent: Option<LoopId>) -> &[LoopId] {
        match parent {
            Some(id) => &self.loops[id.0].children,
            None => &self.roots,
        }
    }

    /// Ancestors of a loop, root-most first, excluding the loop itself.
    pub fn ancestors(&self, id: LoopId) -> Vec<LoopId> {
        let mut chain = Vec::new();
        let mut current = self.loops[id.0].parent;
        while let Some(parent) = current {
            chain.push(parent);
            current = self.loops[parent.0].parent;
        }
        chain.reverse();
        chain
    }

    /// The child of `parent` that `segment` triggers, preferring the trigger
    /// with the most conditions; ties go to spec order.
    pub fn matching_child(&self, parent: Option<LoopId>, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match(self.children(parent).iter().copied(), segment)
    }

    /// Any loop that `segment` triggers, wherever it sits in the structure.
    pub fn matching_any(&self, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match((0..self.loops.len()).map(LoopId), segment)
    }

    fn best_match(
        &self,
        candidates: impl Iterator<Item = LoopId>,
        segment: &Segment<'_>,
    ) -> Option<LoopId> {
        candidates
            .filter(|&id| self.loops[id.0].trigger.matches(segment))
            .min_by_key(|&id| Reverse(self.loops[id.0].trigger.conditions.len()))
    }

    pub(crate) fn from_value(source: Value) -> Result<Spec, SpecError> {
        check_shape(&source)?;
        let raw: RawSpec =
            serde_json::from_value(source.clone()).map_err(|e| SpecError::Schema {
                loop_name: None,
                source: e,
            })?;
        if raw.loops.is_empty() {
            return Err(SpecError::NoLoops {
                spec_name: raw.name,
            });
        }
        let mut defs = Vec::with_capacity(raw.loops.len());
        for (name, value) in &raw.loops {
            let def: RawLoop =
                serde_json::from_value(value.clone()).map_err(|e| SpecError::Schema {
                    loop_name: Some(name.clone()),
                    source: e,
                })?;
            defs.push((name.clone(), def));
        }
        let names: Vec<&str> = raw.loops.keys().map(String::as_str).collect();
        let id_of = |name: &str| names.iter().position(|&n| n == name).map(LoopId);

        let mut loops = Vec::with_capacity(defs.len());
        for (name, def) in &defs {
            let empty_at = |key: String| SpecError::EmptySegmentId {
                loop_name: Some(name.clone()),
                key,
            };
            if def.trigger.segment.is_empty() {
                return Err(empty_at("trigger.segment".into()));
            }
            if let Some(i) = def.segments.iter().position(String::is_empty) {
                return Err(empty_at(format!("segments[{i}]")));
            }
            if def.end.as_deref() == Some("") {
                return Err(empty_at("end".into()));
            }
            let parent = match &def.parent {
                None => None,
                Some(parent) => Some(id_of(parent).ok_or_else(|| SpecError::UnknownParent {
                    loop_name: name.clone(),
                    parent: parent.clone(),
                })?),
            };
            let mut conditions = Vec::with_capacity(def.trigger.conditions.len());
            for (position, value) in &def.trigger.conditions {
                let parsed = parse_position(position).ok_or_else(|| SpecError::BadPosition {
                    loop_name: name.clone(),
                    position: position.clone(),
                })?;
                conditions.push((parsed, value.as_bytes().to_vec()));
            }
            conditions.sort();
            loops.push(LoopDef {
                name: name.clone(),
                parent,
                trigger: Trigger {
                    segment: def.trigger.segment.as_bytes().to_vec(),
                    conditions,
                },
                segments: def.segments.iter().map(|s| s.as_bytes().to_vec()).collect(),
                end: def.end.as_ref().map(|s| s.as_bytes().to_vec()),
                children: Vec::new(),
            });
        }

        check_cycles(&loops)?;

        let parents: Vec<Option<LoopId>> = loops.iter().map(|def| def.parent).collect();
        let mut roots = Vec::new();
        for (index, parent) in parents.into_iter().enumerate() {
            match parent {
                Some(parent) => loops[parent.0].children.push(LoopId(index)),
                None => roots.push(LoopId(index)),
            }
        }

        let mut segments = BTreeMap::new();
        for (id, value) in &raw.segments {
            if id.is_empty() {
                return Err(SpecError::EmptySegmentId {
                    loop_name: None,
                    key: "segments.\"\"".into(),
                });
            }
            let def: RawSegment =
                serde_json::from_value(value.clone()).map_err(|e| SpecError::SegmentSchema {
                    segment: id.clone(),
                    source: e,
                })?;
            let elements = compile_elements(id, &def.elements, None)?;
            segments.insert(id.as_bytes().to_vec(), SegmentDef { elements });
        }

        let spec = Spec {
            name: raw.name,
            loops,
            roots,
            segments,
            source,
        };
        spec.check_ambiguity()?;
        Ok(spec)
    }

    fn check_ambiguity(&self) -> Result<(), SpecError> {
        let groups = std::iter::once(self.roots.as_slice())
            .chain(self.loops.iter().map(|def| def.children.as_slice()));
        for siblings in groups {
            for (i, &first) in siblings.iter().enumerate() {
                for &second in &siblings[i + 1..] {
                    let trigger = &self.loops[first.0].trigger;
                    let other = &self.loops[second.0].trigger;
                    if trigger.segment != other.segment {
                        continue;
                    }
                    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                    let parent = self.loops[first.0]
                        .parent
                        .map(|parent| self.loops[parent.0].name.clone());
                    if trigger == other {
                        return Err(SpecError::AmbiguousTrigger {
                            first: self.loops[first.0].name.clone(),
                            second: self.loops[second.0].name.clone(),
                            parent,
                            segment: text(&trigger.segment),
                            conditions: trigger
                                .conditions
                                .iter()
                                .map(|(position, value)| (*position, text(value)))
                                .collect(),
                        });
                    }
                    // Siblings are told apart when a shared position requires
                    // different values, or when one trigger's conditions
                    // contain the other's: the engine then prefers the one
                    // with more conditions and the other is the catch-all.
                    let excluded = trigger.conditions.iter().any(|(position, value)| {
                        other
                            .conditions
                            .iter()
                            .any(|(p, v)| p == position && v != value)
                    });
                    let contains = |big: &Trigger, small: &Trigger| {
                        small
                            .conditions
                            .iter()
                            .all(|condition| big.conditions.contains(condition))
                    };
                    let nested = contains(trigger, other) || contains(other, trigger);
                    if !excluded && !nested {
                        return Err(SpecError::OverlappingTriggers {
                            parent,
                            a: self.loops[first.0].name.clone(),
                            b: self.loops[second.0].name.clone(),
                            conditions_a: render_trigger(trigger),
                            conditions_b: render_trigger(other),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
fn render_trigger(trigger: &Trigger) -> String {
    let segment = String::from_utf8_lossy(&trigger.segment);
    if trigger.conditions.is_empty() {
        return format!("{segment:?} with no conditions");
    }
    let parts: Vec<String> = trigger
        .conditions
        .iter()
        .map(|(position, value)| format!("{position}: {:?}", String::from_utf8_lossy(value)))
        .collect();
    format!("{segment:?} where {{{}}}", parts.join(", "))
}

/// A 1-based element position written in canonical form (`"1"`, never
/// `"01"` or `"+1"`), so no two keys can name the same position.
fn parse_position(key: &str) -> Option<usize> {
    key.parse::<usize>()
        .ok()
        .filter(|&p| p >= 1 && p.to_string() == key)
}

/// Compiles the elements of `segment`, or the components of the element at
/// `parent` (its key as written) when one is given.
fn compile_elements(
    segment: &str,
    raw: &BTreeMap<String, RawElement>,
    parent: Option<&str>,
) -> Result<BTreeMap<usize, ElementDef>, SpecError> {
    let mut elements = BTreeMap::new();
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, def) in raw {
        let position_text = match parent {
            Some(parent) => format!("{parent}.composite.{key}"),
            None => key.clone(),
        };
        let fail = |reason| SpecError::BadElementDef {
            segment: segment.to_string(),
            position: position_text.clone(),
            reason,
        };
        let position =
            parse_position(key).ok_or_else(|| fail(ElementDefError::NonCanonicalPosition))?;
        if def.name.is_empty() {
            return Err(fail(ElementDefError::EmptyName));
        }
        if let Some(first) = names.insert(def.name.as_str(), key.as_str()) {
            return Err(fail(ElementDefError::DuplicateName {
                name: def.name.clone(),
                first: first.to_string(),
            }));
        }
        let kind = ElementType::parse(&def.kind, def.scale).map_err(fail)?;
        if let (Some(min), Some(max)) = (def.min, def.max)
            && min > max
        {
            return Err(fail(ElementDefError::MinAboveMax { min, max }));
        }
        let composite = if def.composite.is_empty() {
            BTreeMap::new()
        } else if parent.is_some() {
            return Err(fail(ElementDefError::NestedComposite));
        } else if kind != ElementType::An {
            return Err(fail(ElementDefError::CompositeOnNonAn {
                kind: def.kind.clone(),
            }));
        } else {
            compile_elements(segment, &def.composite, Some(key))?
        };
        elements.insert(
            position,
            ElementDef {
                name: def.name.clone(),
                kind,
                required: def.required,
                min: def.min,
                max: def.max,
                composite,
            },
        );
    }
    Ok(elements)
}

/// How a JSON value is described when it is not the object a spec expects.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The value as an object, or [`SpecError::NotAnObject`] naming `path`.
fn object_at<'v>(
    value: &'v Value,
    path: &str,
) -> Result<&'v serde_json::Map<String, Value>, SpecError> {
    value.as_object().ok_or_else(|| SpecError::NotAnObject {
        path: path.to_string(),
        found: kind_of(value),
    })
}

/// Requires an object everywhere the schema has one, before serde sees the
/// value: serde would accept an array in place of a struct, and its message
/// for the wrong kind of value does not say that an object was expected.
/// Missing keys are left to the schema.
fn check_shape(source: &Value) -> Result<(), SpecError> {
    let root = object_at(source, "")?;
    if let Some(loops) = root.get("loops") {
        for (name, def) in object_at(loops, "loops")? {
            let at = format!("loops.{name}");
            let def = object_at(def, &at)?;
            if let Some(trigger) = def.get("trigger") {
                let at = format!("{at}.trigger");
                let trigger = object_at(trigger, &at)?;
                if let Some(conditions) = trigger.get("where") {
                    object_at(conditions, &format!("{at}.where"))?;
                }
            }
        }
    }
    if let Some(segments) = root.get("segments") {
        for (id, def) in object_at(segments, "segments")? {
            let at = format!("segments.{id}");
            let def = object_at(def, &at)?;
            if let Some(elements) = def.get("elements") {
                check_elements_shape(elements, &format!("{at}.elements"))?;
            }
        }
    }
    Ok(())
}

/// Requires every element (and every component) definition to be an object.
fn check_elements_shape(elements: &Value, at: &str) -> Result<(), SpecError> {
    for (position, def) in object_at(elements, at)? {
        let at = format!("{at}.{position}");
        let def = object_at(def, &at)?;
        if let Some(composite) = def.get("composite") {
            check_elements_shape(composite, &format!("{at}.composite"))?;
        }
    }
    Ok(())
}

/// Fails with the first parent cycle found, walking from each loop in spec
/// order. The members start at the loop the walk reaches twice.
fn check_cycles(loops: &[LoopDef]) -> Result<(), SpecError> {
    for start in 0..loops.len() {
        let mut walk: Vec<LoopId> = Vec::new();
        let mut current = Some(LoopId(start));
        while let Some(id) = current {
            if let Some(repeat) = walk.iter().position(|&seen| seen == id) {
                let members = walk[repeat..]
                    .iter()
                    .map(|&member| loops[member.0].name.clone())
                    .collect();
                return Err(SpecError::Cycle { members });
            }
            walk.push(id);
            current = loops[id.0].parent;
        }
    }
    Ok(())
}

/// JSON Merge Patch, RFC 7386: `patch` objects merge into `target` key by key,
/// `null` members delete, and anything that is not an object replaces `target`.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(members) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in members {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_patch(target.entry(key.as_str()).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Tokenizer};

    fn segs(input: &[u8]) -> Vec<Segment<'_>> {
        Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')).collect()
    }

    #[test]
    fn builtin_835_loads() {
        let spec = Spec::builtin_835();
        assert_eq!(spec.name(), "835");
        assert_eq!(spec.loops().len(), 8);
        let interchange = spec.loop_id("interchange").unwrap();
        assert_eq!(spec.roots(), &[interchange]);
        let transaction = spec.loop_id("transaction").unwrap();
        let names: Vec<_> = spec
            .children(Some(transaction))
            .iter()
            .map(|&c| spec.loop_name(c))
            .collect();
        assert_eq!(names, vec!["1000A", "1000B", "2000"]);
        assert_eq!(
            spec.get(spec.loop_id("2110").unwrap()).parent,
            spec.loop_id("2100")
        );
        assert_eq!(spec.get(transaction).end.as_deref(), Some(&b"SE"[..]));
    }

    #[test]
    fn ancestors_are_listed_root_first() {
        let spec = Spec::builtin_835();
        let chain: Vec<_> = spec
            .ancestors(spec.loop_id("2110").unwrap())
            .iter()
            .map(|&c| spec.loop_name(c))
            .collect();
        assert_eq!(
            chain,
            vec!["interchange", "group", "transaction", "2000", "2100"]
        );
        assert!(
            spec.ancestors(spec.loop_id("interchange").unwrap())
                .is_empty()
        );
    }

    #[test]
    fn trigger_conditions_are_checked_against_elements() {
        let spec = Spec::builtin_835();
        let segments = segs(b"N1*PR*PAYER~N1*PE*PAYEE~N1*TT*OTHER~");
        let transaction = spec.loop_id("transaction");
        assert_eq!(
            spec.matching_child(transaction, &segments[0]),
            spec.loop_id("1000A")
        );
        assert_eq!(
            spec.matching_child(transaction, &segments[1]),
            spec.loop_id("1000B")
        );
        assert_eq!(spec.matching_child(transaction, &segments[2]), None);
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            None,
            "N1 is not a root trigger"
        );
    }

    #[test]
    fn matching_any_finds_a_loop_regardless_of_parent() {
        let spec = Spec::builtin_835();
        let segments = segs(b"SVC*HC:1*10*10~ZZZ*1~");
        assert_eq!(spec.matching_any(&segments[0]), spec.loop_id("2110"));
        assert_eq!(spec.matching_any(&segments[1]), None);
    }

    #[test]
    fn more_specific_trigger_wins_over_a_bare_one() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "any":{"trigger":{"segment":"N1"}},
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        )
        .unwrap();
        let segments = segs(b"N1*PR~N1*PE~");
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            spec.loop_id("payer")
        );
        assert_eq!(spec.matching_child(None, &segments[1]), spec.loop_id("any"));
    }

    #[test]
    fn accepts_covers_segments_and_end() {
        let spec = Spec::builtin_835();
        let transaction = spec.get(spec.loop_id("transaction").unwrap());
        assert!(transaction.accepts(b"BPR"));
        assert!(transaction.accepts(b"SE"));
        assert!(!transaction.accepts(b"CLP"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err =
            Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"typo":1}}}"#)
                .unwrap_err();
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "a"),
            "{err}"
        );
    }

    #[test]
    fn unknown_parent_is_rejected_with_both_names() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"parent":"ghost","trigger":{"segment":"AA"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::UnknownParent { loop_name, parent } if loop_name == "a" && parent == "ghost"),
            "{err}"
        );
    }

    #[test]
    fn parent_cycle_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"parent":"b","trigger":{"segment":"AA"}},
                "b":{"parent":"a","trigger":{"segment":"BB"}}
            }}"#,
        )
        .unwrap_err();
        assert!(matches!(err, SpecError::Cycle { .. }), "{err}");
    }

    #[test]
    fn cycle_lists_its_members_from_the_repeated_loop() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"parent":"b","trigger":{"segment":"AA"}},
                "b":{"parent":"c","trigger":{"segment":"BB"}},
                "c":{"parent":"b","trigger":{"segment":"CC"}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Cycle { members } if members == &["b", "c"]),
            "{err:?}"
        );
        assert_eq!(err.to_string(), "loops form a parent cycle: b -> c -> b");
    }

    #[test]
    fn a_loop_that_is_its_own_parent_is_a_cycle() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"parent":"a","trigger":{"segment":"AA"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Cycle { members } if members == &["a"]),
            "{err:?}"
        );
        assert_eq!(err.to_string(), "loops form a parent cycle: a -> a");
    }

    #[test]
    fn a_schema_error_names_the_loop() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "2000":{"trigger":{"segment":"LX"}},
                "2100":{"parent":"2000","trigger":{"segment":"CLP"},"segmnts":["DTM"]}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "2100"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("loop \"2100\" does not match the schema: "),
            "{err}"
        );
        assert!(err.to_string().contains("segmnts"), "{err}");
    }

    #[test]
    fn a_malformed_top_level_is_a_schema_error_without_a_loop() {
        let err = Spec::from_json(r#"{"name":"t"}"#).unwrap_err();
        assert!(
            matches!(
                &err,
                SpecError::Schema {
                    loop_name: None,
                    ..
                }
            ),
            "{err:?}"
        );
        let message = err.to_string();
        assert!(
            message.starts_with("spec does not match the schema: "),
            "{message}"
        );
        assert!(!message.contains("RawSpec"), "{message}");
    }

    #[test]
    fn a_spec_that_is_not_an_object_says_so_in_plain_words() {
        let err = Spec::from_json("[]").unwrap_err();
        assert!(
            matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path.is_empty()),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found an array"
        );
    }

    #[test]
    fn every_object_of_the_loop_schema_is_checked_with_its_path() {
        let cases = [
            (r#"{"name":"t","loops":[]}"#, "loops", "an array"),
            (r#"{"name":"t","loops":{"a":"AA"}}"#, "loops.a", "a string"),
            (
                r#"{"name":"t","loops":{"a":{"trigger":["AA"]}}}"#,
                "loops.a.trigger",
                "an array",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":1}}}}"#,
                "loops.a.trigger.where",
                "a number",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":null}}}}"#,
                "loops.a.trigger.where",
                "null",
            ),
        ];
        for (json, expected_path, expected_found) in cases {
            let err = Spec::from_json(json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{json}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patched_spec_goes_through_the_same_shape_check() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2100":{"trigger":["CLP"]}}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
    }

    #[test]
    fn every_kind_of_json_value_is_named() {
        use serde_json::json;
        assert_eq!(kind_of(&json!(null)), "null");
        assert_eq!(kind_of(&json!(true)), "a boolean");
        assert_eq!(kind_of(&json!(1)), "a number");
        assert_eq!(kind_of(&json!("x")), "a string");
        assert_eq!(kind_of(&json!([])), "an array");
        assert_eq!(kind_of(&json!({})), "an object");
    }

    #[test]
    fn a_spec_without_loops_is_rejected() {
        let err = Spec::from_json(r#"{"name":"t","loops":{}}"#).unwrap_err();
        assert!(
            matches!(&err, SpecError::NoLoops { spec_name } if spec_name == "t"),
            "{err:?}"
        );
    }

    #[test]
    fn several_top_level_loops_are_allowed() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}},"b":{"trigger":{"segment":"BB"}}}}"#,
        )
        .unwrap();
        assert_eq!(spec.roots().len(), 2);
    }

    #[test]
    fn empty_segment_ids_are_rejected_with_the_loop_and_the_key() {
        let cases = [
            (r#"{"trigger":{"segment":""}}"#, "trigger.segment"),
            (
                r#"{"trigger":{"segment":"AA"},"segments":["A1",""]}"#,
                "segments[1]",
            ),
            (r#"{"trigger":{"segment":"AA"},"end":""}"#, "end"),
        ];
        for (def, expected_key) in cases {
            let json = format!(r#"{{"name":"t","loops":{{"a":{def}}}}}"#);
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::EmptySegmentId { loop_name: Some(name), key } if name == "a" && key == expected_key),
                "{def}: {err:?}"
            );
        }
    }

    #[test]
    fn an_empty_segment_id_in_the_segments_section_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"":{"elements":{}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == "segments.\"\""),
            "{err:?}"
        );
    }

    #[test]
    fn non_numeric_zero_or_non_canonical_positions_are_rejected() {
        for position in ["x", "0", "-1", "01", "+1", " 1"] {
            let json = format!(
                r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA","where":{{"{position}":"1"}}}}}}}}}}"#
            );
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::BadPosition { loop_name, position: p } if loop_name == "a" && p == position),
                "{err}"
            );
        }
    }

    #[test]
    fn a_position_spelled_twice_is_rejected_by_its_non_canonical_spelling() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":{"1":"X","01":"Y"}}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::BadPosition { loop_name, position } if loop_name == "a" && position == "01"),
            "{err:?}"
        );
    }

    #[test]
    fn ambiguous_trigger_message_shows_the_parent_and_the_trigger() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "a":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "b":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\"}"
        );
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA"}},
                "b":{"trigger":{"segment":"AA"}}
            }}"#,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
        );
    }

    #[test]
    fn ambiguous_sibling_triggers_are_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA","where":{"1":"X"}}},
                "b":{"trigger":{"segment":"AA","where":{"1":"X"}}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::AmbiguousTrigger { first, second, parent: None, segment, .. } if first == "a" && second == "b" && segment == "AA"),
            "{err}"
        );
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA","where":{"1":"X"}}},
                "b":{"trigger":{"segment":"AA","where":{"1":"Y"}}}
            }}"#,
        );
        assert!(ok.is_ok(), "different conditions are not ambiguous");
    }

    #[test]
    fn siblings_testing_different_positions_overlap_and_are_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "other":{"parent":"transaction","trigger":{"segment":"N1","where":{"2":"X"}}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::OverlappingTriggers { parent: Some(parent), a, b, .. } if parent == "transaction" && a == "other" && b == "payer"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "loops \"other\" and \"payer\" under \"transaction\" can open on the same segment: \"other\" on \"N1\" where {2: \"X\"}, \"payer\" on \"N1\" where {1: \"PR\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "any":{"parent":"transaction","trigger":{"segment":"N1"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        );
        assert!(spec.is_ok(), "{spec:?}");
    }

    #[test]
    fn a_strict_superset_of_conditions_does_not_overlap() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR"}}},
                "acme":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}}
            }}"#,
        )
        .unwrap();
        let segments = segs(b"N1*PR*X~N1*PR*Y~");
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            spec.loop_id("acme"),
            "the more specific trigger wins"
        );
        assert_eq!(
            spec.matching_child(None, &segments[1]),
            spec.loop_id("payer")
        );
    }

    #[test]
    fn siblings_that_differ_at_a_shared_position_do_not_overlap() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}},
                "payee":{"trigger":{"segment":"N1","where":{"1":"PE"}}},
                "other":{"trigger":{"segment":"N3"}}
            }}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
        let builtin = Spec::builtin_835();
        assert!(builtin.loop_id("1000A").is_some() && builtin.loop_id("1000B").is_some());
    }

    #[test]
    fn overlapping_triggers_display_both_loops_and_their_triggers() {
        let err = SpecError::OverlappingTriggers {
            parent: Some("transaction".into()),
            a: "1000A".into(),
            b: "1000C".into(),
            conditions_a: "\"N1\" where {1: \"PR\"}".into(),
            conditions_b: "\"N1\" where {2: \"X\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"1000A\" and \"1000C\" under \"transaction\" can open on the same segment: \"1000A\" on \"N1\" where {1: \"PR\"}, \"1000C\" on \"N1\" where {2: \"X\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::OverlappingTriggers {
            parent: None,
            a: "a".into(),
            b: "b".into(),
            conditions_a: "\"AA\" where {1: \"X\"}".into(),
            conditions_b: "\"AA\" where {2: \"Y\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root can open on the same segment: \"a\" on \"AA\" where {1: \"X\"}, \"b\" on \"AA\" where {2: \"Y\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn triggers_render_with_their_conditions_in_position_order() {
        let bare = Trigger {
            segment: b"N1".to_vec(),
            conditions: Vec::new(),
        };
        assert_eq!(render_trigger(&bare), "\"N1\" with no conditions");
        let conditioned = Trigger {
            segment: b"N1".to_vec(),
            conditions: vec![(1, b"PR".to_vec()), (3, b"X".to_vec())],
        };
        assert_eq!(
            render_trigger(&conditioned),
            "\"N1\" where {1: \"PR\", 3: \"X\"}"
        );
    }

    #[test]
    fn to_json_round_trips_through_from_json() {
        let spec = Spec::builtin_835();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert_eq!(again.loops(), spec.loops());
    }

    fn json_error(text: &str) -> serde_json::Error {
        serde_json::from_str::<Value>(text).unwrap_err()
    }

    #[test]
    fn json_error_displays_the_parser_message() {
        let err = SpecError::Json(json_error("{"));
        assert_eq!(
            err.to_string(),
            "invalid spec JSON: EOF while parsing an object at line 1 column 1"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn schema_error_displays_the_loop_and_the_serde_message() {
        let err = SpecError::Schema {
            loop_name: Some("2100".into()),
            source: serde_json::from_value::<RawLoop>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" does not match the schema: invalid type: integer `1`, expected a loop object"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn schema_error_without_a_loop_displays_the_spec() {
        let err = SpecError::Schema {
            loop_name: None,
            source: serde_json::from_value::<RawSpec>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "spec does not match the schema: invalid type: integer `1`, expected a spec object with \"name\" and \"loops\""
        );
    }

    #[test]
    fn patch_error_displays_the_inner_error_once() {
        let err = SpecError::Patch {
            source: Box::new(SpecError::EmptySegmentId {
                loop_name: Some("a".into()),
                key: "end".into(),
            }),
        };
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"a\" has an empty segment id at end"
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"a\" has an empty segment id at end")
        );
    }

    #[test]
    fn not_an_object_displays_the_path_and_what_was_found() {
        let err = SpecError::NotAnObject {
            path: "loops.2100.trigger".into(),
            found: "an array",
        };
        assert_eq!(
            err.to_string(),
            "spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::NotAnObject {
            path: String::new(),
            found: "a string",
        };
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found a string"
        );
    }

    #[test]
    fn unknown_parent_displays_both_names() {
        let err = SpecError::UnknownParent {
            loop_name: "2100".into(),
            parent: "2000".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" names unknown parent \"2000\""
        );
    }

    #[test]
    fn cycle_displays_the_walk() {
        let err = SpecError::Cycle {
            members: vec!["b".into(), "c".into()],
        };
        assert_eq!(err.to_string(), "loops form a parent cycle: b -> c -> b");
    }

    #[test]
    fn no_loops_displays_the_spec_name() {
        let err = SpecError::NoLoops {
            spec_name: "name".into(),
        };
        assert_eq!(err.to_string(), "spec \"name\" declares no loops");
    }

    #[test]
    fn empty_segment_id_displays_the_loop_and_the_key() {
        let err = SpecError::EmptySegmentId {
            loop_name: Some("2100".into()),
            key: "segments[3]".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" has an empty segment id at segments[3]"
        );
        let err = SpecError::EmptySegmentId {
            loop_name: None,
            key: "segments.\"\"".into(),
        };
        assert_eq!(
            err.to_string(),
            "the spec has an empty segment id at segments.\"\""
        );
    }

    #[test]
    fn bad_position_displays_the_key_and_the_rule() {
        let err = SpecError::BadPosition {
            loop_name: "a".into(),
            position: "01".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"a\" has an invalid \"where\" position \"01\": positions are 1-based integers written in canonical form"
        );
    }

    #[test]
    fn ambiguous_trigger_displays_parent_segment_and_conditions() {
        let err = SpecError::AmbiguousTrigger {
            first: "a".into(),
            second: "b".into(),
            parent: Some("transaction".into()),
            segment: "N1".into(),
            conditions: vec![(1, "PR".into()), (3, "XX".into())],
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\", 3: \"XX\"}"
        );
        let err = SpecError::AmbiguousTrigger {
            first: "a".into(),
            second: "b".into(),
            parent: None,
            segment: "AA".into(),
            conditions: Vec::new(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
        );
    }

    const CLP_ONLY: &str = r#"{"name":"t",
        "loops":{"2100":{"trigger":{"segment":"CLP"},"segments":["ZZ1"]}},
        "segments":{"CLP":{"elements":{
            "1":{"name":"claim_submitter_id","type":"AN","required":true,"min":1,"max":38},
            "3":{"name":"total_claim_charge_amount","type":"R","required":true},
            "12":{"name":"drg_weight","type":"R","scale":4}
        }},
        "SVC":{"elements":{
            "1":{"name":"procedure","type":"AN","required":true,"composite":{
                "1":{"name":"qualifier","type":"ID","required":true,"min":2,"max":2},
                "2":{"name":"code","type":"AN","required":true}
            }},
            "5":{"name":"units","type":"N0"}
        }}}
    }"#;

    fn element_error(elements: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{{"AA":{{"elements":{elements}}}}}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn segments_are_keyed_by_id_and_elements_by_position() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        let first = &clp.elements[&1];
        assert_eq!(first.name, "claim_submitter_id");
        assert_eq!(first.kind, ElementType::An);
        assert!(first.required);
        assert_eq!((first.min, first.max), (Some(1), Some(38)));
        assert_eq!(clp.elements[&3].kind, ElementType::R { scale: 2 });
        assert!(!clp.elements[&12].required, "required defaults to false");
        assert_eq!(clp.elements[&12].kind, ElementType::R { scale: 4 });
        assert_eq!(
            clp.elements.keys().copied().collect::<Vec<_>>(),
            vec![1, 3, 12]
        );
        let svc = spec.segment(b"SVC").unwrap();
        let procedure = &svc.elements[&1];
        assert_eq!(procedure.composite[&1].kind, ElementType::Id);
        assert_eq!(procedure.composite[&2].name, "code");
        assert_eq!(svc.elements[&5].kind, ElementType::N(0));
        let ids: Vec<&[u8]> = spec.segments().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![&b"CLP"[..], &b"SVC"[..]]);
    }

    #[test]
    fn every_type_code_is_read() {
        let cases = [
            ("AN", None, ElementType::An),
            ("ID", None, ElementType::Id),
            ("N0", None, ElementType::N(0)),
            ("N2", None, ElementType::N(2)),
            ("N9", None, ElementType::N(9)),
            ("R", None, ElementType::R { scale: 2 }),
            ("R", Some(6), ElementType::R { scale: 6 }),
            ("DT", None, ElementType::Dt),
            ("TM", None, ElementType::Tm),
        ];
        for (code, scale, expected) in cases {
            assert_eq!(ElementType::parse(code, scale), Ok(expected), "{code}");
        }
        for code in ["an", "N", "N10", "NA", "R2", "", "B"] {
            assert_eq!(
                ElementType::parse(code, None),
                Err(ElementDefError::UnknownType {
                    found: code.to_string()
                }),
                "{code:?}"
            );
        }
    }

    #[test]
    fn element_types_display_their_code_and_meaning() {
        assert_eq!(ElementType::An.to_string(), "AN (string)");
        assert_eq!(ElementType::Id.to_string(), "ID (code)");
        assert_eq!(
            ElementType::N(2).to_string(),
            "N2 (integer with 2 implied decimals)"
        );
        assert_eq!(
            ElementType::R { scale: 2 }.to_string(),
            "R (decimal, scale 2)"
        );
        assert_eq!(ElementType::Dt.to_string(), "DT (date CCYYMMDD or YYMMDD)");
        assert_eq!(
            ElementType::Tm.to_string(),
            "TM (time HHMM, HHMMSS or HHMMSSD..)"
        );
    }

    #[test]
    fn a_segment_a_loop_lists_without_a_definition_stays_opaque() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        assert!(spec.get(spec.loop_id("2100").unwrap()).accepts(b"ZZ1"));
        assert_eq!(spec.segment(b"ZZ1"), None);
    }

    #[test]
    fn a_spec_without_segments_has_none() {
        let spec =
            Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}}}"#).unwrap();
        assert_eq!(spec.segments().count(), 0);
    }

    #[test]
    fn bad_element_definitions_are_rejected_with_segment_position_and_reason() {
        let cases = [
            (
                r#"{"01":{"name":"a","type":"AN"}}"#,
                "01",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"0":{"name":"a","type":"AN"}}"#,
                "0",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"1":{"name":"","type":"AN"}}"#,
                "1",
                ElementDefError::EmptyName,
            ),
            (
                r#"{"1":{"name":"a","type":"AN"},"2":{"name":"a","type":"ID"}}"#,
                "2",
                ElementDefError::DuplicateName {
                    name: "a".into(),
                    first: "1".into(),
                },
            ),
            (
                r#"{"1":{"name":"a","type":"XX"}}"#,
                "1",
                ElementDefError::UnknownType { found: "XX".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"N2","scale":2}}"#,
                "1",
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","min":5,"max":2}}"#,
                "1",
                ElementDefError::MinAboveMax { min: 5, max: 2 },
            ),
            (
                r#"{"1":{"name":"a","type":"ID","composite":{"1":{"name":"b","type":"AN"}}}}"#,
                "1",
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"2":{"name":"b","type":"AN","composite":{"1":{"name":"c","type":"AN"}}}}}}"#,
                "1.composite.2",
                ElementDefError::NestedComposite,
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"1":{"name":"b","type":"AN"},"2":{"name":"b","type":"AN"}}}}"#,
                "1.composite.2",
                ElementDefError::DuplicateName {
                    name: "b".into(),
                    first: "1".into(),
                },
            ),
        ];
        for (elements, expected_position, expected_reason) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == expected_position && *reason == expected_reason),
                "{elements}: {err:?}"
            );
        }
    }

    #[test]
    fn names_only_need_to_be_unique_among_siblings() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{
                "1":{"name":"code","type":"AN","composite":{"1":{"name":"code","type":"AN"}}}
            }}}}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
    }

    #[test]
    fn a_segment_definition_that_breaks_the_schema_names_the_segment() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"AN","lenght":3}}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::SegmentSchema { segment, .. } if segment == "AA"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("segment \"AA\" does not match the schema: "),
            "{err}"
        );
        assert!(err.to_string().contains("lenght"), "{err}");
    }

    #[test]
    fn every_object_of_the_segment_schema_is_checked_with_its_path() {
        let cases = [
            (r#"[]"#, "segments", "an array"),
            (r#"{"CLP":[]}"#, "segments.CLP", "an array"),
            (
                r#"{"CLP":{"elements":[]}}"#,
                "segments.CLP.elements",
                "an array",
            ),
            (
                r#"{"CLP":{"elements":{"1":"claim_id"}}}"#,
                "segments.CLP.elements.1",
                "a string",
            ),
            (
                r#"{"SVC":{"elements":{"1":{"name":"p","type":"AN","composite":{"2":7}}}}}"#,
                "segments.SVC.elements.1.composite.2",
                "a number",
            ),
        ];
        for (segments, expected_path, expected_found) in cases {
            let json = format!(
                r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{segments}}}"#
            );
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{segments}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patch_retouches_one_element_and_keeps_the_rest() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(r#"{"segments":{"CLP":{"elements":{"1":{"max":30}}}}}"#)
            .unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        assert_eq!(clp.elements[&1].max, Some(30));
        assert_eq!(clp.elements[&1].name, "claim_submitter_id");
        assert_eq!(clp.elements.len(), 3);
    }

    #[test]
    fn a_patch_adds_a_segment_definition() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(
                r#"{"segments":{"ZZ1":{"elements":{"1":{"name":"payer_note","type":"AN"}}}}}"#,
            )
            .unwrap();
        assert_eq!(
            spec.segment(b"ZZ1").unwrap().elements[&1].name,
            "payer_note"
        );
    }

    #[test]
    fn to_json_round_trips_the_segments_section() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert!(spec.segments().eq(again.segments()));
        assert_eq!(again.segments().count(), 2);
    }

    #[test]
    fn segment_schema_error_displays_the_segment_and_the_serde_message() {
        let err = SpecError::SegmentSchema {
            segment: "CLP".into(),
            source: serde_json::from_value::<RawSegment>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "segment \"CLP\" does not match the schema: invalid type: integer `1`, expected a segment object"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn bad_element_def_displays_segment_position_and_every_reason() {
        let cases = [
            (
                ElementDefError::NonCanonicalPosition,
                "segment \"CLP\" element \"01\": positions are 1-based integers written in canonical form",
            ),
            (
                ElementDefError::EmptyName,
                "segment \"CLP\" element \"01\": \"name\" is empty",
            ),
            (
                ElementDefError::DuplicateName {
                    name: "claim_id".into(),
                    first: "1".into(),
                },
                "segment \"CLP\" element \"01\": name \"claim_id\" is already used by position \"1\"",
            ),
            (
                ElementDefError::UnknownType { found: "XX".into() },
                "segment \"CLP\" element \"01\": type \"XX\" is not one of AN, ID, N0 to N9, R, DT, TM",
            ),
            (
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
                "segment \"CLP\" element \"01\": \"scale\" applies only to type R; found type \"N2\"",
            ),
            (
                ElementDefError::MinAboveMax { min: 5, max: 2 },
                "segment \"CLP\" element \"01\": \"min\" 5 is greater than \"max\" 2",
            ),
            (
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
                "segment \"CLP\" element \"01\": \"composite\" requires type AN; found type \"ID\"",
            ),
            (
                ElementDefError::NestedComposite,
                "segment \"CLP\" element \"01\": a component cannot declare its own \"composite\"",
            ),
        ];
        for (reason, expected) in cases {
            let err = SpecError::BadElementDef {
                segment: "CLP".into(),
                position: "01".into(),
                reason,
            };
            assert_eq!(err.to_string(), expected);
            assert!(std::error::Error::source(&err).is_none());
        }
    }

    #[test]
    fn patch_adds_a_loop() {
        let spec = Spec::builtin_835().merge_patch(
            r#"{"loops":{"2100-ZZ":{"parent":"2100","trigger":{"segment":"ZZ1"},"segments":["ZZ2"]}}}"#,
        )
        .unwrap();
        assert_eq!(spec.loops().len(), 9);
        let zz = spec.loop_id("2100-ZZ").unwrap();
        assert_eq!(spec.get(zz).parent, spec.loop_id("2100"));
        assert!(spec.children(spec.loop_id("2100")).contains(&zz));
    }

    #[test]
    fn patch_replaces_arrays_wholesale() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"1000A":{"segments":["N3"]}}}"#)
            .unwrap();
        assert_eq!(
            spec.get(spec.loop_id("1000A").unwrap()).segments,
            vec![b"N3".to_vec()]
        );
    }

    #[test]
    fn patch_merges_nested_objects_and_keeps_siblings() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"1000A":{"trigger":{"where":{"2":"ACME"}}}}}"#)
            .unwrap();
        let trigger = &spec.get(spec.loop_id("1000A").unwrap()).trigger;
        assert_eq!(trigger.segment, b"N1");
        assert_eq!(
            trigger.conditions,
            vec![(1, b"PR".to_vec()), (2, b"ACME".to_vec())]
        );
        assert_eq!(spec.loops().len(), 8, "other loops untouched");
    }

    #[test]
    fn patch_null_deletes() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2110":null}}"#)
            .unwrap();
        assert_eq!(spec.loop_id("2110"), None);
        assert!(spec.children(spec.loop_id("2100")).is_empty());
    }

    #[test]
    fn patches_chain_and_to_json_shows_them() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#)
            .unwrap()
            .merge_patch(r#"{"loops":{"b":{"trigger":{"segment":"BB"}}}}"#)
            .unwrap();
        assert_eq!(spec.loops().len(), 10);
        assert!(spec.to_json().contains("\"AA\"") && spec.to_json().contains("\"BB\""));
    }

    #[test]
    fn invalid_patch_result_is_rejected_like_any_spec() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2100":{"parent":"nope"}}}"#)
            .unwrap_err();
        assert!(
            matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::UnknownParent { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn an_unparsable_patch_is_reported_as_a_patch_error() {
        let err = Spec::builtin_835().merge_patch("not json").unwrap_err();
        assert!(
            matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::Json(_))),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("applying patch: invalid spec JSON: "),
            "{err}"
        );
    }

    #[test]
    fn a_patch_that_breaks_a_reference_says_it_came_from_the_patch() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2000":null}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"2100\" names unknown parent \"2000\""
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"2100\" names unknown parent \"2000\"")
        );
    }

    #[test]
    fn merge_patch_follows_rfc_7386() {
        use serde_json::json;
        let cases = [
            (json!({"a": "b"}), json!({"a": "c"}), json!({"a": "c"})),
            (
                json!({"a": "b"}),
                json!({"b": "c"}),
                json!({"a": "b", "b": "c"}),
            ),
            (json!({"a": "b"}), json!({"a": null}), json!({})),
            (
                json!({"a": "b", "b": "c"}),
                json!({"a": null}),
                json!({"b": "c"}),
            ),
            (json!({"a": ["b"]}), json!({"a": "c"}), json!({"a": "c"})),
            (json!({"a": "c"}), json!({"a": ["b"]}), json!({"a": ["b"]})),
            (
                json!({"a": {"b": "c"}}),
                json!({"a": {"b": "d", "c": null}}),
                json!({"a": {"b": "d"}}),
            ),
            (
                json!({"a": [{"b": "c"}]}),
                json!({"a": [1]}),
                json!({"a": [1]}),
            ),
            (json!(["a", "b"]), json!(["c", "d"]), json!(["c", "d"])),
            (json!({"a": "b"}), json!(["c"]), json!(["c"])),
            (json!({"a": "foo"}), json!(null), json!(null)),
            (json!({"a": "foo"}), json!("bar"), json!("bar")),
            (
                json!({"e": null}),
                json!({"a": 1}),
                json!({"e": null, "a": 1}),
            ),
            (
                json!([1, 2]),
                json!({"a": "b", "c": null}),
                json!({"a": "b"}),
            ),
            (
                json!({}),
                json!({"a": {"bb": {"ccc": null}}}),
                json!({"a": {"bb": {}}}),
            ),
        ];
        for (target, patch, expected) in cases {
            let mut result = target.clone();
            merge_patch(&mut result, &patch);
            assert_eq!(result, expected, "target {target} patch {patch}");
        }
    }
}
