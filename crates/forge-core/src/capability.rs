//! Probed device facts. Capability is physics; it is resolved once at first
//! launch, cached, and it outranks every commercial state (AD-9).

use serde::{Deserialize, Serialize};

/// Hardware tier. Ordered: `T0 < T1 < T2`, so "required tier exceeds the
/// device tier" is a plain comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeviceTier {
    T0,
    T1,
    T2,
}
