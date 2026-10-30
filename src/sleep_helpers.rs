//! Rust cancellation/timer semantics from 2.1.88 src/utils/sleep.ts.
use crate::portable_async_runtime::AbortController;
use anyhow::{bail, Result};
use std::{future::Future, time::Duration};

pub async fn sleep(
    duration: Duration,
    signal: Option<&AbortController>,
    throw_on_abort: bool,
    abort_message: Option<&str>,
) -> Result<()> {
    let Some(signal) = signal else {
        tokio::time::sleep(duration).await;
        return Ok(());
    };
    if signal.aborted() {
        if throw_on_abort || abort_message.is_some() {
            bail!("{}", abort_message.unwrap_or("aborted"));
        }
        return Ok(());
    }
    let mut rx = signal.subscribe();
    tokio::select! {
        _ = tokio::time::sleep(duration) => Ok(()),
        _ = async {
            while rx.changed().await.is_ok() {
                if *rx.borrow() {
                    return;
                }
            }
        } => {
            if throw_on_abort || abort_message.is_some() {
                bail!("{}", abort_message.unwrap_or("aborted"));
            }
            Ok(())
        }
    }
}

/// Unlike tokio::time::timeout(future), timing out this wrapper does not
/// cancel the underlying task. The detached handle continues executing,
/// matching the original Promise.race semantics.
pub async fn with_timeout<F, T>(work: F, timeout: Duration, timeout_message: &str) -> Result<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let mut handle = tokio::spawn(work);
    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(result) => Ok(result?),
        Err(_) => bail!("{timeout_message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn abort_resolves_or_rejects_without_waiting_for_timer() {
        let signal = AbortController::new();
        signal.abort();
        assert!(sleep(Duration::from_secs(60), Some(&signal), false, None)
            .await
            .is_ok());
        assert!(sleep(Duration::from_secs(60), Some(&signal), true, None)
            .await
            .is_err());
        let future_signal = AbortController::new();
        let child = future_signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            child.abort();
        });
        assert!(tokio::time::timeout(
            Duration::from_millis(200),
            sleep(Duration::from_secs(120), Some(&future_signal), false, None)
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn timeout_reports_error_but_does_not_cancel_underlying_work() {
        let finished = Arc::new(AtomicBool::new(false));
        let reference = finished.clone();
        let outcome = with_timeout(
            async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                reference.store(true, Ordering::SeqCst);
            },
            Duration::from_millis(5),
            "Work took too long",
        )
        .await;
        assert!(outcome
            .unwrap_err()
            .to_string()
            .contains("Work took too long"));
        tokio::time::sleep(Duration::from_millis(70)).await;
        assert!(finished.load(Ordering::SeqCst));
    }
}
