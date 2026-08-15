//! Energy model from Sec. 3.2 of Zhang (2023).
//!
//! Power: `P = P_s + h*(P_ind + C_ef * S^m)`, with static power `P_s = 0`
//! (ignored, per the paper's stated common DVFS-research practice) and
//! `h = 1` while active. Default parameters match the paper exactly:
//! `P_ind = 0.01`, `C_ef = 1`, `m = 3`, giving critical speed
//! `S_crit = (P_ind / ((m-1)*C_ef))^(1/m) ~= 0.17`.
//!
//! Two energy quantities are provided:
//!  - [`PowerModel::normalized_energy_lo_mode`]: the *closed-form*
//!    normalized energy consumption of a task set's LO-mode workload at a
//!    given speed `S` (Eq. 2), which is what Figs. 5-8 / Sec. 6 of the
//!    paper actually plot and what reproduces the paper's headline
//!    "24.55% average energy reduction" result.
//!  - [`EnergyModel::calculate_schedule_energy`]: energy computed by
//!    integrating power over an *actual simulated schedule* (a
//!    [`crate::schedule::Schedule`]), useful for the illustrative Table 1 /
//!    Figs. 1-4 example and for sanity-checking the closed form.

use crate::algorithm::Utilizations;
use crate::error::Result;
use crate::schedule::{Mode, Speed};
use crate::task::TaskSet;
use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerModel {
    /// P_ind: speed-independent (I/O, memory) active power.
    pub p_ind: f64,
    /// C_ef: effective switching capacitance (dynamic-power coefficient).
    pub c_ef: f64,
    /// m: dynamic-power speed exponent (m >= 2).
    pub m: f64,
    /// P_s: static (leakage) power. The paper sets this to 0.
    pub p_static: f64,
}

impl PowerModel {
    pub fn new(p_ind: f64, c_ef: f64, m: f64, p_static: f64) -> Result<Self> {
        if p_ind < 0.0 {
            return Err(Error::EnergyError("P_ind must be >= 0".into()));
        }
        if c_ef <= 0.0 {
            return Err(Error::EnergyError("C_ef must be > 0".into()));
        }
        if m < 2.0 {
            return Err(Error::EnergyError("m must be >= 2".into()));
        }
        if p_static < 0.0 {
            return Err(Error::EnergyError("P_static must be >= 0".into()));
        }
        Ok(Self { p_ind, c_ef, m, p_static })
    }

    /// Paper's default parameters (Sec. 3.2): P_ind=0.01, C_ef=1, m=3, P_s=0.
    pub fn paper_default() -> Self {
        Self { p_ind: 0.01, c_ef: 1.0, m: 3.0, p_static: 0.0 }
    }

    /// Active power at normalized speed `s`: `P_ind + C_ef * s^m` (plus
    /// static power, if any).
    pub fn power_active(&self, s: f64) -> f64 {
        self.p_static + self.p_ind + self.c_ef * s.powf(self.m)
    }

    /// Energy-efficient critical speed: `S_crit = (P_ind/((m-1)*C_ef))^(1/m)`.
    pub fn critical_speed(&self) -> f64 {
        (self.p_ind / ((self.m - 1.0) * self.c_ef)).powf(1.0 / self.m)
    }

    /// Energy consumed running actively at speed `s` for `duration` time
    /// units.
    pub fn energy(&self, s: f64, duration: f64) -> f64 {
        self.power_active(s) * duration
    }

    /// Eq. (2): normalized energy consumption over one hyper-period of a
    /// task set's LO-mode workload, run entirely at normalized speed `s`:
    ///
    /// `NE(Gamma, S) = (P_ind + C_ef*S^m) * (U_LO_LO(Gamma) + U_LO_HI(Gamma)) / S`
    ///
    /// This is the quantity plotted in Figs. 5-8 (the paper states it
    /// focuses only on LO-mode normalized energy consumption, which is
    /// independent of the number of tasks and the hyper-period, depending
    /// only on total LO-mode utilization and speed).
    pub fn normalized_energy_lo_mode(&self, taskset: &TaskSet, s: f64) -> f64 {
        let u = Utilizations::of(taskset);
        self.normalized_energy_lo_mode_for_u(&u, s)
    }

