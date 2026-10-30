//! Vim operator execution, ported from `src/vim/operators.ts` (2.1.88).
//! Pure functions over a [`VimCtx`] that owns text/offset plus the
//! persistent state the original threaded through `OperatorContext`.

use super::cursor::VimCursor;
use super::motions::{is_inclusive_motion, is_linewise_motion, resolve_motion};
use super::text_objects::{find_text_object, TextObjectRange};
use super::types::{FindType, Operator, RecordedChange, TextObjScope};

/// Everything the operators need. Mirrors the original `OperatorContext`
/// (cursor/text/setText/setOffset/enterInsert/register/lastFind/record).
pub struct VimCtx {
    pub text: String,
    pub offset: usize,
    pub persistent: super::types::PersistentState,
    /// A transition called `enterInsert` — the adapter flips the mode.
    pub enter_insert: bool,
    /// A transition requested undo (`u`).
    pub undo_requested: bool,
    /// A transition requested dot-repeat (`.`).
    pub dot_repeat_requested: bool,
}

impl VimCtx {
    pub fn new(text: String, offset: usize) -> Self {
        VimCtx {
            text,
            offset,
            persistent: Default::default(),
            enter_insert: false,
            undo_requested: false,
            dot_repeat_requested: false,
        }
    }
    pub fn cursor(&self) -> VimCursor<'_> {
        VimCursor::new(&self.text, self.offset)
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset.min(self.text.len());
    }
    pub fn enter_insert(&mut self, offset: usize) {
        self.set_offset(offset);
        self.enter_insert = true;
    }
    pub fn set_last_find(&mut self, find: FindType, ch: char) {
        self.persistent.last_find = Some((find, ch));
    }
    pub fn set_register(&mut self, content: String, linewise: bool) {
        self.persistent.register = content;
        self.persistent.register_is_linewise = linewise;
    }
    pub fn record_change(&mut self, change: RecordedChange) {
        self.persistent.last_change = Some(change);
    }
    /// Logical line index of the cursor (`countCharInString(..., '\n')`).
    pub fn line_of(&self, offset: usize) -> usize {
        self.text[..offset].bytes().filter(|b| *b == b'\n').count()
    }
}

/// Length of the last char, used for delete-end clamping
/// (`lastGrapheme(newText).length || 1`).
fn last_char_len(text: &str) -> usize {
    text.chars().next_back().map_or(1, char::len_utf8)
}

/// Offset of a line's start across the joined line array
/// (`getLineStartOffset`).
fn line_start_offset(lines: &[&str], line_index: usize) -> usize {
    lines[..line_index].join("\n").len() + usize::from(line_index > 0)
}

/// `applyOperator`: mutate text for the operator over a char range.
fn apply_operator(ctx: &mut VimCtx, op: Operator, from: usize, to: usize, linewise: bool) {
    let mut content = ctx.text[from..to.min(ctx.text.len())].to_owned();
    if linewise && !content.ends_with('\n') {
        content.push('\n');
    }
    ctx.set_register(content, linewise);
    match op {
        Operator::Yank => {
            ctx.set_offset(from);
        }
        Operator::Delete => {
            let new_text = format!(
                "{}{}",
                &ctx.text[..from],
                &ctx.text[to.min(ctx.text.len())..]
            );
            let max_off = new_text.len().saturating_sub(last_char_len(&new_text));
            ctx.set_text(new_text);
            ctx.set_offset(from.min(max_off));
        }
        Operator::Change => {
            let new_text = format!(
                "{}{}",
                &ctx.text[..from],
                &ctx.text[to.min(ctx.text.len())..]
            );
            ctx.set_text(new_text);
            ctx.enter_insert(from);
        }
    }
}

