//! Balancing rules: amounts that must add up inside one instance of a loop,
//! their JSON shape check, their compilation against the loops and the
//! `segments` section, and the text that describes them in messages.
//!
//! A rule names the loop whose every instance it checks (`per`), a `target`
//! and a `sum`, each a list of signed values. A value reads one or more
//! elements (or one component of each) of every appearance of an occurrence,
//! in the `per` instance itself or in a loop below it; every appearance adds
//! to the side it belongs to. The rule holds when both sides are equal, to
//! the last decimal place.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::Spec;
use super::balance_error::BalanceError;
use super::error::SpecError;
use super::loops::LoopId;
use super::occurrence_error::place;
use super::occurrences::{element_def, type_code};
use super::raw::{RawBalance, RawTerm};
use super::render::{child, render_key, render_value};
use super::segments::ElementType;
use super::shape::{Leaf, check_keys, check_leaf, check_member, kind_of, object_at};

/// One signed value of a balancing rule: elements of an occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceTerm {
    /// The loop of the occurrence: the rule's `per` loop or one below it.
    pub loop_id: LoopId,
    /// Index of the occurrence in its loop's occurrences.
    pub occurrence: usize,
    /// 1-based positions of the elements read, in the order written.
    pub elements: Vec<usize>,
    /// 1-based component read in each element, when the amounts are components.
    pub component: Option<usize>,
    /// `true` when the value is subtracted (`"sign": "-"`).
    pub negative: bool,
}

/// A balancing rule: in every instance of `per`, the values of `target` add
/// up to the values of `sum`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceRule {
    /// Name used in the JSON, e.g. `claim_balance`.
    pub name: String,
    /// The loop whose every instance the rule checks.
    pub per: LoopId,
    /// The amounts the rule expects.
    pub target: Vec<BalanceTerm>,
    /// The amounts that must add up to the target.
    pub sum: Vec<BalanceTerm>,
    /// Decimal places the rule adds at: the largest scale of its elements.
    pub scale: u8,
}

/// An array at `at`, or [`SpecError::WrongType`] naming it.
fn array_at<'v>(
    value: &'v Value,
    at: &str,
    expected: &'static str,
) -> Result<&'v [Value], SpecError> {
    match value {
        Value::Array(items) => Ok(items),
        other => Err(SpecError::WrongType {
            path: at.to_string(),
            expected,
            found: kind_of(other),
            value: render_value(other),
        }),
    }
}

