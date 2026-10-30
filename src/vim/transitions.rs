//! Vim state transitions, ported from `src/vim/transitions.ts` (2.1.88).
//! The scannable source of truth: to understand what happens in any state,
//! look up that state's transition function.

use super::operators::{
    execute_indent, execute_join, execute_line_op, execute_open_line, execute_operator_find,
    execute_operator_g, execute_operator_gg, execute_operator_motion, execute_operator_text_obj,
    execute_paste, execute_replace, execute_toggle_case, execute_repeat_find, execute_x, VimCtx,
};
use super::types::{is_text_obj_type, is_text_obj_scope_key, FindType, Operator, TextObjScope, MAX_VIM_COUNT};
use super::motions::resolve_motion;
use super::types::{FIND_KEYS, SIMPLE_MOTIONS};

/// Pending command state (`CommandState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CommandState {
    #[default]
    Idle,
    Count { digits: String },
    Operator { op: Operator, count: usize },
    OperatorCount { op: Operator, count: usize, digits: String },
    OperatorFind { op: Operator, count: usize, find: FindType },
    OperatorTextObj { op: Operator, count: usize, scope: TextObjScope },
    Find { find: FindType, count: usize },
    G { count: usize },
    OperatorG { op: Operator, count: usize },
    Replace { count: usize },
    Indent { dir: char, count: usize },
}

fn parse_find(key: char) -> FindType {
    match key {
        'f' => FindType::FForward,
        'F' => FindType::FBackward,
        't' => FindType::TTill,
        _ => FindType::TBackTill,
    }
}

fn is_find_key(c: char) -> bool {
    FIND_KEYS.contains(&c)
}

/// Outcome of a normal/operator input handler.
enum Outcome {
    /// Execute ran to completion; command state resets to idle.
    Executed,
    /// Awaiting further keys; the machine moves to this state.
    Pending(CommandState),
    /// Input is not recognized; the original falls through (insert).
    Unrecognized,
}

/// Transition the state machine on a raw input string (empty input models
/// Backspace/Delete reaching literal-char states). Returns `true` when the
/// input was recognized or consumed.
pub fn transition(state: &mut CommandState, input: &str, ctx: &mut VimCtx) -> bool {
    let snapshot = state.clone();
    match &snapshot {
        CommandState::Idle => from_idle(state, input, ctx),
        CommandState::Count { digits } => from_count(state, digits, input, ctx),
        CommandState::Operator { op, count } => {
            from_operator(state, *op, *count, input, ctx)
        }
        CommandState::OperatorCount { op, count, digits } => {
            from_operator_count(state, *op, *count, digits, input, ctx)
        }
        CommandState::OperatorFind { op, count, find } => {
            if let Some(ch) = input.chars().next().filter(|_| input.chars().count() == 1) {
                execute_operator_find(ctx, *op, *find, ch, *count);
            }
            *state = CommandState::Idle;
            true
        }
        CommandState::OperatorTextObj { op, count, scope } => {
            let (op, scope, count) = (*op, *scope, *count);
            *state = CommandState::Idle;
            if let Some(first) = input.chars().next() {
                if input.chars().count() == 1 && is_text_obj_type(first) {
                    execute_operator_text_obj(ctx, op, first, scope, count);
                    return true;
                }
            }
            false
        }
        CommandState::Find { find, count } => {
            let (find, count) = (*find, *count);
            *state = CommandState::Idle;
            if let Some(ch) = input.chars().next().filter(|_| input.chars().count() == 1) {
                if let Some(offset) = ctx.cursor().find_character(ch, find, count) {
                    ctx.set_offset(offset);
                    ctx.set_last_find(find, ch);
                }
            }
            true
        }
        CommandState::G { count } => from_g(state, *count, input, ctx),
        CommandState::OperatorG { op, count } => from_operator_g(state, *op, *count, input, ctx),
        CommandState::Replace { count } => {
            let count = *count;
            *state = CommandState::Idle;
            // Backspace arrives as empty input: r<BS> cancels the replace
            // (otherwise executeReplace("") would delete under the cursor).
            if input.is_empty() {
                return true;
            }
            if let Some(ch) = input.chars().next() {
                execute_replace(ctx, ch, count);
            }
            true
        }
        CommandState::Indent { dir, count } => {
            let (dir, count) = (*dir, *count);
            *state = CommandState::Idle;
            // The doubled key executes; a mismatched key cancels.
            if input == dir.to_string() {
                execute_indent(ctx, dir, count);
            }
            true
        }
    }
}

