//! What a package changes in the model it builds on: the model before and
//! after a step, compared facet by facet, each difference judged narrower,
//! wider or only different.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::change::{Change, ChangeKind};
use super::id::PackageId;
use crate::message::render_key;
use crate::spec::Usage;
use crate::spec_load::model::{
    ElementEntryModel, ElementModel, LoopModel, OccurrenceModel, QualifierModel, SegmentModel,
    SpecModel,
};

/// Every difference of `next` from `prev`, in a fixed order: the segments in
/// key order with their elements and components, the loops with their
/// occurrences, the version, then the balancing rules, tables, summaries and
/// the default table. What only `next` holds is an addition.
pub(super) fn differences(package: &PackageId, prev: &SpecModel, next: &SpecModel) -> Vec<Change> {
    let mut walk = Walk::default();
    walk.entries("segments", &prev.segments, &next.segments, Walk::segment);
    walk.entries("loops", &prev.loops, &next.loops, Walk::loop_def);
    walk.optional("version", &prev.version, &next.version, Walk::whole);
    walk.entries("balancing", &prev.balancing, &next.balancing, Walk::whole);
    walk.entries("tables", &prev.tables, &next.tables, Walk::whole);
    walk.entries("summaries", &prev.summaries, &next.summaries, Walk::whole);
    walk.optional(
        "default_table",
        &prev.default_table,
        &next.default_table,
        Walk::whole,
    );
    walk.found
        .into_iter()
        .map(|(path, kind)| Change {
            package: package.clone(),
            path,
            kind,
        })
        .collect()
}

/// The end of a length a model bounds.
#[derive(Clone, Copy)]
enum Limit {
    Min,
    Max,
}

// The walk's position as a dotted path, and what it found so far.
#[derive(Default)]
struct Walk {
    path: String,
    found: Vec<(String, ChangeKind)>,
}

impl Walk {
    // Runs `inside` one key deeper.
    fn at(&mut self, key: &str, inside: impl FnOnce(&mut Walk)) {
        let mark = self.path.len();
        if mark > 0 {
            self.path.push('.');
        }
        self.path.push_str(&render_key(key));
        inside(self);
        self.path.truncate(mark);
    }

    fn record(&mut self, kind: ChangeKind) {
        self.found.push((self.path.clone(), kind));
    }

    // Compares the two maps under `name`, each key of `before` once: a key
    // `after` lacks is removed, a key both hold goes to `each`; the keys only
    // `after` holds are added, after those.
    fn entries<K: Ord + ToString, V: Serialize>(
        &mut self,
        name: &str,
        before: &BTreeMap<K, V>,
        after: &BTreeMap<K, V>,
        each: fn(&mut Walk, &V, &V),
    ) {
        self.at(name, |walk| walk.each_key(before, after, each));
    }

    fn each_key<K: Ord + ToString, V: Serialize>(
        &mut self,
        before: &BTreeMap<K, V>,
        after: &BTreeMap<K, V>,
        each: fn(&mut Walk, &V, &V),
    ) {
        for (key, value) in before {
            self.at(&key.to_string(), |walk| match after.get(key) {
                Some(other) => each(walk, value, other),
                None => walk.record(ChangeKind::Removed),
            });
        }
        for (key, value) in after.iter().filter(|(key, _)| !before.contains_key(key)) {
            let added = ChangeKind::Added { after: json(value) };
            self.at(&key.to_string(), |walk| walk.record(added));
        }
    }

    fn segment(&mut self, before: &SegmentModel, after: &SegmentModel) {
        self.entries("elements", &before.elements, &after.elements, Walk::element);
    }

    // An element and, the same way, each of its components.
    fn element(&mut self, before: &ElementModel, after: &ElementModel) {
        self.plain("name", &before.name, &after.name);
        if before.kind != after.kind {
            let text = |kind: crate::value::ElementType| format!("{:?}", kind.code());
            self.changed("type", text(before.kind), text(after.kind));
        }
        self.usage(before.usage, after.usage);
        self.limit("min", Limit::Min, before.min, after.min);
        self.limit("max", Limit::Max, before.max, after.max);
        self.codes("codes", before.codes.as_deref(), after.codes.as_deref());
        self.entries(
            "composite",
            &before.composite,
            &after.composite,
            Walk::element,
        );
    }

    // A loop: where it sits and what opens it are structure, its usage and
    // maximum allow more or less, and its occurrences are compared by name.
    fn loop_def(&mut self, before: &LoopModel, after: &LoopModel) {
        self.plain("parent", &before.parent, &after.parent);
        self.plain("end", &before.end, &after.end);
        self.plain("trigger", &before.trigger, &after.trigger);
        self.optional("control", &before.control, &after.control, Walk::whole);
        self.usage(before.usage, after.usage);
        self.limit("max", Limit::Max, before.max, after.max);
        let (held, now) = (&before.occurrences, &after.occurrences);
        self.entries("occurrences", held, now, Walk::occurrence);
    }

