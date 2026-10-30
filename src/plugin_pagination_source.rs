//! Rust port of src/commands/plugin/usePagination.ts.
//! The UI owns the selected index; this state retains the source hook's
//! scroll offset across renders.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageWindow {
    pub current_page: usize,
    pub total_pages: usize,
    pub start_index: usize,
    pub end_index: usize,
    pub needs_pagination: bool,
    pub page_size: usize,
    pub scroll_current: usize,
    pub scroll_total: usize,
    pub can_scroll_up: bool,
    pub can_scroll_down: bool,
}

impl PageWindow {
    pub fn visible_items<'a, T>(&self, items: &'a [T]) -> &'a [T] {
        if !self.needs_pagination {
            return items;
        }
        let from = self.start_index.min(items.len());
        let to = self.end_index.min(items.len());
        &items[from..to.max(from)]
    }

    pub fn to_actual_index(&self, visible_index: usize) -> usize {
        self.start_index + visible_index
    }

    pub fn is_on_current_page(&self, actual_index: usize) -> bool {
        actual_index >= self.start_index && actual_index < self.end_index
    }

    pub fn handle_page_navigation(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Default)]
pub struct PaginationState {
    scroll_offset: usize,
}

impl PaginationState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        total_items: usize,
        max_visible: usize,
        selected_index: usize,
    ) -> PageWindow {
        assert!(max_visible > 0, "maxVisible must be greater than zero");
        let needs_pagination = total_items > max_visible;
        let scroll_offset = if !needs_pagination {
            0
        } else if selected_index < self.scroll_offset {
            self.scroll_offset = selected_index;
            selected_index
        } else if selected_index >= self.scroll_offset.saturating_add(max_visible) {
            self.scroll_offset = selected_index - max_visible + 1;
            self.scroll_offset
        } else {
            self.scroll_offset = self.scroll_offset.min(total_items - max_visible);
            self.scroll_offset
        };
        PageWindow {
            current_page: scroll_offset / max_visible,
            total_pages: total_items.div_ceil(max_visible).max(1),
            start_index: scroll_offset,
            end_index: scroll_offset.saturating_add(max_visible).min(total_items),
            needs_pagination,
            page_size: max_visible,
            scroll_current: selected_index.saturating_add(1),
            scroll_total: total_items,
            can_scroll_up: scroll_offset > 0,
            can_scroll_down: scroll_offset.saturating_add(max_visible) < total_items,
        }
    }

    pub fn handle_selection_change(new_index: i64, total_items: usize) -> usize {
        new_index.max(0).min(total_items.saturating_sub(1) as i64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_scroll_keeps_selection_visible_and_retains_offset() {
        let mut p = PaginationState::new();
        let s = p.update(12, 5, 0);
        assert_eq!(
            (s.start_index, s.end_index, s.current_page, s.total_pages),
            (0, 5, 0, 3)
        );
        assert_eq!(
            s.visible_items(&(0..12).collect::<Vec<_>>()),
            &[0, 1, 2, 3, 4]
        );
        let s = p.update(12, 5, 6);
        assert_eq!((s.start_index, s.end_index), (2, 7));
        assert!(s.is_on_current_page(6));
        assert_eq!(s.to_actual_index(1), 3);
        assert!(s.can_scroll_up && s.can_scroll_down);
        assert!(!s.handle_page_navigation());
        let s = p.update(12, 5, 4);
        assert_eq!(s.start_index, 2);
        let s = p.update(12, 5, 0);
        assert_eq!(s.start_index, 0);
    }
    #[test]
    fn changing_item_count_clamps_offset_and_uses_source_ref_behavior() {
        let mut p = PaginationState::new();
        assert_eq!(p.update(20, 5, 18).start_index, 14);
        assert_eq!(p.update(8, 5, 4).start_index, 4);
        let small = p.update(2, 5, 0);
        assert_eq!(
            (small.start_index, small.end_index, small.total_pages),
            (0, 2, 1)
        );
        assert!(!small.needs_pagination);
        assert_eq!(small.visible_items(&[42, 43]), &[42, 43]);
        assert_eq!(p.update(20, 5, 4).start_index, 4);
    }
    #[test]
    fn selection_changes_and_empty_items() {
        assert_eq!(PaginationState::handle_selection_change(-9, 4), 0);
        assert_eq!(PaginationState::handle_selection_change(9, 4), 3);
        assert_eq!(PaginationState::handle_selection_change(3, 0), 0);
        let mut p = PaginationState::new();
        let empty = p.update(0, 5, 0);
        assert_eq!(empty.total_pages, 1);
        assert_eq!(empty.scroll_current, 1);
        assert_eq!(empty.scroll_total, 0);
    }
}
