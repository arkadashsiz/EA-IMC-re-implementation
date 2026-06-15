use crate::schedule::{Mode, Speed};
use crate::task::TaskSet;
use crate::{Error};
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerModel {
    pub static_power: f64,
    pub dynamic_coefficient: f64,
    pub voltage_at_max_freq: f64,
}

impl PowerModel {
    pub fn new(static_power: f64, dynamic_coefficient: f64, voltage_at_max_freq: f64) -> Result<Self> {
        if static_power < 0.0 {
            return Err(Error::EnergyError("Static power must be >= 0".into()));
        }
        if dynamic_coefficient <= 0.0 {
            return Err(Error::EnergyError("Dynamic coefficient must be > 0".into()));
        }
        if voltage_at_max_freq <= 0.0 {
            return Err(Error::EnergyError("Voltage at max freq must be > 0".into()));
        }
        Ok(Self {
            static_power,
            dynamic_coefficient,
            voltage_at_max_freq,
        })
    }

    pub fn voltage(&self, speed: Speed) -> f64 {
        self.voltage_at_max_freq * speed.value().sqrt()
    }

    pub fn frequency(&self, speed: Speed) -> f64 {
        speed.value()
    }

    pub fn power(&self, speed: Speed) -> f64 {
        let v = self.voltage(speed);
        let f = self.frequency(speed);
        self.static_power + self.dynamic_coefficient * v * v * f
    }

    pub fn energy(&self, speed: Speed, duration: f64) -> f64 {
        self.power(speed) * duration
    }
}

impl Default for PowerModel {
    fn default() -> Self {
        Self {
            static_power: 0.1,
            dynamic_coefficient: 1.0,
            voltage_at_max_freq: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyResult {
    pub total_energy: f64,
    pub lo_mode_energy: f64,
    pub hi_mode_energy: f64,
    pub idle_energy: f64,
    pub mode_switch_energy: f64,
    pub per_task_energy: std::collections::HashMap<String, f64>,
    pub energy_breakdown: EnergyBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyBreakdown {
    pub static_energy: f64,
    pub dynamic_energy: f64,
    pub by_mode: std::collections::HashMap<Mode, f64>,
    pub by_speed: Vec<(Speed, f64)>,
}

impl EnergyResult {
    pub fn new() -> Self {
        Self {
            total_energy: 0.0,
            lo_mode_energy: 0.0,
            hi_mode_energy: 0.0,
            idle_energy: 0.0,
            mode_switch_energy: 0.0,
            per_task_energy: std::collections::HashMap::new(),
            energy_breakdown: EnergyBreakdown {
                static_energy: 0.0,
                dynamic_energy: 0.0,
                by_mode: std::collections::HashMap::new(),
                by_speed: Vec::new(),
            },
        }
    }
}

impl Default for EnergyResult {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EnergyModel {
    power_model: PowerModel,
}

impl EnergyModel {
    pub fn new(power_model: PowerModel) -> Self {
        Self { power_model }
    }

    pub fn calculate_schedule_energy(
    &self,
    schedule: &crate::schedule::Schedule,
    taskset: &TaskSet,
    slo: Speed,
) -> EnergyResult {
    let mut result = EnergyResult::new();
    let mut last_time = 0.0;
    let mut current_speed = slo;
    let mut current_mode = Mode::LO;

    for event in &schedule.events {
        let duration = event.time - last_time;
        if duration > 1e-9 {
            let energy = self.power_model.energy(current_speed, duration);
            result.total_energy += energy;

            match current_mode {
                Mode::LO => result.lo_mode_energy += energy,
                Mode::HI => result.hi_mode_energy += energy,
            }

            *result.energy_breakdown.by_mode.entry(current_mode).or_default() += energy;
            // Push to vector instead of using entry
            result.energy_breakdown.by_speed.push((current_speed, energy));

            let v = self.power_model.voltage(current_speed);
            let f = self.power_model.frequency(current_speed);
            let static_e = self.power_model.static_power * duration;
            let dynamic_e = self.power_model.dynamic_coefficient * v * v * f * duration;
            result.energy_breakdown.static_energy += static_e;
            result.energy_breakdown.dynamic_energy += dynamic_e;
        }

        if event.event_type == crate::schedule::EventType::ModeSwitch {
            current_mode = Mode::HI;
            current_speed = Speed::MAX;
            result.mode_switch_energy += self.power_model.energy(Speed::MAX, 0.001);
        }

        if event.event_type == crate::schedule::EventType::JobStart
            || event.event_type == crate::schedule::EventType::JobResume
        {
            current_speed = event.speed;
        }

        last_time = event.time;
    }

    result
}
}