    fn occurrence(&mut self, before: &OccurrenceModel, after: &OccurrenceModel) {
        self.plain("segment", &before.segment, &after.segment);
        self.plain("pos", &before.pos, &after.pos);
        self.usage(before.usage, after.usage);
        self.limit("max", Limit::Max, before.max, after.max);
        self.optional(
            "qualifier",
            &before.qualifier,
            &after.qualifier,
            Walk::qualifier_codes,
        );
        self.entries(
            "elements",
            &before.elements,
            &after.elements,
            Walk::element_entry,
        );
    }

    // An entry narrows its element in the facets it sets; a usage it leaves out
    // is the loosest one.
    fn element_entry(&mut self, before: &ElementEntryModel, after: &ElementEntryModel) {
        self.usage(
            before.usage.unwrap_or_default(),
            after.usage.unwrap_or_default(),
        );
        self.limit("min", Limit::Min, before.min, after.min);
        self.limit("max", Limit::Max, before.max, after.max);
        self.codes("codes", before.codes.as_deref(), after.codes.as_deref());
    }

    fn qualifier_codes(&mut self, before: &QualifierModel, after: &QualifierModel) {
        self.plain("element", &before.element, &after.element);
        self.plain("component", &before.component, &after.component);
        self.codes("codes", Some(&before.codes), Some(&after.codes));
    }

    // A required usage allows less than a situational one.
    fn usage(&mut self, before: Usage, after: Usage) {
        let word = |usage| match usage {
            Usage::Required => "\"required\"",
            Usage::Situational => "\"situational\"",
        };
        let required = |usage| usage == Usage::Required;
        if before != after {
            let order = required(after).cmp(&required(before));
            self.judged(
                "usage",
                order,
                word(before).to_owned(),
                word(after).to_owned(),
            );
        }
    }

    // A value a spec may leave out: one the package drops is removed, one it
    // adds is added, and one both hold goes to `both`.
    fn optional<T: Serialize>(
        &mut self,
        name: &str,
        before: &Option<T>,
        after: &Option<T>,
        both: fn(&mut Walk, &T, &T),
    ) {
        self.at(name, |walk| match (before, after) {
            (Some(held), Some(now)) => both(walk, held, now),
            (Some(_), None) => walk.record(ChangeKind::Removed),
            (None, Some(now)) => walk.record(ChangeKind::Added { after: json(now) }),
            (None, None) => {}
        });
    }

    // A value that is neither narrower nor wider when it differs, here at the
    // key of its own entry.
    fn whole<T: Serialize + PartialEq>(&mut self, before: &T, after: &T) {
        if before != after {
            self.record(ChangeKind::Changed {
                before: json(before),
                after: json(after),
            });
        }
    }

    // A value that is neither narrower nor wider when it differs.
    fn plain<T: Serialize + PartialEq>(&mut self, name: &str, before: &T, after: &T) {
        if before != after {
            self.changed(name, json(before), json(after));
        }
    }

    fn changed(&mut self, name: &str, before: String, after: String) {
        self.at(name, |walk| {
            walk.record(ChangeKind::Changed { before, after })
        });
    }

    // A length bound: a larger minimum and a smaller maximum allow less, and
    // no bound allows the most.
    fn limit(&mut self, name: &str, end: Limit, before: Option<usize>, after: Option<usize>) {
        if before == after {
            return;
        }
        let (order, none) = match end {
            Limit::Min => (after.unwrap_or(0).cmp(&before.unwrap_or(0)), "no minimum"),
            Limit::Max => (
                before
                    .unwrap_or(usize::MAX)
                    .cmp(&after.unwrap_or(usize::MAX)),
                "no maximum",
            ),
        };
        let text = |bound: Option<usize>| bound.map_or_else(|| none.to_owned(), |n| n.to_string());
        if order != Ordering::Equal {
            self.judged(name, order, text(before), text(after));
        }
    }

    // A list of codes: a list allows less than none, and a list inside the
    // base's allows less than the base's.
    fn codes(&mut self, name: &str, before: Option<&[String]>, after: Option<&[String]>) {
        self.at(name, |walk| walk.listing(before, after));
    }

    fn listing(&mut self, before: Option<&[String]>, after: Option<&[String]>) {
        let order = match (before, after) {
            (Some(base), Some(codes)) => inclusion(base, codes),
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
        };
        if order == Ordering::Equal {
            return;
        }
        let text = |list: Option<&[String]>| list.map_or_else(|| "any code".to_owned(), json);
        self.verdict(order, text(before), text(after));
    }

    // `order` says how much `after` allows against `before`: greater is less.
    fn judged(&mut self, name: &str, order: Ordering, before: String, after: String) {
        self.at(name, |walk| walk.verdict(order, before, after));
    }

    fn verdict(&mut self, order: Ordering, before: String, after: String) {
        self.record(match order {
            Ordering::Less => ChangeKind::Widened { before, after },
            Ordering::Equal | Ordering::Greater => ChangeKind::Narrowed { before, after },
        });
    }
}

// How much `codes` allows against `base`: greater is less, equal when both
// hold the same codes whatever their order.
fn inclusion(base: &[String], codes: &[String]) -> Ordering {
    let held: BTreeSet<&String> = base.iter().collect();
    let now: BTreeSet<&String> = codes.iter().collect();
    if held == now {
        Ordering::Equal
    } else if now.is_subset(&held) {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

// A value as a spec writes it; a value that does not serialize prints why.
fn json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|err| err.to_string())
}
