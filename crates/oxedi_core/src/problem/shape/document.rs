//! The shape of a document error.

use super::{Place, Problem};
use crate::document::DocumentError;

problem!(DocumentError [Isa, Size] this => {
        match this {
            DocumentError::Isa(_) => Place::File {
                segment: Some(0),
                element: None,
                component: None,
                path: &[],
            },
            DocumentError::Size(_) => Place::Nowhere,
        }
});
