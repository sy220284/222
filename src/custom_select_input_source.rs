//! Native event decisions from `CustomSelect/use-select-input.ts`.
//! The terminal runtime decides when to call external onChange/onCancel, and
//! relays those signals instead of invoking React hooks in this port.
use std::collections::HashMap;
use std::hash::Hash;

use crate::custom_select_source::SelectState;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SelectKey {
    pub tab: bool,
    pub up: bool,
    pub down: bool,
    pub page_up: bool,
    pub page_down: bool,
    pub control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledSelection {
    No,
    Yes,
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectInputAction<T> {
    None,
    HandToTextInput,
    HandToAttachmentSelector,
    FocusMoved,
    FocusBlockedAtTop,
    FocusBlockedAtBottom,
    ToggleInputMode(T),
    FocusInput(T),
    Selected(T),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct SelectInputSettings<T> {
    pub is_disabled: bool,
    pub disable_selection: DisabledSelection,
    pub multi_select: bool,
    pub on_up_from_first: bool,
    pub on_down_from_last: bool,
    pub has_input_mode_toggle: bool,
    pub has_cancel_handler: bool,
    pub images_selected: bool,
    pub image_selection_available: bool,
    pub input_values: HashMap<T, String>,
    pub disabled_values: Vec<T>,
    pub empty_submit_to_cancel_values: Vec<T>,
}

impl<T> Default for SelectInputSettings<T> {
    fn default() -> Self {
        Self {
            is_disabled: false,
            disable_selection: DisabledSelection::No,
            multi_select: false,
            on_up_from_first: false,
            on_down_from_last: false,
            has_input_mode_toggle: false,
            has_cancel_handler: false,
            images_selected: false,
            image_selection_available: false,
            input_values: HashMap::new(),
            disabled_values: Vec::new(),
            empty_submit_to_cancel_values: Vec::new(),
        }
    }
}

fn normalize_full_width_digits(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\u{ff10}'..='\u{ff19}' => {
                char::from_u32(c as u32 - 0xff10 + 0x30).expect("ASCII digit")
            }
            other => other,
        })
        .collect()
}

impl<T: Clone + Eq + Hash> SelectState<T> {
    /// `useKeybindings`: up/down/accept/cancel, disabled in an active input field
    /// except cancel. Returning `Selected` corresponds to the source onChange.
    pub fn handle_binding(
        &mut self,
        binding: &str,
        settings: &SelectInputSettings<T>,
    ) -> SelectInputAction<T> {
        if settings.is_disabled {
            return SelectInputAction::None;
        }
        if binding == "select:cancel" {
            return if settings.has_cancel_handler {
                SelectInputAction::Cancelled
            } else {
                SelectInputAction::None
            };
        }
        if self.navigation.is_in_input() {
            return SelectInputAction::HandToTextInput;
        }
        match binding {
            "select:next" => self.move_down(settings),
            "select:previous" => self.move_up(settings),
            "select:accept" => {
                if settings.disable_selection == DisabledSelection::Yes {
                    return SelectInputAction::None;
                }
                let Some(focus) = self.navigation.focused_value.clone() else {
                    return SelectInputAction::None;
                };
                if settings.disabled_values.contains(&focus) {
                    return SelectInputAction::None;
                }
                self.select_focused_option();
                SelectInputAction::Selected(focus)
            }
            _ => SelectInputAction::None,
        }
    }

    fn move_down(&mut self, settings: &SelectInputSettings<T>) -> SelectInputAction<T> {
        if settings.on_down_from_last
            && self.navigation.focused_value.as_ref()
                == self.navigation.options.last().map(|o| &o.value)
        {
            return SelectInputAction::FocusBlockedAtBottom;
        }
        self.navigation.focus_next_option();
        SelectInputAction::FocusMoved
    }

    fn move_up(&mut self, settings: &SelectInputSettings<T>) -> SelectInputAction<T> {
        if settings.on_up_from_first
            && self.navigation.visible_from_index == 0
            && self.navigation.focused_value.as_ref()
                == self.navigation.options.first().map(|o| &o.value)
        {
            return SelectInputAction::FocusBlockedAtTop;
        }
        self.navigation.focus_previous_option();
        SelectInputAction::FocusMoved
    }

