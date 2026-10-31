//! Real Rust equivalent of original src/utils/CircularBuffer.ts.
//! Kept in its own production module to make direct-source regression possible.
use anyhow::Result;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct CircularBuffer<T> {
    capacity: usize,
    buffer: VecDeque<T>,
}
impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Result<Self> {
        // Original JS accepts zero-capacity buffers: add is a no-op.
        Ok(Self { capacity, buffer: VecDeque::with_capacity(capacity) })
    }
    pub fn add(&mut self, value: T) {
        if self.capacity == 0 {
            return;
        }
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }
    pub fn add_all(&mut self, values: impl IntoIterator<Item = T>) {
        for value in values {
            self.add(value);
        }
    }
    pub fn get_recent(&self, count: usize) -> Vec<&T> {
        self.buffer.iter().skip(self.buffer.len().saturating_sub(count)).collect()
    }
    pub fn to_array(&self) -> Vec<&T> {
        self.buffer.iter().collect()
    }
    pub fn clear(&mut self) {
        self.buffer.clear()
    }
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_zero_capacity_does_not_throw_and_never_stores_items() {
        let mut buffer = CircularBuffer::new(0).unwrap();
        buffer.add(12);
        buffer.add_all([2, 3]);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.to_array().is_empty());
        assert!(buffer.get_recent(10).is_empty());
    }
    #[test]
    fn original_wrap_and_clear_reuse() {
        let mut buffer = CircularBuffer::new(2).unwrap();
        buffer.add_all([1, 2, 3]);
        assert_eq!(buffer.to_array(), vec![&2, &3]);
        buffer.clear();
        buffer.add_all([9, 10, 11]);
        assert_eq!(buffer.to_array(), vec![&10, &11]);
    }
}
