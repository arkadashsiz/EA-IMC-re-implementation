//! Implementation of the EA-IMC algorithm from:
//!
//! Zhang, Y.-W. "DVFS-based energy-aware scheduling of imprecise
//! mixed-criticality real-time tasks." Journal of Systems Architecture 137
//! (2023) 102849.
//!
//! This module implements:
//!  - the EDF-VD schedulability conditions for the IMC task model
//!    (Lemma 1, Lemma 2, Lemma 3),
//!  - the derivation of the energy-efficient LO-mode speed `S_LO`
//!    (Theorem 1, Proposition 1),
//!  - the EA-IMC scheduling procedure (Sec. 5.2), and
//!  - a discrete-event simulator that produces a `Schedule` trace so the
//!    resulting timeline (a la Figs. 1-4 of the paper) can be inspected.
//!
//! ## A note on OCR ambiguity
//! The source PDF's math renders sub/superscripts inconsistently once
//! extracted to text (e.g. `U^{LO}_{HI}` vs `U^{HI}_{LO}` become visually
//! similar). Equation (4) (Lemma 2) as extracted reads
//! `x*U^{LO}_{LO} + (1-x)*U^{LO}_{HI} + U^{HI}_{HI} <= 1`, which is
//! inconsistent with the derivation carried out in the proof of Theorem 1
//! (equation (10)), which clearly uses `(1-x)*U^{HI}_{LO}`. This
//! implementation follows the Theorem 1 derivation, since it is internally
//! consistent and has been numerically verified against the paper's own
//! worked example (Sec. 5.3: S_LO = 0.83 for the Table 1 task set at
//! x = 0.5, matching the paper exactly).

use crate::error::{Error, Result};
use crate::schedule::{EventType, Mode, Schedule, ScheduleEvent, Speed};
use crate::task::{Criticality, TaskId, TaskSet};

/// Energy-efficient critical speed S_crit used throughout the paper
/// (Sec. 3.2), for the paper's default power-model parameters
/// (P_ind = 0.01, C_ef = 1, m = 3): `S_crit = (P_ind / ((m-1)*C_ef))^(1/m)`.
pub const S_CRIT: f64 = 0.17;

/// Minimum normalized processor speed allowed by the DVFS-enabled
/// processor (Sec. 3.2).
pub const S_MIN: f64 = 0.01;

/// The four aggregate utilization quantities defined in Sec. 3.1 that all
/// schedulability tests and the S_LO derivation are expressed in terms of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Utilizations {
    /// U^{LO}_{LO}(Gamma): LO-mode utilization of LO-criticality tasks.
    pub u_lo_lo: f64,
    /// U^{HI}_{LO}(Gamma): HI-mode (degraded) utilization of LO-criticality tasks.
    pub u_hi_lo: f64,
    /// U^{LO}_{HI}(Gamma): LO-mode utilization of HI-criticality tasks.
    pub u_lo_hi: f64,
    /// U^{HI}_{HI}(Gamma): HI-mode utilization of HI-criticality tasks.
    pub u_hi_hi: f64,
}

impl Utilizations {
    pub fn of(taskset: &TaskSet) -> Self {
        Self {
            u_lo_lo: taskset.lo_utilization_lo(1.0),
            u_hi_lo: taskset.lo_utilization_hi(1.0),
            u_lo_hi: taskset.hi_utilization_lo(1.0),
            u_hi_hi: taskset.hi_utilization_hi(1.0),
        }
    }

    /// Sum of the LO-mode WCET utilization of *all* tasks (U^{LO}_{LO} +
    /// U^{LO}_{HI}), i.e. the quantity the paper's normalized-energy
    /// formula (Eq. 2) is expressed in terms of.
    pub fn u_lo_total(&self) -> f64 {
        self.u_lo_lo + self.u_lo_hi
    }
}

/// Lemma 1: sufficient EDF-VD schedulability condition in LO mode.
pub fn lemma1_lo_mode_schedulable(u: &Utilizations, x: f64) -> bool {
    x > 0.0 && x <= 1.0 && u.u_lo_lo + u.u_lo_hi / x <= 1.0 + 1e-9
}

