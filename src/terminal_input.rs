//! Interactive command entry for the terminal chat session. Source references:
//! Upstream 2.1.88 BaseTextInput, useTextInput and keybindings.
//! Piped input keeps the existing line-oriented command/SDK semantics.
use crate::{generated_ports::text_input_semantics, keybindings};
use anyhow::{bail, Result};
use serde_json::json;
use std::io::{self, IsTerminal, Read, Write};

const MAX_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum PromptRead {
    Line(String),
    Cancelled,
    Eof,
}

#[derive(Default)]
struct Editor {
    text: String,
    cursor: usize,
    history_position: Option<usize>,
    draft: String,
}
impl Editor {
    fn insert(&mut self, ch: char) -> Result<()> {
        if self.text.len().saturating_add(ch.len_utf8()) > MAX_INPUT_BYTES {
            bail!("Interactive input exceeds 1 MiB");
        }
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        Ok(())
    }
    fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }
    fn right(&mut self) {
        self.cursor += self.text[self.cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0);
    }
    fn backspace(&mut self) {
        if self.cursor > 0 {
            let end = self.cursor;
            self.left();
            self.text.drain(self.cursor..end);
        }
    }
    fn delete(&mut self) {
        if self.cursor < self.text.len() {
            let end = self.cursor
                + self.text[self.cursor..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0);
            self.text.drain(self.cursor..end);
        }
    }
    fn previous(&mut self, history: &[String]) {
        let next = self.history_position.map_or(0, |i| i.saturating_add(1));
        if next >= history.len() {
            return;
        }
        if self.history_position.is_none() {
            self.draft = self.text.clone();
        }
        self.history_position = Some(next);
        self.text = history[next].clone();
        self.cursor = self.text.len();
    }
    fn next(&mut self, history: &[String]) {
        let Some(i) = self.history_position else {
            return;
        };
        if i == 0 {
            self.history_position = None;
            self.text = std::mem::take(&mut self.draft);
        } else {
            self.history_position = Some(i - 1);
            self.text = history[i - 1].clone();
        }
        self.cursor = self.text.len();
    }
    fn kill_end(&mut self) {
        let removed = self.text.split_off(self.cursor);
        crate::generated_ports::terminal_kill_ring::with_global_kill_ring(|ring| {
            ring.push(&removed, false)
        });
    }
    fn kill_start(&mut self) {
        let removed: String = self.text.drain(..self.cursor).collect();
        self.cursor = 0;
        crate::generated_ports::terminal_kill_ring::with_global_kill_ring(|ring| {
            ring.push(&removed, true)
        });
    }
    fn yank(&mut self) -> Result<()> {
        let text = crate::generated_ports::terminal_kill_ring::with_global_kill_ring(|ring| {
            ring.last().to_owned()
        });
        if self.text.len().saturating_add(text.len()) > MAX_INPUT_BYTES {
            bail!("Interactive input exceeds 1 MiB");
        }
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
        Ok(())
    }
}

/// Vim input adapter: the editor owns terminal I/O and undo snapshots,
/// while src/vim/transitions.rs owns NORMAL-mode command semantics.
#[derive(Debug)]
struct VimKeys {
    enabled: bool,
    insert: bool,
    command: crate::vim::transitions::CommandState,
    persistent: crate::vim::types::PersistentState,
    register: String,
    undo: Vec<(String, usize)>,
    inserted_text: String,
    insert_session_recorded: bool,
}

