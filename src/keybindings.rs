//! Keyboard mapping, configuration, validation and chord resolution.
//!
//! Migrated against src/keybindings/{parser,match,resolver,defaultBindings,
//! loadUserBindings,validate,reservedShortcuts,template,shortcutFormat}.ts.
//! Configurations are user-owned and intentionally not coupled to provider accounts.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const CONTEXTS: &[&str] = &[
    "Global",
    "Chat",
    "Autocomplete",
    "Confirmation",
    "Help",
    "Transcript",
    "HistorySearch",
    "Task",
    "ThemePicker",
    "Settings",
    "Tabs",
    "Attachments",
    "Footer",
    "MessageSelector",
    "DiffDialog",
    "ModelPicker",
    "Select",
    "Plugin",
    "Scroll",
];

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Keystroke {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub super_key: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub chord: Vec<Keystroke>,
    pub action: Option<String>,
    pub context: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub context: String,
    pub bindings: BTreeMap<String, Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub bindings: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Warning {
    pub kind: String,
    pub severity: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    Match(String),
    None,
    Unbound,
    ChordStarted(Vec<Keystroke>),
    ChordCancelled,
}

/// Represents the Ink terminal key event, including the legacy Escape/meta quirk.
#[derive(Clone, Debug, Default)]
pub struct TerminalKey<'a> {
    pub input: &'a str,
    pub named: Option<&'a str>,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub super_key: bool,
}

impl TerminalKey<'_> {
    pub fn as_keystroke(&self) -> Option<Keystroke> {
        let key = if let Some(name) = self.named {
            match name {
                "return" => "enter".to_owned(),
                "esc" => "escape".to_owned(),
                "upArrow" => "up".to_owned(),
                "downArrow" => "down".to_owned(),
                "leftArrow" => "left".to_owned(),
                "rightArrow" => "right".to_owned(),
                "wheelUp" => "wheelup".to_owned(),
                "wheelDown" => "wheeldown".to_owned(),
                other => other.to_lowercase(),
            }
        } else if self.input.chars().count() == 1 {
            self.input.to_lowercase()
        } else {
            return None;
        };
        Some(Keystroke {
            key: key.clone(),
            ctrl: self.ctrl,
            alt: self.meta && key != "escape",
            meta: self.meta && key != "escape",
            shift: self.shift,
            super_key: self.super_key,
        })
    }
}

/// Preserve the original parser's alias handling. Config validation is separate.
pub fn parse_keystroke(input: &str) -> Keystroke {
    let mut ks = Keystroke::default();
    for part in input.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ks.ctrl = true,
            "alt" | "opt" | "option" => ks.alt = true,
            "shift" => ks.shift = true,
            "meta" => ks.meta = true,
            "cmd" | "command" | "super" | "win" => ks.super_key = true,
            "esc" => ks.key = "escape".to_owned(),
            "return" => ks.key = "enter".to_owned(),
            "space" => ks.key = " ".to_owned(),
            "↑" => ks.key = "up".to_owned(),
            "↓" => ks.key = "down".to_owned(),
            "←" => ks.key = "left".to_owned(),
            "→" => ks.key = "right".to_owned(),
            other => ks.key = other.to_owned(),
        }
    }
    ks
}

pub fn parse_chord(input: &str) -> Vec<Keystroke> {
    if input == " " {
        return vec![parse_keystroke("space")];
    }
    input.split_whitespace().map(parse_keystroke).collect()
}

pub fn display_keystroke(ks: &Keystroke, platform: &str) -> String {
    let mut parts = Vec::new();
    if ks.ctrl {
        parts.push("ctrl");
    }
    if ks.alt || ks.meta {
        parts.push(if platform == "macos" { "opt" } else { "alt" });
    }
    if ks.shift {
        parts.push("shift");
    }
    if ks.super_key {
        parts.push(if platform == "macos" { "cmd" } else { "super" });
    }
    let name = match ks.key.as_str() {
        "escape" => "Esc",
        " " => "Space",
        "enter" => "Enter",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "home" => "Home",
        "end" => "End",
        other => other,
    };
    parts.push(name);
    parts.join("+")
}

