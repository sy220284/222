//! Rust equivalent of `components/CustomSelect/index.ts`.
//! Keep the public selection API in one place as the original barrel module does.
#![allow(unused_imports)]
pub use crate::custom_multi_select_source::{
    MultiAction, MultiConfig, MultiKey, MultiOption, MultiSelect,
};
pub use crate::custom_select_input_source::{
    DisabledSelection, SelectInputAction, SelectInputSettings, SelectKey,
};
pub use crate::custom_select_source::{
    OptionMap, OptionMapItem, SelectNavigation, SelectOption, SelectState, VisibleOption,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_exports_allow_a_single_selection_controller() {
        let items = vec![SelectOption {
            value: "a",
            label: "Alpha".into(),
            description: None,
            is_input: false,
        }];
        let state = SelectState::new(items.clone(), 5, None, None);
        assert_eq!(state.navigation.validated_focus(), Some(&"a"));
        let multi = MultiSelect::new(
            vec![MultiOption {
                option: items[0].clone(),
                initial_value: None,
            }],
            vec![],
            5,
            None,
            MultiConfig::default(),
        );
        assert_eq!(multi.navigation.visible_options().len(), 1);
        assert_eq!(OptionMap::new(&items).len(), 1);
    }
}
