//! Complete vim mode state machine, a faithful port of the five
//! `src/vim/*.ts` modules (2.1.88): types / motions / textObjects /
//! operators / transitions. Pure logic with no terminal I/O so it can be
//! driven by the line editor in `terminal_input`.

pub mod cursor;
pub mod motions;
pub mod operators;
pub mod text_objects;
pub mod transitions;
pub mod types;

pub use cursor::{is_vim_punctuation, is_vim_whitespace, is_vim_word_char, VimCursor};
pub use operators::VimCtx;
pub use transitions::{transition, CommandState};
pub use types::{
    is_text_obj_scope_key, FindType, Operator, PersistentState, RecordedChange, TextObjScope,
    MAX_VIM_COUNT,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive a key sequence through the NORMAL-mode state machine.
    fn drive(ctx: &mut VimCtx, state: &mut CommandState, keys: &str) {
        for c in keys.chars() {
            transition(state, &c.to_string(), ctx);
        }
    }

    #[test]
    fn operator_text_object_two_key_sequence() {
        let mut c = VimCtx::new("say (hello world) ok".into(), 5);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "di");
        assert!(matches!(s, CommandState::OperatorTextObj { .. }));
        assert!(transition(&mut s, "(", &mut c));
        assert_eq!(c.text, "say () ok");
        let mut c = VimCtx::new("say \"hello\" ok".into(), 5);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "ya");
        transition(&mut s, "\"", &mut c);
        assert_eq!(c.persistent.register, "\"hello\"");
    }

    #[test]
    fn operator_find_two_key_sequence() {
        let mut c = VimCtx::new("one two three".into(), 0);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "df");
        assert!(transition(&mut s, "t", &mut c));
        assert_eq!(c.text, "t two three");
        assert!(c.persistent.last_find.is_some());
    }

    #[test]
    fn operator_g_and_indent_sequences() {
        let mut c = VimCtx::new("l1\nl2\nl3".into(), 0);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "dG");
        assert_eq!(c.text, "l1");
        let mut c = VimCtx::new("l1\nl2\nl3".into(), 3);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "d2");
        assert!(transition(&mut s, "G", &mut c));
        assert_eq!(c.text, "l3");
        let mut c = VimCtx::new("a\nb".into(), 0);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, ">>");
        assert_eq!(c.text, "  a\nb");
        // doubled indent key executes; mismatched key cancels
        let mut c = VimCtx::new("a\nb".into(), 0);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "<<");
        assert_eq!(c.text, "a\nb");
    }

    #[test]
    fn operator_count_then_motion() {
        let mut c = VimCtx::new("a b c d".into(), 0);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "d2");
        assert!(matches!(s, CommandState::OperatorCount { .. }));
        assert!(transition(&mut s, "w", &mut c));
        assert_eq!(c.text, "c d");
    }

    #[test]
    fn operator_gj_alias_is_linewise() {
        let mut c = VimCtx::new("one\ntwo".into(), 2);
        let mut s = CommandState::Idle;
        drive(&mut c, &mut s, "dg");
        assert!(transition(&mut s, "j", &mut c));
        assert_eq!(c.text, "two");
    }
}