/// Lemma 2: sufficient EDF-VD schedulability condition in HI mode.
pub fn lemma2_hi_mode_schedulable(u: &Utilizations, x: f64) -> bool {
    x > 0.0 && x <= 1.0 && x * u.u_lo_lo + (1.0 - x) * u.u_hi_lo + u.u_hi_hi <= 1.0 + 1e-9
}

/// Lemma 3: feasibility condition + feasible interval for the deadline
/// scaling factor `x`, given the IMC task set's utilizations.
///
/// Returns `Some((x_lo, x_up))` if the task set is feasible under EDF-VD,
/// `None` otherwise.
pub fn lemma3_feasible_x_range(u: &Utilizations) -> Option<(f64, f64)> {
    if !(u.u_hi_hi + u.u_hi_lo < 1.0 && u.u_lo_lo < 1.0 && u.u_lo_lo > u.u_hi_lo) {
        return None;
    }
    let lhs = u.u_lo_hi / (1.0 - u.u_lo_lo);
    let rhs = (1.0 - (u.u_hi_hi + u.u_hi_lo)) / (u.u_lo_lo - u.u_hi_lo);
    if lhs > rhs + 1e-9 {
        return None;
    }
    let x_up = rhs.min(1.0);
    let x_lo = lhs.max(0.0);
    if x_lo > x_up + 1e-9 {
        return None;
    }
    Some((x_lo, x_up))
}

/// Theorem 1: the smallest LO-mode speed `S_LO` for which EDF-VD remains
/// schedulable at deadline scaling factor `x`, i.e. `S_t1 = max(S_L^L,
/// S_H^L)` from Eq. (7)-(8).
///
/// Returns `None` if `x` is not a valid scaling factor (outside `(0, 1]`)
/// or if the HI-mode denominator degenerates.
pub fn theorem1_s_lo(u: &Utilizations, x: f64) -> Option<f64> {
    if x <= 0.0 || x > 1.0 {
        return None;
    }
    let s_l_l = u.u_lo_lo + u.u_lo_hi / x;
    let denom = 1.0 - u.u_hi_lo - u.u_hi_hi + x * u.u_hi_lo;
    if denom <= 0.0 {
        return None;
    }
    let s_h_l = x * u.u_lo_lo / denom;
    Some(s_l_l.max(s_h_l))
}

/// Proposition 1: clamp a candidate LO-mode speed to the energy-efficient
/// critical speed, `max(S_crit, S_LO)`. Running below `S_crit` never saves
/// energy (Sec. 3.2), so the optimal achievable speed is this maximum.
pub fn clamp_to_critical_speed(s_lo: f64, s_crit: f64) -> f64 {
    s_lo.max(s_crit)
}

/// Full configuration produced by the EA-IMC algorithm for a given task set:
/// the chosen deadline scaling factor `x` and the resulting LO-mode speed
/// `S_LO` (already clamped to the critical speed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EaImcConfig {
    pub x: f64,
    pub s_lo: f64,
    pub s_max: f64,
    pub utilizations: Utilizations,
}

/// A degenerate "IMC without DVFS" baseline config: schedules everything at
/// `S_max = 1` in both modes, as used for comparison throughout the paper
/// (e.g. Figs. 5-8, "IMC without DVFS").
pub fn imc_without_dvfs_config(taskset: &TaskSet) -> Result<EaImcConfig> {
    let u = Utilizations::of(taskset);
    let (x_lo, x_up) = lemma3_feasible_x_range(&u).ok_or_else(|| {
        Error::SchedulabilityFailed(
            "task set infeasible under EDF-VD: Lemma 3 condition violated".into(),
        )
    })?;
    // Any feasible x works for IMC-without-DVFS since it never affects
    // energy at S_max = 1; we report the midpoint for concreteness.
    Ok(EaImcConfig {
        x: 0.5 * (x_lo + x_up),
        s_lo: 1.0,
        s_max: 1.0,
        utilizations: u,
    })
}

/// The EA-IMC scheduler: computes the deadline scaling factor and
/// energy-efficient LO-mode speed for a task set, and can simulate the
/// resulting schedule.
pub struct EaImcScheduler {
    pub s_crit: f64,
}