impl VimKeys {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            insert: true,
            command: crate::vim::transitions::CommandState::Idle,
            persistent: crate::vim::types::PersistentState::default(),
            register: String::new(),
            undo: Vec::new(),
            inserted_text: String::new(),
            insert_session_recorded: false,
        }
    }

    fn record(&mut self, editor: &Editor) {
        self.undo.push((editor.text.clone(), editor.cursor));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    fn execute_repeat(ctx: &mut crate::vim::operators::VimCtx) {
        use crate::vim::operators::*;
        use crate::vim::types::RecordedChange;
        let Some(last) = ctx.persistent.last_change.clone() else {
            return;
        };
        match last {
            RecordedChange::Insert { text } => {
                if ctx.text.is_char_boundary(ctx.offset) {
                    ctx.text.insert_str(ctx.offset, &text);
                    ctx.offset += text.len();
                }
            }
            RecordedChange::Operator { op, motion, count } => {
                if motion == op.key().to_string() {
                    execute_line_op(ctx, op, count);
                } else if motion == "G" {
                    execute_operator_g(ctx, op, count);
                } else if motion == "gg" {
                    execute_operator_gg(ctx, op, count);
                } else {
                    execute_operator_motion(ctx, op, &motion, count);
                }
            }
            RecordedChange::OperatorTextObj {
                op,
                obj_type,
                scope,
                count,
            } => {
                execute_operator_text_obj(ctx, op, obj_type, scope, count);
            }
            RecordedChange::OperatorFind {
                op,
                find,
                ch,
                count,
            } => {
                execute_operator_find(ctx, op, find, ch, count);
            }
            RecordedChange::Replace { ch, count } => execute_replace(ctx, ch, count),
            RecordedChange::X { count } => execute_x(ctx, count),
            RecordedChange::ToggleCase { count } => execute_toggle_case(ctx, count),
            RecordedChange::Indent { dir, count } => execute_indent(ctx, dir, count),
            RecordedChange::OpenLine { below } => execute_open_line(ctx, below),
            RecordedChange::Join { count } => execute_join(ctx, count),
        }
    }

    /// Escape switches to NORMAL; normal keystrokes use the shared source-
    /// matched state machine instead of a second simplified Vim parser.
    fn handle(&mut self, editor: &mut Editor, key: &Key) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        if *key == Key::Escape {
            if self.insert && !self.inserted_text.is_empty() {
                self.persistent.last_change = Some(crate::vim::types::RecordedChange::Insert {
                    text: std::mem::take(&mut self.inserted_text),
                });
            }
            self.insert = false;
            self.insert_session_recorded = false;
            self.command = crate::vim::transitions::CommandState::Idle;
            return Ok(true);
        }
        if self.insert {
            if matches!(key, Key::Char(_) | Key::Backspace | Key::Delete) {
                if !self.insert_session_recorded {
                    self.record(editor);
                    self.insert_session_recorded = true;
                }
                match key {
                    Key::Char(ch) => self.inserted_text.push(*ch),
                    Key::Backspace => {
                        self.inserted_text.pop();
                    }
                    _ => {}
                }
            }
            return Ok(false);
        }

        let input = match key {
            Key::Char(ch) => ch.to_string(),
            Key::Backspace | Key::Delete => String::new(),
            Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End => {
                let cursor = crate::vim::cursor::VimCursor::new(&editor.text, editor.cursor);
                editor.cursor = match key {
                    Key::Left => cursor.left().offset,
                    Key::Right => cursor.right().offset,
                    Key::Up => cursor.up_logical_line().offset,
                    Key::Down => cursor.down_logical_line().offset,
                    Key::Home => cursor.start_of_logical_line().offset,
                    Key::End => cursor.end_of_logical_line().offset,
                    _ => unreachable!(),
                };
                return Ok(true);
            }
            _ => return Ok(false),
        };

        let mut ctx = crate::vim::operators::VimCtx::new(editor.text.clone(), editor.cursor);
        ctx.persistent = self.persistent.clone();
        let _handled = crate::vim::transitions::transition(&mut self.command, &input, &mut ctx);
        if ctx.undo_requested {
            if let Some((text, cursor)) = self.undo.pop() {
                editor.text = text;
                editor.cursor = cursor;
            }
            return Ok(true);
        }
        if ctx.dot_repeat_requested {
            Self::execute_repeat(&mut ctx);
        }
        if ctx.text.len() > MAX_INPUT_BYTES {
            bail!("Interactive input exceeds 1 MiB");
        }
        let changed = ctx.text != editor.text;
        if changed {
            self.record(editor);
        }
        editor.text = ctx.text;
        editor.cursor = ctx.offset;
        self.persistent = ctx.persistent;
        self.register = self.persistent.register.clone();
        if ctx.enter_insert {
            self.insert = true;
            self.insert_session_recorded = changed;
            self.inserted_text.clear();
            self.command = crate::vim::transitions::CommandState::Idle;
        }
        // An unrecognized NORMAL key must never be inserted as plain text.
        Ok(true)
    }
}