    /// Same as [`Self::normalized_energy_lo_mode`] but taking pre-computed
    /// utilizations directly (useful for sweeps that vary utilization
    /// without constructing a concrete task set, as in Sec. 6.2).
    pub fn normalized_energy_lo_mode_for_u(&self, u: &Utilizations, s: f64) -> f64 {
        if s <= 0.0 {
            return f64::INFINITY;
        }
        self.power_active(s) * u.u_lo_total() / s
    }
}

impl Default for PowerModel {
    fn default() -> Self {
        Self::paper_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyBreakdown {
    pub static_energy: f64,
    pub dynamic_energy: f64,
    pub by_mode: std::collections::HashMap<Mode, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyResult {
    pub total_energy: f64,
    pub lo_mode_energy: f64,
    pub hi_mode_energy: f64,
    pub mode_switch_energy: f64,
    pub energy_breakdown: EnergyBreakdown,
}

impl EnergyResult {
    pub fn new() -> Self {
        Self {
            total_energy: 0.0,
            lo_mode_energy: 0.0,
            hi_mode_energy: 0.0,
            mode_switch_energy: 0.0,
            energy_breakdown: EnergyBreakdown {
                static_energy: 0.0,
                dynamic_energy: 0.0,
                by_mode: std::collections::HashMap::new(),
            },
        }
    }
}

impl Default for EnergyResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes energy by integrating the power model over an actual simulated
/// [`crate::schedule::Schedule`] (including idle intervals, which draw
/// `power_active(s)` at whatever the "current" mode's nominal speed is,
/// matching the paper's treatment of DVFS-exploitable idle time, Sec. 4.1).
pub struct EnergyModel {
    power_model: PowerModel,
}

impl EnergyModel {
    pub fn new(power_model: PowerModel) -> Self {
        Self { power_model }
    }

    pub fn power_model(&self) -> &PowerModel {
        &self.power_model
    }

    /// Computes energy by integrating the power model over an actual simulated
    /// [`crate::schedule::Schedule`], but **only over intervals where the
    /// processor is actively executing a job**. Per Eq. (1), power is
    /// `P_s + h*(P_ind + C_ef*S^m)` with `h = 1` only "if the system is
    /// active" -- idle intervals (the `IdleStart` events the simulator
    /// emits whenever the ready queue empties) draw `h = 0`, i.e. no
    /// dynamic/independent power at all (this matches the closed-form
    /// Eq. (2), which only integrates over jobs' execution time and
    /// likewise assigns zero energy to idle slack -- see Sec. 4.1's
    /// discussion of "idle intervals ... exploited to reduce energy
    /// consumption").
    pub fn calculate_schedule_energy(
        &self,
        schedule: &crate::schedule::Schedule,
        s_lo: Speed,
        s_max: Speed,
    ) -> EnergyResult {
        let mut result = EnergyResult::new();
        let mut last_time = 0.0;
        let mut mode = Mode::LO;
        let mut active = false;

        let speed_for = |mode: Mode| if mode == Mode::LO { s_lo.value() } else { s_max.value() };

        for event in &schedule.events {
            let duration = event.time - last_time;
            if duration > 1e-9 && active {
                let s = speed_for(mode);
                let e = self.power_model.energy(s, duration);
                result.total_energy += e;
                match mode {
                    Mode::LO => result.lo_mode_energy += e,
                    Mode::HI => result.hi_mode_energy += e,
                }
                *result.energy_breakdown.by_mode.entry(mode).or_default() += e;
                result.energy_breakdown.static_energy += self.power_model.p_static * duration;
                result.energy_breakdown.dynamic_energy +=
                    self.power_model.c_ef * s.powf(self.power_model.m) * duration;
            }

            match event.event_type {
                crate::schedule::EventType::ModeSwitch => mode = Mode::HI,
                crate::schedule::EventType::JobStart | crate::schedule::EventType::JobResume => {
                    active = true;
                }
                crate::schedule::EventType::IdleStart => active = false,
                _ => {}
            }

            last_time = event.time;
        }

        result
    }
}
