//! The commercial seam. Entitlement is swappable (AD-9); capability outranks
//! it and the precedence rule lives in `availability.rs`, never here.
//!
//! Credits are spent from a locally-held signed reserve drawn in blocks while
//! online, so a run never blocks on a network round-trip.
//! [`CreditReserve::spend`] is pure arithmetic and issues no I/O at all.

use serde::{Deserialize, Serialize};

use crate::availability::{resolve_availability, NodeAvailability, NodePricing};
use crate::capability::SocProfile;
use crate::graph::NodeKind;

/// Commercial state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entitlement {
    Free,
    Pro { perpetual_version: Option<String> },
}

/// Why the entitlement store could not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EntitlementError {
    /// The balance cannot cover the requested reserve.
    #[error("balance {available} cannot cover a reserve of {requested}")]
    InsufficientBalance { requested: u32, available: u32 },
    /// The store rejected the request or could not be reached.
    #[error("entitlement store: {0}")]
    Store(String),
    /// A locally-held reserve failed its signature check.
    #[error("credit reserve signature is not valid")]
    InvalidSignature,
}

/// Why a gated run was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// The node resolved to a state that is not runnable. The state travels
    /// with the error so the UI renders the matching sheet — a tier-limited
    /// refusal must never be presented as a paywall.
    #[error("node is not runnable: {0:?}")]
    NotRunnable(NodeAvailability),
    /// The node is metered and the reserve cannot cover one unit.
    #[error("not enough credits")]
    InsufficientCredits,
    /// The entitlement store failed.
    #[error(transparent)]
    Entitlement(#[from] EntitlementError),
}

/// A locally-held signed block of credits, spent offline and reconciled later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditReserve {
    pub granted: u32,
    pub spent: u32,
    pub signature: Vec<u8>,
}

impl CreditReserve {
    /// A reserve of `granted` units carrying the store's signature.
    pub fn new(granted: u32, signature: Vec<u8>) -> Self {
        CreditReserve {
            granted,
            spent: 0,
            signature,
        }
    }

    /// Units still available in this block.
    pub fn remaining(&self) -> u32 {
        self.granted.saturating_sub(self.spent)
    }

    /// Spend `n` units. Pure arithmetic — this never issues a network call, so
    /// a generation in airplane mode proceeds at full speed.
    pub fn spend(&mut self, n: u32) -> Result<(), GateError> {
        if self.remaining() < n {
            return Err(GateError::InsufficientCredits);
        }
        self.spent += n;
        Ok(())
    }
}

/// The swappable seam. RevenueCat is the authoritative implementation behind
/// the Kotlin `BillingBridge`; nothing in the core is bound to a processor.
pub trait EntitlementService {
    fn entitlement(&self) -> Entitlement;
    fn credit_balance(&self) -> u32;
    fn reserve_credits(&mut self, n: u32) -> Result<CreditReserve, EntitlementError>;
    fn reconcile(&mut self) -> Result<(), EntitlementError>;

    /// Current per-node credit prices. The entitlement layer is the only
    /// component that knows pricing, and `gated_with_entitlement` must be able
    /// to tell a metered node from a locked one. Defaulted to an empty table so
    /// an implementation that sells no metered nodes need not override it.
    fn pricing(&self) -> NodePricing {
        NodePricing::default()
    }
}

/// The single choke point where gating is enforced.
///
/// `resolve_availability` decides; this function only obeys. The closure runs
/// for `Ready`, `Accelerated` and `Experimental` and for nothing else, so a
/// tier-limited node cannot be executed by any path — and, because the tier
/// test returns first, it also cannot spend a credit or raise a paywall.
///
/// `model_present` is passed as `true`: a node only reaches this point once the
/// scheduler has its weights resident, so absence is not a state reachable
/// here.
pub fn gated_with_entitlement<T>(
    kind: NodeKind,
    svc: &mut dyn EntitlementService,
    caps: &SocProfile,
    f: impl FnOnce() -> T,
) -> Result<T, GateError> {
    let entitlement = svc.entitlement();
    let balance = svc.credit_balance();
    let pricing = svc.pricing();
    let state = resolve_availability(kind, caps, &entitlement, balance, &pricing, true);

    match state {
        NodeAvailability::Ready(_)
        | NodeAvailability::Accelerated
        | NodeAvailability::Experimental { .. } => Ok(f()),
        NodeAvailability::Metered { .. } => {
            let mut reserve = svc.reserve_credits(1).map_err(|e| match e {
                EntitlementError::InsufficientBalance { .. } => GateError::InsufficientCredits,
                other => GateError::Entitlement(other),
            })?;
            reserve.spend(1)?;
            Ok(f())
        }
        refused => Err(GateError::NotRunnable(refused)),
    }
}

/// The seam with nothing behind it: always free, never any credits. This is
/// what `forge-cli` uses, which is why the desktop harness needs no billing
/// stack to exercise the whole pipeline.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NullEntitlementService;

impl EntitlementService for NullEntitlementService {
    fn entitlement(&self) -> Entitlement {
        Entitlement::Free
    }

    fn credit_balance(&self) -> u32 {
        0
    }

    fn reserve_credits(&mut self, n: u32) -> Result<CreditReserve, EntitlementError> {
        if n == 0 {
            return Ok(CreditReserve::new(0, Vec::new()));
        }
        Err(EntitlementError::InsufficientBalance {
            requested: n,
            available: 0,
        })
    }