#[derive(PartialEq, Eq, Debug)]
enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Escape,
    Ctrl(char),
    PasteStart,
    PasteEnd,
}

#[cfg(unix)]
struct RawMode(libc::termios);
#[cfg(unix)]
impl RawMode {
    fn enter() -> io::Result<Self> {
        let mut old = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: STDIN_FILENO is an attached TTY; old is valid writable storage.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, old.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: tcgetattr wrote a fully initialized termios.
        let old = unsafe { old.assume_init() };
        let mut raw = old;
        // SAFETY: raw is a valid initialized termios.
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1;
        // SAFETY: fd points to the same terminal whose state is restored on Drop.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(old))
    }
}
#[cfg(unix)]
impl Drop for RawMode {
    fn drop(&mut self) {
        // SAFETY: the terminal attributes came from this same fd.
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.0);
        }
    }
}
#[cfg(unix)]
fn byte(reader: &mut impl Read) -> io::Result<Option<u8>> {
    let mut b = [0_u8];
    if reader.read(&mut b)? == 0 {
        Ok(None)
    } else {
        Ok(Some(b[0]))
    }
}
#[cfg(unix)]
/// Decode one terminal event and return an optional byte to replay on the
/// next event. ESC followed by a normal character is two distinct inputs;
/// consuming the character while recognizing ESC loses real Vim/shortcut keys.
fn decode(reader: &mut impl Read, head: u8) -> io::Result<(Key, Option<u8>)> {
    match head {
        b'\r' | b'\n' => Ok((Key::Enter, None)),
        8 | 127 => Ok((Key::Backspace, None)),
        27 => {
            match byte(reader)? {
                Some(b'[') => {}
                other => return Ok((Key::Escape, other)),
            }
            let mut seq = String::new();
            for _ in 0..12 {
                let Some(b) = byte(reader)? else {
                    return Ok((Key::Escape, None));
                };
                seq.push(b as char);
                if b.is_ascii_alphabetic() || b == b'~' {
                    break;
                }
            }
            Ok((
                match seq.as_str() {
                    "A" => Key::Up,
                    "B" => Key::Down,
                    "C" => Key::Right,
                    "D" => Key::Left,
                    "H" | "1~" => Key::Home,
                    "F" | "4~" => Key::End,
                    "3~" => Key::Delete,
                    "200~" => Key::PasteStart,
                    "201~" => Key::PasteEnd,
                    _ => Key::Escape,
                },
                None,
            ))
        }
        n @ 1..=26 => Ok((Key::Ctrl((b'a' + n - 1) as char), None)),
        n if n < 32 => Ok((Key::Escape, None)),
        n if n < 128 => Ok((Key::Char(n as char), None)),
        n => {
            let size = if n & 0xe0 == 0xc0 {
                2
            } else if n & 0xf0 == 0xe0 {
                3
            } else if n & 0xf8 == 0xf0 {
                4
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid UTF-8 input",
                ));
            };
            let mut bytes = vec![n];
            while bytes.len() < size {
                let next = byte(reader)?.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete UTF-8 input")
                })?;
                bytes.push(next);
            }
            let s = std::str::from_utf8(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok((
                Key::Char(s.chars().next().expect("validated UTF-8 scalar")),
                None,
            ))
        }
    }
}
/// ANSI rendering of original PromptInput keyword hints. The lexical source
/// ranges are computed from the raw text; styling never mutates user input or
/// changes the cursor's visible character count.
#[cfg(unix)]
fn highlight_advanced_keywords(input: &str) -> String {
    let mut spans = crate::local_advanced_keywords::ultraplan_positions(input);
    spans.extend(crate::local_advanced_keywords::ultrareview_positions(input));
    spans.sort_by_key(|r| r.start);
    let mut out = String::new();
    let mut from = 0;
    const RAINBOW: [u8; 6] = [203, 208, 220, 82, 45, 129];
    for span in spans {
        if span.start < from {
            continue;
        }
        out.push_str(&input[from..span.start]);
        for (i, ch) in input[span.start..span.end].chars().enumerate() {
            out.push_str(&format!("\x1b[38;5;{}m{ch}", RAINBOW[i % RAINBOW.len()]));
        }
        out.push_str("\x1b[0m");
        from = span.end;
    }
    out.push_str(&input[from..]);
    out.replace('\n', "↵").replace('\t', "⇥")
}

