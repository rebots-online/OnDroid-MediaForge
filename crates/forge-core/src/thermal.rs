//! The thermal governor: degrade, then degrade again, and only pause last.
//!
//! Headroom is the platform's own measure, where values approaching `1.0` mean
//! approaching the limit. The governor walks a fixed ladder and never skips a
//! rung, which is the whole point: a user watching a long job sees the job get
//! slower before it ever sees it stop, and progress never resets (AD-8).

use serde::{Deserialize, Serialize};

use crate::capability::Backend;

/// Five-state heat model driving the UI chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalState {
    Idle,
    Running,
    Sustained,
    Throttling,
    Cooling,
}

/// Governor output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalAction {
    Continue,
    Derate(Backend),
    WidenStride(u32),
    Pause,
}

/// Thresholds and ladder shape. Held separately from the governor so a device
/// profile can tune the ladder without the escalation logic changing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThermalPolicy {
    /// Headroom at or above which the governor climbs one rung.
    pub escalate_at: f32,
    /// Headroom at or below which it steps back down one rung.
    pub recover_at: f32,
    /// Headroom at or above which a job is reported as `Sustained` rather than
    /// `Running`, even while the ladder is still at rest.
    pub sustained_at: f32,
    /// The stride multiplier applied at the first widening rung; the second
    /// widening rung doubles it.
    pub stride_step: u32,
    /// Sustained throughput as a fraction of burst throughput. Planning a long
    /// job at burst rate is what produces a progress bar that stalls.
    pub sustained_fraction: f32,
}

impl Default for ThermalPolicy {
    fn default() -> Self {
        ThermalPolicy {
            escalate_at: 0.75,
            recover_at: 0.50,
            sustained_at: 0.60,
            stride_step: 2,
            sustained_fraction: 0.7,
        }
    }
}

impl ThermalPolicy {
    /// Throughput to plan a long job with, given a measured burst rate.
    pub fn sustained_from_burst(&self, burst: f32) -> f32 {
        burst * self.sustained_fraction
    }
}

/// Ladder rungs, in the order the governor is required to walk them. `Pause`
/// is unreachable without passing through every rung below it.
const RUNG_CONTINUE: u8 = 0;
const RUNG_DERATE_GPU: u8 = 1;
const RUNG_WIDEN: u8 = 2;
const RUNG_WIDEN_MORE: u8 = 3;
const RUNG_PAUSE: u8 = 4;

/// Degrades before pausing: NPU burst to GPU sustained, then widen stride,
/// then widen further, and only then pause.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalGovernor {
    policy: ThermalPolicy,
    rung: u8,
    state: ThermalState,
}

impl Default for ThermalGovernor {
    fn default() -> Self {
        ThermalGovernor::new(ThermalPolicy::default())
    }
}

impl ThermalGovernor {
    /// A governor at rest under `policy`.
    pub fn new(policy: ThermalPolicy) -> Self {
        ThermalGovernor {
            policy,
            rung: RUNG_CONTINUE,
            state: ThermalState::Idle,
        }
    }

    /// The heat state the UI chip renders.
    pub fn state(&self) -> ThermalState {
        self.state
    }

    /// The policy in force.
    pub fn policy(&self) -> &ThermalPolicy {
        &self.policy
    }

    /// Advance the governor with the current thermal headroom.
    ///
    /// One rung per call in either direction. Because escalation is single-step
    /// and the ladder is ordered, a device that heats instantly still emits
    /// `Derate`, then `WidenStride` twice, before the first `ThermalAction::Pause`.
    pub fn step(&mut self, headroom: f32) -> ThermalAction {
        let previous = self.rung;

        if headroom >= self.policy.escalate_at {
            self.rung = (self.rung + 1).min(RUNG_PAUSE);
        } else if headroom <= self.policy.recover_at {
            self.rung = self.rung.saturating_sub(1);
        }

        self.state = if self.rung > RUNG_CONTINUE {
            if self.rung < previous {
                ThermalState::Cooling
            } else {
                ThermalState::Throttling
            }
        } else if previous > RUNG_CONTINUE {
            ThermalState::Cooling
        } else if headroom >= self.policy.sustained_at {
            ThermalState::Sustained
        } else {
            ThermalState::Running
        };

        self.action()
    }

    fn action(&self) -> ThermalAction {
        match self.rung {
            RUNG_DERATE_GPU => ThermalAction::Derate(Backend::Gpu),
            RUNG_WIDEN => ThermalAction::WidenStride(self.policy.stride_step),
            RUNG_WIDEN_MORE => ThermalAction::WidenStride(self.policy.stride_step * 2),
            RUNG_PAUSE => ThermalAction::Pause,
            _ => ThermalAction::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headroom climbing steadily from cool to the limit.
    fn rising() -> Vec<f32> {
        (0..=20).map(|i| i as f32 * 0.05).collect()
    }

    #[test]
    fn a_rising_series_derates_at_least_three_times_before_pausing() {
        let mut governor = ThermalGovernor::default();
        let mut before_pause = Vec::new();
        let mut paused = false;
        for headroom in rising() {
            let action = governor.step(headroom);
            if action == ThermalAction::Pause {
                paused = true;
                break;
            }
            before_pause.push(action);
        }
        assert!(paused, "a series reaching 1.0 headroom must eventually pause");
        assert!(
            before_pause.len() >= 3,
            "only {} non-Pause actions before the first Pause: {before_pause:?}",
            before_pause.len()
        );

        let derations: Vec<ThermalAction> = before_pause
            .into_iter()
            .filter(|a| *a != ThermalAction::Continue)
            .collect();
        assert_eq!(
            derations,
            vec![
                ThermalAction::Derate(Backend::Gpu),
                ThermalAction::WidenStride(2),
                ThermalAction::WidenStride(4),
            ]
        );
    }

    #[test]
    fn a_device_at_the_limit_still_walks_every_rung() {
        let mut governor = ThermalGovernor::default();
        let actions: Vec<ThermalAction> = (0..4).map(|_| governor.step(1.0)).collect();
        assert_eq!(
            actions,
            vec![
                ThermalAction::Derate(Backend::Gpu),
                ThermalAction::WidenStride(2),
                ThermalAction::WidenStride(4),
                ThermalAction::Pause,
            ]
        );
        assert_eq!(governor.state(), ThermalState::Throttling);
    }

    #[test]
    fn cooling_walks_the_ladder_back_down() {
        let mut governor = ThermalGovernor::default();
        for _ in 0..4 {
            governor.step(1.0);
        }
        assert_eq!(governor.step(0.1), ThermalAction::WidenStride(4));
        assert_eq!(governor.state(), ThermalState::Cooling);
        assert_eq!(governor.step(0.1), ThermalAction::WidenStride(2));
        assert_eq!(governor.step(0.1), ThermalAction::Derate(Backend::Gpu));
        assert_eq!(governor.step(0.1), ThermalAction::Continue);
        assert_eq!(governor.step(0.1), ThermalAction::Continue);
        assert_eq!(governor.state(), ThermalState::Running);
    }

    #[test]
    fn a_warm_but_stable_device_reports_sustained_without_derating() {
        let mut governor = ThermalGovernor::default();
        for _ in 0..10 {
            assert_eq!(governor.step(0.65), ThermalAction::Continue);
        }
        assert_eq!(governor.state(), ThermalState::Sustained);
    }

    #[test]
    fn sustained_planning_is_seven_tenths_of_burst() {
        let policy = ThermalPolicy::default();
        assert_eq!(policy.sustained_fraction, 0.7);
        assert!((policy.sustained_from_burst(30.0) - 21.0).abs() < 1e-6);
    }
}
