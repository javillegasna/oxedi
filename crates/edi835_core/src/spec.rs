//! Loop specifications: the data that tells the engine how a file is structured.
//!
//! A spec is plain JSON: a map of loops, each naming its parent, the segment
//! (and optional element conditions) that opens it, the segments it may hold
//! and an optional segment that closes it. Loading compiles that into
//! index-based definitions so the engine never compares strings, and keeps the
//! JSON value so patches can be applied on top.

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

/// Why a spec could not be loaded. Each variant names the loop at fault.
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
    /// A trigger has an empty segment id.
    EmptySegmentId {
        /// The loop with the empty trigger.
        loop_name: String,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The loop with the bad key.
        loop_name: String,
        /// The key as written.
        position: String,
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
            SpecError::EmptySegmentId { loop_name } => {
                write!(f, "loop {loop_name:?} has an empty trigger segment")
            }
            SpecError::BadPosition {
                loop_name,
                position,
            } => write!(
                f,
                "loop {loop_name:?} has an invalid \"where\" position {position:?}: \
                 positions are 1-based integers written in canonical form"
            ),
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
        }
    }
}

impl std::error::Error for SpecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpecError::Json(e) | SpecError::Schema { source: e, .. } => Some(e),
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

/// A loaded, validated loop structure.
#[derive(Debug, Clone)]
pub struct Spec {
    name: String,
    loops: Vec<LoopDef>,
    roots: Vec<LoopId>,
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
            if def.trigger.segment.is_empty() {
                return Err(SpecError::EmptySegmentId {
                    loop_name: name.clone(),
                });
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
                // Only the canonical spelling is accepted, so no two keys can
                // name the same position.
                let parsed = position
                    .parse::<usize>()
                    .ok()
                    .filter(|&p| p >= 1 && p.to_string() == *position)
                    .ok_or_else(|| SpecError::BadPosition {
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

        let spec = Spec {
            name: raw.name,
            loops,
            roots,
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
                    if *trigger == self.loops[second.0].trigger {
                        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                        return Err(SpecError::AmbiguousTrigger {
                            first: self.loops[first.0].name.clone(),
                            second: self.loops[second.0].name.clone(),
                            parent: self.loops[first.0]
                                .parent
                                .map(|parent| self.loops[parent.0].name.clone()),
                            segment: text(&trigger.segment),
                            conditions: trigger
                                .conditions
                                .iter()
                                .map(|(position, value)| (*position, text(value)))
                                .collect(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
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
        let err = Spec::from_json("[]").unwrap_err();
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
    fn empty_trigger_segment_is_rejected() {
        let err = Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":""}}}}"#)
            .unwrap_err();
        assert!(
            matches!(&err, SpecError::EmptySegmentId { loop_name } if loop_name == "a"),
            "{err}"
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
                loop_name: "a".into(),
            }),
        };
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"a\" has an empty trigger segment"
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"a\" has an empty trigger segment")
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
    fn empty_segment_id_displays_the_loop() {
        let err = SpecError::EmptySegmentId {
            loop_name: "a".into(),
        };
        assert_eq!(err.to_string(), "loop \"a\" has an empty trigger segment");
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
