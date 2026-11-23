//! What a package changes in the model it builds on: the model before and
//! after a step, compared facet by facet, each difference judged narrower,
//! wider or only different.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::change::{Change, ChangeKind};
use super::id::PackageId;
use crate::message::render_key;
use crate::spec_load::model::{ElementModel, SegmentModel, SpecModel};

/// Every difference of `next` from `prev`, in a fixed order: the segments in
/// key order, then each of their elements and components. What only `next`
/// holds is an addition, never a difference.
pub(super) fn differences(package: &PackageId, prev: &SpecModel, next: &SpecModel) -> Vec<Change> {
    let mut walk = Walk::default();
    walk.entries("segments", &prev.segments, &next.segments, Walk::segment);
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
    // `after` lacks is removed, a key both hold goes to `each`.
    fn entries<K: Ord + ToString, V>(
        &mut self,
        name: &str,
        before: &BTreeMap<K, V>,
        after: &BTreeMap<K, V>,
        each: fn(&mut Walk, &V, &V),
    ) {
        self.at(name, |walk| walk.each_key(before, after, each));
    }

    fn each_key<K: Ord + ToString, V>(
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
        self.flag(
            "required",
            ["not required", "required"],
            before.required,
            after.required,
        );
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

    // A yes or no where yes allows less; `words` are its texts, no then yes.
    fn flag(&mut self, name: &str, words: [&str; 2], before: bool, after: bool) {
        if before != after {
            let (from, to) = (words[usize::from(before)], words[usize::from(after)]);
            self.judged(name, after.cmp(&before), from.to_owned(), to.to_owned());
        }
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
        self.judged(name, order, text(before), text(after));
    }

    // A list of codes: a list allows less than none, and a list inside the
    // base's allows less than the base's.
    fn codes(&mut self, name: &str, before: Option<&[String]>, after: Option<&[String]>) {
        if before == after {
            return;
        }
        let order = match (before, after) {
            (Some(base), Some(codes)) => {
                let held: BTreeSet<&String> = base.iter().collect();
                if codes.iter().all(|code| held.contains(code)) {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (None, _) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
        };
        let text = |list: Option<&[String]>| list.map_or_else(|| "any code".to_owned(), json);
        self.judged(name, order, text(before), text(after));
    }

    // `order` says how much `after` allows against `before`: greater is less.
    fn judged(&mut self, name: &str, order: Ordering, before: String, after: String) {
        let kind = match order {
            Ordering::Less => ChangeKind::Widened { before, after },
            Ordering::Equal | Ordering::Greater => ChangeKind::Narrowed { before, after },
        };
        self.at(name, |walk| walk.record(kind));
    }
}

// A value as a spec writes it; a value that does not serialize prints why.
fn json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|err| err.to_string())
}