/// `fromIdle`
fn from_idle(state: &mut CommandState, input: &str, ctx: &mut VimCtx) -> bool {
    // 0 is line-start motion, not a count prefix
    if input.len() == 1 {
        let first = input.chars().next().unwrap();
        if first.is_ascii_digit() && first != '0' {
            *state = CommandState::Count { digits: input.to_string() };
            return true;
        }
        if first == '0' {
            let offset = ctx.cursor().start_of_logical_line().offset;
            ctx.set_offset(offset);
            return true;
        }
    }
    match handle_normal_input(input, 1, ctx) {
        Outcome::Executed => {
            *state = CommandState::Idle;
            true
        }
        Outcome::Pending(next) => {
            *state = next;
            true
        }
        Outcome::Unrecognized => false,
    }
}

/// `fromCount`
fn from_count(state: &mut CommandState, digits: &str, input: &str, ctx: &mut VimCtx) -> bool {
    if input.len() == 1 {
        let first = input.chars().next().unwrap();
        if first.is_ascii_digit() {
            let new_digits = format!("{digits}{input}");
            let count: usize = new_digits.parse().unwrap_or(MAX_VIM_COUNT).min(MAX_VIM_COUNT);
            *state = CommandState::Count { digits: count.to_string() };
            return true;
        }
    }
    let count: usize = digits.parse().unwrap_or(1);
    match handle_normal_input(input, count, ctx) {
        Outcome::Executed => {
            *state = CommandState::Idle;
            true
        }
        Outcome::Pending(next) => {
            *state = next;
            true
        }
        Outcome::Unrecognized => {
            *state = CommandState::Idle;
            false
        }
    }
}

/// `handleNormalInput`
fn handle_normal_input(input: &str, count: usize, ctx: &mut VimCtx) -> Outcome {
    let first = match input.chars().next() {
        Some(c) if input.chars().count() == 1 => c,
        _ => return Outcome::Unrecognized,
    };

    if let Some(op) = Operator::parse(first) {
        return Outcome::Pending(CommandState::Operator { op, count });
    }
    if SIMPLE_MOTIONS.contains(&first) {
        let target = resolve_motion(input, &ctx.cursor(), count);
        let offset = target.offset;
        ctx.set_offset(offset);
        return Outcome::Executed;
    }
    if is_find_key(first) {
        return Outcome::Pending(CommandState::Find { find: parse_find(first), count });
    }
    match first {
        'g' => Outcome::Pending(CommandState::G { count }),
        'r' => Outcome::Pending(CommandState::Replace { count }),
        '>' | '<' => Outcome::Pending(CommandState::Indent { dir: first, count }),
        '~' => {
            execute_toggle_case(ctx, count);
            Outcome::Executed
        }
        'x' => {
            execute_x(ctx, count);
            Outcome::Executed
        }
        'J' => {
            execute_join(ctx, count);
            Outcome::Executed
        }
        'p' | 'P' => {
            execute_paste(ctx, first == 'p', count);
            Outcome::Executed
        }
        'D' => {
            execute_operator_motion(ctx, Operator::Delete, "$", 1);
            Outcome::Executed
        }
        'C' => {
            execute_operator_motion(ctx, Operator::Change, "$", 1);
            Outcome::Executed
        }
        'Y' => {
            execute_line_op(ctx, Operator::Yank, count);
            Outcome::Executed
        }
        'G' => {
            // count=1 means no count given: go to last line; otherwise line N.
            let offset = if count <= 1 {
                ctx.cursor().start_of_last_line().offset
            } else {
                ctx.cursor().go_to_line(count).offset
            };
            ctx.set_offset(offset);
            Outcome::Executed
        }
        '.' => {
            ctx.dot_repeat_requested = true;
            Outcome::Executed
        }
        ';' | ',' => {
            execute_repeat_find(ctx, first == ',', count);
            Outcome::Executed
        }
        'u' => {
            ctx.undo_requested = true;
            Outcome::Executed
        }
        'i' => {
            let offset = ctx.cursor().offset;
            ctx.enter_insert(offset);
            Outcome::Executed
        }
        'I' => {
            let offset = ctx.cursor().first_non_blank_in_logical_line().offset;
            ctx.enter_insert(offset);
            Outcome::Executed
        }
        'a' => {
            let cur = ctx.cursor();
            let offset = if cur.is_at_end() { cur.offset } else { cur.right().offset };
            ctx.enter_insert(offset);
            Outcome::Executed
        }
        'A' => {
            let offset = ctx.cursor().end_of_logical_line().offset;
            ctx.enter_insert(offset);
            Outcome::Executed
        }
        'o' => {
            execute_open_line(ctx, true);
            Outcome::Executed
        }
        'O' => {
            execute_open_line(ctx, false);
            Outcome::Executed
        }
        _ => Outcome::Unrecognized,
    }
}

