//! Terminal interaction, geometry, display and input behavior ported from 2.1.88.
#![allow(dead_code)]
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

// src/ink/events/event.ts
#[derive(Debug, Clone, Default)]
pub struct TerminalEventState {
    stopped: Arc<AtomicBool>,
}
impl TerminalEventState {
    pub fn stop_immediate_propagation(&self) {
        self.stopped.store(true, Ordering::Release)
    }
    pub fn did_stop_immediate_propagation(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
}
// src/ink/events/terminal-event.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    None,
    Capturing,
    AtTarget,
    Bubbling,
}
#[derive(Clone, Debug)]
pub struct DomTerminalEvent {
    pub event_type: String,
    pub time_stamp: std::time::Instant,
    pub bubbles: bool,
    pub cancelable: bool,
    pub target: Option<u64>,
    pub current_target: Option<u64>,
    pub phase: EventPhase,
    pub base: TerminalEventState,
    propagation_stopped: bool,
    default_prevented: bool,
}
impl DomTerminalEvent {
    pub fn new(kind: &str, bubbles: bool, cancelable: bool) -> Self {
        Self {
            event_type: kind.into(),
            time_stamp: std::time::Instant::now(),
            bubbles,
            cancelable,
            target: None,
            current_target: None,
            phase: EventPhase::None,
            base: TerminalEventState::default(),
            propagation_stopped: false,
            default_prevented: false,
        }
    }
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true
    }
    pub fn stop_immediate_propagation(&mut self) {
        self.base.stop_immediate_propagation();
        self.propagation_stopped = true;
    }
    pub fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true
        }
    }
    pub fn is_default_prevented(&self) -> bool {
        self.default_prevented
    }
    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }
    pub fn set_target(&mut self, target: Option<u64>) {
        self.target = target
    }
    pub fn set_current_target(&mut self, target: Option<u64>) {
        self.current_target = target
    }
    pub fn set_phase(&mut self, phase: EventPhase) {
        self.phase = phase
    }
}
// src/ink/events/keyboard-event.ts
#[derive(Clone, Debug, Default)]
pub struct ParsedKey {
    pub sequence: Option<String>,
    pub name: Option<String>,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub option: bool,
    pub super_key: bool,
    pub fn_key: bool,
}
#[derive(Clone, Debug)]
pub struct KeyboardEvent {
    pub base: DomTerminalEvent,
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub super_key: bool,
    pub fn_key: bool,
}
impl KeyboardEvent {
    pub fn from_parsed(parsed: &ParsedKey) -> Self {
        let seq = parsed.sequence.as_deref().unwrap_or("");
        let name = parsed.name.as_deref().unwrap_or("");
        let literal = seq.chars().count() == 1
            && seq
                .chars()
                .next()
                .is_some_and(|c| c as u32 >= 0x20 && c as u32 != 0x7f);
        let key = if parsed.ctrl {
            name
        } else if literal {
            seq
        } else if !name.is_empty() {
            name
        } else {
            seq
        };
        Self {
            base: DomTerminalEvent::new("keydown", true, true),
            key: key.into(),
            ctrl: parsed.ctrl,
            shift: parsed.shift,
            meta: parsed.meta || parsed.option,
            super_key: parsed.super_key,
            fn_key: parsed.fn_key,
        }
    }
}