pub fn display_chord(chord: &[Keystroke], platform: &str) -> String {
    chord
        .iter()
        .map(|s| display_keystroke(s, platform))
        .collect::<Vec<_>>()
        .join(" ")
}

fn same(a: &Keystroke, b: &Keystroke) -> bool {
    a.key == b.key
        && a.ctrl == b.ctrl
        && a.shift == b.shift
        && a.super_key == b.super_key
        && (a.alt || a.meta) == (b.alt || b.meta)
}

fn same_chord(a: &[Keystroke], b: &[Keystroke]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
}

fn prefix_of(a: &[Keystroke], b: &[Keystroke]) -> bool {
    a.len() < b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
}

pub fn parse_blocks(blocks: &[Block]) -> Vec<Binding> {
    blocks
        .iter()
        .flat_map(|block| {
            block
                .bindings
                .iter()
                .map(|(chord, action)| Binding {
                    chord: parse_chord(chord),
                    action: action.clone(),
                    context: block.context.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Honors the source resolver's last-wins rule, null overrides and chord prefixes.
/// Chord prefixes masked by a later null action do not swallow the key.
pub fn resolve(
    event: &TerminalKey<'_>,
    contexts: &[&str],
    bindings: &[Binding],
    pending: Option<&[Keystroke]>,
) -> Resolution {
    if event.named == Some("escape") && pending.is_some() {
        return Resolution::ChordCancelled;
    }
    let Some(current) = event.as_keystroke() else {
        return if pending.is_some() {
            Resolution::ChordCancelled
        } else {
            Resolution::None
        };
    };
    let mut test = pending.unwrap_or(&[]).to_vec();
    test.push(current);
    let candidates: Vec<_> = bindings
        .iter()
        .filter(|b| contexts.contains(&b.context.as_str()))
        .collect();
    let mut longer: Vec<(&Binding, usize)> = Vec::new();
    for (i, binding) in candidates.iter().enumerate() {
        if prefix_of(&test, &binding.chord) {
            longer.push((binding, i));
        }
    }
    // Keys are canonicalized by terminal equivalence so alt and meta alias one another.
    let active = longer.iter().any(|(b, index)| {
        let superseded = longer
            .iter()
            .skip_while(|(_, j)| j <= index)
            .any(|(later, _)| same_chord(&b.chord, &later.chord));
        !superseded && b.action.is_some()
    });
    if active {
        return Resolution::ChordStarted(test);
    }
    let exact = candidates
        .iter()
        .filter(|b| same_chord(&b.chord, &test))
        .last();
    match exact {
        Some(b) => b
            .action
            .as_ref()
            .map(|a| Resolution::Match(a.clone()))
            .unwrap_or(Resolution::Unbound),
        None if pending.is_some() => Resolution::ChordCancelled,
        None => Resolution::None,
    }
}

pub fn display_for_action(
    action: &str,
    context: &str,
    bindings: &[Binding],
    platform: &str,
) -> Option<String> {
    bindings
        .iter()
        .rev()
        .find(|b| b.action.as_deref() == Some(action) && b.context == context)
        .map(|b| display_chord(&b.chord, platform))
}

/// Included to let the same shortcuts work with an external Ink-equivalent terminal input layer.
pub fn resolve_single(
    event: &TerminalKey<'_>,
    contexts: &[&str],
    bindings: &[Binding],
) -> Resolution {
    let Some(current) = event.as_keystroke() else {
        return Resolution::None;
    };
    bindings
        .iter()
        .filter(|b| {
            b.chord.len() == 1
                && contexts.contains(&b.context.as_str())
                && same(&b.chord[0], &current)
        })
        .last()
        .map_or(Resolution::None, |b| {
            b.action
                .as_ref()
                .map(|a| Resolution::Match(a.clone()))
                .unwrap_or(Resolution::Unbound)
        })
}

/// Default shortcuts are feature-independent; experimental shortcuts are enabled
/// by the corresponding Rust feature only when that feature's UI is present.
pub fn defaults() -> Vec<Block> {
    let platform = if cfg!(windows) { "windows" } else { "unix" };
    let mode = if platform == "windows" {
        "meta+m"
    } else {
        "shift+tab"
    };
    let paste = if platform == "windows" {
        "alt+v"
    } else {
        "ctrl+v"
    };
    let blocks: &[(&str, &[(&str, &str)])] = &[
        (
            "Global",
            &[
                ("ctrl+c", "app:interrupt"),
                ("ctrl+d", "app:exit"),
                ("ctrl+l", "app:redraw"),
                ("ctrl+t", "app:toggleTodos"),
                ("ctrl+o", "app:toggleTranscript"),
                ("ctrl+shift+o", "app:toggleTeammatePreview"),
                ("ctrl+r", "history:search"),
            ],
        ),
        (
            "Chat",
            &[
                ("escape", "chat:cancel"),
                ("ctrl+x ctrl+k", "chat:killAgents"),
                ("meta+p", "chat:modelPicker"),
                ("meta+o", "chat:fastMode"),
                ("meta+t", "chat:thinkingToggle"),
                ("enter", "chat:submit"),
                ("up", "history:previous"),
                ("down", "history:next"),
                ("ctrl+_", "chat:undo"),
                ("ctrl+shift+-", "chat:undo"),
                ("ctrl+x ctrl+e", "chat:externalEditor"),
                ("ctrl+g", "chat:externalEditor"),
                ("ctrl+s", "chat:stash"),
            ],
        ),
        (
            "Autocomplete",
            &[
                ("tab", "autocomplete:accept"),
                ("escape", "autocomplete:dismiss"),
                ("up", "autocomplete:previous"),
                ("down", "autocomplete:next"),
            ],
        ),
        (
            "Settings",
            &[
                ("escape", "confirm:no"),
                ("up", "select:previous"),
                ("down", "select:next"),
                ("k", "select:previous"),
                ("j", "select:next"),
                ("ctrl+p", "select:previous"),
                ("ctrl+n", "select:next"),
                ("space", "select:accept"),
                ("enter", "settings:close"),
                ("/", "settings:search"),
                ("r", "settings:retry"),
            ],
        ),
        (
            "Confirmation",
            &[
                ("y", "confirm:yes"),
                ("n", "confirm:no"),
                ("enter", "confirm:yes"),
                ("escape", "confirm:no"),
                ("up", "confirm:previous"),
                ("down", "confirm:next"),
                ("tab", "confirm:nextField"),
                ("space", "confirm:toggle"),
                ("shift+tab", "confirm:cycleMode"),
                ("ctrl+e", "confirm:toggleExplanation"),
                ("ctrl+d", "permission:toggleDebug"),
            ],
        ),
        (
            "Tabs",
            &[
                ("tab", "tabs:next"),
                ("shift+tab", "tabs:previous"),
                ("right", "tabs:next"),
                ("left", "tabs:previous"),
            ],
        ),
        (
            "Transcript",
            &[
                ("ctrl+e", "transcript:toggleShowAll"),
                ("ctrl+c", "transcript:exit"),
                ("escape", "transcript:exit"),
                ("q", "transcript:exit"),
            ],
        ),
        (
            "HistorySearch",
            &[
                ("ctrl+r", "historySearch:next"),
                ("escape", "historySearch:accept"),
                ("tab", "historySearch:accept"),
                ("ctrl+c", "historySearch:cancel"),
                ("enter", "historySearch:execute"),
            ],
        ),
        ("Task", &[("ctrl+b", "task:background")]),
        (
            "ThemePicker",
            &[("ctrl+t", "theme:toggleSyntaxHighlighting")],
        ),
        (
            "Scroll",
            &[
                ("pageup", "scroll:pageUp"),
                ("pagedown", "scroll:pageDown"),
                ("wheelup", "scroll:lineUp"),
                ("wheeldown", "scroll:lineDown"),
                ("ctrl+home", "scroll:top"),
                ("ctrl+end", "scroll:bottom"),
                ("ctrl+shift+c", "selection:copy"),
                ("cmd+c", "selection:copy"),
            ],
        ),
        ("Help", &[("escape", "help:dismiss")]),
        (
            "Attachments",
            &[
                ("right", "attachments:next"),
                ("left", "attachments:previous"),
                ("backspace", "attachments:remove"),
                ("delete", "attachments:remove"),
                ("down", "attachments:exit"),
                ("escape", "attachments:exit"),
            ],
        ),
        (
            "Footer",
            &[
                ("up", "footer:up"),
                ("ctrl+p", "footer:up"),
                ("down", "footer:down"),
                ("ctrl+n", "footer:down"),
                ("right", "footer:next"),
                ("left", "footer:previous"),
                ("enter", "footer:openSelected"),
                ("escape", "footer:clearSelection"),
            ],
        ),
        (
            "MessageSelector",
            &[
                ("up", "messageSelector:up"),
                ("down", "messageSelector:down"),
                ("k", "messageSelector:up"),
                ("j", "messageSelector:down"),
                ("ctrl+p", "messageSelector:up"),
                ("ctrl+n", "messageSelector:down"),
                ("ctrl+up", "messageSelector:top"),
                ("shift+up", "messageSelector:top"),
                ("meta+up", "messageSelector:top"),
                ("shift+k", "messageSelector:top"),
                ("ctrl+down", "messageSelector:bottom"),
                ("shift+down", "messageSelector:bottom"),
                ("meta+down", "messageSelector:bottom"),
                ("shift+j", "messageSelector:bottom"),
                ("enter", "messageSelector:select"),
            ],
        ),
        (
            "DiffDialog",
            &[
                ("escape", "diff:dismiss"),
                ("left", "diff:previousSource"),
                ("right", "diff:nextSource"),
                ("up", "diff:previousFile"),
                ("down", "diff:nextFile"),
                ("enter", "diff:viewDetails"),
            ],
        ),
        (
            "ModelPicker",
            &[
                ("left", "modelPicker:decreaseEffort"),
                ("right", "modelPicker:increaseEffort"),
            ],
        ),
        (
            "Select",
            &[
                ("up", "select:previous"),
                ("down", "select:next"),
                ("j", "select:next"),
                ("k", "select:previous"),
                ("ctrl+n", "select:next"),
                ("ctrl+p", "select:previous"),
                ("enter", "select:accept"),
                ("escape", "select:cancel"),
            ],
        ),
        (
            "Plugin",
            &[("space", "plugin:toggle"), ("i", "plugin:install")],
        ),
    ];
    blocks
        .iter()
        .map(|(context, pairs)| {
            let mut bindings: BTreeMap<_, _> = pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), Some((*v).to_owned())))
                .collect();
            if *context == "Chat" {
                bindings.insert(mode.to_owned(), Some("chat:cycleMode".into()));
                bindings.insert(paste.to_owned(), Some("chat:imagePaste".into()));
            }
            Block {
                context: (*context).to_owned(),
                bindings,
            }
        })
        .collect()
}

fn warning(kind: &str, severity: &str, message: impl Into<String>) -> Warning {
    Warning {
        kind: kind.into(),
        severity: severity.into(),
        message: message.into(),
    }
}

fn reserved(key: &str) -> Option<&'static str> {
    match key {
        "ctrl+c" | "ctrl+d" | "ctrl+m" => Some("cannot be rebound"),
        "ctrl+z" | "ctrl+\\" => Some("terminal-owned shortcut"),
        _ => None,
    }
}

pub fn validate(blocks: &[Block]) -> Vec<Warning> {
    let mut warnings = Vec::new();
    let mut seen: HashMap<(&str, String), Option<&str>> = HashMap::new();
    for block in blocks {
        if !CONTEXTS.contains(&block.context.as_str()) {
            warnings.push(warning(
                "invalid_context",
                "error",
                format!("Unknown keybinding context: {}", block.context),
            ));
        }
        for (chord, action) in &block.bindings {
            let parsed = parse_chord(chord);
            if parsed.is_empty()
                || parsed.iter().any(|ks| ks.key.is_empty())
                || chord.contains("++")
            {
                warnings.push(warning(
                    "parse_error",
                    "error",
                    format!("Invalid shortcut: {chord}"),
                ));
            }
            let canonical = parsed
                .iter()
                .map(|ks| {
                    format!(
                        "{}{}{}{}{}:{}",
                        ks.ctrl,
                        ks.alt || ks.meta,
                        ks.shift,
                        ks.super_key,
                        ks.key.len(),
                        ks.key
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            if let Some(previous) = seen.insert((&block.context, canonical), action.as_deref()) {
                if previous != action.as_deref() {
                    warnings.push(warning(
                        "duplicate",
                        "warning",
                        format!("Conflicting shortcut {chord} in {}", block.context),
                    ));
                }
            }
            if parsed.len() == 1 {
                let ks = &parsed[0];
                let canonical = display_chord(&parsed, "linux").to_ascii_lowercase();
                if let Some(reason) = reserved(&canonical) {
                    let severity = if canonical == "ctrl+z" {
                        "warning"
                    } else {
                        "error"
                    };
                    warnings.push(warning("reserved", severity, format!("{chord}: {reason}")));
                }
                if action.as_deref() == Some("voice:pushToTalk")
                    && ks.key.len() == 1
                    && ks.key.chars().all(char::is_alphabetic)
                    && !ks.ctrl
                    && !ks.alt
                    && !ks.meta
                    && !ks.shift
                    && !ks.super_key
                {
                    warnings.push(warning(
                        "invalid_action",
                        "warning",
                        format!("{chord}: voice hold-to-talk should use a modifier or Space"),
                    ));
                }
            }
            if let Some(action) = action {
                if action.starts_with("command:")
                    && (block.context != "Chat"
                        || action[8..].chars().any(|ch| {
                            !(ch.is_ascii_alphanumeric() || ch == ':' || ch == '-' || ch == '_')
                        }))
                {
                    warnings.push(warning(
                        "invalid_action",
                        "warning",
                        format!(
                            "Command shortcut {chord} requires Chat context and valid command name"
                        ),
                    ));
                }
            }
        }
    }
    warnings
}

pub fn keybindings_path(home: &Path) -> PathBuf {
    home.join("keybindings.json")
}

pub fn config_home() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ATLAS_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(dirs::home_dir()
        .context("Unable to determine home directory")?
        .join(".atlas"))
}

pub fn create_template(path: &Path) -> Result<bool> {
    let non_rebindable: HashSet<_> = ["ctrl+c", "ctrl+d", "ctrl+m"].into_iter().collect();
    let mut default = defaults();
    for block in &mut default {
        block
            .bindings
            .retain(|key, _| !non_rebindable.contains(key.as_str()));
    }
    let content = serde_json::to_string_pretty(&serde_json::json!({
        "$schema":"https://json.schemastore.org/claude-code-keybindings.json",
        "bindings":default,
    }))?;
    fs::create_dir_all(path.parent().context("Invalid keybindings path")?)?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(format!("{content}\n").as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e).with_context(|| format!("Cannot create {}", path.display())),
    }
}

#[derive(Clone, Debug)]
pub struct Loaded {
    pub bindings: Vec<Binding>,
    pub warnings: Vec<Warning>,
}

pub fn load_from(path: &Path) -> Loaded {
    let mut loaded = Loaded {
        bindings: parse_blocks(&defaults()),
        warnings: Vec::new(),
    };
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return loaded,
        Err(e) => {
            loaded.warnings.push(warning(
                "parse_error",
                "error",
                format!("Cannot read {}: {e}", path.display()),
            ));
            return loaded;
        }
    };
    let configuration: Configuration = match serde_json::from_str(&contents) {
        Ok(c) => c,
        Err(e) => {
            loaded.warnings.push(warning(
                "parse_error",
                "error",
                format!("Invalid keybindings.json: {e}"),
            ));
            return loaded;
        }
    };
    // Invalid bindings must not compromise the known-working defaults.
    let warnings = validate(&configuration.bindings);
    let fatal = warnings.iter().any(|w| w.severity == "error");
    loaded.warnings = warnings;
    if !fatal {
        loaded
            .bindings
            .extend(parse_blocks(&configuration.bindings));
    }
    loaded
}

/// Read from disk on each invocation: changes become effective immediately and do not
/// require an independently owned watcher/cache which could race user edits.
pub fn load_current() -> Result<Loaded> {
    Ok(load_from(&keybindings_path(&config_home()?)))
}

/// Runtime action availability is owned by callers; this only resolves gestures.
pub fn lookup_action(action: &str, context: &str) -> Result<Option<String>> {
    let loaded = load_current()?;
    Ok(display_for_action(
        action,
        context,
        &loaded.bindings,
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        },
    ))
}

