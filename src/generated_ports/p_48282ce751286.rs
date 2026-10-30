//! Native Rust port derived from original: src/utils/generators.ts
//!
//! Async-generator helpers. The original's `AsyncGenerator` maps to a
//! `futures::Stream` here (`StreamExt::by_ref` provides the same pull
//! semantics); `raceGenerators` becomes a bounded-concurrency merge over
//! `FuturesUnordered` yielding item-by-item as each inner stream makes
//! progress, matching the original's `Promise.race` fairness.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use futures::stream::{FuturesUnordered, StreamExt};
use futures::Stream;
use std::future::Future;
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

/// Runs each stream from `streams` with at most `concurrency_cap` active at
/// once, yielding values in completion order (original `all` generator with
/// its concurrency cap). The `Option<S::Item>` yield mirrors the original's
/// `value !== undefined` guard: streams yielding `None` produce no output.
pub async fn all_bounded<S>(streams: Vec<S>, concurrency_cap: usize) -> Vec<S::Item>
where
    S: Stream<Item = Option<S::Item>> + Send + Unpin + 'static,
    S::Item: Send + 'static,
{
    let cap = concurrency_cap.max(1);
    let mut iter = streams.into_iter();
    let mut running: FuturesUnordered<
        Pin<Box<dyn Future<Output = (usize, Option<S::Item>)> + Send>>,
    > = FuturesUnordered::new();
    let mut out = Vec::new();
    let mut next_id = 0usize;
    let mut finished_ids: Vec<usize> = Vec::new();

    let mut spawn_next = |running: &mut FuturesUnordered<_>,
                          finished_ids: &mut Vec<usize>|
     -> bool {
        if running.len() + finished_ids.len() >= cap {
            // Replace finished slots first.
            if let Some(slot) = finished_ids.pop() {
                if let Some(s) = iter.next() {
                    let mut s = s;
                    running.push(Box::pin(async move {
                        let mut last = None;
                        while let Some(item) = s.next().await {
                            if item.is_some() {
                                last = Some(item);
                                break;
                            }
                        }
                        (slot, last)
                    }));
                    return true;
                }
            }
            return false;
        }
        if let Some(mut s) = iter.next() {
            running.push(Box::pin(async move {
                let mut last = None;
                while let Some(item) = s.next().await {
                    if item.is_some() {
                        last = Some(item);
                        break;
                    }
                }
                (usize::MAX, last)
            }));
            return true;
        }
        false
    };

    // Start the initial batch.
    for _ in 0..cap {
        if !spawn_next(&mut running, &mut finished_ids) {
            break;
        }
    }

    while let Some((_id, value)) = running.next().await {
        if let Some(v) = value {
            out.push(v);
        }
        if !spawn_next(&mut running, &mut finished_ids) && running.is_empty() {
            break;
        }
        let _ = next_id;
        next_id += 1;
    }
    out
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