/// `getOperatorRange`: resolve operator bounds, including the cw special
/// case, linewise extension and inclusive motions. (The original's
/// `snapOutOfImageRef` chip handling has no counterpart here: the Rust line
/// editor carries no image placeholders.)
fn get_operator_range(
    cursor_offset: usize,
    text: &str,
    target: &VimCursor<'_>,
    motion: &str,
    op: Operator,
    count: usize,
) -> (usize, usize, bool) {
    let mut from = cursor_offset.min(target.offset);
    let mut to = cursor_offset.max(target.offset);
    let mut linewise = false;
    if op == Operator::Change && (motion == "w" || motion == "W") {
        let mut word_cursor = VimCursor::new(text, cursor_offset);
        for _ in 0..count.saturating_sub(1) {
            word_cursor = if motion == "w" {
                word_cursor.next_vim_word()
            } else {
                word_cursor.next_word_big()
            };
        }
        let word_end = if motion == "w" {
            word_cursor.end_of_vim_word()
        } else {
            word_cursor.end_of_word_big()
        };
        to = word_end.right().offset;
    } else if is_linewise_motion(motion) {
        linewise = true;
        let next_newline = text[to..].find('\n').map(|i| to + i);
        match next_newline {
            None => {
                to = text.len();
                if from > 0 && text.as_bytes().get(from - 1) == Some(&b'\n') {
                    from -= 1;
                }
            }
            Some(idx) => to = idx + 1,
        }
    } else if is_inclusive_motion(motion) && cursor_offset <= target.offset {
        to = VimCursor::new(text, to).right().offset;
    }
    (from, to, linewise)
}

/// `executeOperatorMotion`: resolve the motion target then apply.
pub fn execute_operator_motion(ctx: &mut VimCtx, op: Operator, motion: &str, count: usize) {
    let target = resolve_motion(motion, &ctx.cursor(), count);
    if target.offset == ctx.offset {
        return;
    }
    let (from, to, linewise) =
        get_operator_range(ctx.offset, &ctx.text, &target, motion, op, count);
    apply_operator(ctx, op, from, to, linewise);
    ctx.record_change(RecordedChange::Operator {
        op,
        motion: motion.to_owned(),
        count,
    });
}

/// `getOperatorRangeForFind` + `executeOperatorFind`.
pub fn execute_operator_find(
    ctx: &mut VimCtx,
    op: Operator,
    find_type: FindType,
    target_char: char,
    count: usize,
) {
    let Some(target) = ctx
        .cursor()
        .find_character(target_char, find_type, count.max(1))
    else {
        return;
    };
    let from = ctx.offset.min(target);
    let to = VimCursor::new(&ctx.text, ctx.offset.max(target))
        .right()
        .offset;
    apply_operator(ctx, op, from, to, false);
    ctx.set_last_find(find_type, target_char);
    ctx.record_change(RecordedChange::OperatorFind {
        op,
        find: find_type,
        ch: target_char,
        count,
    });
}

/// `executeRepeatFind`: repeat the last f/F/t/T find, optionally reversed.
pub fn execute_repeat_find(ctx: &mut VimCtx, reverse: bool, count: usize) {
    let Some((find_type, ch)) = ctx.persistent.last_find else {
        return;
    };
    let find_type = if reverse { find_type.flip() } else { find_type };
    let Some(target) = ctx.cursor().find_character(ch, find_type, count.max(1)) else {
        return;
    };
    ctx.set_offset(target);
}

