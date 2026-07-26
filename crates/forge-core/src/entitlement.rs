//! The commercial seam. Entitlement is swappable (AD-9); capability outranks
//! it and the precedence is expressed in `availability.rs`, never here.

use serde::{Deserialize, Serialize};

/// Commercial state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entitlement {
    Free,
    Pro { perpetual_version: Option<String> },
}