impl Default for EaImcScheduler {
    fn default() -> Self {
        Self { s_crit: S_CRIT }
    }
}

impl EaImcScheduler {
    pub fn new(s_crit: f64) -> Self {
        Self { s_crit }
    }

    /// Step 1 of Sec. 5.2: compute the feasible x-range (Lemma 3).
    pub fn feasible_x_range(&self, taskset: &TaskSet) -> Result<(f64, f64)> {
        let u = Utilizations::of(taskset);
        lemma3_feasible_x_range(&u).ok_or_else(|| {
            Error::SchedulabilityFailed(
                "task set infeasible under EDF-VD: Lemma 3 condition violated".into(),
            )
        })
    }

    /// Compute the EA-IMC configuration for an explicit, user-chosen `x`
    /// (must lie within the feasible interval from Lemma 3).
    pub fn config_for_x(&self, taskset: &TaskSet, x: f64) -> Result<EaImcConfig> {
        let u = Utilizations::of(taskset);
        let (x_lo, x_up) = lemma3_feasible_x_range(&u).ok_or_else(|| {
            Error::SchedulabilityFailed(
                "task set infeasible under EDF-VD: Lemma 3 condition violated".into(),
            )
        })?;
        if x < x_lo - 1e-9 || x > x_up + 1e-9 {
            return Err(Error::SchedulabilityFailed(format!(
                "x = {x} outside feasible interval [{x_lo}, {x_up}]"
            )));
        }
        let s_t1 = theorem1_s_lo(&u, x)
            .ok_or_else(|| Error::SchedulabilityFailed("could not compute S_LO for given x".into()))?;
        if s_t1 > 1.0 {
            return Err(Error::SchedulabilityFailed(format!(
                "S_LO = {s_t1} > 1: task set unschedulable at x = {x}"
            )));
        }
        let s_lo = clamp_to_critical_speed(s_t1.max(S_MIN), self.s_crit);
        Ok(EaImcConfig {
            x,
            s_lo,
            s_max: 1.0,
            utilizations: u,
        })
    }

    /// The full EA-IMC procedure (Sec. 5.2): search the feasible x-range
    /// for the value that minimizes `S_LO` (and hence, per Proposition 1,
    /// minimizes normalized LO-mode energy consumption).
    pub fn optimal_config(&self, taskset: &TaskSet) -> Result<EaImcConfig> {
        let u = Utilizations::of(taskset);
        let (x_lo, x_up) = lemma3_feasible_x_range(&u).ok_or_else(|| {
            Error::SchedulabilityFailed(
                "task set infeasible under EDF-VD: Lemma 3 condition violated".into(),
            )
        })?;

        const STEPS: usize = 2000;
        let mut best_x = x_lo;
        let mut best_s = f64::INFINITY;
        for i in 0..=STEPS {
            let x = x_lo + (x_up - x_lo) * (i as f64) / (STEPS as f64);
            if let Some(s) = theorem1_s_lo(&u, x) {
                if s <= 1.0 + 1e-9 && s < best_s {
                    best_s = s;
                    best_x = x;
                }
            }
        }
        if !best_s.is_finite() {
            return Err(Error::SchedulabilityFailed(
                "no feasible x in range yields S_LO <= 1".into(),
            ));
        }
        let s_lo = clamp_to_critical_speed(best_s.max(S_MIN), self.s_crit);
        Ok(EaImcConfig {
            x: best_x,
            s_lo,
            s_max: 1.0,
            utilizations: u,
        })
    }

    /// Simulate the EA-IMC schedule over `horizon` and return the resulting
    /// event trace. See [`Overrun`] and [`SimHorizon`] for configuring the
    /// scenario (mirrors Figs. 1-4 of the paper: "no overrun" reproduces
    /// the LO-mode-only trace, while specifying an `Overrun` reproduces a
    /// mode-switch trace).
    pub fn simulate(
        &self,
        taskset: &TaskSet,
        config: &EaImcConfig,
        overrun: Overrun,
        horizon: SimHorizon,
    ) -> Result<Schedule> {
        simulate_schedule(taskset, config, overrun, horizon)
    }
}