// src/ink/events/focus-event.ts
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusEventKind {
    Focus,
    Blur,
}
#[derive(Debug, Clone)]
pub struct FocusEvent {
    pub kind: FocusEventKind,
    pub related_target: Option<String>,
    pub bubbles: bool,
    pub cancelable: bool,
    pub event: TerminalEventState,
}
impl FocusEvent {
    pub fn new(kind: FocusEventKind, related_target: Option<String>) -> Self {
        Self {
            kind,
            related_target,
            bubbles: true,
            cancelable: false,
            event: TerminalEventState::default(),
        }
    }
}
// src/ink/events/terminal-focus-event.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalFocusEventType {
    Focus,
    Blur,
}
pub fn parse_terminal_focus(bytes: &str) -> Option<TerminalFocusEventType> {
    if bytes.ends_with("\x1b[I") {
        Some(TerminalFocusEventType::Focus)
    } else if bytes.ends_with("\x1b[O") {
        Some(TerminalFocusEventType::Blur)
    } else {
        None
    }
}
// src/ink/events/click-event.ts
#[derive(Debug, Clone)]
pub struct ClickEvent {
    pub col: usize,
    pub row: usize,
    pub local_col: isize,
    pub local_row: isize,
    pub cell_is_blank: bool,
    pub event: TerminalEventState,
}
impl ClickEvent {
    pub fn new(col: usize, row: usize, cell_is_blank: bool) -> Self {
        Self {
            col,
            row,
            local_col: 0,
            local_row: 0,
            cell_is_blank,
            event: TerminalEventState::default(),
        }
    }
    pub fn relative_to(&mut self, x: isize, y: isize) {
        self.local_col = self.col as isize - x;
        self.local_row = self.row as isize - y;
    }
}
// src/ink/events/emitter.ts
pub type EventHandler = Arc<dyn Fn(&TerminalEventState) + Send + Sync>;
#[derive(Default)]
pub struct EventEmitter {
    listeners: Mutex<HashMap<String, Vec<(u64, EventHandler)>>>,
    next_id: std::sync::atomic::AtomicU64,
}
impl EventEmitter {
    pub fn on(&self, kind: &str, handler: EventHandler) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.listeners
            .lock()
            .unwrap()
            .entry(kind.into())
            .or_default()
            .push((id, handler));
        id
    }
    pub fn off(&self, kind: &str, id: u64) {
        if let Some(handlers) = self.listeners.lock().unwrap().get_mut(kind) {
            handlers.retain(|(n, _)| *n != id);
        }
    }
    pub fn emit(&self, kind: &str, event: &TerminalEventState) -> bool {
        let listeners = self
            .listeners
            .lock()
            .unwrap()
            .get(kind)
            .cloned()
            .unwrap_or_default();
        if listeners.is_empty() {
            return false;
        }
        for (_, listener) in listeners {
            listener(event);
            if event.did_stop_immediate_propagation() {
                break;
            }
        }
        true
    }
}
// src/ink/get-max-width.ts
#[derive(Clone, Copy, Debug, Default)]
pub struct ComputedBox {
    pub width: f64,
    pub height: f64,
    pub padding_left: f64,
    pub padding_right: f64,
    pub border_left: f64,
    pub border_right: f64,
}
pub fn max_text_width(node: ComputedBox) -> f64 {
    node.width - node.padding_left - node.padding_right - node.border_left - node.border_right
}
// src/ink/line-width-cache.ts
const MAX_CACHE_SIZE: usize = 4096;
#[derive(Default)]
pub struct LineWidthCache {
    values: HashMap<String, usize>,
}
impl LineWidthCache {
    pub fn line_width(&mut self, line: &str) -> usize {
        if let Some(width) = self.values.get(line) {
            return *width;
        }
        let width = crate::generated_ports::p_6aa0ce358bf6a::string_width(line);
        if self.values.len() >= MAX_CACHE_SIZE {
            self.values.clear()
        }
        self.values.insert(line.into(), width);
        width
    }
}
// src/ink/measure-element.ts
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredElement {
    pub width: f64,
    pub height: f64,
}
pub fn measure_element(box_opt: Option<ComputedBox>) -> MeasuredElement {
    box_opt.map_or(
        MeasuredElement {
            width: 0.,
            height: 0.,
        },
        |v| MeasuredElement {
            width: v.width,
            height: v.height,
        },
    )
}
// src/ink/warn.ts
pub fn not_integer_warning(value: Option<f64>, name: &str) -> Option<String> {
    let value = value?;
    if value.is_finite() && value.fract() == 0. {
        None
    } else {
        Some(format!("{name} should be an integer, got {value}"))
    }
}
// src/ink/widest-line.ts
pub fn widest_line(text: &str, cache: &mut LineWidthCache) -> usize {
    text.split('\n')
        .map(|line| cache.line_width(line))
        .max()
        .unwrap_or(0)
}
// src/ink/wrapAnsi.ts
#[derive(Default, Debug, Clone, Copy)]
pub struct WrapOptions {
    pub hard: bool,
    pub word_wrap: bool,
    pub trim: bool,
}
pub fn wrap_ansi(text: &str, columns: usize, options: WrapOptions) -> String {
    if columns == 0 {
        return text.into();
    }
    let mut rows = Vec::new();
    let mut cache = LineWidthCache::default();
    for input_line in text.split('\n') {
        let mut current = String::new();
        for word in input_line.split_inclusive(' ') {
            let width = cache.line_width(&current) + cache.line_width(word);
            if width > columns && !current.is_empty() {
                rows.push(if options.trim {
                    current.trim_end().into()
                } else {
                    current.clone()
                });
                current.clear();
            }
            if options.hard && cache.line_width(word) > columns {
                for ch in word.chars() {
                    if cache.line_width(&current) >= columns {
                        rows.push(std::mem::take(&mut current));
                    }
                    current.push(ch);
                }
            } else {
                current.push_str(word)
            }
        }
        rows.push(if options.trim {
            current.trim_end().into()
        } else {
            current
        });
    }
    rows.join("\n")
}
// src/ink/hooks/use-terminal-focus.ts
#[derive(Default)]
pub struct TerminalFocusState {
    focused: AtomicBool,
}
impl TerminalFocusState {
    pub fn new() -> Self {
        Self {
            focused: AtomicBool::new(true),
        }
    }
    pub fn on_event(&self, event: TerminalFocusEventType) {
        self.focused
            .store(event == TerminalFocusEventType::Focus, Ordering::Release)
    }
    pub fn is_focused(&self) -> bool {
        self.focused.load(Ordering::Acquire)
    }
}
// src/ink/hooks/use-terminal-title.ts
pub fn osc_terminal_title(title: Option<&str>) -> Option<String> {
    let title = title?;
    let mut clean = String::new();
    let mut ansi = false;
    for ch in title.chars() {
        if ch == '\x1b' {
            ansi = true;
            continue;
        }
        if ansi {
            if ('@'..='~').contains(&ch) {
                ansi = false
            };
            continue;
        }
        if ch != '\x07' && ch != '\x1b' {
            clean.push(ch)
        }
    }
    Some(format!("\x1b]0;{clean}\x07"))
}
// src/components/agents/utils.ts
pub fn agent_source_label(source: &str) -> String {
    match source {
        "all" => "Agents",
        "built-in" => "Built-in agents",
        "plugin" => "Plugin agents",
        "user" => "User",
        "project" => "Project",
        "local" => "Local",
        "policySettings" => "Managed policy",
        "managed" => "Managed",
        other => return other.to_owned(),
    }
    .into()
}
// src/components/design-system/color.ts
pub fn colorize_with_theme(
    input: &str,
    key: Option<&str>,
    palette: &HashMap<String, String>,
) -> String {
    let Some(key) = key else { return input.into() };
    let code = palette.get(key).map(String::as_str).unwrap_or(key);
    if let Some(hex) = code.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(value) = u32::from_str_radix(hex, 16) {
                return format!(
                    "\x1b[38;2;{};{};{}m{input}\x1b[39m",
                    (value >> 16) & 0xff,
                    (value >> 8) & 0xff,
                    value & 0xff
                );
            }
        }
    }
    input.into()
}
// src/components/StructuredDiff/colorDiff.ts
pub fn syntax_highlighting_enabled(config: Option<&str>) -> bool {
    !config.is_some_and(|v| {
        matches!(
            v.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}
// src/components/PromptInput/inputModes.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Prompt,
    Bash,
}
pub fn prepend_mode_character(input: &str, mode: InputMode) -> String {
    if mode == InputMode::Bash {
        format!("!{input}")
    } else {
        input.into()
    }
}
pub fn mode_from_input(input: &str) -> InputMode {
    if input.starts_with('!') {
        InputMode::Bash
    } else {
        InputMode::Prompt
    }
}
pub fn value_from_input(input: &str) -> &str {
    if mode_from_input(input) == InputMode::Bash {
        &input[1..]
    } else {
        input
    }
}
pub fn is_input_mode_character(input: &str) -> bool {
    input == "!"
}
// src/constants/errorIds.ts
pub const TOOL_USE_SUMMARY_GENERATION_FAILED: u32 = 344;
// src/constants/messages.ts
pub const NO_CONTENT_MESSAGE: &str = "(no content)";
// src/constants/turnCompletionVerbs.ts
pub const TURN_COMPLETION_VERBS: &[&str] = &[
    "Baked",
    "Brewed",
    "Churned",
    "Cogitated",
    "Cooked",
    "Crunched",
    "Sautéed",
    "Worked",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_exported_constants_match_full_original_values() {
        assert_eq!(TOOL_USE_SUMMARY_GENERATION_FAILED, 344);
        assert_eq!(NO_CONTENT_MESSAGE, "(no content)");
        let expected: &[&str] = &[
            "Baked",
            "Brewed",
            "Churned",
            "Cogitated",
            "Cooked",
            "Crunched",
            "Sautéed",
            "Worked",
        ];
        assert_eq!(TURN_COMPLETION_VERBS, expected);
    }
    #[test]
    fn event_propagation_and_click_geometry() {
        let emitter = EventEmitter::default();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let first = count.clone();
        emitter.on(
            "click",
            Arc::new(move |e| {
                first.fetch_add(1, Ordering::Relaxed);
                e.stop_immediate_propagation();
            }),
        );
        let second = count.clone();
        emitter.on(
            "click",
            Arc::new(move |_| {
                second.fetch_add(1, Ordering::Relaxed);
            }),
        );
        assert!(emitter.emit("click", &TerminalEventState::default()));
        assert_eq!(count.load(Ordering::Relaxed), 1);
        let mut click = ClickEvent::new(10, 9, false);
        click.relative_to(3, 2);
        assert_eq!((click.local_col, click.local_row), (7, 7));
    }
    #[test]
    fn ascii_control_width_and_widest_line_match_original_string_width() {
        let mut cache = LineWidthCache::default();
        assert_eq!(cache.line_width("a\tb"), 2);
        assert_eq!(cache.line_width("\t"), 0);
        assert_eq!(cache.line_width("a\r\nb"), 2);
        assert_eq!(cache.line_width("line\u{001b}[31m red\u{001b}[0m"), 8);
        assert_eq!(widest_line("a\tb\nhello", &mut cache), 5);
        assert_eq!(widest_line("", &mut cache), 0);
        // A second lookup must return exactly the same cached width.
        assert_eq!(cache.line_width("a\tb"), 2);
    }

    #[test]
    fn line_width_reuses_ansi_and_unicode_grapheme_measurement() {
        let mut cache = LineWidthCache::default();
        assert_eq!(cache.line_width("👨‍👩‍👧‍👦"), 2);
        assert_eq!(cache.line_width("🇺🇸🇨🇳"), 4);
        assert_eq!(cache.line_width("🇺"), 1);
        assert_eq!(cache.line_width("e\u{301}"), 1);
        assert_eq!(cache.line_width("1\u{fe0f}\u{20e3}"), 2);
        assert_eq!(cache.line_width("\u{1b}[31m红\u{1b}[0m"), 2);
        assert_eq!(widest_line("hi\n🇺🇸🇨🇳", &mut cache), 4);
    }

    #[test]
    fn wrapping_and_input_mode() {
        let mut cache = LineWidthCache::default();
        assert_eq!(widest_line("x\n世界", &mut cache), 4);
        assert_eq!(
            max_text_width(ComputedBox {
                width: 100.,
                padding_left: 10.,
                padding_right: 5.,
                border_left: 2.,
                border_right: 1.,
                ..Default::default()
            }),
            82.
        );
        assert_eq!(mode_from_input("!ls"), InputMode::Bash);
        assert_eq!(value_from_input("!ls"), "ls");
        assert_eq!(prepend_mode_character("ls", InputMode::Bash), "!ls");
        let parsed = ParsedKey {
            name: Some("c".into()),
            sequence: Some("\u{3}".into()),
            ctrl: true,
            ..Default::default()
        };
        let key = KeyboardEvent::from_parsed(&parsed);
        assert_eq!(key.key, "c");
        assert!(key.base.cancelable);
        let mut e = DomTerminalEvent::new("focus", true, false);
        e.prevent_default();
        assert!(!e.is_default_prevented());
        e.stop_immediate_propagation();
        assert!(e.is_propagation_stopped());
        assert_eq!(cache.line_width("👍"), 2);
        assert_eq!(NO_CONTENT_MESSAGE, "(no content)");
        assert_eq!(TURN_COMPLETION_VERBS.len(), 8);
        assert_eq!(
            osc_terminal_title(Some("Hello")).unwrap(),
            "\x1b]0;Hello\x07"
        );
    }
}
