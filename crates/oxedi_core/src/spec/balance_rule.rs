//! Balancing rules as data: a rule, its signed terms, and the text that describes them.

use super::Spec;
use super::loops::LoopId;
use crate::message::{place, render_key};

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

/// One term of a balancing rule's side with the names a message writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTerm {
    /// `true` when the term is subtracted.
    pub negative: bool,
    /// `true` when the term reads as a sum: it adds several elements, reads a
    /// loop below the rule's `per`, or reads an occurrence that may appear
    /// more than once.
    pub summed: bool,
    /// The segment id of the occurrence the term reads.
    pub segment: Vec<u8>,
    /// 1-based positions of the elements read, in the order written.
    pub elements: Vec<usize>,
    /// 1-based component read in each element, when the amounts are components.
    pub component: Option<usize>,
    /// The name of the term's loop.
    pub loop_name: String,
    /// The name of the term's occurrence.
    pub occurrence: String,
}

/// One side of a balancing rule, as a message names it. Its `Display` is the
/// text a message writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms(pub Vec<NamedTerm>);

impl Terms {
    /// The terms of one side of a rule over the loop `per`, with the names
    /// of their loops and occurrences.
    pub(crate) fn of(spec: &Spec, per: LoopId, terms: &[BalanceTerm]) -> Terms {
        Terms(terms.iter().map(|term| named(spec, per, term)).collect())
    }
}

fn named(spec: &Spec, per: LoopId, term: &BalanceTerm) -> NamedTerm {
    let def = spec.get(term.loop_id);
    // The loader creates every occurrence index a term holds, so a missing one
    // does not occur; it reads as an empty name that sums.
    let occurrence = def.occurrences.get(term.occurrence);
    let repeats =
        term.loop_id != per || occurrence.is_none_or(|occurrence| occurrence.max != Some(1));
    NamedTerm {
        negative: term.negative,
        summed: term.elements.len() > 1 || repeats,
        segment: occurrence
            .map(|occurrence| occurrence.segment.clone())
            .unwrap_or_default(),
        elements: term.elements.clone(),
        component: term.component,
        loop_name: def.name.clone(),
        occurrence: occurrence
            .map(|occurrence| occurrence.name.clone())
            .unwrap_or_default(),
    }
}

/// What precedes a term, by position (first, later) and sign (added,
/// subtracted): the first term is bare unless negative.
const SEPARATORS: [[&str; 2]; 2] = [["", "- "], [" + ", " - "]];

/// The signed values of one side of a rule, e.g. `sum of CLP04 of 2100
/// "claim_payment_information" - sum of PLB04, PLB06 of transaction
/// "provider_adjustment"`. A term reads as a sum when it adds several
/// elements, reads a loop below `per`, or reads an occurrence that may
/// appear more than once.
pub(crate) fn render_terms(spec: &Spec, per: LoopId, terms: &[BalanceTerm]) -> String {
    let mut out = String::new();
    for (i, term) in terms.iter().enumerate() {
        out.push_str(SEPARATORS[usize::from(i > 0)][usize::from(term.negative)]);
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
