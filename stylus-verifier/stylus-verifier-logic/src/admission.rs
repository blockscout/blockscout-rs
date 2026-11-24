// SPDX-License-Identifier: LicenseRef-Blockscout

//! Bounded admission to the build slots: a request takes a free slot, or waits in a queue of
//! limited length for a limited time, or is turned away.

use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Rejected {
    #[error("too many verifications in progress; try again later")]
    QueueFull,
    #[error("no build slot became free in time; try again later")]
    TimedOut,
}

pub struct Admission {
    slots: Semaphore,
    queue: Semaphore,
    max_wait: Duration,
}

impl Admission {
    pub fn new(slots: usize, queue: usize, max_wait: Duration) -> Self {
        Self {
            slots: Semaphore::new(slots),
            queue: Semaphore::new(queue),
            max_wait,
        }
    }

    /// The returned permit holds the slot until it is dropped.
    pub async fn enter(&self) -> Result<SemaphorePermit<'_>, Rejected> {
        // The semaphore is fair: while requests wait, no permit is free for this to take ahead
        // of them.
        if let Ok(slot) = self.slots.try_acquire() {
            return Ok(slot);
        }
        let _place = self.queue.try_acquire().map_err(|_| Rejected::QueueFull)?;
        match tokio::time::timeout(self.max_wait, self.slots.acquire()).await {
            Ok(slot) => Ok(slot.expect("the slots semaphore is never closed")),
            Err(_) => Err(Rejected::TimedOut),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_secs(60);

    #[tokio::test(start_paused = true)]
    async fn rejects_when_the_queue_is_full() {
        let admission = Admission::new(1, 1, WAIT);
        let running = admission.enter().await.unwrap();

        let mut queued = Box::pin(admission.enter());
        assert!(futures_poll(&mut queued).await.is_none());

        assert_eq!(admission.enter().await.unwrap_err(), Rejected::QueueFull);

        drop(running);
        assert!(queued.await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn no_queue_rejects_while_every_slot_is_taken() {
        let admission = Admission::new(2, 0, WAIT);
        let _first = admission.enter().await.unwrap();
        let second = admission.enter().await.unwrap();
        assert_eq!(admission.enter().await.unwrap_err(), Rejected::QueueFull);

        drop(second);
        assert!(admission.enter().await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiting_request_times_out_and_frees_its_place() {
        let admission = Admission::new(1, 1, WAIT);
        let _running = admission.enter().await.unwrap();

        let started = tokio::time::Instant::now();
        assert_eq!(admission.enter().await.unwrap_err(), Rejected::TimedOut);
        assert_eq!(started.elapsed(), WAIT);

        // The place in the queue was given back.
        assert_eq!(admission.enter().await.unwrap_err(), Rejected::TimedOut);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_request_frees_its_place() {
        let admission = Admission::new(1, 1, WAIT);
        let _running = admission.enter().await.unwrap();

        let mut queued = Box::pin(admission.enter());
        assert!(futures_poll(&mut queued).await.is_none());
        drop(queued);

        let mut queued = Box::pin(admission.enter());
        assert!(futures_poll(&mut queued).await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_requests_are_served_in_order() {
        let admission = Admission::new(1, 2, WAIT);
        let running = admission.enter().await.unwrap();

        let mut first = Box::pin(admission.enter());
        let mut second = Box::pin(admission.enter());
        assert!(futures_poll(&mut first).await.is_none());
        assert!(futures_poll(&mut second).await.is_none());

        drop(running);
        assert!(futures_poll(&mut second).await.is_none());
        let first = first.await.unwrap();
        // No slot is left over for a newcomer to take ahead of the one still waiting.
        assert!(admission.slots.try_acquire().is_err());
        drop(first);
        assert!(second.await.is_ok());
    }

    /// Polls the future once; `None` while it is pending.
    async fn futures_poll<F: std::future::Future + Unpin>(future: &mut F) -> Option<F::Output> {
        std::future::poll_fn(|cx| {
            std::task::Poll::Ready(match std::pin::Pin::new(&mut *future).poll(cx) {
                std::task::Poll::Ready(output) => Some(output),
                std::task::Poll::Pending => None,
            })
        })
        .await
    }
}
