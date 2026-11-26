//! The shape of a conformance kit error.

use super::{Place, Problem};
use crate::spec_load::KitError;

problem!(KitError [FormatTooNew, NoCases, RepeatedCase, NoOutputs] this => {
        match this {
            KitError::FormatTooNew { .. } | KitError::NoCases => Place::Kit {
                case: None,
                file: None,
            },
            KitError::RepeatedCase { case } | KitError::NoOutputs { case } => Place::Kit {
                case: Some(case),
                file: None,
            },
        }
});