    fn reconcile(&mut self) -> Result<(), EntitlementError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Backend, DeviceTier, PROBE_SCHEMA_VERSION};

    struct TestService {
        entitlement: Entitlement,
        balance: u32,
        pricing: NodePricing,
        reserved: u32,
    }

    impl TestService {
        fn free(balance: u32) -> Self {
            TestService {
                entitlement: Entitlement::Free,
                balance,
                pricing: NodePricing::new([(NodeKind::GenerativeFill, 4)]),
                reserved: 0,
            }
        }
    }

    impl EntitlementService for TestService {
        fn entitlement(&self) -> Entitlement {
            self.entitlement.clone()
        }
        fn credit_balance(&self) -> u32 {
            self.balance
        }
        fn reserve_credits(&mut self, n: u32) -> Result<CreditReserve, EntitlementError> {
            if self.balance < n {
                return Err(EntitlementError::InsufficientBalance {
                    requested: n,
                    available: self.balance,
                });
            }
            self.balance -= n;
            self.reserved += n;
            Ok(CreditReserve::new(n, vec![0xAB]))
        }
        fn reconcile(&mut self) -> Result<(), EntitlementError> {
            Ok(())
        }
        fn pricing(&self) -> NodePricing {
            self.pricing.clone()
        }
    }

    fn profile(tier: DeviceTier, backends: Vec<Backend>) -> SocProfile {
        SocProfile {
            soc_id: "test".to_string(),
            soc_name: "test".to_string(),
            tier,
            ram_bytes: 12 * 1024 * 1024 * 1024,
            model_budget_bytes: 2 * 1024 * 1024 * 1024,
            backends,
            npu_experimental: false,
            probe_schema_version: PROBE_SCHEMA_VERSION,
        }
    }

    fn t0() -> SocProfile {
        profile(DeviceTier::T0, vec![Backend::Cpu])
    }

    fn t2() -> SocProfile {
        profile(
            DeviceTier::T2,
            vec![Backend::Npu, Backend::Gpu, Backend::Cpu],
        )
    }

    #[test]
    fn a_ready_node_runs_the_closure() {
        let mut svc = TestService::free(0);
        let out = gated_with_entitlement(NodeKind::AudioDenoise, &mut svc, &t0(), || 7);
        assert_eq!(out, Ok(7));
    }

    #[test]
    fn a_metered_node_spends_exactly_one_credit_and_runs() {
        let mut svc = TestService::free(3);
        let mut ran = false;
        let out = gated_with_entitlement(NodeKind::GenerativeFill, &mut svc, &t2(), || {
            ran = true;
            7
        });
        assert_eq!(out, Ok(7));
        assert!(ran);
        assert_eq!(svc.balance, 2);
        assert_eq!(svc.reserved, 1);
    }

    #[test]
    fn a_metered_node_with_no_credits_is_refused_and_does_not_run() {
        let mut svc = TestService::free(0);
        let mut ran = false;
        let out = gated_with_entitlement(NodeKind::GenerativeFill, &mut svc, &t2(), || {
            ran = true;
            7
        });
        assert_eq!(out, Err(GateError::InsufficientCredits));
        assert!(!ran);
        assert_eq!(svc.balance, 0);
    }

    #[test]
    fn a_pro_locked_node_is_refused_and_does_not_run() {
        let mut svc = TestService::free(9);
        svc.pricing = NodePricing::default();
        let mut ran = false;
        let out = gated_with_entitlement(NodeKind::GenerativeFill, &mut svc, &t2(), || {
            ran = true;
            7
        });
        assert_eq!(out, Err(GateError::NotRunnable(NodeAvailability::ProLocked)));
        assert!(!ran);
        assert_eq!(svc.balance, 9, "a refused run must not spend");
    }

    /// The precedence rule reaching the choke point: on a device that cannot
    /// run the node, the refusal is tier-limited and no credit moves, even
    /// though the node is priced and the user is funded.
    #[test]
    fn a_tier_limited_node_is_refused_without_spending() {
        let mut svc = TestService::free(9);
        let mut ran = false;
        let out = gated_with_entitlement(NodeKind::GenerativeFill, &mut svc, &t0(), || {
            ran = true;
            7
        });
        assert_eq!(
            out,
            Err(GateError::NotRunnable(NodeAvailability::TierLimited {
                required: DeviceTier::T2,
                substitute: Some(NodeKind::ImageObjectRemove),
            }))
        );
        assert!(!ran);
        assert_eq!(svc.balance, 9);
        assert_eq!(svc.reserved, 0);
    }

    #[test]
    fn a_reserve_cannot_be_overspent() {
        let mut reserve = CreditReserve::new(2, vec![0x01]);
        assert_eq!(reserve.spend(1), Ok(()));
        assert_eq!(reserve.remaining(), 1);
        assert_eq!(reserve.spend(2), Err(GateError::InsufficientCredits));
        assert_eq!(reserve.spent, 1, "a refused spend must not be recorded");
    }

    #[test]
    fn the_null_service_is_free_with_no_credits() {
        let mut svc = NullEntitlementService;
        assert_eq!(svc.entitlement(), Entitlement::Free);
        assert_eq!(svc.credit_balance(), 0);
        assert_eq!(
            svc.reserve_credits(1),
            Err(EntitlementError::InsufficientBalance {
                requested: 1,
                available: 0,
            })
        );
        assert_eq!(svc.reconcile(), Ok(()));
    }
}