/// Which job (if any) overruns its LO-mode WCET, triggering the mode
/// switch. `job_index` is zero-based (the k-th job release of that task,
/// counting from 0).
#[derive(Debug, Clone, Copy)]
pub enum Overrun {
    /// No task overruns: the system stays in LO mode for the whole horizon.
    None,
    /// The `job_index`-th job of `task_id` runs to `C_i(HI)` instead of
    /// `C_i(LO)`, triggering the switch to HI mode.
    At { task_id: TaskId, job_index: u32 },
}

/// How long to simulate.
#[derive(Debug, Clone, Copy)]
pub enum SimHorizon {
    /// Simulate for exactly one hyperperiod.
    OneHyperperiod,
    /// Simulate for an explicit duration.
    Duration(f64),
}

#[derive(Debug, Clone)]
struct JobState {
    task_idx: usize,
    task_id: TaskId,
    name: String,
    criticality: Criticality,
    release: f64,
    /// Current EDF priority key (virtual deadline while in LO mode for HI
    /// tasks, actual deadline otherwise).
    priority_deadline: f64,
    /// Total execution budget for this job in *unscaled* WCET units, i.e.
    /// the amount of work (at speed 1) the job must perform before it may
    /// legally stop. Updated when the mode changes mid-job.
    budget: f64,
    executed: f64,
    /// Speed the remaining work of this job executes at.
    speed: f64,
    /// Whether this specific job instance is the designated overrunning one.
    is_overrun_job: bool,
    started: bool,
    suspended: bool,
}

#[allow(clippy::too_many_arguments)]
fn release_due(
    t: f64,
    taskset: &TaskSet,
    next_release: &mut [f64],
    job_counters: &mut [u32],
    ready: &mut Vec<JobState>,
    mode: Mode,
    config: &EaImcConfig,
    overrun: Overrun,
    schedule: &mut Schedule,
) {
    for (idx, task) in taskset.tasks().iter().enumerate() {
        while next_release[idx] <= t + 1e-9 {
            let release = next_release[idx];
            let job_no = job_counters[idx];
            let is_overrun_job = matches!(
                overrun,
                Overrun::At { task_id, job_index } if task_id == task.id && job_index == job_no
            );

            let (budget, speed, priority_deadline) = match (task.criticality, mode) {
                (Criticality::HI, Mode::LO) => {
                    let virtual_dl = release + config.x * task.period;
                    let b = if is_overrun_job { task.wcet.hi } else { task.wcet.lo };
                    (b, config.s_lo, virtual_dl)
                }
                (Criticality::HI, Mode::HI) => (task.wcet.hi, config.s_max, release + task.period),
                (Criticality::LO, Mode::LO) => (task.wcet.lo, config.s_lo, release + task.period),
                (Criticality::LO, Mode::HI) => (task.wcet.hi, config.s_max, release + task.period),
            };

            schedule.add_event(ScheduleEvent {
                time: release,
                task_id: task.id,
                task_name: task.name.clone(),
                criticality: task.criticality,
                mode,
                speed: Speed(speed),
                event_type: EventType::JobRelease,
                remaining_execution: budget,
            });

            ready.push(JobState {
                task_idx: idx,
                task_id: task.id,
                name: task.name.clone(),
                criticality: task.criticality,
                release,
                priority_deadline,
                budget,
                executed: 0.0,
                speed,
                is_overrun_job,
                started: false,
                suspended: false,
            });

            job_counters[idx] += 1;
            next_release[idx] += task.period;
        }
    }
}