/// `executeLineOp`: dd/cy/yy linewise operation over `count` lines.
pub fn execute_line_op(ctx: &mut VimCtx, op: Operator, count: usize) {
    let current_line = ctx.line_of(ctx.offset);
    let original_text = ctx.text.clone();
    let lines: Vec<&str> = original_text.split('\n').collect();
    let lines_to_affect = count.max(1).min(lines.len() - current_line);
    let line_start = ctx.cursor().start_of_logical_line().offset;
    let mut line_end = line_start;
    for _ in 0..lines_to_affect {
        line_end = match ctx.text[line_end..].find('\n') {
            Some(i) => line_end + i + 1,
            None => ctx.text.len(),
        };
    }

    let mut content = ctx.text[line_start..line_end].to_owned();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    ctx.set_register(content, true);

    match op {
        Operator::Yank => {
            ctx.set_offset(line_start);
        }
        Operator::Delete => {
            let mut delete_start = line_start;
            let delete_end = line_end;
            if delete_end == ctx.text.len()
                && delete_start > 0
                && ctx.text.as_bytes().get(delete_start - 1) == Some(&b'\n')
            {
                delete_start -= 1;
            }
            let new_text = format!("{}{}", &ctx.text[..delete_start], &ctx.text[delete_end..]);
            let max_off = new_text.len().saturating_sub(last_char_len(&new_text));
            ctx.set_text(new_text);
            ctx.set_offset(delete_start.min(max_off));
        }
        Operator::Change => {
            if lines.len() == 1 {
                ctx.set_text(String::new());
                ctx.enter_insert(0);
            } else {
                // Delete all affected lines, replace with single empty line.
                let mut result: Vec<&str> = lines[..current_line].to_vec();
                result.push("");
                result.extend_from_slice(&lines[current_line + lines_to_affect..]);
                let new_text = result.join("\n");
                ctx.set_text(new_text);
                ctx.enter_insert(line_start);
            }
        }
    }
    ctx.record_change(RecordedChange::Operator {
        op,
        motion: op.key().to_string(),
        count: count.max(1),
    });
}

/// `executeX`: delete `count` characters at the cursor.
pub fn execute_x(ctx: &mut VimCtx, count: usize) {
    let from = ctx.offset;
    if from >= ctx.text.len() {
        return;
    }
    let mut end_cursor = ctx.cursor();
    for _ in 0..count.max(1) {
        if end_cursor.is_at_end() {
            break;
        }
        end_cursor = end_cursor.right();
    }
    let to = end_cursor.offset;
    let deleted = ctx.text[from..to].to_owned();
    let new_text = format!("{}{}", &ctx.text[..from], &ctx.text[to..]);
    ctx.set_register(deleted, false);
    let max_off = new_text.len().saturating_sub(last_char_len(&new_text));
    ctx.set_text(new_text);
    ctx.set_offset(from.min(max_off));
    ctx.record_change(RecordedChange::X {
        count: count.max(1),
    });
}

/// `executeReplace`: overwrite `count` characters with `ch`.
pub fn execute_replace(ctx: &mut VimCtx, ch: char, count: usize) {
    let mut offset = ctx.offset;
    let mut new_text = ctx.text.clone();
    for _ in 0..count.max(1) {
        if offset >= new_text.len() {
            break;
        }
        let grapheme_len = new_text[offset..].chars().next().map_or(1, char::len_utf8);
        new_text.replace_range(offset..offset + grapheme_len, &ch.to_string());
        offset += ch.len_utf8();
    }
    ctx.set_text(new_text);
    ctx.set_offset(offset.saturating_sub(ch.len_utf8()));
    ctx.record_change(RecordedChange::Replace {
        ch,
        count: count.max(1),
    });
}

/// `executeToggleCase`: flip case of `count` characters at the cursor.
pub fn execute_toggle_case(ctx: &mut VimCtx, count: usize) {
    let start_offset = ctx.offset;
    if start_offset >= ctx.text.len() {
        return;
    }
    let mut new_text = ctx.text.clone();
    let mut offset = start_offset;
    let mut toggled = 0;
    while offset < new_text.len() && toggled < count.max(1) {
        let grapheme = match new_text[offset..].chars().next() {
            Some(c) => c,
            None => break,
        };
        let upper: String = grapheme.to_uppercase().collect();
        let lower: String = grapheme.to_lowercase().collect();
        let replacement = if grapheme.to_string() == upper {
            lower
        } else {
            upper
        };
        // Caseless graphemes re-insert unchanged (matches the JS fallback).
        new_text.replace_range(offset..offset + grapheme.len_utf8(), &replacement);
        offset += replacement.len();
        toggled += 1;
    }
    ctx.set_text(new_text);
    ctx.set_offset(offset);
    ctx.record_change(RecordedChange::ToggleCase {
        count: count.max(1),
    });
}

