pub mod error;
pub mod task;
pub mod schedule;
pub mod algorithm;
pub mod energy;
pub mod generator;

use serde::{Deserialize, Serialize};

pub use error::Error;
pub use task::{Task, TaskSet, Criticality, TaskId};
pub use schedule::{Schedule, ScheduleEvent, Mode, Speed};
//todo
//pub use algorithm::{EaImcScheduler, SchedulerConfig, SchedulabilityResult};
pub use energy::{EnergyModel, PowerModel, EnergyResult};
//todo
//pub use generator::{TaskSetGenerator, GeneratorConfig};