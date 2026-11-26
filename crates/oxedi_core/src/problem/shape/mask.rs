//! The shape of a mask error.

use super::{Place, Problem};
use crate::mask::MaskError;

problem!(MaskError [ShortKey, UnknownDateMode] _this => {
        Place::Nowhere
});