/// Bash-only ghost completion: mirror the original history lookup on a
/// current, already-loaded prompt-history snapshot. Never complete mid-cursor
/// or while Vim is in NORMAL mode.
#[cfg(unix)]
fn bash_ghost_suffix<'a>(e: &Editor, history: &'a [String], vim: &VimKeys) -> Option<&'a str> {
    if e.cursor != e.text.len() || (vim.enabled && !vim.insert) {
        return None;
    }
    let bash_input = e.text.strip_prefix('!')?;
    crate::shell_history_completion::get_shell_history_completion(bash_input, history)
        .map(|matched| matched.suffix)
}

#[cfg(unix)]
fn redraw(
    out: &mut impl Write,
    prompt: &str,
    e: &Editor,
    vim: &VimKeys,
    history: &[String],
) -> io::Result<()> {
    let mode = if vim.enabled && !vim.insert {
        "[NORMAL] "
    } else {
        ""
    };
    let displayed = e.text.replace('\n', "↵").replace('\t', "⇥");
    let highlighted = highlight_advanced_keywords(&e.text);
    let to_cursor = e.text[..e.cursor].replace('\n', "↵").replace('\t', "⇥");
    // Ink measures terminal cells, not Unicode scalar count: CJK and emoji
    // occupy two cells while combining marks occupy none. A scalar-count
    // cursor shift lands inside glyphs and corrupts the next edit position.
    let tail = crate::generated_ports::terminal_truncate::display_width(&displayed).saturating_sub(
        crate::generated_ports::terminal_truncate::display_width(&to_cursor),
    );
    write!(out, "\r\x1b[2K{mode}{prompt}{highlighted}")?;
    if e.text.is_empty() {
        let view = text_input_semantics::render_placeholder(&json!({
            "placeholder":"Type a message or /help","value":"","showCursor":false,"focus":true,"terminalFocus":true
        }));
        if let Some(value) = view["renderedPlaceholder"].as_str() {
            write!(out, "{value}")?;
        }
    } else if tail > 0 {
        write!(out, "\x1b[{tail}D")?;
    } else if let Some(suffix) = bash_ghost_suffix(e, history, vim) {
        let ghost = suffix.replace('\n', "↵").replace('\t', "⇥");
        // Ghost text is visual only. Restore the cursor before user input;
        // it is accepted only by an explicit Right/Ctrl-F keystroke.
        write!(
            out,
            "\x1b[2m{ghost}\x1b[22m\x1b[{}D",
            crate::generated_ports::terminal_truncate::display_width(&ghost)
        )?;
    }
    out.flush()
}
#[cfg(unix)]
fn interactive(prompt: &str, history: &[String], vim_enabled: bool) -> Result<PromptRead> {
    let _raw = RawMode::enter()?;
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let file = keybindings::keybindings_path(&keybindings::config_home()?);
    let mut bindings = keybindings::KeybindingRuntime::from_disk(&file);
    bindings.register_context("Chat");
    let mut e = Editor::default();
    let mut vim = VimKeys::new(vim_enabled);
    let mut paste = false;
    let mut replay_byte = None;
    write!(out, "\x1b[?2004h")?;
    let result = (|| -> Result<PromptRead> {
        redraw(&mut out, prompt, &e, &vim, history)?;
        loop {
            let next = match replay_byte.take() {
                Some(replayed) => Some(replayed),
                None => byte(&mut input)?,
            };
            let Some(b) = next else {
                continue;
            };
            let (key, replay) = decode(&mut input, b)?;
            replay_byte = replay;
            if key == Key::PasteStart {
                paste = true;
                continue;
            }
            if key == Key::PasteEnd {
                paste = false;
                redraw(&mut out, prompt, &e, &vim, history)?;
                continue;
            }
            if paste {
                match key {
                    Key::Char(c) => e.insert(c)?,
                    Key::Enter => e.insert('\n')?,
                    Key::Backspace => e.insert('\u{8}')?,
                    _ => {}
                }
                redraw(&mut out, prompt, &e, &vim, history)?;
                continue;
            }
            if vim.handle(&mut e, &key)? {
                redraw(&mut out, prompt, &e, &vim, history)?;
                continue;
            }
            let (input_string, named, ctrl) = match &key {
                Key::Char(c) => (c.to_string(), None, false),
                Key::Ctrl(c) => (c.to_string(), None, true),
                Key::Enter => (String::new(), Some("return"), false),
                Key::Backspace => (String::new(), Some("backspace"), false),
                Key::Delete => (String::new(), Some("delete"), false),
                Key::Left => (String::new(), Some("leftArrow"), false),
                Key::Right => (String::new(), Some("rightArrow"), false),
                Key::Up => (String::new(), Some("upArrow"), false),
                Key::Down => (String::new(), Some("downArrow"), false),
                Key::Home => (String::new(), Some("home"), false),
                Key::End => (String::new(), Some("end"), false),
                Key::Escape => (String::new(), Some("escape"), false),
                Key::PasteStart | Key::PasteEnd => unreachable!(),
            };
            let event = keybindings::TerminalKey {
                input: &input_string,
                named,
                ctrl,
                ..Default::default()
            };
            let dispatch = bindings.handle_key(&event, std::time::Instant::now());
            use keybindings::Resolution;
            if matches!(
                dispatch.resolution,
                Resolution::ChordStarted(_) | Resolution::Unbound
            ) {
                continue;
            }
            let action = match &dispatch.resolution {
                Resolution::Match(a) => Some(a.as_str()),
                _ => None,
            };
            if matches!(action, Some("chat:submit")) || (action.is_none() && key == Key::Enter) {
                writeln!(out)?;
                return Ok(PromptRead::Line(e.text));
            }
            if matches!(action, Some("chat:cancel" | "app:interrupt"))
                || (action.is_none() && key == Key::Ctrl('c'))
            {
                writeln!(out, "^C")?;
                return Ok(PromptRead::Cancelled);
            }
            if matches!(action, Some("app:exit")) || (action.is_none() && key == Key::Ctrl('d')) {
                if e.text.is_empty() {
                    writeln!(out)?;
                    return Ok(PromptRead::Eof);
                }
                e.delete();
            } else {
                match (action, &key) {
                    (Some("history:previous"), _) | (None, Key::Up) => e.previous(history),
                    (Some("history:next"), _) | (None, Key::Down) => e.next(history),
                    (None, Key::Char(c)) => e.insert(*c)?,
                    (None, Key::Backspace) => e.backspace(),
                    (None, Key::Delete) => e.delete(),
                    (None, Key::Left | Key::Ctrl('b')) => e.left(),
                    (None, Key::Right | Key::Ctrl('f')) => {
                        if let Some(suffix) = bash_ghost_suffix(&e, history, &vim) {
                            if e.text.len().saturating_add(suffix.len()) > MAX_INPUT_BYTES {
                                bail!("Interactive input exceeds 1 MiB");
                            }
                            e.text.push_str(suffix);
                            e.cursor = e.text.len();
                        } else {
                            e.right();
                        }
                    }
                    (None, Key::Home | Key::Ctrl('a')) => e.cursor = 0,
                    (None, Key::End | Key::Ctrl('e')) => e.cursor = e.text.len(),
                    (None, Key::Ctrl('k')) => e.kill_end(),
                    (None, Key::Ctrl('u')) => e.kill_start(),
                    (None, Key::Ctrl('y')) => e.yank()?,
                    _ => {}
                }
            }
            redraw(&mut out, prompt, &e, &vim, history)?;
        }
    })();
    write!(out, "\x1b[?2004l")?;
    out.flush()?;
    result
}
pub fn read_prompt(prompt: &str, history: &[String], vim_enabled: bool) -> Result<PromptRead> {
    #[cfg(unix)]
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return interactive(prompt, history, vim_enabled);
    }
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        Ok(PromptRead::Eof)
    } else {
        Ok(PromptRead::Line(line))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_cursor_and_delete_are_utf8_safe() {
        let mut e = Editor::default();
        for c in "猫😀a".chars() {
            e.insert(c).unwrap();
        }
        e.left();
        e.left();
        e.backspace();
        assert_eq!(e.text, "😀a");
        assert_eq!(e.cursor, 0);
        e.delete();
        assert_eq!(e.text, "a");
    }
    #[test]
    fn history_restores_original_draft() {
        let mut e = Editor::default();
        e.insert('x').unwrap();
        let h = vec!["newest".into(), "older".into()];
        e.previous(&h);
        assert_eq!(e.text, "newest");
        e.previous(&h);
        assert_eq!(e.text, "older");
        e.next(&h);
        assert_eq!(e.text, "newest");
        e.next(&h);
        assert_eq!(e.text, "x");
    }
    #[cfg(unix)]
    #[test]
    fn redraw_uses_terminal_cell_width_for_chinese_and_emoji_cursor_positions() {
        let mut editor = Editor::default();
        for ch in "猫😀ab".chars() {
            editor.insert(ch).unwrap();
        }
        editor.cursor = "猫".len();
        let mut output = Vec::new();
        redraw(&mut output, "> ", &editor, &VimKeys::new(false), &[]).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("\x1b[4D"));
        let mut editor = Editor::default();
        for ch in "e\u{301}猫x".chars() {
            editor.insert(ch).unwrap();
        }
        editor.cursor = "e\u{301}".len();
        let mut output = Vec::new();
        redraw(&mut output, "> ", &editor, &VimKeys::new(false), &[]).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("\x1b[3D"));
    }

    #[cfg(unix)]
    #[test]
    fn bash_history_ghost_only_when_inserting_at_end() {
        let mut e = Editor::default();
        let vim = VimKeys::new(false);
        let history = vec!["!git status".to_owned(), "!git stash".to_owned()];
        for ch in "!git st".chars() {
            e.insert(ch).unwrap();
        }
        assert_eq!(bash_ghost_suffix(&e, &history, &vim), Some("atus"));
        e.left();
        assert_eq!(bash_ghost_suffix(&e, &history, &vim), None);
        e.cursor = e.text.len();
        e.text = "git st".to_owned();
        e.cursor = e.text.len();
        assert_eq!(bash_ghost_suffix(&e, &history, &vim), None);
        e.text = "!git st".to_owned();
        e.cursor = e.text.len();
        let mut normal = VimKeys::new(true);
        normal.insert = false;
        assert_eq!(bash_ghost_suffix(&e, &history, &normal), None);
    }

    #[test]
    fn size_limit_and_kill_yank() {
        let mut e = Editor::default();
        e.text = "hello world".into();
        e.cursor = 5;
        e.kill_end();
        assert_eq!(e.text, "hello");
        e.yank().unwrap();
        assert_eq!(e.text, "hello world");
        e.text = "a".repeat(MAX_INPUT_BYTES);
        e.cursor = e.text.len();
        assert!(e.insert('b').is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rainbow_hints_match_original_keyword_boundaries_without_editing_input() {
        let input = "please ultraplan work; ultrareview after";
        let highlighted = highlight_advanced_keywords(input);
        assert!(highlighted.contains("\x1b[38;5;203mu"));
        let quoted = "\u{60}ultraplan\u{60} src/ultrareview.tsx ultraplan?";
        assert_eq!(highlight_advanced_keywords(quoted), quoted);
        let with_newline = highlight_advanced_keywords("fix\nultraplan this");
        assert!(with_newline.contains("↵"));
        assert!(with_newline.contains("\x1b[38;5;203mu"));
    }

    #[cfg(unix)]
    #[test]
    fn escape_keystroke_never_consumes_the_next_user_character() {
        assert_eq!(
            decode(&mut &b"i"[..], 27).unwrap(),
            (Key::Escape, Some(b'i'))
        );
        assert_eq!(decode(&mut &b""[..], 27).unwrap(), (Key::Escape, None));
        // A complete UTF-8 character following ESC must retain its leading
        // byte and remain decodable when the remaining bytes arrive.
        let chinese = "中".as_bytes();
        let (escape, replay) = decode(&mut &chinese[..], 27).unwrap();
        assert_eq!(escape, Key::Escape);
        let (character, leftover) =
            decode(&mut &chinese[1..], replay.expect("replay leading byte")).unwrap();
        assert_eq!(character, Key::Char('中'));
        assert_eq!(leftover, None);
        let (next, replay) = decode(&mut &b"[A"[..], 27).unwrap();
        assert_eq!((next, replay), (Key::Up, None));
    }

    #[cfg(unix)]
    #[test]
    fn decode_terminal_controls_and_bracketed_paste() {
        assert_eq!(decode(&mut &b"[D"[..], 27).unwrap(), (Key::Left, None));
        assert_eq!(decode(&mut &b"[200~"[..], 27).unwrap(), (Key::PasteStart, None));
        assert_eq!(decode(&mut &b"[201~"[..], 27).unwrap(), (Key::PasteEnd, None));
        assert_eq!(
            decode(&mut &"猫".as_bytes()[1..], "猫".as_bytes()[0]).unwrap(),
            (Key::Char('猫'), None)
        );
        assert_eq!(decode(&mut &b""[..], 3).unwrap(), (Key::Ctrl('c'), None));
    }
}

#[cfg(test)]
mod vim_tests {
    use super::*;
    fn input(e: &mut Editor, state: &mut VimKeys, keys: &str) {
        for c in keys.chars() {
            if !state.handle(e, &Key::Char(c)).unwrap() {
                e.insert(c).unwrap();
            }
        }
    }
    #[test]
    fn vim_first_insert_session_is_undoable_as_one_edit() {
        let mut vim = VimKeys::new(true);
        let mut editor = Editor::default();
        input(&mut editor, &mut vim, "abc");
        vim.handle(&mut editor, &Key::Escape).unwrap();
        input(&mut editor, &mut vim, "u");
        assert_eq!(editor.text, "");
    }

    #[test]
    fn vim_insert_backspace_excludes_deleted_character_from_dot_repeat() {
        let mut vim = VimKeys::new(true);
        let mut editor = Editor::default();
        input(&mut editor, &mut vim, "ab");
        assert!(!vim.handle(&mut editor, &Key::Backspace).unwrap());
        editor.backspace();
        input(&mut editor, &mut vim, "c");
        vim.handle(&mut editor, &Key::Escape).unwrap();
        assert_eq!(editor.text, "ac");
        input(&mut editor, &mut vim, ".");
        assert_eq!(editor.text, "acac");
    }

    #[test]
    fn vim_defaults_to_insert_and_escape_switches_to_normal() {
        let mut vim = VimKeys::new(true);
        let mut e = Editor::default();
        input(&mut e, &mut vim, "你好 world");
        assert_eq!(e.text, "你好 world");
        assert!(vim.insert);
        assert!(vim.handle(&mut e, &Key::Escape).unwrap());
        assert!(!vim.insert);
        input(&mut e, &mut vim, "0wxi!");
        assert_eq!(e.text, "你好 !orld");
        assert!(vim.insert);
        assert!(vim.handle(&mut e, &Key::Escape).unwrap());
        input(&mut e, &mut vim, "u");
        assert_eq!(e.text, "你好 orld");
    }
    #[test]
    fn vim_line_motions_copy_delete_and_paste() {
        let mut vim = VimKeys::new(true);
        let mut e = Editor::default();
        e.text = "alpha\nbeta\ngamma".into();
        e.cursor = 7;
        vim.handle(&mut e, &Key::Escape).unwrap();
        input(&mut e, &mut vim, "yy");
        assert_eq!(vim.register, "beta\n");
        input(&mut e, &mut vim, "dd");
        assert_eq!(e.text, "alpha\ngamma");
        input(&mut e, &mut vim, "u");
        assert_eq!(e.text, "alpha\nbeta\ngamma");
        input(&mut e, &mut vim, "0");
        assert_eq!(e.cursor, 6);
        input(&mut e, &mut vim, "$k");
        assert_eq!(e.cursor, 4);
        input(&mut e, &mut vim, "j");
        assert_eq!(e.cursor, 10);
    }
}
