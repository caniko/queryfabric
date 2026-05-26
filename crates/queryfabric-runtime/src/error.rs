use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime path not implemented")]
    NotImplemented,
    #[error("isolated execution driver failed: {0}")]
    Driver(#[from] DriverError),
    #[error("runtime execution cancelled")]
    Cancelled,
    #[error("adapter execution failed: {0}")]
    Adapter(String),
}

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("failed to spawn isolated worker: {0}")]
    Spawn(String),
    #[error("isolated worker timed out")]
    Timeout,
    #[error("isolated worker cancelled")]
    Cancelled,
    #[error("isolated worker exited with code {exit_code}: {message}")]
    WorkerFailure { exit_code: i32, message: String },
}