/// `executePaste`: paste register charwise/linewise with count.
pub fn execute_paste(ctx: &mut VimCtx, after: bool, count: usize) {
    let register = ctx.persistent.register.clone();
    if register.is_empty() {
        return;
    }
    let is_linewise = register.ends_with('\n');
    let content = if is_linewise {
        register[..register.len() - 1].to_owned()
    } else {
        register
    };
    if is_linewise {
        let lines: Vec<&str> = ctx.text.split('\n').collect();
        let current_line = ctx.line_of(ctx.offset);
        let insert_line = if after {
            current_line + 1
        } else {
            current_line
        };
        let content_lines: Vec<&str> = content.split('\n').collect();
        let mut new_lines: Vec<String> = lines[..insert_line.min(lines.len())]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        for _ in 0..count.max(1) {
            new_lines.extend(content_lines.iter().map(|s| (*s).to_owned()));
        }
        new_lines.extend(
            lines[insert_line.min(lines.len())..]
                .iter()
                .map(|s| (*s).to_owned()),
        );
        let line_refs: Vec<&str> = new_lines.iter().map(String::as_str).collect();
        let new_text = new_lines.join("\n");
        let target = line_start_offset(&line_refs, insert_line.min(line_refs.len()));
        ctx.set_text(new_text);
        ctx.set_offset(target);
    } else {
        let text_to_insert = content.repeat(count.max(1));
        let insert_point = if after && ctx.offset < ctx.text.len() {
            VimCursor::new(&ctx.text, ctx.offset).right().offset
        } else {
            ctx.offset
        };
        let new_text = format!(
            "{}{}{}",
            &ctx.text[..insert_point],
            text_to_insert,
            &ctx.text[insert_point..]
        );
        let new_offset = insert_point
            + text_to_insert
                .len()
                .saturating_sub(last_char_len(&text_to_insert));
        ctx.set_text(new_text);
        ctx.set_offset(insert_point.max(new_offset));
    }
}

/// `executeJoin`: join the current line with up to `count` following lines.
pub fn execute_join(ctx: &mut VimCtx, count: usize) {
    let lines: Vec<&str> = ctx.text.split('\n').collect();
    let current_line = ctx.line_of(ctx.offset);
    if current_line >= lines.len() - 1 {
        return;
    }
    let lines_to_join = count.max(1).min(lines.len() - current_line - 1);
    let mut joined_line = lines[current_line].to_owned();
    let cursor_pos = joined_line.len();
    for i in 1..=lines_to_join {
        let next_line = lines[current_line + i].trim_start();
        if !next_line.is_empty() {
            if !joined_line.ends_with(' ') && !joined_line.is_empty() {
                joined_line.push(' ');
            }
            joined_line.push_str(next_line);
        }
    }
    let new_text = {
        let mut owned: Vec<String> = lines[..current_line]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        owned.push(joined_line.clone());
        owned.extend(
            lines[current_line + lines_to_join + 1..]
                .iter()
                .map(|s| (*s).to_owned()),
        );
        owned.join("\n")
    };
    let target = {
        let refs: Vec<&str> = new_text.split('\n').collect();
        refs.get(current_line)
            .map(|_| line_start_offset(&refs, current_line) + cursor_pos)
            .unwrap_or(0)
    };
    ctx.set_text(new_text);
    ctx.set_offset(target);
    ctx.record_change(RecordedChange::Join {
        count: count.max(1),
    });
}