    /// `useInput`: handles tab, pages, fullwidth digits/space, and the input
    /// field's arrow keys without swallowing ordinary text or number typing.
    pub fn handle_input(
        &mut self,
        input: &str,
        key: SelectKey,
        settings: &SelectInputSettings<T>,
    ) -> SelectInputAction<T> {
        if settings.is_disabled {
            return SelectInputAction::None;
        }
        let Some(focus) = self.navigation.focused_value.clone() else {
            return SelectInputAction::None;
        };
        if key.tab && settings.has_input_mode_toggle {
            return SelectInputAction::ToggleInputMode(focus);
        }
        if self.navigation.is_in_input() {
            if settings.images_selected {
                return SelectInputAction::HandToAttachmentSelector;
            }
            if key.down && settings.image_selection_available {
                return SelectInputAction::HandToAttachmentSelector;
            }
            if key.down || (key.control && input == "n") {
                return self.move_down(settings);
            }
            if key.up || (key.control && input == "p") {
                return self.move_up(settings);
            }
            return SelectInputAction::HandToTextInput;
        }
        if key.page_down {
            self.navigation.focus_next_page();
        }
        if key.page_up {
            self.navigation.focus_previous_page();
        }
        if settings.disable_selection == DisabledSelection::Yes {
            return if key.page_down || key.page_up {
                SelectInputAction::FocusMoved
            } else {
                SelectInputAction::None
            };
        }
        let normalized = normalize_full_width_digits(input);
        if settings.multi_select
            && (input == " " || input == "\u{3000}")
            && !settings.disabled_values.contains(&focus)
        {
            self.select_focused_option();
            return SelectInputAction::Selected(focus);
        }
        if settings.disable_selection != DisabledSelection::Numeric
            && !normalized.is_empty()
            && normalized.bytes().all(|b| b.is_ascii_digit())
        {
            // JS parseInt does not overflow, so a huge numeric key should be
            // ignored as an out-of-range index, not wrap to another option.
            let Some(number) = normalized.parse::<usize>().ok() else {
                return SelectInputAction::None;
            };
            let Some(index) = number.checked_sub(1) else {
                return SelectInputAction::None;
            };
            let Some(option) = self.navigation.options.get(index) else {
                return SelectInputAction::None;
            };
            let value = option.value.clone();
            if settings.disabled_values.contains(&value) {
                return SelectInputAction::None;
            }
            if option.is_input {
                let existing = settings
                    .input_values
                    .get(&value)
                    .map(String::as_str)
                    .unwrap_or("");
                if !existing.trim().is_empty()
                    || settings.empty_submit_to_cancel_values.contains(&value)
                {
                    // Source only calls onChange for numeric submission;
                    // it does not mutate the selected internal state.
                    return SelectInputAction::Selected(value);
                }
                self.navigation.focus_option(Some(&value));
                return SelectInputAction::FocusInput(value);
            }
            return SelectInputAction::Selected(value);
        }
        if key.page_down || key.page_up {
            SelectInputAction::FocusMoved
        } else {
            SelectInputAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_select_source::SelectOption;
    fn state() -> SelectState<usize> {
        SelectState::new(
            (0..8)
                .map(|value| SelectOption {
                    label: value.to_string(),
                    value,
                    description: None,
                    is_input: value == 3,
                })
                .collect(),
            4,
            None,
            None,
        )
    }
    #[test]
    fn bindings_scroll_select_and_callback_edges() {
        let mut state = state();
        let mut settings = SelectInputSettings::default();
        settings.on_up_from_first = true;
        assert_eq!(
            state.handle_binding("select:previous", &settings),
            SelectInputAction::FocusBlockedAtTop
        );
        assert_eq!(
            state.handle_binding("select:next", &settings),
            SelectInputAction::FocusMoved
        );
        assert_eq!(state.navigation.focused_value, Some(1));
        settings.disabled_values.push(1);
        assert_eq!(
            state.handle_binding("select:accept", &settings),
            SelectInputAction::None
        );
        settings.disabled_values.clear();
        assert_eq!(
            state.handle_binding("select:accept", &settings),
            SelectInputAction::Selected(1)
        );
        assert_eq!(state.value, Some(1));
        assert_eq!(
            state.handle_binding("select:cancel", &settings),
            SelectInputAction::None
        );
        settings.has_cancel_handler = true;
        assert_eq!(
            state.handle_binding("select:cancel", &settings),
            SelectInputAction::Cancelled
        );
    }
    #[test]
    fn fullwidth_digit_disabled_numeric_and_multiselect_space() {
        let mut state = state();
        let mut settings = SelectInputSettings::default();
        assert_eq!(
            state.handle_input("２", SelectKey::default(), &settings),
            SelectInputAction::Selected(1)
        );
        assert_eq!(state.value, None);
        settings.disable_selection = DisabledSelection::Numeric;
        assert_eq!(
            state.handle_input("2", SelectKey::default(), &settings),
            SelectInputAction::None
        );
        settings.multi_select = true;
        assert_eq!(
            state.handle_input("\u{3000}", SelectKey::default(), &settings),
            SelectInputAction::Selected(0)
        );
        assert_eq!(state.value, Some(0));
        assert_eq!(
            state.handle_input(
                "99999999999999999999999999",
                SelectKey::default(),
                &settings
            ),
            SelectInputAction::None
        );
    }
    #[test]
    fn input_mode_numbers_are_text_and_arrows_move_or_enter_images() {
        let mut state = state();
        let mut settings = SelectInputSettings::default();
        state.navigation.focus_option(Some(&3));
        assert_eq!(
            state.handle_input("４", SelectKey::default(), &settings),
            SelectInputAction::HandToTextInput
        );
        settings.image_selection_available = true;
        assert_eq!(
            state.handle_input(
                "",
                SelectKey {
                    down: true,
                    ..Default::default()
                },
                &settings
            ),
            SelectInputAction::HandToAttachmentSelector
        );
        settings.image_selection_available = false;
        assert_eq!(
            state.handle_input(
                "n",
                SelectKey {
                    control: true,
                    ..Default::default()
                },
                &settings
            ),
            SelectInputAction::FocusMoved
        );
        assert_eq!(state.navigation.focused_value, Some(4));
    }
    #[test]
    fn number_selecting_input_requires_prefill_or_empty_cancel() {
        let mut state = state();
        let mut settings = SelectInputSettings::default();
        assert_eq!(
            state.handle_input("4", SelectKey::default(), &settings),
            SelectInputAction::FocusInput(3)
        );
        assert_eq!(state.navigation.focused_value, Some(3));
        state.navigation.focus_option(Some(&0));
        settings.input_values.insert(3, " hello ".to_owned());
        assert_eq!(
            state.handle_input("4", SelectKey::default(), &settings),
            SelectInputAction::Selected(3)
        );
        state.navigation.focus_option(Some(&0));
        settings.input_values.clear();
        settings.empty_submit_to_cancel_values.push(3);
        assert_eq!(
            state.handle_input("4", SelectKey::default(), &settings),
            SelectInputAction::Selected(3)
        );
    }
    #[test]
    fn page_motion_allowed_while_selection_disabled_and_tab_toggle() {
        let mut state = state();
        let mut settings = SelectInputSettings::default();
        settings.disable_selection = DisabledSelection::Yes;
        assert_eq!(
            state.handle_input(
                "",
                SelectKey {
                    page_down: true,
                    ..Default::default()
                },
                &settings
            ),
            SelectInputAction::FocusMoved
        );
        assert_eq!(state.navigation.focused_value, Some(4));
        settings.has_input_mode_toggle = true;
        assert_eq!(
            state.handle_input(
                "",
                SelectKey {
                    tab: true,
                    ..Default::default()
                },
                &settings
            ),
            SelectInputAction::ToggleInputMode(4)
        );
        settings.is_disabled = true;
        assert_eq!(
            state.handle_input(
                "",
                SelectKey {
                    tab: true,
                    ..Default::default()
                },
                &settings
            ),
            SelectInputAction::None
        );
    }
}
