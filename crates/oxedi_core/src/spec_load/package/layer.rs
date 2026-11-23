//! The layers of a vocabulary.

use std::fmt;

use crate::vocabulary::{Vocabulary, serde_by_word};

/// One layer of a vocabulary, in layer order (each builds on the ones before).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// The complete base of a stack: the envelope and everything below it.
    Envelope,
    /// The segments, elements and components of a release.
    Dictionary,
    /// The loops of one transaction set.
    TransactionSet,
    /// The usage and codes an implementation guide fixes.
    Guide,
    /// The tables, summaries and default table read from the loops.
    Views,
    /// A change to any key of the layers below.
    Patch,
}

impl Layer {
    /// Every layer, in layer order.
    pub const ALL: [Layer; 6] = [
        Layer::Envelope,
        Layer::Dictionary,
        Layer::TransactionSet,
        Layer::Guide,
        Layer::Views,
        Layer::Patch,
    ];

    /// The word that names the layer in an id and in a header.
    pub const fn word(self) -> &'static str {
        match self {
            Layer::Envelope => "envelope",
            Layer::Dictionary => "dictionary",
            Layer::TransactionSet => "transaction-set",
            Layer::Guide => "guide",
            Layer::Views => "views",
            Layer::Patch => "patch",
        }
    }

    /// The layer a word names, or `None` when it names none.
    pub fn parse(word: &str) -> Option<Layer> {
        Self::from_word(word)
    }
}

impl Vocabulary for Layer {
    const KEY: &'static str = "layer";
    const ALL: &'static [Layer] = &Layer::ALL;

    fn word(self) -> &'static str {
        Layer::word(self)
    }
}

serde_by_word!(Layer);

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}
