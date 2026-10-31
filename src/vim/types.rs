//! Vim state machine types, mirrored from `src/vim/types.ts` (2.1.88).
//!
//! The types are the documentation: reading them explains how the system
//! works. See the state diagram in the original file.

/// Operator for text-mutating commands (d/c/y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}
impl Operator {
    /// Single-character key that starts the operator (`dd`/`cc`/`yy` detect
    /// doubling via this key).
    pub fn key(self) -> char {
        match self {
            Operator::Delete => 'd',
            Operator::Change => 'c',
            Operator::Yank => 'y',
        }
    }
    pub fn parse(key: char) -> Option<Operator> {
        match key {
            'd' => Some(Operator::Delete),
            'c' => Some(Operator::Change),
            'y' => Some(Operator::Yank),
            _ => None,
        }
    }
}

/// f/F/t/T find variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindType {
    FForward,
    FBackward,
    TTill,
    TBackTill,
}
impl FindType {
    pub fn parse(key: char) -> Option<FindType> {
        match key {
            'f' => Some(FindType::FForward),
            'F' => Some(FindType::FBackward),
            't' => Some(FindType::TTill),
            'T' => Some(FindType::TBackTill),
            _ => None,
        }
    }
    /// `;` repeats the find, `,` flips the direction.
    pub fn flip(self) -> FindType {
        match self {
            FindType::FForward => FindType::FBackward,
            FindType::FBackward => FindType::FForward,
            FindType::TTill => FindType::TBackTill,
            FindType::TBackTill => FindType::TTill,
        }
    }
}

/// Inner (`iw`) vs around (`aw`) text object scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjScope {
    Inner,
    Around,
}
pub fn is_text_obj_scope_key(key: char) -> bool {
    key == 'i' || key == 'a'
}
pub fn text_obj_scope(key: char) -> Option<TextObjScope> {
    match key {
        'i' => Some(TextObjScope::Inner),
        'a' => Some(TextObjScope::Around),
        _ => None,
    }
}

/// Text object types: word/WORD, quotes, bracket pairs.
pub const TEXT_OBJ_TYPES: &[char] = &[
    'w', 'W', '"', '\'', '`', '(', ')', 'b', '[', ']', '{', '}', 'B', '<', '>',
];
pub fn is_text_obj_type(key: char) -> bool {
    TEXT_OBJ_TYPES.contains(&key)
}

pub const FIND_KEYS: &[char] = &['f', 'F', 't', 'T'];
pub fn is_find_key(key: char) -> bool {
    FIND_KEYS.contains(&key)
}

/// Simple (single-key) motions accepted after an operator or on their own.
pub const SIMPLE_MOTIONS: &[char] = &[
    'h', 'l', 'j', 'k', // Basic movement
    'w', 'b', 'e', 'W', 'B', 'E', // Word motions
    '0', '^', '$', // Line positions
];
pub fn is_simple_motion(key: char) -> bool {
    SIMPLE_MOTIONS.contains(&key)
}

/// Hard cap so `9999999dd` cannot DoS the editor.
pub const MAX_VIM_COUNT: usize = 10_000;

/// A recorded change, sufficient to replay the command (dot repeat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedChange {
    Insert {
        text: String,
    },
    Operator {
        op: Operator,
        motion: String,
        count: usize,
    },
    OperatorTextObj {
        op: Operator,
        obj_type: char,
        scope: TextObjScope,
        count: usize,
    },
    OperatorFind {
        op: Operator,
        find: FindType,
        ch: char,
        count: usize,
    },
    Replace {
        ch: char,
        count: usize,
    },
    X {
        count: usize,
    },
    ToggleCase {
        count: usize,
    },
    Indent {
        dir: char,
        count: usize,
    },
    OpenLine {
        below: bool,
    },
    Join {
        count: usize,
    },
}

/// Persistent "memory" of vim that survives across commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersistentState {
    pub last_change: Option<RecordedChange>,
    pub last_find: Option<(FindType, char)>,
    pub register: String,
    pub register_is_linewise: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_groups_match_original_sets() {
        assert_eq!(SIMPLE_MOTIONS.len(), 13);
        assert_eq!(FIND_KEYS.len(), 4);
        assert_eq!(TEXT_OBJ_TYPES.len(), 15);
        assert_eq!(MAX_VIM_COUNT, 10000);
        assert_eq!(Operator::parse('d'), Some(Operator::Delete));
        assert!(is_text_obj_scope_key('a') && !is_text_obj_scope_key('w'));
        assert_eq!(FindType::TTill.flip(), FindType::TBackTill);
    }
}
