use crate::task::{Criticality, Task, TaskId};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    LO,
    HI,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::LO => write!(f, "LO"),
            Mode::HI => write!(f, "HI"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Speed(pub f64);

impl Speed {
    pub const MAX: Speed = Speed(1.0);
    pub const MIN: Speed = Speed(0.01);

    pub fn new(value: f64) -> Result<Self, String> {
        if value <= 0.0 || value > 1.0 {
            return Err(format!("Speed must be in (0, 1], got {}", value));
        }
        Ok(Speed(value))
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S={:.3}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEvent {
    pub time: f64,
    pub task_id: TaskId,
    pub task_name: String,
    pub criticality: Criticality,
    pub mode: Mode,
    pub speed: Speed,
    pub event_type: EventType,
    pub remaining_execution: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    JobRelease,
    JobStart,
    JobResume,
    JobPreempt,
    JobComplete,
    JobSuspend,
    ModeSwitch,
    IdleStart,
    IdleEnd,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::JobRelease => write!(f, "RELEASE"),
            EventType::JobStart => write!(f, "START"),
            EventType::JobResume => write!(f, "RESUME"),
            EventType::JobPreempt => write!(f, "PREEMPT"),
            EventType::JobComplete => write!(f, "COMPLETE"),
            EventType::JobSuspend => write!(f, "SUSPEND"),
            EventType::ModeSwitch => write!(f, "MODE_SWITCH"),
            EventType::IdleStart => write!(f, "IDLE_START"),
            EventType::IdleEnd => write!(f, "IDLE_END"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub events: Vec<ScheduleEvent>,
    pub mode_switch_time: Option<f64>,
    pub total_energy: f64,
    pub hyperperiod: f64,
    pub schedulable: bool,
}

impl Schedule {
    pub fn new(hyperperiod: f64) -> Self {
        Self {
            events: Vec::new(),
            mode_switch_time: None,
            total_energy: 0.0,
            hyperperiod,
            schedulable: true,
        }
    }

    pub fn add_event(&mut self, event: ScheduleEvent) {
        if event.event_type == EventType::ModeSwitch {
            self.mode_switch_time = Some(event.time);
        }
        self.events.push(event);
    }

    pub fn events_for_task(&self, task_id: TaskId) -> Vec<&ScheduleEvent> {
        self.events.iter().filter(|e| e.task_id == task_id).collect()
    }

    pub fn mode_switch_events(&self) -> Vec<&ScheduleEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == EventType::ModeSwitch)
            .collect()
    }

    pub fn completion_events(&self) -> Vec<&ScheduleEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == EventType::JobComplete)
            .collect()
    }

    pub fn deadline_misses(&self, taskset: &crate::task::TaskSet) -> Vec<(TaskId, f64, f64)> {
        let mut misses = Vec::new();
        let mut job_deadlines: std::collections::HashMap<TaskId, Vec<f64>> =
            std::collections::HashMap::new();

        for event in &self.events {
            if event.event_type == EventType::JobRelease {
                let deadline = event.time
                    + taskset
                        .task(event.task_id)
                        .map(|t| t.effective_deadline())
                        .unwrap_or(0.0);
                job_deadlines
                    .entry(event.task_id)
                    .or_default()
                    .push(deadline);
            }
        }

        for event in &self.events {
            if event.event_type == EventType::JobComplete {
                if let Some(deadlines) = job_deadlines.get_mut(&event.task_id) {
                    if let Some(deadline) = deadlines.first() {
                        if event.time > *deadline + 1e-9 {
                            misses.push((event.task_id, event.time, *deadline));
                        }
                        deadlines.remove(0);
                    }
                }
            }
        }

        misses
    }
}