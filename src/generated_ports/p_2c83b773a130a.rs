//! Rust exports derived from original: src/components/agents/types.ts
//!
//! Agent management UI state machine types and the AGENT_PATHS constant,
//! transcribed from the original union/intersection types.

pub const AGENT_PATHS_FOLDER_NAME: &str = ".claude";
pub const AGENT_PATHS_AGENTS_DIR: &str = "agents";

/// `SettingSource | 'all' | 'built-in'` — the agent list source filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSource {
    /// A settings-layer source key (user/project/local policy file stems).
    Settings(String),
    All,
    BuiltIn,
}

/// `AgentDefinition` — the loadAgentsDir definition surface used by the
/// agent menus. Structural subset kept name-compatible.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDefinition {
    pub agent_type: String,
    pub when_to_use: String,
}

/// `ModeState` — the agents screen state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum ModeState {
    MainMenu,
    ListAgents {
        source: AgentSource,
    },
    AgentMenu {
        agent: AgentDefinition,
        previous_mode: Box<ModeState>,
    },
    ViewAgent {
        agent: AgentDefinition,
        previous_mode: Box<ModeState>,
    },
    CreateAgent,
    EditAgent {
        agent: AgentDefinition,
        previous_mode: Box<ModeState>,
    },
    DeleteConfirm {
        agent: AgentDefinition,
        previous_mode: Box<ModeState>,
    },
}

/// `AgentValidationResult`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}