/// Runs the discrete-event EDF-VD simulation described in Sec. 3.1 and 5.2.
///
/// Execution times are treated as deterministic and equal to the relevant
/// WCET (LO-mode WCET normally; HI-mode WCET after a task's job has been
/// designated to overrun, or once the system is in HI mode). This mirrors
/// how the paper's illustrative examples (Figs. 1-4) are constructed: a
/// specific "what-if" execution scenario is fixed, and EDF-VD is played out
/// exactly against it.
fn simulate_schedule(
    taskset: &TaskSet,
    config: &EaImcConfig,
    overrun: Overrun,
    horizon: SimHorizon,
) -> Result<Schedule> {
    let hp = taskset.hyperperiod();
    let horizon = match horizon {
        SimHorizon::OneHyperperiod => hp,
        SimHorizon::Duration(d) => d,
    };

    let mut schedule = Schedule::new(hp);
    let mut mode = Mode::LO;

    let mut next_release: Vec<f64> = vec![0.0; taskset.len()];
    let mut job_counters: Vec<u32> = vec![0; taskset.len()];
    let mut ready: Vec<JobState> = Vec::new();
    let mut switched = false;
    let mut switch_time: Option<f64> = None;

    let mut t = 0.0_f64;
    let mut running_key: Option<(TaskId, u64)> = None; // (task_id, release-time bits) of the job physically running

    release_due(
        t, taskset, &mut next_release, &mut job_counters, &mut ready, mode, config, overrun,
        &mut schedule,
    );

    let mut iterations = 0usize;
    let max_iterations = 200_000;

    while t < horizon - 1e-9 && iterations < max_iterations {
        iterations += 1;

        ready.retain(|j| !j.suspended && j.executed < j.budget - 1e-9);

        let sel = ready
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.priority_deadline.partial_cmp(&b.priority_deadline).unwrap())
            .map(|(i, _)| i);

        let next_rel = next_release
            .iter()
            .cloned()
            .filter(|&r| r <= horizon + 1e-9)
            .fold(f64::INFINITY, f64::min);

        let completion_time = sel.map(|i| {
            let job = &ready[i];
            t + (job.budget - job.executed) / job.speed.max(1e-9)
        });

        let mut next_t = next_rel.min(completion_time.unwrap_or(f64::INFINITY));
        next_t = next_t.min(horizon);
        if !next_t.is_finite() || next_t <= t + 1e-9 {
            if next_rel.is_finite() && next_rel <= horizon + 1e-9 {
                next_t = next_rel;
            } else {
                break;
            }
        }

        if let Some(i) = sel {
            let job = &ready[i];
            let key = (job.task_id, job.release.to_bits());
            if running_key != Some(key) {
                let job = &mut ready[i];
                schedule.add_event(ScheduleEvent {
                    time: t,
                    task_id: job.task_id,
                    task_name: job.name.clone(),
                    criticality: job.criticality,
                    mode,
                    speed: Speed(job.speed),
                    event_type: if job.started { EventType::JobResume } else { EventType::JobStart },
                    remaining_execution: job.budget - job.executed,
                });
                job.started = true;
                running_key = Some(key);
            }
        } else if running_key.is_some() {
            schedule.add_event(ScheduleEvent {
                time: t,
                task_id: TaskId(usize::MAX),
                task_name: "idle".into(),
                criticality: Criticality::LO,
                mode,
                speed: Speed(0.0),
                event_type: EventType::IdleStart,
                remaining_execution: 0.0,
            });
            running_key = None;
        }

        if let Some(i) = sel {
            let dt = next_t - t;
            let job = &mut ready[i];
            job.executed += job.speed * dt;

            // Check for HI-task overrun crossing C_i(LO) while in LO mode:
            // this is the mode-switch trigger (Sec. 3.1).
            if !switched && job.criticality == Criticality::HI && mode == Mode::LO {
                let task = &taskset.tasks()[job.task_idx];
                if job.is_overrun_job && job.executed >= task.wcet.lo - 1e-9 {
                    let already = job.executed - job.speed * dt;
                    let sw_t = t + (task.wcet.lo - already) / job.speed;
                    let dt1 = sw_t - t;
                    job.executed = already + job.speed * dt1;
                    next_t = sw_t;
                }
            }
        }

        t = next_t;

        if !switched {
            if let Overrun::At { task_id, job_index } = overrun {
                if let Some(job) = ready.iter().find(|j| {
                    j.task_id == task_id
                        && j.is_overrun_job
                        && job_counters[j.task_idx].saturating_sub(1) >= job_index
                }) {
                    let task = &taskset.tasks()[job.task_idx];
                    if job.executed >= task.wcet.lo - 1e-6 && job.executed < task.wcet.hi - 1e-9 {
                        switched = true;
                        switch_time = Some(t);
                        mode = Mode::HI;
                        let s_max = config.s_max;

                        schedule.add_event(ScheduleEvent {
                            time: t,
                            task_id,
                            task_name: task.name.clone(),
                            criticality: task.criticality,
                            mode,
                            speed: Speed(s_max),
                            event_type: EventType::ModeSwitch,
                            remaining_execution: task.wcet.hi - job.executed,
                        });

                        for j in ready.iter_mut() {
                            let jt = &taskset.tasks()[j.task_idx];
                            match j.criticality {
                                Criticality::HI => {
                                    j.budget = jt.wcet.hi;
                                    j.priority_deadline = j.release + jt.period;
                                    j.speed = s_max;
                                }
                                Criticality::LO => {
                                    if j.executed >= jt.wcet.hi - 1e-9 {
                                        j.suspended = true;
                                        schedule.add_event(ScheduleEvent {
                                            time: t,
                                            task_id: j.task_id,
                                            task_name: j.name.clone(),
                                            criticality: j.criticality,
                                            mode,
                                            speed: Speed(s_max),
                                            event_type: EventType::JobSuspend,
                                            remaining_execution: 0.0,
                                        });
                                    } else {
                                        j.budget = jt.wcet.hi;
                                        j.speed = s_max;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        release_due(
            t, taskset, &mut next_release, &mut job_counters, &mut ready, mode, config, overrun,
            &mut schedule,
        );

        for j in ready.iter() {
            if !j.suspended && j.executed >= j.budget - 1e-6 {
                schedule.add_event(ScheduleEvent {
                    time: t,
                    task_id: j.task_id,
                    task_name: j.name.clone(),
                    criticality: j.criticality,
                    mode,
                    speed: Speed(j.speed),
                    event_type: EventType::JobComplete,
                    remaining_execution: 0.0,
                });
                // Note: `running_key` is deliberately *not* cleared here.
                // It represents "the job identity last reported to the
                // event log as running", and is only ever updated by the
                // Start/Resume/Idle emission block at the top of the loop,
                // which compares it against the next iteration's freshly
                // computed `sel` to decide whether a JobStart/JobResume/
                // IdleStart event is due. Clearing it here would make that
                // comparison spuriously succeed (both sides `None`) and
                // silently suppress the IdleStart event for any gap that
                // follows a completion with nothing else ready -- which is
                // exactly the common case (see e.g. the [10, 12] idle gap
                // in the Table 1 example's LO-mode trace).
            }
        }
        ready.retain(|j| !j.suspended && j.executed < j.budget - 1e-6);
    }

    schedule.mode_switch_time = switch_time;
    schedule.schedulable = schedule.deadline_misses(taskset).is_empty();
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Criticality, Task, TaskId, TaskSet};

    fn table1_taskset() -> TaskSet {
        TaskSet::new(vec![
            Task::new(TaskId(1), "tau1", Criticality::LO, 8.0, 2.0, 1.0).unwrap(),
            Task::new(TaskId(2), "tau2", Criticality::HI, 12.0, 2.0, 3.0).unwrap(),
            Task::new(TaskId(3), "tau3", Criticality::LO, 16.0, 4.0, 2.0).unwrap(),
        ])
        .unwrap()
    }

    #[test]
    fn table1_utilizations_match_paper() {
        let u = Utilizations::of(&table1_taskset());
        assert!((u.u_lo_lo - 0.5).abs() < 1e-9);
        assert!((u.u_hi_lo - 0.25).abs() < 1e-9);
        assert!((u.u_lo_hi - 1.0 / 6.0).abs() < 1e-9);
        assert!((u.u_hi_hi - 0.25).abs() < 1e-9);
    }

    #[test]
    fn table1_feasible_x_range_matches_paper() {
        let u = Utilizations::of(&table1_taskset());
        let (x_lo, x_up) = lemma3_feasible_x_range(&u).unwrap();
        assert!((x_lo - 1.0 / 3.0).abs() < 1e-9);
        assert!((x_up - 1.0).abs() < 1e-9);
    }

    #[test]
    fn table1_s_lo_at_x_half_matches_paper() {
        // Sec. 5.3 worked example: S_LO = max(0.83, 0.4) = 0.83 at x = 0.5.
        let u = Utilizations::of(&table1_taskset());
        let s = theorem1_s_lo(&u, 0.5).unwrap();
        assert!((s - 0.8333333).abs() < 1e-4);
    }

    #[test]
    fn ea_imc_never_exceeds_imc_without_dvfs_energy() {
        let ts = table1_taskset();
        let sched = EaImcScheduler::default();
        let opt = sched.optimal_config(&ts).unwrap();
        assert!(opt.s_lo <= 1.0);
        assert!(opt.s_lo >= S_CRIT - 1e-9);
    }

    #[test]
    fn schedulability_lemmas_hold_at_feasible_x() {
        let ts = table1_taskset();
        let u = Utilizations::of(&ts);
        assert!(lemma1_lo_mode_schedulable(&u, 0.5));
        assert!(lemma2_hi_mode_schedulable(&u, 0.5));
    }

    /// Regression test for a bug where idle intervals were never emitted
    /// as `IdleStart` events in the simulated schedule (the internal
    /// `running_key` tracker was cleared too early, on job completion,
    /// instead of only when the Start/Resume/Idle emission logic itself
    /// observed a transition). This silently made every gap in the
    /// schedule "invisible" to any energy/visualization code that trusts
    /// the event log, and inflated schedule-based energy figures by
    /// roughly 1/(utilization) since idle time was implicitly charged
    /// full active power.
    #[test]
    fn simulator_emits_idle_events_for_genuine_gaps() {
        let ts = table1_taskset();
        let sched = EaImcScheduler::default();
        let cfg = EaImcConfig {
            x: 0.5,
            s_lo: 1.0,
            s_max: 1.0,
            utilizations: Utilizations::of(&ts),
        };
        let trace = sched
            .simulate(&ts, &cfg, Overrun::None, SimHorizon::OneHyperperiod)
            .unwrap();
        let idle_count = trace
            .events
            .iter()
            .filter(|e| e.event_type == EventType::IdleStart)
            .count();
        // At S_max = 1 the Table 1 task set has total utilization 2/3, so
        // roughly a third of the hyperperiod must be idle, split across
        // several distinct gaps (not just one at the very end).
        assert!(idle_count >= 3, "expected several idle gaps, got {idle_count}");
    }

    /// The energy computed by integrating the power model over the
    /// simulated schedule (only counting busy intervals) should match the
    /// closed-form Eq. (2) energy for the same scenario when the whole
    /// horizon runs in a single mode (no mode switch): both are just
    /// `(P_ind + C_ef*S^m) * (total execution time) / hyperperiod`.
    #[test]
    fn schedule_energy_matches_closed_form_when_no_mode_switch() {
        use crate::energy::{EnergyModel, PowerModel};
        use crate::schedule::Speed;

        let ts = table1_taskset();
        let sched = EaImcScheduler::default();
        let cfg = EaImcConfig {
            x: 0.5,
            s_lo: 1.0,
            s_max: 1.0,
            utilizations: Utilizations::of(&ts),
        };
        let trace = sched
            .simulate(&ts, &cfg, Overrun::None, SimHorizon::OneHyperperiod)
            .unwrap();

        let pm = PowerModel::paper_default();
        let em = EnergyModel::new(pm);
        let result = em.calculate_schedule_energy(&trace, Speed(1.0), Speed(1.0));
        let ne_simulated = result.total_energy / trace.hyperperiod;

        let ne_closed_form = pm.normalized_energy_lo_mode(&ts, 1.0);
        assert!(
            (ne_simulated - ne_closed_form).abs() < 1e-6,
            "simulated NE = {ne_simulated}, closed-form NE = {ne_closed_form}"
        );
        // Also matches the paper's own reported figure for this scenario
        // (Sec. 5.3, energy of Fig. 1's trace): 0.67.
        assert!((ne_simulated - 0.6733).abs() < 1e-3);
    }
}
