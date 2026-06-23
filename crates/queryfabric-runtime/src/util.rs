use std::any::Any;
use std::borrow::Cow;
use std::future::Future;
use std::panic::AssertUnwindSafe;

use futures::FutureExt;

/// Spawn a background task with panic logging.
///
/// Catches panics in the spawned future, logs them via `tracing::error!`
/// with the task `name`, and discards the panic payload — the process
/// continues running.
pub fn spawn_traced(name: &'static str, future: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(async move {
        if let Err(payload) = AssertUnwindSafe(future).catch_unwind().await {
            tracing::error!(
                task = name,
                panic = %panic_message(payload.as_ref()),
                "background task panicked"
            );
        }
    });
}

fn panic_message(payload: &(dyn Any + Send)) -> Cow<'static, str> {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        Cow::Borrowed(*message)
    } else if let Some(message) = payload.downcast_ref::<String>() {
        Cow::Owned(message.clone())
    } else {
        Cow::Borrowed("<non-string panic payload>")
    }
}
