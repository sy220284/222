//! Native Rust state model of the upstream CustomSelect option map, navigation,
//! and selection hooks (`option-map.ts`, `use-select-navigation.ts`,
//! `use-select-state.ts`). Rendering and keyboard-event wiring are separate.
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption<T> {
    pub label: String,
    pub value: T,
    pub description: Option<String>,
    pub is_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionMapItem<T> {
    pub label: String,
    pub value: T,
    pub description: Option<String>,
    pub previous: Option<usize>,
    pub next: Option<usize>,
    pub index: usize,
}

/// The map preserves the original insertion-order links; duplicate keys
/// replace their lookup entry, as in JavaScript's `Map` constructor.
#[derive(Debug, Clone)]
pub struct OptionMap<T> {
    pub items: Vec<OptionMapItem<T>>,
    lookup: HashMap<T, usize>,
    pub first: Option<usize>,
    pub last: Option<usize>,
}

impl<T: Clone + Eq + Hash> OptionMap<T> {
    pub fn new(options: &[SelectOption<T>]) -> Self {
        let mut lookup = HashMap::with_capacity(options.len());
        let mut items: Vec<OptionMapItem<T>> = Vec::with_capacity(options.len());
        for (index, option) in options.iter().enumerate() {
            if let Some(previous) = items.last_mut() {
                previous.next = Some(index);
            }
            items.push(OptionMapItem {
                label: option.label.clone(),
                value: option.value.clone(),
                description: option.description.clone(),
                previous: index.checked_sub(1),
                next: None,
                index,
            });
            lookup.insert(option.value.clone(), index);
        }
        Self {
            first: (!items.is_empty()).then_some(0),
            last: items.len().checked_sub(1),
            lookup,
            items,
        }
    }

    pub fn len(&self) -> usize {
        self.lookup.len()
    }

    pub fn get(&self, value: &T) -> Option<&OptionMapItem<T>> {
        self.lookup.get(value).map(|index| &self.items[*index])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleOption<T> {
    pub option: SelectOption<T>,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct SelectNavigation<T> {
    pub options: Vec<SelectOption<T>>,
    pub option_map: OptionMap<T>,
    pub visible_option_count: usize,
    pub focused_value: Option<T>,
    pub visible_from_index: usize,
    pub visible_to_index: usize,
}

impl<T: Clone + Eq + Hash> SelectNavigation<T> {
    /// The hook's default is 5; the reducer's initializer clips it to the
    /// length of the supplied options.
    pub fn new(
        options: Vec<SelectOption<T>>,
        visible_option_count: usize,
        focus: Option<T>,
    ) -> Self {
        Self::reset_state(options, visible_option_count, focus, None)
    }

    fn reset_state(
        options: Vec<SelectOption<T>>,
        visible_option_count: usize,
        initial_focus: Option<T>,
        current_viewport: Option<(usize, usize)>,
    ) -> Self {
        let count = visible_option_count.min(options.len());
        let option_map = OptionMap::new(&options);
        let item = initial_focus
            .as_ref()
            .and_then(|value| option_map.get(value));
        let focused_value = item
            .map(|entry| entry.value.clone())
            .or_else(|| option_map.first.map(|i| option_map.items[i].value.clone()));
        let mut from = 0;
        let mut to = count;
        if let Some(focused) = item {
            let index = focused.index;
            if let Some((old_from, old_to)) = current_viewport {
                if index >= old_from && index < old_to {
                    from = old_from;
                    to = option_map.len().min(old_to);
                } else if index < old_from {
                    from = index;
                    to = option_map.len().min(from + count);
                } else {
                    to = option_map.len().min(index + 1);
                    from = to.saturating_sub(count);
                }
            } else if index >= count {
                to = option_map.len().min(index + 1);
                from = to.saturating_sub(count);
            }
            from = from.min(option_map.len().saturating_sub(1));
            to = option_map.len().min(to.max(count));
        }
        Self {
            options,
            option_map,
            visible_option_count: count,
            focused_value,
            visible_from_index: from,
            visible_to_index: to,
        }
    }

    fn focused_item(&self) -> Option<&OptionMapItem<T>> {
        self.focused_value
            .as_ref()
            .and_then(|value| self.option_map.get(value))
    }

    pub fn validated_focus(&self) -> Option<&T> {
        self.focused_value
            .as_ref()
            .filter(|value| self.options.iter().any(|option| &option.value == *value))
            .or_else(|| self.options.first().map(|option| &option.value))
    }

    pub fn focused_index(&self) -> usize {
        self.validated_focus()
            .and_then(|value| {
                self.options
                    .iter()
                    .position(|option| &option.value == value)
            })
            .map_or(0, |index| index + 1)
    }

    pub fn is_in_input(&self) -> bool {
        self.validated_focus()
            .and_then(|value| self.options.iter().find(|option| &option.value == value))
            .is_some_and(|option| option.is_input)
    }

    pub fn visible_options(&self) -> Vec<VisibleOption<T>> {
        self.options
            .iter()
            .enumerate()
            .skip(self.visible_from_index)
            .take(
                self.visible_to_index
                    .saturating_sub(self.visible_from_index),
            )
            .map(|(index, option)| VisibleOption {
                option: option.clone(),
                index,
            })
            .collect()
    }

    pub fn focus_next_option(&mut self) {
        let Some(item) = self.focused_item() else {
            return;
        };
        let old_next = item.next;
        let Some(next) = old_next.or(self.option_map.first) else {
            return;
        };
        let target = self.option_map.items[next].clone();
        if old_next.is_none() {
            self.focused_value = Some(target.value);
            self.visible_from_index = 0;
            self.visible_to_index = self.visible_option_count;
            return;
        }
        if target.index >= self.visible_to_index {
            self.visible_to_index = self.option_map.len().min(self.visible_to_index + 1);
            self.visible_from_index = self
                .visible_to_index
                .saturating_sub(self.visible_option_count);
        }
        self.focused_value = Some(target.value);
    }

    pub fn focus_previous_option(&mut self) {
        let Some(item) = self.focused_item() else {
            return;
        };
        let old_previous = item.previous;
        let Some(previous) = old_previous.or(self.option_map.last) else {
            return;
        };
        let target = self.option_map.items[previous].clone();
        if old_previous.is_none() {
            self.visible_to_index = self.option_map.len();
            self.visible_from_index = self
                .visible_to_index
                .saturating_sub(self.visible_option_count);
            self.focused_value = Some(target.value);
            return;
        }
        if target.index <= self.visible_from_index {
            self.visible_from_index = self.visible_from_index.saturating_sub(1);
            self.visible_to_index = self.visible_from_index + self.visible_option_count;
        }
        self.focused_value = Some(target.value);
    }

    pub fn focus_next_page(&mut self) {
        let Some(item) = self.focused_item() else {
            return;
        };
        let target_index =
            (item.index + self.visible_option_count).min(self.option_map.len().saturating_sub(1));
        let Some(target) = self.option_map.items.get(target_index) else {
            return;
        };
        self.focused_value = Some(target.value.clone());
        self.visible_to_index = self.option_map.len().min(target.index + 1);
        self.visible_from_index = self
            .visible_to_index
            .saturating_sub(self.visible_option_count);
    }

    pub fn focus_previous_page(&mut self) {
        let Some(item) = self.focused_item() else {
            return;
        };
        let target_index = item.index.saturating_sub(self.visible_option_count);
        let Some(target) = self.option_map.items.get(target_index) else {
            return;
        };
        self.focused_value = Some(target.value.clone());
        self.visible_from_index = target.index;
        self.visible_to_index = self
            .option_map
            .len()
            .min(self.visible_from_index + self.visible_option_count);
    }

    /// Unknown and missing values leave state unchanged, as in `set-focus`.
    pub fn focus_option(&mut self, value: Option<&T>) {
        let Some(value) = value else { return };
        let Some(item) = self.option_map.get(value) else {
            return;
        };
        if self.focused_value.as_ref() == Some(value) {
            return;
        }
        let index = item.index;
        if index < self.visible_from_index {
            self.visible_from_index = index;
            self.visible_to_index = self.option_map.len().min(index + self.visible_option_count);
        } else if index >= self.visible_to_index {
            self.visible_to_index = self.option_map.len().min(index + 1);
            self.visible_from_index = self
                .visible_to_index
                .saturating_sub(self.visible_option_count);
        }
        self.focused_value = Some(value.clone());
    }

    /// Same-options updates preserve focus; changes rebuild the map and keep
    /// the old viewport when it remains valid.
    pub fn update_options(&mut self, options: Vec<SelectOption<T>>, focus_value: Option<T>) {
        if self.options == options {
            self.focus_option(focus_value.as_ref());
            return;
        }
        let preferred = focus_value.or_else(|| self.focused_value.clone());
        *self = Self::reset_state(
            options,
            self.visible_option_count,
            preferred,
            Some((self.visible_from_index, self.visible_to_index)),
        );
    }
}

/// Local selection state intentionally does not invoke the external onChange
/// callback on focus or selection: upstream `selectFocusedOption` only sets
/// the internal selected value. Event callback wiring remains with the caller.
#[derive(Debug, Clone)]
pub struct SelectState<T> {
    pub navigation: SelectNavigation<T>,
    pub value: Option<T>,
}

impl<T: Clone + Eq + Hash> SelectState<T> {
    pub fn new(
        options: Vec<SelectOption<T>>,
        visible_count: usize,
        default: Option<T>,
        focus: Option<T>,
    ) -> Self {
        Self {
            navigation: SelectNavigation::new(options, visible_count, focus),
            value: default,
        }
    }

    pub fn select_focused_option(&mut self) {
        self.value = self.navigation.focused_value.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options(count: usize) -> Vec<SelectOption<usize>> {
        (0..count)
            .map(|value| SelectOption {
                label: format!("Option {value}"),
                value,
                description: None,
                is_input: value == 3,
            })
            .collect()
    }

    #[test]
    fn option_map_preserves_order_links_and_overwrites_duplicate_lookup() {
        let mut entries = options(3);
        entries.push(entries[1].clone());
        let map = OptionMap::new(&entries);
        assert_eq!(map.len(), 3);
        assert_eq!(map.first, Some(0));
        assert_eq!(map.last, Some(3));
        assert_eq!(map.get(&1).unwrap().index, 3);
        assert_eq!(map.items[1].previous, Some(0));
        assert_eq!(map.items[1].next, Some(2));
    }

    #[test]
    fn default_first_focus_and_empty_options() {
        let nav = SelectNavigation::new(options(8), 5, None);
        assert_eq!(nav.focused_value, Some(0));
        assert_eq!(nav.focused_index(), 1);
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (0, 5));
        assert_eq!(nav.visible_options().len(), 5);
        let empty = SelectNavigation::<usize>::new(Vec::new(), 5, None);
        assert_eq!(empty.focused_value, None);
        assert_eq!(empty.focused_index(), 0);
        assert!(empty.visible_options().is_empty());
    }

    #[test]
    fn next_previous_wrap_and_single_row_scrolling() {
        let mut nav = SelectNavigation::new(options(8), 3, None);
        for _ in 0..3 {
            nav.focus_next_option();
        }
        assert_eq!(nav.focused_value, Some(3));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (1, 4));
        assert!(nav.is_in_input());
        nav.focus_previous_option();
        assert_eq!(nav.focused_value, Some(2));
        nav.focus_option(Some(&0));
        nav.focus_previous_option();
        assert_eq!(nav.focused_value, Some(7));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (5, 8));
        nav.focus_next_option();
        assert_eq!(nav.focused_value, Some(0));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (0, 3));
    }

    #[test]
    fn page_navigation_clamps_at_edges_and_does_not_wrap() {
        let mut nav = SelectNavigation::new(options(12), 5, None);
        nav.focus_next_page();
        assert_eq!(nav.focused_value, Some(5));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (1, 6));
        nav.focus_next_page();
        assert_eq!(nav.focused_value, Some(10));
        nav.focus_next_page();
        assert_eq!(nav.focused_value, Some(11));
        nav.focus_next_page();
        assert_eq!(nav.focused_value, Some(11));
        nav.focus_previous_page();
        assert_eq!(nav.focused_value, Some(6));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (6, 11));
    }

    #[test]
    fn direct_focus_uses_minimal_scroll_and_ignores_unknown() {
        let mut nav = SelectNavigation::new(options(10), 4, None);
        nav.focus_option(Some(&7));
        assert_eq!(
            (
                nav.focused_value,
                nav.visible_from_index,
                nav.visible_to_index
            ),
            (Some(7), 4, 8)
        );
        nav.focus_option(Some(&3));
        assert_eq!(
            (
                nav.focused_value,
                nav.visible_from_index,
                nav.visible_to_index
            ),
            (Some(3), 3, 7)
        );
        nav.focus_option(Some(&999));
        assert_eq!(
            (
                nav.focused_value,
                nav.visible_from_index,
                nav.visible_to_index
            ),
            (Some(3), 3, 7)
        );
    }

    #[test]
    fn changed_options_preserve_visible_focus_or_fall_back_to_first() {
        let mut nav = SelectNavigation::new(options(9), 4, Some(6));
        assert_eq!((nav.visible_from_index, nav.visible_to_index), (3, 7));
        let mut modified = options(9);
        modified[1].label = "Changed".to_owned();
        nav.update_options(modified, None);
        assert_eq!(
            (
                nav.focused_value,
                nav.visible_from_index,
                nav.visible_to_index
            ),
            (Some(6), 3, 7)
        );
        nav.update_options(options(3), None);
        assert_eq!(nav.focused_value, Some(0));
        assert_eq!(nav.focused_index(), 1);
    }

    #[test]
    fn selection_remains_distinct_from_navigation_and_callbacks() {
        let mut state = SelectState::new(options(7), 3, Some(2), None);
        assert_eq!(state.value, Some(2));
        state.navigation.focus_option(Some(&3));
        assert_eq!(state.value, Some(2));
        state.select_focused_option();
        assert_eq!(state.value, Some(3));
    }
}
