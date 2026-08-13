pub mod error;
pub mod task;
pub mod schedule;
pub mod algorithm;
pub mod energy;
pub mod generator;

pub use error::Error;
pub use task::{Task, TaskSet, Criticality, TaskId};
pub use schedule::{Schedule, ScheduleEvent, Mode, Speed};
pub use algorithm::{
    EaImcConfig, EaImcScheduler, Overrun, SimHorizon, Utilizations, S_CRIT, S_MIN,
};
pub use energy::{EnergyModel, PowerModel, EnergyResult};
pub use generator::{GeneratorConfig, TaskSetGenerator};