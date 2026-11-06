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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// textDocument/prepareRename answer: the renameable range plus the
/// text the client pre-fills.
#[derive(Debug, Clone, Serialize)]
pub struct PrepareRenameResult {
    pub range: Range,
    pub placeholder: String,
}

/// One call-hierarchy node (3.16): macros are Function-kind items; the
/// item round-trips through the client between prepare and the calls
/// requests, so it is Deserialize too.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: i32,
    pub uri: String,
    pub range: Range,
    #[serde(rename = "selectionRange")]
    pub selection_range: Range,
}

#[derive(Debug, Serialize)]
pub struct CallHierarchyIncomingCall {
    pub from: CallHierarchyItem,
    #[serde(rename = "fromRanges")]
    pub from_ranges: Vec<Range>,
}

/// callHierarchy/incomingCalls (and outgoingCalls) params.
#[derive(Debug, Deserialize)]
pub struct CallHierarchyCallsParams {
    pub item: CallHierarchyItem,
}
