use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Criticality {
    LO,
    HI,
}

impl fmt::Display for Criticality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Criticality::LO => write!(f, "LO"),
            Criticality::HI => write!(f, "HI"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub usize);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "τ{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wcet {
    pub lo: f64,
    pub hi: f64,
}

impl Wcet {
    /// Construct a WCET pair without enforcing an ordering between `lo` and `hi`.
    ///
    /// Per the IMC task model (Zhang 2023, Sec. 3.1): for a HI-criticality task
    /// `C_i(LO) <= C_i(HI)` (execution time increases after the mode switch),
    /// whereas for a LO-criticality task `C_i(LO) >= C_i(HI)` (the task receives
    /// *degraded* service in HI mode). Because the correct ordering depends on
    /// the task's criticality level, it is validated in [`Task::new`] instead of
    /// here.
    pub fn new(lo: f64, hi: f64) -> Result<Self> {
        if lo < 0.0 || hi < 0.0 {
            return Err(Error::InvalidTaskSet("WCET values must be non-negative".into()));
        }
        Ok(Self { lo, hi })
    }

    pub fn utilization(&self, period: f64, speed: f64) -> f64 {
        if speed <= 0.0 || period <= 0.0 {
            return f64::INFINITY;
        }
        self.lo / (period * speed)
    }

    pub fn utilization_hi(&self, period: f64, speed: f64) -> f64 {
        if speed <= 0.0 || period <= 0.0 {
            return f64::INFINITY;
        }
        self.hi / (period * speed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub criticality: Criticality,
    pub period: f64,
    pub wcet: Wcet,
    pub deadline: Option<f64>,
}

impl Task {
    pub fn new(
        id: TaskId,
        name: impl Into<String>,
        criticality: Criticality,
        period: f64,
        wcet_lo: f64,
        wcet_hi: f64,
    ) -> Result<Self> {
        if period <= 0.0 {
            return Err(Error::InvalidTaskSet("Period must be positive".into()));
        }
        let wcet = Wcet::new(wcet_lo, wcet_hi)?;
        match criticality {
            Criticality::HI => {
                if wcet.hi < wcet.lo {
                    return Err(Error::InvalidTaskSet(format!(
                        "HI task {}: C(HI)={} must be >= C(LO)={} (execution grows in HI mode)",
                        id, wcet.hi, wcet.lo
                    )));
                }
            }
            Criticality::LO => {
                if wcet.hi > wcet.lo {
                    return Err(Error::InvalidTaskSet(format!(
                        "LO task {}: C(HI)={} must be <= C(LO)={} (degraded service in HI mode)",
                        id, wcet.hi, wcet.lo
                    )));
                }
            }
        }
        let deadline = None;
        Ok(Self {
            id,
            name: name.into(),
            criticality,
            period,
            wcet,
            deadline,
        })
    }

    pub fn with_deadline(mut self, deadline: f64) -> Result<Self> {
        if deadline <= 0.0 {
            return Err(Error::InvalidTaskSet("Deadline must be positive".into()));
        }
        self.deadline = Some(deadline);
        Ok(self)
    }

    pub fn effective_deadline(&self) -> f64 {
        self.deadline.unwrap_or(self.period)
    }

    pub fn utilization_lo(&self, speed: f64) -> f64 {
        self.wcet.utilization(self.period, speed)
    }

    pub fn utilization_hi(&self, speed: f64) -> f64 {
        self.wcet.utilization_hi(self.period, speed)
    }

    pub fn is_hi(&self) -> bool {
        self.criticality == Criticality::HI
    }

    pub fn is_lo(&self) -> bool {
        self.criticality == Criticality::LO
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSet {
    tasks: Vec<Task>,
    id_map: HashMap<TaskId, usize>,
}

impl TaskSet {
    pub fn new(tasks: Vec<Task>) -> Result<Self> {
        if tasks.is_empty() {
            return Err(Error::InvalidTaskSet("Task set cannot be empty".into()));
        }

        let mut id_map = HashMap::new();
        for (idx, task) in tasks.iter().enumerate() {
            if id_map.contains_key(&task.id) {
                return Err(Error::InvalidTaskSet(format!(
                    "Duplicate task ID: {}",
                    task.id
                )));
            }
            id_map.insert(task.id, idx);
        }

        Ok(Self { tasks, id_map })
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.id_map.get(&id).map(|&idx| &self.tasks[idx])
    }

    pub fn hi_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.is_hi()).collect()
    }

    pub fn lo_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.is_lo()).collect()
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn total_utilization_lo(&self, speed: f64) -> f64 {
        self.tasks.iter().map(|t| t.utilization_lo(speed)).sum()
    }

    pub fn total_utilization_hi(&self, speed: f64) -> f64 {
        self.tasks.iter().map(|t| t.utilization_hi(speed)).sum()
    }

    pub fn hi_utilization_lo(&self, speed: f64) -> f64 {
        self.hi_tasks().iter().map(|t| t.utilization_lo(speed)).sum()
    }

    pub fn lo_utilization_lo(&self, speed: f64) -> f64 {
        self.lo_tasks().iter().map(|t| t.utilization_lo(speed)).sum()
    }

    /// U^{HI}_{HI}(Γ): sum of HI-mode utilization of HI-criticality tasks.
    pub fn hi_utilization_hi(&self, speed: f64) -> f64 {
        self.hi_tasks().iter().map(|t| t.utilization_hi(speed)).sum()
    }

    /// U^{HI}_{LO}(Γ): sum of HI-mode (degraded) utilization of LO-criticality tasks.
    pub fn lo_utilization_hi(&self, speed: f64) -> f64 {
        self.lo_tasks().iter().map(|t| t.utilization_hi(speed)).sum()
    }

    /// Hyper-period of the task set, i.e. LCM of all periods. Periods are
    /// rounded to the nearest integer millisecond-like unit for the LCM
    /// computation, which is exact for the integer-period task sets used
    /// throughout the paper (Tables 1-3).
    pub fn hyperperiod(&self) -> f64 {
        fn gcd(a: u64, b: u64) -> u64 {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        fn lcm(a: u64, b: u64) -> u64 {
            a / gcd(a, b) * b
        }
        let mut hp: u64 = 1;
        for t in &self.tasks {
            let p = t.period.round().max(1.0) as u64;
            hp = lcm(hp, p);
        }
        hp as f64
    }
}

impl TryFrom<Vec<Task>> for TaskSet {
    type Error = Error;

    fn try_from(tasks: Vec<Task>) -> Result<Self> {
        Self::new(tasks)
    }
}