pub fn inspect(action: &str, context: &str) -> Result<String> {
    let loaded = load_current()?;
    if !CONTEXTS.contains(&context) {
        bail!("Unknown shortcut context: {context}");
    }
    let mut out = String::new();
    for warning in loaded.warnings {
        out.push_str(&format!("{}: {}\n", warning.severity, warning.message));
    }
    let value = display_for_action(
        action,
        context,
        &loaded.bindings,
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        },
    );
    out.push_str(&value.unwrap_or_else(|| "(unbound)".to_owned()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_and_display_support_aliases_space_and_utf8_arrows() {
        let ks = parse_keystroke("control+option+shift+CMD+Esc");
        assert!(ks.ctrl && ks.alt && ks.shift && ks.super_key);
        assert_eq!(ks.key, "escape");
        assert_eq!(parse_chord(" ")[0].key, " ");
        assert_eq!(
            display_chord(&parse_chord("ctrl+↑ meta+return"), "macos"),
            "ctrl+↑ opt+Enter"
        );
    }
    #[test]
    fn last_wins_null_override_and_chord_behave_like_source() {
        let mut blocks = defaults();
        blocks.push(Block {
            context: "Chat".into(),
            bindings: BTreeMap::from([
                ("ctrl+x ctrl+k".into(), None),
                ("ctrl+x ctrl+e".into(), None),
                ("ctrl+x".into(), Some("chat:submit".into())),
            ]),
        });
        let bindings = parse_blocks(&blocks);
        let input = TerminalKey {
            input: "x",
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(
            resolve(&input, &["Chat"], &bindings, None),
            Resolution::Match("chat:submit".into())
        );
        let bindings = parse_blocks(&defaults());
        let state = resolve(&input, &["Chat"], &bindings, None);
        let Resolution::ChordStarted(pending) = state else {
            panic!("first chord not recognized")
        };
        assert_eq!(
            resolve(
                &TerminalKey {
                    input: "k",
                    ctrl: true,
                    ..Default::default()
                },
                &["Chat"],
                &bindings,
                Some(&pending)
            ),
            Resolution::Match("chat:killAgents".into())
        );
        assert_eq!(
            resolve(
                &TerminalKey {
                    named: Some("escape"),
                    meta: true,
                    ..Default::default()
                },
                &["Chat"],
                &bindings,
                Some(&pending)
            ),
            Resolution::ChordCancelled
        );
    }
    #[test]
    fn file_override_error_and_template_are_actual_disk_operations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        assert!(create_template(&path).unwrap());
        assert!(!create_template(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"ctrl+c\""));
        let v = load_from(&path);
        assert!(v.warnings.is_empty());
        fs::write(
            &path,
            r#"{"bindings":[{"context":"Chat","bindings":{"ctrl+o":"chat:submit"}}]}"#,
        )
        .unwrap();
        let v = load_from(&path);
        assert_eq!(
            display_for_action("chat:submit", "Chat", &v.bindings, "linux"),
            Some("ctrl+o".into())
        );
        fs::write(&path, r#"{"bindings":false}"#).unwrap();
        let v = load_from(&path);
        assert_eq!(v.warnings[0].severity, "error");
        assert_eq!(
            display_for_action("chat:submit", "Chat", &v.bindings, "linux"),
            Some("Enter".into())
        );
    }
    #[test]
    fn resolver_handles_meta_aliases_and_escape_exception() {
        let bindings = parse_blocks(&[Block {
            context: "Chat".into(),
            bindings: BTreeMap::from([
                ("alt+k".into(), Some("chat:stash".into())),
                ("escape".into(), Some("chat:cancel".into())),
            ]),
        }]);
        assert_eq!(
            resolve_single(
                &TerminalKey {
                    input: "k",
                    meta: true,
                    ..Default::default()
                },
                &["Chat"],
                &bindings
            ),
            Resolution::Match("chat:stash".into())
        );
        assert_eq!(
            resolve_single(
                &TerminalKey {
                    named: Some("escape"),
                    meta: true,
                    ..Default::default()
                },
                &["Chat"],
                &bindings
            ),
            Resolution::Match("chat:cancel".into())
        );
    }
}