/// `executeIndent`: indent/outdent `count` lines with two spaces.
pub fn execute_indent(ctx: &mut VimCtx, dir: char, count: usize) {
    let indent = "  ";
    let lines: Vec<&str> = ctx.text.split('\n').collect();
    let current_line = ctx.line_of(ctx.offset);
    let lines_to_affect = count.max(1).min(lines.len() - current_line);
    let mut owned_lines: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
    for i in 0..lines_to_affect {
        let line_idx = current_line + i;
        let line = owned_lines.get(line_idx).cloned().unwrap_or_default();
        if dir == '>' {
            owned_lines[line_idx] = format!("{indent}{line}");
        } else if line.starts_with(indent) {
            owned_lines[line_idx] = line[indent.len()..].to_owned();
        } else if line.starts_with('\t') {
            owned_lines[line_idx] = line[1..].to_owned();
        } else {
            let mut removed = 0usize;
            let mut idx = 0usize;
            let bytes: Vec<char> = line.chars().collect();
            while idx < bytes.len() && removed < indent.len() && bytes[idx].is_whitespace() {
                removed += 1;
                idx += 1;
            }
            owned_lines[line_idx] = bytes[idx..].iter().collect();
        }
    }
    let first_non_blank = owned_lines
        .get(current_line)
        .map(|l| l.len() - l.trim_start_matches(|c: char| c.is_whitespace()).len())
        .unwrap_or(0);
    let refs: Vec<&str> = owned_lines.iter().map(String::as_str).collect();
    let new_text = owned_lines.join("\n");
    let target = line_start_offset(&refs, current_line) + first_non_blank;
    ctx.set_text(new_text);
    ctx.set_offset(target);
    ctx.record_change(RecordedChange::Indent {
        dir,
        count: count.max(1),
    });
}

/// `executeOpenLine`: insert an empty line above/below and enter insert.
pub fn execute_open_line(ctx: &mut VimCtx, below: bool) {
    let lines: Vec<&str> = ctx.text.split('\n').collect();
    let current_line = ctx.line_of(ctx.offset);
    let insert_line = if below {
        current_line + 1
    } else {
        current_line
    };
    let mut result: Vec<String> = lines[..insert_line.min(lines.len())]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    result.push(String::new());
    result.extend(
        lines[insert_line.min(lines.len())..]
            .iter()
            .map(|s| (*s).to_owned()),
    );
    let refs: Vec<&str> = result.iter().map(String::as_str).collect();
    let new_text = result.join("\n");
    let target = line_start_offset(&refs, insert_line.min(refs.len()));
    ctx.set_text(new_text);
    ctx.enter_insert(target);
    ctx.record_change(RecordedChange::OpenLine { below });
}

/// `executeOperatorTextObj`: apply an operator over a discovered text object.
pub fn execute_operator_text_obj(
    ctx: &mut VimCtx,
    op: Operator,
    obj_type: char,
    scope: TextObjScope,
    count: usize,
) {
    let Some(TextObjectRange { start, end }) =
        find_text_object(&ctx.text, ctx.offset, obj_type, scope)
    else {
        return;
    };
    apply_operator(ctx, op, start, end, false);
    ctx.record_change(RecordedChange::OperatorTextObj {
        op,
        obj_type,
        scope,
        count: count.max(1),
    });
}

/// `executeOperatorG`: operator to end of file (linewise).
pub fn execute_operator_g(ctx: &mut VimCtx, op: Operator, count: usize) {
    let target = if count <= 1 {
        ctx.cursor().start_of_last_line()
    } else {
        ctx.cursor().go_to_line(count)
    };
    if target.offset == ctx.offset {
        return;
    }
    let (from, to, linewise) = get_operator_range(ctx.offset, &ctx.text, &target, "G", op, count);
    apply_operator(ctx, op, from, to, linewise);
    ctx.record_change(RecordedChange::Operator {
        op,
        motion: "G".into(),
        count,
    });
}

/// `executeOperatorGg`: operator from start of line 1 to start of line N.
pub fn execute_operator_gg(ctx: &mut VimCtx, op: Operator, count: usize) {
    let target = ctx.cursor().go_to_line(count);
    let (from, to, _) = get_operator_range(ctx.offset, &ctx.text, &target, "gg", op, count);
    apply_operator(ctx, op, from, to, true);
    ctx.record_change(RecordedChange::Operator {
        op,
        motion: "gg".into(),
        count,
    });
}
