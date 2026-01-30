use core::fmt;
use std::{any::Any, fmt::Formatter};

use super::shapes::Point;

/// A Glyph.
#[derive(Debug, Clone)]
pub struct Glyph {
    pub id: u16,
    pub position: Point,
}

#[derive(Debug, Clone)]
pub enum FontStyle {
    Normal,
    Italic,
    // Oblique,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
pub enum FontWidth {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}
