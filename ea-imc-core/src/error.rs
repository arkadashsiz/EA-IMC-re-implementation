use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid task set: {0}")]
    InvalidTaskSet(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Scheduling failed: {0}")]
    SchedulingFailed(String),

    #[error("Schedulability test failed: {0}")]
    SchedulabilityFailed(String),

    #[error("Invalid speed value: {0} (must be in (0, 1])")]
    InvalidSpeed(f64),

    #[error("Invalid utilization: {0} (must be >= 0)")]
    InvalidUtilization(f64),

    #[error("Simulation error: {0}")]
    SimulationError(String),

    #[error("Energy calculation error: {0}")]
    EnergyError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;