/// Requires the `balancing` section to be an object of rule objects with the
/// keys and scalar kinds the schema defines.
pub(super) fn check_balancing_shape(balancing: &Value) -> Result<(), SpecError> {
    for (name, rule) in object_at(balancing, "balancing")? {
        let at = child("balancing", name);
        let rule = object_at(rule, &at)?;
        let keys = ["per", "target", "sum"];
        check_keys(rule, &at, &keys, &keys)?;
        check_member(rule, &at, "per", Leaf::Text, false)?;
        for side in ["target", "sum"] {
            let Some(terms) = rule.get(side) else {
                continue;
            };
            let at = child(&at, side);
            for (i, term) in array_at(terms, &at, "an array of objects")?
                .iter()
                .enumerate()
            {
                let at = format!("{at}[{i}]");
                let term = object_at(term, &at)?;
                check_keys(
                    term,
                    &at,
                    &["loop", "occurrence", "elements", "component", "sign"],
                    &["occurrence", "elements"],
                )?;
                check_member(term, &at, "loop", Leaf::Text, true)?;
                check_member(term, &at, "occurrence", Leaf::Text, false)?;
                check_member(term, &at, "component", Leaf::Count, true)?;
                check_member(term, &at, "sign", Leaf::Text, true)?;
                if let Some(elements) = term.get("elements") {
                    let at = child(&at, "elements");
                    let positions = array_at(elements, &at, "an array of non-negative integers")?;
                    for (j, position) in positions.iter().enumerate() {
                        check_leaf(position, &format!("{at}[{j}]"), Leaf::Count)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Compiles the `balancing` section against the loops and segments of `spec`.
pub(super) fn compile_balancing(
    spec: &Spec,
    raw: BTreeMap<&str, &Value>,
) -> Result<Vec<BalanceRule>, SpecError> {
    let mut rules = Vec::with_capacity(raw.len());
    for (name, value) in raw {
        let def = RawBalance::deserialize(value).map_err(|e| SpecError::BalanceSchema {
            rule: name.to_string(),
            source: e,
        })?;
        rules.push(
            compile_rule(spec, name, &def).map_err(|reason| SpecError::BadBalance {
                rule: name.to_string(),
                reason: Box::new(reason),
            })?,
        );
    }
    Ok(rules)
}

/// The elements a rule has counted so far, with where each was named.
type Counted = Vec<((LoopId, usize, usize, Option<usize>), String)>;

fn compile_rule(spec: &Spec, name: &str, def: &RawBalance) -> Result<BalanceRule, BalanceError> {
    if name.is_empty() {
        return Err(BalanceError::EmptyName);
    }
    let per = spec
        .loop_id(&def.per)
        .ok_or_else(|| BalanceError::UnknownLoop {
            key: "per".into(),
            name: def.per.clone(),
        })?;
    let mut counted = Counted::new();
    let mut scale = 0;
    let mut sides = [Vec::new(), Vec::new()];
    for ((key, raw), side) in [("target", &def.target), ("sum", &def.sum)]
        .into_iter()
        .zip(sides.iter_mut())
    {
        if raw.is_empty() {
            return Err(BalanceError::NoTerms { key });
        }
        for (i, term) in raw.iter().enumerate() {
            let at = format!("{key}[{i}]");
            side.push(compile_term(
                spec,
                per,
                &at,
                term,
                &mut counted,
                &mut scale,
            )?);
        }
    }
    let [target, sum] = sides;
    Ok(BalanceRule {
        name: name.to_string(),
        per,
        target,
        sum,
        scale,
    })
}

fn compile_term(
    spec: &Spec,
    per: LoopId,
    at: &str,
    term: &RawTerm,
    counted: &mut Counted,
    scale: &mut u8,
) -> Result<BalanceTerm, BalanceError> {
    let loop_id = match &term.loop_name {
        None => per,
        Some(name) => {
            let key = format!("{at}.loop");
            let id = spec
                .loop_id(name)
                .ok_or_else(|| BalanceError::UnknownLoop {
                    key: key.clone(),
                    name: name.clone(),
                })?;
            if id != per && !spec.ancestors(id).contains(&per) {
                return Err(BalanceError::LoopOutsidePer {
                    key,
                    loop_name: name.clone(),
                    per: spec.loop_name(per).to_string(),
                });
            }
            id
        }
    };
    let def = spec.get(loop_id);
    let occurrence = def
        .occurrences
        .iter()
        .position(|occurrence| occurrence.name == term.occurrence)
        .ok_or_else(|| BalanceError::UnknownOccurrence {
            key: format!("{at}.occurrence"),
            loop_name: def.name.clone(),
            occurrence: term.occurrence.clone(),
        })?;
    if term.elements.is_empty() {
        return Err(BalanceError::NoElements {
            key: format!("{at}.elements"),
        });
    }
    let segment = def
        .occurrences
        .get(occurrence)
        .map(|occurrence| occurrence.segment.clone())
        .unwrap_or_default();
    let segment_text = String::from_utf8_lossy(&segment).into_owned();
    for (j, &element) in term.elements.iter().enumerate() {
        let key = format!("{at}.elements[{j}]");
        let place = place(&segment_text, element, term.component);
        let element_def = element_def(&spec.segments, &segment, element, term.component)
            .ok_or_else(|| BalanceError::UndefinedElement {
                key: key.clone(),
                place: place.clone(),
            })?;
        match element_def.kind {
            ElementType::R { scale: own } if element_def.composite.is_empty() => {
                *scale = (*scale).max(own);
            }
            kind => {
                return Err(BalanceError::NotDecimal {
                    key,
                    place,
                    found: type_code(kind),
                });
            }
        }
        let address = (loop_id, occurrence, element, term.component);
        if let Some((_, first)) = counted.iter().find(|(seen, _)| *seen == address) {
            return Err(BalanceError::RepeatedValue {
                key,
                first: first.clone(),
                place,
            });
        }
        counted.push((address, key));
    }
    let negative = match term.sign.as_deref() {
        None | Some("+") => false,
        Some("-") => true,
        Some(found) => {
            return Err(BalanceError::UnknownSign {
                key: format!("{at}.sign"),
                found: found.to_string(),
            });
        }
    };
    Ok(BalanceTerm {
        loop_id,
        occurrence,
        elements: term.elements.clone(),
        component: term.component,
        negative,
    })
}

/// The signed values of one side of a rule, e.g. `sum of CLP04 of 2100
/// "claim_payment_information" - sum of PLB04, PLB06 of transaction
/// "provider_adjustment"`. A term reads as a sum when it adds several
/// elements, reads a loop below `per`, or reads an occurrence that may
/// appear more than once.
pub(crate) fn render_terms(spec: &Spec, per: LoopId, terms: &[BalanceTerm]) -> String {
    let mut out = String::new();
    for (i, term) in terms.iter().enumerate() {
        match (i, term.negative) {
            (0, true) => out.push_str("- "),
            (0, false) => {}
            (_, true) => out.push_str(" - "),
            (_, false) => out.push_str(" + "),
        }
        let def = spec.get(term.loop_id);
        let (segment, occurrence) = def
            .occurrences
            .get(term.occurrence)
            .map(|occurrence| {
                (
                    String::from_utf8_lossy(&occurrence.segment).into_owned(),
                    occurrence.name.as_str(),
                )
            })
            .unwrap_or_default();
        let places: Vec<String> = term
            .elements
            .iter()
            .map(|&element| place(&segment, element, term.component))
            .collect();
        let repeats = term.loop_id != per
            || def
                .occurrences
                .get(term.occurrence)
                .is_none_or(|occurrence| occurrence.max != Some(1));
        if places.len() > 1 || repeats {
            out.push_str("sum of ");
        }
        out.push_str(&places.join(", "));
        out.push_str(&format!(" of {} {occurrence:?}", render_key(&def.name)));
    }
    out
}

/// A scaled amount as decimal text with `scale` places, e.g. `-12.50`.
pub(crate) fn render_amount(value: i128, scale: u8) -> String {
    let digits = value.unsigned_abs().to_string();
    let scale = usize::from(scale);
    let sign = if value < 0 { "-" } else { "" };
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    format!("{sign}{whole}.{fraction}")
}
