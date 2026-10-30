//! Native Rust port derived from original: src/utils/generators.ts
//!
//! Async-generator helpers. The original's `AsyncGenerator` maps to a
//! `futures::Stream` here (`StreamExt::by_ref` provides the same pull
//! semantics); `raceGenerators` becomes a bounded-concurrency merge over
//! `FuturesUnordered` yielding item-by-item as each inner stream makes
//! progress, matching the original's `Promise.race` fairness.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use futures::stream::{SelectAll, StreamExt};
use futures::Stream;
use std::pin::Pin;

/// Returns the last value emitted by the stream, or `None` when empty.
pub async fn last_x<S>(stream: S) -> Option<S::Item>
where
    S: Stream + Unpin,
{
    let mut last = None;
    futures::pin_mut!(stream);
    while let Some(item) = stream.next().await {
        last = Some(item);
    }
    last
}

/// Collects every yielded value into a vector (original `toArray`).
pub async fn to_array<S>(stream: S) -> Vec<S::Item>
where
    S: Stream + Unpin,
{
    let mut out = Vec::new();
    futures::pin_mut!(stream);
    while let Some(item) = stream.next().await {
        out.push(item);
    }
    out
}

/// Bridges a plain list into a stream (original `fromArray`).
pub fn from_array<I>(values: I) -> impl Stream<Item = I::Item>
where
    I: IntoIterator + Send + 'static,
    I::Item: Send + 'static,
{
    futures::stream::iter(values)
}

/// Each active generator remains live until exhausted, and finishing one starts
/// the next queued generator. Values are emitted in readiness order rather
/// than collecting only each source's first available item.
enum MergeEvent<T> {
    Value(Option<T>),
    Finished,
}

fn source_events<S, T>(stream: S) -> Pin<Box<dyn Stream<Item = MergeEvent<T>> + Send>>
where
    S: Stream<Item = Option<T>> + Send + 'static,
    T: Send + 'static,
{
    Box::pin(
        stream
            .map(MergeEvent::Value)
            .chain(futures::stream::once(async { MergeEvent::Finished })),
    )
}

pub async fn all_bounded<S, T>(streams: Vec<S>, concurrency_cap: usize) -> Vec<T>
where
    S: Stream<Item = Option<T>> + Send + 'static,
    T: Send + 'static,
{
    let cap = concurrency_cap.max(1);
    let mut queued = streams.into_iter();
    let mut active: SelectAll<Pin<Box<dyn Stream<Item = MergeEvent<T>> + Send>>> =
        SelectAll::new();
    for _ in 0..cap {
        let Some(stream) = queued.next() else { break };
        active.push(source_events(stream));
    }
    let mut output = Vec::new();
    while let Some(event) = active.next().await {
        match event {
            MergeEvent::Value(Some(value)) => output.push(value),
            MergeEvent::Value(None) => {}
            MergeEvent::Finished => {
                if let Some(stream) = queued.next() {
                    active.push(source_events(stream));
                }
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn last_x_returns_final_item() {
        let s = futures::stream::iter(vec![1, 2, 3]);
        assert_eq!(last_x(s).await, Some(3));
        let empty = futures::stream::iter(Vec::<i32>::new());
        assert_eq!(last_x(empty).await, None);
    }

    #[tokio::test]
    async fn to_array_collects_all() {
        let s = futures::stream::iter(vec!["a", "b"]);
        assert_eq!(to_array(s).await, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn bounded_merge_yields_all_values() {
        let streams = vec![
            futures::stream::iter(vec![Some(1), Some(2), None]),
            futures::stream::iter(vec![Some(10), None]),
        ];
        let mut got = all_bounded(streams, 2).await;
        got.sort();
        assert_eq!(got, vec![1, 2, 10]);
    }
}
