//! Balancing rules: amounts that must add up inside one instance of a loop,
//! their compilation against the loops and the `segments` section, and the
//! text that describes them in messages. Their JSON shape is in `schema`.
//!
//! A rule names the loop whose every instance it checks (`per`), a `target`
//! and a `sum`, each a list of signed values. A value reads one or more
//! elements (or one component of each) of every appearance of an occurrence,
//! in the `per` instance itself or in a loop below it; every appearance adds
//! to the side it belongs to. The rule holds when both sides are equal, to
//! the last decimal place.

use std::collections::BTreeMap;

use serde_json::Value;

use super::Spec;
use super::balance_error::BalanceError;
use super::error::SpecError;
use super::loops::LoopId;
use super::model::{BalanceModel, TermModel, read_entry};
use super::occurrences::element_def;
use super::segments::ElementDef;
use crate::message::{place, render_key};
use crate::value::ElementType;

/// A balancing term's `sign`, written `"+"` or `"-"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sign {
    /// Written `"+"`: the value is added.
    #[default]
    Plus,
    /// Written `"-"`: the value is subtracted.
    Minus,
}

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

/// Compiles the `balancing` section against the loops and segments of `spec`.
pub(super) fn compile_balancing(
    spec: &Spec,
    raw: BTreeMap<&str, &Value>,
) -> Result<Vec<BalanceRule>, SpecError> {
    raw.into_iter()
        .map(|(name, value)| {
            let bad = |reason| SpecError::BadBalance {
                rule: name.to_string(),
                reason: Box::new(reason),
            };
            if name.is_empty() {
                return Err(bad(BalanceError::EmptyName));
            }
            let def: BalanceModel = read_entry("balancing", name, value)?;
            compile_rule(spec, name, &def).map_err(bad)
        })
        .collect()
}

/// The elements a rule has counted so far, with where each was named.
type Counted = Vec<((LoopId, usize, usize, Option<usize>), String)>;

/// The terms of one rule while they compile: the elements counted so far,
/// each with the key it was named at, and the largest scale among them.
struct RuleTerms<'s> {
    spec: &'s Spec,
    per: LoopId,
    counted: Counted,
    scale: u8,
}

impl<'s> RuleTerms<'s> {
    fn new(spec: &'s Spec, per: LoopId) -> RuleTerms<'s> {
        RuleTerms {
            spec,
            per,
            counted: Counted::new(),
            scale: 0,
        }
    }

    /// Compiles one side of the rule, written at `key` (`target` or `sum`):
    /// it lists at least one value, and each compiles in the order written.
    fn side(
        &mut self,
        key: &'static str,
        terms: &[TermModel],
    ) -> Result<Vec<BalanceTerm>, BalanceError> {
        if terms.is_empty() {
            return Err(BalanceError::NoTerms { key });
        }
        terms
            .iter()
            .enumerate()
            .map(|(i, term)| self.compile_term(&format!("{key}[{i}]"), term))
            .collect()
    }

    fn compile_term(&mut self, at: &str, term: &TermModel) -> Result<BalanceTerm, BalanceError> {
        let spec = self.spec;
        let loop_id = term_loop(spec, self.per, at, term.loop_name.as_deref())?;
        let def = spec.get(loop_id);
        let (occurrence, segment) = def
            .occurrences
            .iter()
            .enumerate()
            .find(|(_, occurrence)| occurrence.name == term.occurrence)
            .map(|(index, occurrence)| (index, occurrence.segment.as_slice()))
            .ok_or_else(|| BalanceError::UnknownOccurrence {
                key: format!("{at}.occurrence"),
                loop_name: def.name.clone(),
                occurrence: term.occurrence.clone(),
            })?;
        let read = BalanceTerm {
            loop_id,
            occurrence,
            elements: term.elements.clone(),
            component: term.component,
            negative: term.sign == Sign::Minus,
        };
        self.read_elements(at, &read, segment)?;
        Ok(read)
    }

    /// Checks the elements `term` reads, written at `{at}.elements`, from its
    /// occurrence of `segment`: there is at least one, each is a decimal and
    /// none is read by the rule already. Records each as counted and raises
    /// the scale to the largest among them.
    fn read_elements(
        &mut self,
        at: &str,
        term: &BalanceTerm,
        segment: &[u8],
    ) -> Result<(), BalanceError> {
        if term.elements.is_empty() {
            return Err(BalanceError::NoElements {
                key: format!("{at}.elements"),
            });
        }
        for (j, &element) in term.elements.iter().enumerate() {
            let key = format!("{at}.elements[{j}]");
            let place = place(segment, element, term.component);
            let found = element_def(&self.spec.segments, segment, element, term.component);
            self.scale = self.scale.max(decimal_scale(found, &key, &place)?);
            let address = (term.loop_id, term.occurrence, element, term.component);
            if let Some((_, first)) = self.counted.iter().find(|(seen, _)| *seen == address) {
                return Err(BalanceError::RepeatedValue {
                    key,
                    first: first.clone(),
                    place,
                });
            }
            self.counted.push((address, key));
        }
        Ok(())
    }
}

fn compile_rule(spec: &Spec, name: &str, def: &BalanceModel) -> Result<BalanceRule, BalanceError> {
    let per = spec
        .loop_id(&def.per)
        .ok_or_else(|| BalanceError::UnknownLoop {
            key: "per".into(),
            name: def.per.clone(),
        })?;
    let mut terms = RuleTerms::new(spec, per);
    let target = terms.side("target", &def.target)?;
    let sum = terms.side("sum", &def.sum)?;
    Ok(BalanceRule {
        name: name.to_string(),
        per,
        target,
        sum,
        scale: terms.scale,
    })
}

/// The loop a term reads, written at `{at}.loop`: `per` when the term names
/// none, otherwise the named loop, which must be `per` or one below it.
fn term_loop(
    spec: &Spec,
    per: LoopId,
    at: &str,
    name: Option<&str>,
) -> Result<LoopId, BalanceError> {
    let Some(name) = name else {
        return Ok(per);
    };
    let key = format!("{at}.loop");
    let id = spec
        .loop_id(name)
        .ok_or_else(|| BalanceError::UnknownLoop {
            key: key.clone(),
            name: name.to_string(),
        })?;
    if id != per && !spec.ancestors(id).contains(&per) {
        return Err(BalanceError::LoopOutsidePer {
            key,
            loop_name: name.to_string(),
            per: spec.loop_name(per).to_string(),
        });
    }
    Ok(id)
}

/// The scale of an element a term reads, named `place` at `key`: the element
/// must be defined and be a simple `R` element or an `R` component.
fn decimal_scale(def: Option<&ElementDef>, key: &str, place: &str) -> Result<u8, BalanceError> {
    match def {
        None => Err(BalanceError::UndefinedElement {
            key: key.to_string(),
            place: place.to_string(),
        }),
        Some(ElementDef {
            kind: ElementType::R { scale },
            composite,
            ..
        }) if composite.is_empty() => Ok(*scale),
        Some(def) => Err(BalanceError::NotDecimal {
            key: key.to_string(),
            place: place.to_string(),
            found: def.kind.code(),
        }),
    }
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
