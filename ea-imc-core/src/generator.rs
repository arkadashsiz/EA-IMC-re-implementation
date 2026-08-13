//! Random IMC task-set generation, mirroring the extensive-experiment setup
//! of Sec. 6.2 / Table 3.
//!
//! Table 3's template gives each HI task WCET pair `[C_i(LO), T_i]` (i.e.
//! `C_i(HI) = T_i`, the worst case where the HI-mode WCET consumes the
//! whole period) and each LO task WCET pair `[1, C_i(LO)]` (i.e.
//! `C_i(HI) = 1`, maximally degraded). The generator instead exposes the
//! four aggregate utilizations directly (as Sec. 6.2 sweeps do) via
//! UUnifast, which is more useful for reproducing Figs. 6-8, while still
//! respecting `C_i(HI) >= C_i(LO)` for HI tasks and `C_i(HI) <= C_i(LO)`
//! for LO tasks.

use crate::error::{Error, Result};
use crate::task::{Criticality, Task, TaskId, TaskSet};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Configuration for random IMC task-set generation.
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    pub n_hi: usize,
    pub n_lo: usize,
    pub period_min: f64,
    pub period_max: f64,
    /// Target U^{LO}_{LO}(Gamma): total LO-mode utilization of LO tasks.
    pub u_lo_lo: f64,
    /// Target U^{HI}_{LO}(Gamma): total HI-mode (degraded) utilization of LO tasks.
    pub u_hi_lo: f64,
    /// Target U^{LO}_{HI}(Gamma): total LO-mode utilization of HI tasks.
    pub u_lo_hi: f64,
    /// Target U^{HI}_{HI}(Gamma): total HI-mode utilization of HI tasks.
    pub u_hi_hi: f64,
    pub seed: u64,
}

impl GeneratorConfig {
    /// Mirrors the extensive-experiment defaults of Sec. 6.2:
    /// `U_HI_HI=0.5`, `U_LO_HI=0.6*U_HI_HI`, `U_HI_LO=0.6*U_LO_LO`, with
    /// `U_LO_LO` left for the caller to sweep.
    pub fn section_6_2_defaults(n_hi: usize, n_lo: usize, u_lo_lo: f64, seed: u64) -> Self {
        let u_hi_hi = 0.5;
        let u_lo_hi = 0.6 * u_hi_hi;
        let u_hi_lo = 0.6 * u_lo_lo;
        Self {
            n_hi,
            n_lo,
            period_min: 100.0,
            period_max: 5000.0,
            u_lo_lo,
            u_hi_lo,
            u_lo_hi,
            u_hi_hi,
            seed,
        }
    }
}

/// UUnifast algorithm (Bini & Buttazzo, 2005): generates `n` utilization
/// values that sum to `total_u`, uniformly distributed over the simplex.
fn uunifast(n: usize, total_u: f64, rng: &mut StdRng) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let mut sum_u = total_u;
    let mut result = Vec::with_capacity(n);
    for i in 1..n {
        let next_sum_u = sum_u * rng.gen::<f64>().powf(1.0 / (n - i) as f64);
        result.push(sum_u - next_sum_u);
        sum_u = next_sum_u;
    }
    result.push(sum_u);
    result
}

pub struct TaskSetGenerator {
    rng: StdRng,
}

impl TaskSetGenerator {
    pub fn new(seed: u64) -> Self {
        Self { rng: StdRng::seed_from_u64(seed) }
    }

    /// Generates a random IMC task set whose four aggregate utilizations
    /// match `config` (via UUnifast per criticality class), with periods
    /// drawn uniformly from `[period_min, period_max]`.
    pub fn generate(&mut self, config: &GeneratorConfig) -> Result<TaskSet> {
        if config.n_hi == 0 && config.n_lo == 0 {
            return Err(Error::InvalidConfig("need at least one task".into()));
        }
        if config.u_hi_lo > config.u_lo_lo + 1e-9 {
            return Err(Error::InvalidConfig(
                "U_HI_LO must be <= U_LO_LO (LO tasks can only be degraded, not amplified)".into(),
            ));
        }
        if config.u_lo_hi > config.u_hi_hi + 1e-9 {
            return Err(Error::InvalidConfig(
                "U_LO_HI must be <= U_HI_HI (HI tasks' HI-mode WCET must be >= LO-mode WCET)".into(),
            ));
        }

        let hi_periods: Vec<f64> = (0..config.n_hi)
            .map(|_| self.rng.gen_range(config.period_min..=config.period_max).round())
            .collect();
        let lo_periods: Vec<f64> = (0..config.n_lo)
            .map(|_| self.rng.gen_range(config.period_min..=config.period_max).round())
            .collect();

        let hi_u_lo = uunifast(config.n_hi, config.u_lo_hi, &mut self.rng);
        let hi_u_hi = uunifast(config.n_hi, config.u_hi_hi, &mut self.rng);
        let lo_u_lo = uunifast(config.n_lo, config.u_lo_lo, &mut self.rng);
        let lo_u_hi = uunifast(config.n_lo, config.u_hi_lo, &mut self.rng);

        let mut tasks = Vec::with_capacity(config.n_hi + config.n_lo);
        let mut id = 0usize;

        for i in 0..config.n_hi {
            let period = hi_periods[i];
            let c_lo = (hi_u_lo[i] * period).max(1.0).round();
            let c_hi = (hi_u_hi[i] * period).max(c_lo).round();
            tasks.push(Task::new(
                TaskId(id),
                format!("tau{id}"),
                Criticality::HI,
                period,
                c_lo,
                c_hi,
            )?);
            id += 1;
        }

        for i in 0..config.n_lo {
            let period = lo_periods[i];
            let c_lo = (lo_u_lo[i] * period).max(1.0).round();
            let c_hi = (lo_u_hi[i] * period).min(c_lo).max(1.0).round();
            tasks.push(Task::new(
                TaskId(id),
                format!("tau{id}"),
                Criticality::LO,
                period,
                c_lo,
                c_hi,
            )?);
            id += 1;
        }

        TaskSet::new(tasks)
    }
}
