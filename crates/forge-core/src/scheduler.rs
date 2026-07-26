//! Segment planning and execution.

use serde::{Deserialize, Serialize};

/// Identity of one segment within a plan. Stable across a resume: it is the
/// segment's position in the plan, and the plan is emitted in full before
/// execution starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SegmentId(pub u64);

impl std::fmt::Display for SegmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
