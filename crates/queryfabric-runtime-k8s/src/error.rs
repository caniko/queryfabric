use queryfabric::DriverError;

pub type DriverResult<T> = Result<T, DriverError>;

pub(crate) fn spawn_error(context: impl AsRef<str>, error: impl std::fmt::Display) -> DriverError {
    DriverError::Spawn(format!("{}: {error}", context.as_ref()))
}

pub(crate) fn spawn_message(message: impl Into<String>) -> DriverError {
    DriverError::Spawn(message.into())
}

pub(crate) fn tonic_to_runtime(error: tonic::Status) -> queryfabric::RuntimeError {
    queryfabric::RuntimeError::Driver(DriverError::WorkerFailure {
        exit_code: 1,
        message: format!("worker Flight stream failed: {error}"),
    })
}

pub(crate) fn flight_to_runtime(
    error: arrow_flight::error::FlightError,
) -> queryfabric::RuntimeError {
    queryfabric::RuntimeError::Driver(DriverError::WorkerFailure {
        exit_code: 1,
        message: format!("worker Flight decode failed: {error}"),
    })
}
