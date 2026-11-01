//! Core position/range/location structures (01 §2.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub line: u32,
    #[serde(default)]
    pub character: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    #[serde(default)]
    pub start: Position,
    #[serde(default)]
    pub end: Position,
}

impl Range {
    pub fn zero_width(pos: Position) -> Self {
        Range {
            start: pos,
            end: pos,
        }
    }
    pub fn is_zero(&self) -> bool {
        *self == Range::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}
