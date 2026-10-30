//! Stateful Rust port of CustomSelect/use-multi-select-state.ts.
//! External React callbacks and Ink event dispatch are represented as actions:
//! the caller can deliver onChange/onFocus/onSubmit/onCancel in original order.
use std::collections::HashMap;
use std::hash::Hash;

use crate::custom_select_source::{SelectNavigation, SelectOption};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiOption<T> {
    pub option: SelectOption<T>,
    pub initial_value: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MultiKey {
    pub tab: bool,
    pub shift: bool,
    pub control: bool,
    pub up: bool,
    pub down: bool,
    pub page_up: bool,
    pub page_down: bool,
    pub enter: bool,
    pub escape: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiAction<T> {
    None,
    FocusMoved(Option<T>),
    SubmitButtonFocused,
    SelectionChanged(Vec<T>),
    InputUpdated {
        value: T,
        text: String,
        selected: Vec<T>,
    },
    Submitted(Vec<T>),
    CancelledStopPropagation,
    UpFromFirst,
    DownFromLast,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MultiConfig {
    pub disabled: bool,
    pub hide_indexes: bool,
    pub submit_button: bool,
    pub has_on_submit: bool,
    pub has_on_up_from_first: bool,
    pub has_on_down_from_last: bool,
    pub initial_focus_last: bool,
}

#[derive(Debug, Clone)]
pub struct MultiSelect<T> {
    pub navigation: SelectNavigation<T>,
    pub selected_values: Vec<T>,
    pub input_values: HashMap<T, String>,
    pub is_submit_focused: bool,
    pub config: MultiConfig,
    options: Vec<MultiOption<T>>,
}

impl<T: Clone + Eq + Hash> MultiSelect<T> {
    pub fn new(
        options: Vec<MultiOption<T>>,
        default_values: Vec<T>,
        visible_count: usize,
        focus_value: Option<T>,
        config: MultiConfig,
    ) -> Self {
        let mut input_values = HashMap::new();
        for item in &options {
            if item.option.is_input {
                if let Some(initial) = item.initial_value.as_ref().filter(|v| !v.is_empty()) {
                    input_values.insert(item.option.value.clone(), initial.clone());
                }
            }
        }
        let initial_focus = if config.initial_focus_last {
            options.last().map(|v| v.option.value.clone())
        } else {
            focus_value
        };
        let navigation = SelectNavigation::new(
            options.iter().map(|item| item.option.clone()).collect(),
            visible_count,
            initial_focus,
        );
        Self {
            navigation,
            selected_values: default_values,
            input_values,
            is_submit_focused: false,
            config,
            options,
        }
    }

    /// Deep-equal option lists do not reset selection. When the supplied list
    /// actually changes, the upstream hook resets selectedValues to defaultValue,
    /// but retains the inputValues map (which was only initialized on mount).
    pub fn update_options(
        &mut self,
        new_options: Vec<MultiOption<T>>,
        defaults: Vec<T>,
        focus: Option<T>,
    ) {
        if self.options != new_options {
            self.selected_values = defaults;
            self.options = new_options;
        }
        self.navigation.update_options(
            self.options
                .iter()
                .map(|item| item.option.clone())
                .collect(),
            focus,
        );
    }

    pub fn update_input_value(&mut self, value: T, text: String) -> MultiAction<T> {
        self.input_values.insert(value.clone(), text.clone());
        if !text.is_empty() {
            if !self.selected_values.contains(&value) {
                self.selected_values.push(value.clone());
            }
        } else {
            self.selected_values.retain(|v| *v != value);
        }
        // Upstream always invokes onChange for input edits, even if the selection
        // array is unchanged. Caller must also invoke the input option's onChange.
        MultiAction::InputUpdated {
            value,
            text,
            selected: self.selected_values.clone(),
        }
    }

    fn toggle(&mut self, value: T) -> MultiAction<T> {
        if self.selected_values.contains(&value) {
            self.selected_values.retain(|v| *v != value);
        } else {
            self.selected_values.push(value);
        }
        MultiAction::SelectionChanged(self.selected_values.clone())
    }

    fn moved(&self) -> MultiAction<T> {
        MultiAction::FocusMoved(self.navigation.focused_value.clone())
    }

    pub fn handle_input(&mut self, input: &str, key: MultiKey) -> MultiAction<T> {
        if self.config.disabled {
            return MultiAction::None;
        }
        let focused = self.navigation.focused_value.clone();
        let is_input = self.navigation.is_in_input();
        if is_input
            && !(key.up
                || key.down
                || key.escape
                || key.tab
                || key.enter
                || (key.control && (input == "n" || input == "p")))
        {
            return MultiAction::None;
        }
        let first = self.options.first().map(|item| &item.option.value);
        let last = self.options.last().map(|item| &item.option.value);
        let has_button = self.config.submit_button && self.config.has_on_submit;

        if key.tab && !key.shift {
            if has_button && focused.as_ref() == last && !self.is_submit_focused {
                self.is_submit_focused = true;
                return MultiAction::SubmitButtonFocused;
            }
            if !self.is_submit_focused {
                self.navigation.focus_next_option();
                return self.moved();
            }
            return MultiAction::None;
        }
        if key.tab && key.shift {
            if has_button && self.is_submit_focused {
                self.is_submit_focused = false;
                self.navigation.focus_option(last);
            } else {
                self.navigation.focus_previous_option();
            }
            return self.moved();
        }
        if key.down || (key.control && input == "n") || (!key.control && !key.shift && input == "j")
        {
            if self.is_submit_focused && self.config.has_on_down_from_last {
                return MultiAction::DownFromLast;
            }
            if has_button && focused.as_ref() == last && !self.is_submit_focused {
                self.is_submit_focused = true;
                return MultiAction::SubmitButtonFocused;
            }
            if !self.config.submit_button
                && self.config.has_on_down_from_last
                && focused.as_ref() == last
            {
                return MultiAction::DownFromLast;
            }
            if !self.is_submit_focused {
                self.navigation.focus_next_option();
                return self.moved();
            }
            return MultiAction::None;
        }
        if key.up || (key.control && input == "p") || (!key.control && !key.shift && input == "k") {
            if has_button && self.is_submit_focused {
                self.is_submit_focused = false;
                self.navigation.focus_option(last);
            } else if self.config.has_on_up_from_first && focused.as_ref() == first {
                return MultiAction::UpFromFirst;
            } else {
                self.navigation.focus_previous_option();
            }
            return self.moved();
        }
        if key.page_down {
            self.navigation.focus_next_page();
            return self.moved();
        }
        if key.page_up {
            self.navigation.focus_previous_page();
            return self.moved();
        }
        if key.enter || input == " " || input == "\u{3000}" {
            if (key.control && key.enter && is_input && self.config.has_on_submit)
                || (self.is_submit_focused && self.config.has_on_submit)
                || (key.enter && !self.config.submit_button && self.config.has_on_submit)
            {
                return MultiAction::Submitted(self.selected_values.clone());
            }
            return focused.map_or(MultiAction::None, |v| self.toggle(v));
        }
        if !self.config.hide_indexes {
            let normalized: String = input
                .chars()
                .map(|c| {
                    if ('\u{ff10}'..='\u{ff19}').contains(&c) {
                        char::from_u32(c as u32 - 0xff10 + 0x30).expect("digit")
                    } else {
                        c
                    }
                })
                .collect();
            if !normalized.is_empty() && normalized.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(value) = normalized.parse::<usize>() {
                    if let Some(option) = value.checked_sub(1).and_then(|i| self.options.get(i)) {
                        return self.toggle(option.option.value.clone());
                    }
                }
                return MultiAction::None;
            }
        }
        if key.escape {
            return MultiAction::CancelledStopPropagation;
        }
        MultiAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opts() -> Vec<MultiOption<u32>> {
        (1..=6)
            .map(|n| MultiOption {
                option: SelectOption {
                    value: n,
                    label: n.to_string(),
                    description: None,
                    is_input: n == 3,
                },
                initial_value: (n == 3).then(|| "seed".to_string()),
            })
            .collect()
    }
    fn state(config: MultiConfig) -> MultiSelect<u32> {
        MultiSelect::new(opts(), vec![2], 3, None, config)
    }
    #[test]
    fn initialization_and_input_defaults_follow_source() {
        let a = state(MultiConfig::default());
        assert_eq!(a.navigation.focused_value, Some(1));
        assert_eq!(a.selected_values, vec![2]);
        assert_eq!(a.input_values.get(&3).map(String::as_str), Some("seed"));
        let a = state(MultiConfig {
            initial_focus_last: true,
            ..Default::default()
        });
        assert_eq!(a.navigation.focused_value, Some(6));
    }
    #[test]
    fn option_changes_reset_selection_but_not_input_cache() {
        let mut a = state(MultiConfig::default());
        a.update_input_value(3, "typed".into());
        a.update_options(opts(), vec![1], None);
        assert_eq!(a.selected_values, vec![2, 3]);
        let mut changed = opts();
        changed[0].option.label = "changed".into();
        a.update_options(changed, vec![5], None);
        assert_eq!(a.selected_values, vec![5]);
        assert_eq!(a.input_values.get(&3).map(String::as_str), Some("typed"));
    }
    #[test]
    fn input_value_adds_removes_selection_and_notifies_even_for_same_value() {
        let mut a = state(MultiConfig::default());
        assert_eq!(
            a.update_input_value(3, "x".into()),
            MultiAction::InputUpdated {
                value: 3,
                text: "x".into(),
                selected: vec![2, 3],
            }
        );
        assert_eq!(
            a.update_input_value(3, "x".into()),
            MultiAction::InputUpdated {
                value: 3,
                text: "x".into(),
                selected: vec![2, 3],
            }
        );
        assert_eq!(
            a.update_input_value(3, "".into()),
            MultiAction::InputUpdated {
                value: 3,
                text: "".into(),
                selected: vec![2],
            }
        );
    }
    #[test]
    fn space_and_fullwidth_digits_toggle_in_order() {
        let mut a = state(MultiConfig::default());
        assert_eq!(
            a.handle_input(" ", MultiKey::default()),
            MultiAction::SelectionChanged(vec![2, 1])
        );
        assert_eq!(
            a.handle_input("２", MultiKey::default()),
            MultiAction::SelectionChanged(vec![1])
        );
        assert_eq!(a.handle_input("10", MultiKey::default()), MultiAction::None);
        assert_eq!(
            a.handle_input("　", MultiKey::default()),
            MultiAction::SelectionChanged(vec![])
        );
        let mut hidden = state(MultiConfig {
            hide_indexes: true,
            ..Default::default()
        });
        assert_eq!(
            hidden.handle_input("4", MultiKey::default()),
            MultiAction::None
        );
    }
    #[test]
    fn submit_button_enter_and_tab_are_independent() {
        let mut a = state(MultiConfig {
            submit_button: true,
            has_on_submit: true,
            ..Default::default()
        });
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    enter: true,
                    ..Default::default()
                }
            ),
            MultiAction::SelectionChanged(vec![2, 1])
        );
        a.navigation.focus_option(Some(&6));
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    tab: true,
                    ..Default::default()
                }
            ),
            MultiAction::SubmitButtonFocused
        );
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    enter: true,
                    ..Default::default()
                }
            ),
            MultiAction::Submitted(vec![2, 1])
        );
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    tab: true,
                    shift: true,
                    ..Default::default()
                }
            ),
            MultiAction::FocusMoved(Some(6))
        );
        assert!(!a.is_submit_focused);
        let mut b = state(MultiConfig {
            has_on_submit: true,
            ..Default::default()
        });
        assert_eq!(
            b.handle_input(
                "",
                MultiKey {
                    enter: true,
                    ..Default::default()
                }
            ),
            MultiAction::Submitted(vec![2])
        );
    }
    #[test]
    fn disabled_input_and_escape_are_handled_like_upstream() {
        let mut a = state(MultiConfig {
            disabled: true,
            ..Default::default()
        });
        assert_eq!(a.handle_input(" ", MultiKey::default()), MultiAction::None);
        a.config.disabled = false;
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    escape: true,
                    ..Default::default()
                }
            ),
            MultiAction::CancelledStopPropagation
        );
        a.navigation.focus_option(Some(&3));
        assert_eq!(a.handle_input("j", MultiKey::default()), MultiAction::None);
        assert_eq!(a.handle_input(" ", MultiKey::default()), MultiAction::None);
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    control: true,
                    enter: true,
                    ..Default::default()
                }
            ),
            MultiAction::SelectionChanged(vec![2, 3])
        );
        a.config.has_on_submit = true;
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    control: true,
                    enter: true,
                    ..Default::default()
                }
            ),
            MultiAction::Submitted(vec![2, 3])
        );
    }
    #[test]
    fn boundaries_paging_and_submit_button_navigation() {
        let mut a = state(MultiConfig {
            has_on_up_from_first: true,
            has_on_down_from_last: true,
            submit_button: true,
            has_on_submit: true,
            ..Default::default()
        });
        assert_eq!(
            a.handle_input("k", MultiKey::default()),
            MultiAction::UpFromFirst
        );
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    page_down: true,
                    ..Default::default()
                }
            ),
            MultiAction::FocusMoved(Some(4))
        );
        a.navigation.focus_option(Some(&6));
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    down: true,
                    ..Default::default()
                }
            ),
            MultiAction::SubmitButtonFocused
        );
        assert_eq!(
            a.handle_input(
                "",
                MultiKey {
                    down: true,
                    ..Default::default()
                }
            ),
            MultiAction::DownFromLast
        );
    }
}
