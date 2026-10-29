//! Generic array primitives with Rust type-safe semantics.
use std::{collections::HashSet, hash::Hash};

pub fn intersperse<T: Clone, F: FnMut(usize) -> T>(items: &[T], mut separator: F) -> Vec<T> {
    let mut result = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            result.push(separator(index));
        }
        result.push(item.clone());
    }
    result
}
pub fn count<T, F: FnMut(&T) -> bool>(items: &[T], mut pred: F) -> usize {
    items.iter().filter(|item| pred(item)).count()
}
pub fn uniq<T: Eq + Hash + Clone>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersperse_indexes_and_empty_inputs() {
        assert_eq!(intersperse::<i32, _>(&[], |i| i as i32), Vec::<i32>::new());
        assert_eq!(intersperse(&[5], |i| i as i32), vec![5]);
        assert_eq!(
            intersperse(&[7, 8, 9], |i| -(i as i32)),
            vec![7, -1, 8, -2, 9]
        );
    }
    #[test]
    fn count_truth_predicates_and_uniqueness() {
        assert_eq!(count(&[1, 2, 3, 4], |x| x % 2 == 0), 2);
        assert_eq!(count(&[] as &[i32], |_| true), 0);
        assert_eq!(uniq([3, 1, 3, 2, 1, 4]), vec![3, 1, 2, 4]);
        assert_eq!(uniq(Vec::<i32>::new()), Vec::<i32>::new());
    }
}
