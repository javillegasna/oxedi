//! The text of the values that select a loop or an occurrence.

use std::fmt;

use crate::message::{Conditions, ElementRef, MAX_LISTED_CODES, render_key, render_names};
use crate::spec::{NamedTerm, Selector, Terms};

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selector::Trigger(trigger) => {
                let segment = String::from_utf8_lossy(&trigger.segment);
                if trigger.conditions.is_empty() {
                    return write!(f, "{segment:?} with no conditions");
                }
                write!(f, "{segment:?}{}", Conditions(&trigger.conditions))
            }
            Selector::Occurrence {
                segment,
                qualifier: None,
            } => write!(f, "{:?}", String::from_utf8_lossy(segment)),
            Selector::Occurrence {
                segment,
                qualifier: Some(qualifier),
            } => {
                let at = ElementRef::new(segment, qualifier.element, qualifier.component);
                write!(
                    f,
                    "{:?} where {at} {}",
                    String::from_utf8_lossy(segment),
                    Codes(&qualifier.codes)
                )
            }
        }
    }
}

/// The codes of a qualifier: one code, a short list or only the count.
struct Codes<'a>(&'a [String]);

impl fmt::Display for Codes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [code] => write!(f, "is {code:?}"),
            codes if codes.len() <= MAX_LISTED_CODES => {
                write!(f, "is one of {}", render_names(codes))
            }
            codes => write!(f, "is one of {} codes", codes.len()),
        }
    }
}

/// What precedes a term, by position (first, later) and sign (added,
/// subtracted): the first term is bare unless negative.
const SEPARATORS: [[&str; 2]; 2] = [["", "- "], [" + ", " - "]];

impl fmt::Display for Terms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, term) in self.0.iter().enumerate() {
            f.write_str(SEPARATORS[usize::from(i > 0)][usize::from(term.negative)])?;
            write!(f, "{term}")?;
        }
        Ok(())
    }
}

/// One term: `sum of CAS03, CAS06 of 2100 "claim_adjustment"`.
impl fmt::Display for NamedTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.summed {
            f.write_str("sum of ")?;
        }
        for (i, &element) in self.elements.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(
                f,
                "{}",
                ElementRef::new(&self.segment, element, self.component)
            )?;
        }
        write!(
            f,
            " of {} {:?}",
            render_key(&self.loop_name),
            self.occurrence
        )
    }
}

#[cfg(test)]
mod proof {
    use super::*;
    use crate::spec::{Spec, render_terms};
    use crate::spec_load::Builtin;
    use crate::validate::{Validation, validate};

    #[test]
    fn sides_render_as_render_terms() {
        let compat = match validate(
            include_str!("../../specs/edi_835_parser.json"),
            Builtin::packages(),
        ) {
            Validation::Yes(valid) => valid.loaded.spec,
            Validation::No(invalid) => panic!("{invalid}"),
        };
        let specs = [
            Spec::builtin_835(),
            Spec::builtin_835_4010(),
            (*compat).clone(),
        ];
        let (mut negative, mut summed) = (false, false);
        for spec in &specs {
            for rule in spec.balancing() {
                for side in [&rule.target, &rule.sum] {
                    let terms = Terms::of(spec, rule.per, side);
                    negative |= terms.0.iter().any(|term| term.negative);
                    summed |= terms.0.iter().any(|term| term.summed);
                    assert_eq!(terms.to_string(), render_terms(spec, rule.per, side));
                }
            }
        }
        assert!(negative && summed);
    }
}