/// `fromOperator`
fn from_operator(
    state: &mut CommandState,
    op: Operator,
    count: usize,
    input: &str,
    ctx: &mut VimCtx,
) -> bool {
    if input.chars().count() == 1 {
        let first = input.chars().next().unwrap();
        if first == op.key() {
            execute_line_op(ctx, op, count);
            *state = CommandState::Idle;
            return true;
        }
        if first.is_ascii_digit() {
            *state = CommandState::OperatorCount { op, count, digits: first.to_string() };
            return true;
        }
    }
    match handle_operator_input(op, count, input, ctx) {
        Outcome::Executed => {
            *state = CommandState::Idle;
            true
        }
        Outcome::Pending(next) => {
            *state = next;
            true
        }
        Outcome::Unrecognized => {
            *state = CommandState::Idle;
            false
        }
    }
}

/// `handleOperatorInput`
fn handle_operator_input(op: Operator, count: usize, input: &str, ctx: &mut VimCtx) -> Outcome {
    let first = match input.chars().next() {
        Some(c) if input.chars().count() == 1 => c,
        _ => return Outcome::Unrecognized,
    };
    if is_text_obj_scope_key(first) {
        return Outcome::Pending(CommandState::OperatorTextObj {
            op,
            count,
            scope: if first == 'i' { TextObjScope::Inner } else { TextObjScope::Around },
        });
    }
    if is_find_key(first) {
        return Outcome::Pending(CommandState::OperatorFind { op, count, find: parse_find(first) });
    }
    if SIMPLE_MOTIONS.contains(&first) {
        execute_operator_motion(ctx, op, input, count);
        return Outcome::Executed;
    }
    if first == 'G' {
        execute_operator_g(ctx, op, count);
        return Outcome::Executed;
    }
    if first == 'g' {
        return Outcome::Pending(CommandState::OperatorG { op, count });
    }
    Outcome::Unrecognized
}

/// `fromOperatorCount`
fn from_operator_count(
    state: &mut CommandState,
    op: Operator,
    count: usize,
    digits: &str,
    input: &str,
    ctx: &mut VimCtx,
) -> bool {
    if input.chars().count() == 1 {
        let first = input.chars().next().unwrap();
        if first.is_ascii_digit() {
            let new_digits = format!("{digits}{input}");
            let parsed: usize = new_digits.parse().unwrap_or(MAX_VIM_COUNT).min(MAX_VIM_COUNT);
            *state = CommandState::OperatorCount { op, count, digits: parsed.to_string() };
            return true;
        }
    }
    let motion_count: usize = digits.parse().unwrap_or(1);
    let effective = count.saturating_mul(motion_count);
    match handle_operator_input(op, effective, input, ctx) {
        Outcome::Executed => {
            *state = CommandState::Idle;
            true
        }
        Outcome::Pending(next) => {
            *state = next;
            true
        }
        Outcome::Unrecognized => {
            *state = CommandState::Idle;
            false
        }
    }
}

/// `fromG`
fn from_g(state: &mut CommandState, count: usize, input: &str, ctx: &mut VimCtx) -> bool {
    *state = CommandState::Idle;
    let first = match input.chars().next() {
        Some(c) if input.chars().count() == 1 => c,
        _ => return false,
    };
    if first == 'j' || first == 'k' {
        let target = resolve_motion(&format!("g{first}"), &ctx.cursor(), count);
        let offset = target.offset;
        ctx.set_offset(offset);
        return true;
    }
    if first == 'g' {
        // If count provided (e.g. 5gg) go to that line; otherwise first line.
        let offset = if count > 1 {
            ctx.cursor().go_to_line(count).offset
        } else {
            ctx.cursor().start_of_first_line().offset
        };
        ctx.set_offset(offset);
        return true;
    }
    false
}

/// `fromOperatorG` — any other input cancels the operator.
fn from_operator_g(
    state: &mut CommandState,
    op: Operator,
    count: usize,
    input: &str,
    ctx: &mut VimCtx,
) -> bool {
    *state = CommandState::Idle;
    let first = match input.chars().next() {
        Some(c) if input.chars().count() == 1 => c,
        _ => return false,
    };
    if first == 'j' || first == 'k' {
        execute_operator_motion(ctx, op, &format!("g{first}"), count);
        return true;
    }
    if first == 'g' {
        execute_operator_gg(ctx, op, count);
        return true;
    }
    false
}
