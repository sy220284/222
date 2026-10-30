//! Native source export adapter registry, group 01.
#![allow(dead_code)]
use serde_json::Value;
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    match source {
        "src/components/grove/Grove.tsx" => match export {
            _ => None,
        },
        "src/components/mcp/utils/reconnectHelpers.tsx" => match export {
            _ => None,
        },
        "src/components/permissions/BashPermissionRequest/bashToolUseOptions.tsx" => match export {
            _ => None,
        },
        "src/components/permissions/PowerShellPermissionRequest/powershellToolUseOptions.tsx" => {
            match export {
                _ => None,
            }
        }
        "src/constants/cyberRiskInstruction.ts" => match export {
            "CYBER_RISK_INSTRUCTION" => Some(super::p_96fa9b93b1ef7::CYBER_RISK_INSTRUCTION()),
            _ => None,
        },
        "src/constants/outputStyles.ts" => match export {
            "DEFAULT_OUTPUT_STYLE_NAME" => {
                Some(super::p_630eea798f5af::DEFAULT_OUTPUT_STYLE_NAME())
            }
            _ => None,
        },
        "src/constants/prompts.ts" => match export {
            "SYSTEM_PROMPT_DYNAMIC_BOUNDARY" => {
                Some(super::p_4573ed881a545::SYSTEM_PROMPT_DYNAMIC_BOUNDARY())
            }
            _ => None,
        },
        "src/constants/spinnerVerbs.ts" => match export {
            "SPINNER_VERBS" => Some(super::p_7763c000d96fd::SPINNER_VERBS()),
            _ => None,
        },
        "src/hooks/useApiKeyVerification.ts" => match export {
            _ => None,
        },
        "src/hooks/useDoublePress.ts" => match export {
            "DOUBLE_PRESS_TIMEOUT_MS" => Some(super::p_74bc565d88e8d::DOUBLE_PRESS_TIMEOUT_MS()),
            _ => None,
        },
        "src/hooks/useMemoryUsage.ts" => match export {
            _ => None,
        },
        "src/hooks/useNotifyAfterTimeout.ts" => match export {
            "DEFAULT_INTERACTION_THRESHOLD_MS" => {
                Some(super::p_a0553423b570f::DEFAULT_INTERACTION_THRESHOLD_MS())
            }
            _ => None,
        },
        "src/ink/colorize.ts" => match export {
            _ => None,
        },
        "src/ink/dom.ts" => match export {
            _ => None,
        },
        "src/ink/events/event-handlers.ts" => match export {
            "HANDLER_FOR_EVENT" => Some(super::p_9066c9e862a6f::HANDLER_FOR_EVENT()),
            _ => None,
        },
        "src/ink/frame.ts" => match export {
            _ => None,
        },
        "src/ink/hooks/use-tab-status.ts" => match export {
            _ => None,
        },
        "src/ink/render-border.ts" => match export {
            "CUSTOM_BORDER_STYLES" => Some(super::p_8fdccc85c5cbd::CUSTOM_BORDER_STYLES()),
            _ => None,
        },
        "src/ink/styles.ts" => match export {
            _ => None,
        },
        "src/ink/supports-hyperlinks.ts" => match export {
            "ADDITIONAL_HYPERLINK_TERMINALS" => {
                Some(super::p_95ad2edddae92::ADDITIONAL_HYPERLINK_TERMINALS())
            }
            _ => None,
        },
        "src/ink/terminal-focus-state.ts" => match export {
            _ => None,
        },
        "src/ink/termio/dec.ts" => match export {
            "DEC" => Some(super::p_5753cc2fee99d::DEC()),
            _ => None,
        },
        "src/keybindings/validate.ts" => match export {
            _ => None,
        },
        "src/plugins/builtinPlugins.ts" => match export {
            "BUILTIN_MARKETPLACE_NAME" => Some(super::p_2f9a1e26806ee::BUILTIN_MARKETPLACE_NAME()),
            _ => None,
        },
        "src/tools.ts" => match export {
            "TOOL_PRESETS" => Some(super::p_522c7b52c49b4::TOOL_PRESETS()),
            _ => None,
        },
        "src/types/generated/google/protobuf/timestamp.ts" => match export {
            _ => None,
        },
        "src/types/plugin.ts" => match export {
            _ => None,
        },
        "src/utils/api.ts" => match export {
            _ => None,
        },
        "src/utils/bash/ParsedCommand.ts" => match export {
            _ => None,
        },
        "src/utils/bash/parser.ts" => match export {
            _ => None,
        },
        "src/utils/bash/shellCompletion.ts" => match export {
            _ => None,
        },
        "src/utils/contextSuggestions.ts" => match export {
            _ => None,
        },
        "src/utils/debug.ts" => match export {
            _ => None,
        },
        "src/utils/doctorDiagnostic.ts" => match export {
            _ => None,
        },
        "src/utils/effort.ts" => match export {
            "EFFORT_LEVELS" => Some(super::p_062c5079e1519::EFFORT_LEVELS()),
            _ => None,
        },
        "src/utils/env.ts" => match export {
            "JETBRAINS_IDES" => Some(super::p_1580d3dd08c1e::JETBRAINS_IDES()),
            _ => None,
        },
        "src/utils/errors.ts" => match export {
            _ => None,
        },
        "src/utils/file.ts" => match export {
            "FILE_NOT_FOUND_CWD_NOTE" => Some(super::p_0ff2be80aab56::FILE_NOT_FOUND_CWD_NOTE()),
            _ => None,
        },
        "src/utils/fileRead.ts" => match export {
            _ => None,
        },
        "src/utils/fileStateCache.ts" => match export {
            "READ_FILE_STATE_CACHE_SIZE" => {
                Some(super::p_199ff998619e9::READ_FILE_STATE_CACHE_SIZE())
            }
            _ => None,
        },
        "src/utils/fingerprint.ts" => match export {
            "FINGERPRINT_SALT" => Some(super::p_2cc5445fbf7a6::FINGERPRINT_SALT()),
            _ => None,
        },
        "src/utils/ghPrStatus.ts" => match export {
            _ => None,
        },
        "src/utils/imagePaste.ts" => match export {
            "PASTE_THRESHOLD" => Some(super::p_9d14b23b8f34e::PASTE_THRESHOLD()),
            _ => None,
        },
        "src/utils/logoV2Utils.ts" => match export {
            _ => None,
        },
        "src/utils/managedEnvConstants.ts" => match export {
            "DANGEROUS_SHELL_SETTINGS" => Some(super::p_6f80998bb4902::DANGEROUS_SHELL_SETTINGS()),
            _ => None,
        },
        "src/utils/memoryFileDetection.ts" => match export {
            _ => None,
        },
        "src/utils/model/providers.ts" => match export {
            _ => None,
        },
        "src/utils/nativeInstaller/download.ts" => match export {
            "ARTIFACTORY_REGISTRY_URL" => Some(super::p_be04dcc4b58ca::ARTIFACTORY_REGISTRY_URL()),
            _ => None,
        },
        "src/utils/nativeInstaller/installer.ts" => match export {
            "VERSION_RETENTION_COUNT" => Some(super::p_878997a490a39::VERSION_RETENTION_COUNT()),
            _ => None,
        },
        "src/utils/nativeInstaller/packageManagers.ts" => match export {
            _ => None,
        },
        "src/utils/plugins/officialMarketplaceStartupCheck.ts" => match export {
            _ => None,
        },
        "src/utils/preflightChecks.tsx" => match export {
            _ => None,
        },
        "src/utils/sessionActivity.ts" => match export {
            _ => None,
        },
        "src/utils/sessionState.ts" => match export {
            _ => None,
        },
        "src/utils/settings/types.ts" => match export {
            "CUSTOMIZATION_SURFACES" => Some(super::p_71c09040fae60::CUSTOMIZATION_SURFACES()),
            _ => None,
        },
        "src/utils/shell/specPrefix.ts" => match export {
            "DEPTH_RULES" => Some(super::p_10d5fd29bc5b5::DEPTH_RULES()),
            _ => None,
        },
        "src/utils/stats.ts" => match export {
            _ => None,
        },
        "src/utils/statusNoticeDefinitions.tsx" => match export {
            _ => None,
        },
        "src/utils/streamJsonStdoutGuard.ts" => match export {
            "STDOUT_GUARD_MARKER" => Some(super::p_39a1e33432530::STDOUT_GUARD_MARKER()),
            _ => None,
        },
        "src/utils/swarm/backends/detection.ts" => match export {
            "IT2_COMMAND" => Some(super::p_5ac170fd5c1d4::IT2_COMMAND()),
            _ => None,
        },
        "src/utils/swarm/backends/teammateModeSnapshot.ts" => match export {
            _ => None,
        },
        "src/utils/systemTheme.ts" => match export {
            _ => None,
        },
        "src/utils/theme.ts" => match export {
            "THEME_NAMES" => Some(super::p_0f699233d6b14::THEME_NAMES()),
            _ => None,
        },
        "src/utils/toolSearch.ts" => match export {
            _ => None,
        },
        "src/utils/unaryLogging.ts" => match export {
            _ => None,
        },
        "src/utils/warningHandler.ts" => match export {
            "MAX_WARNING_KEYS" => Some(super::p_6123ef3cb4c62::MAX_WARNING_KEYS()),
            _ => None,
        },
        "src/buddy/useBuddyNotification.tsx" => match export {
            _ => None,
        },
        "src/cli/handlers/mcp.tsx" => match export {
            _ => None,
        },
        "src/commands/agents/agents.tsx" => match export {
            _ => None,
        },
        "src/commands/btw/btw.tsx" => match export {
            _ => None,
        },
        "src/commands/config/config.tsx" => match export {
            _ => None,
        },
        "src/commands/context/context.tsx" => match export {
            _ => None,
        },
        "src/commands/copy/copy.tsx" => match export {
            _ => None,
        },
        "src/commands/desktop/desktop.tsx" => match export {
            _ => None,
        },
        "src/commands/diff/diff.tsx" => match export {
            _ => None,
        },
        "src/commands/doctor/doctor.tsx" => match export {
            _ => None,
        },
        "src/commands/effort/effort.tsx" => match export {
            _ => None,
        },
        "src/commands/exit/exit.tsx" => match export {
            _ => None,
        },
        "src/commands/fast/fast.tsx" => match export {
            _ => None,
        },
        "src/commands/help/help.tsx" => match export {
            _ => None,
        },
        "src/commands/hooks/hooks.tsx" => match export {
            _ => None,
        },
        "src/commands/ide/ide.tsx" => match export {
            _ => None,
        },
        "src/commands/install.tsx" => match export {
            _ => None,
        },
        "src/commands/mcp/mcp.tsx" => match export {
            _ => None,
        },
        _ => None,
    }
}
pub fn catalog() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("src/components/grove/Grove.tsx", vec!["GroveDecision"]),
        (
            "src/components/mcp/utils/reconnectHelpers.tsx",
            vec!["ReconnectResult"],
        ),
        (
            "src/components/permissions/BashPermissionRequest/bashToolUseOptions.tsx",
            vec!["BashToolUseOption"],
        ),
        (
            "src/components/permissions/PowerShellPermissionRequest/powershellToolUseOptions.tsx",
            vec!["PowerShellToolUseOption"],
        ),
        (
            "src/constants/cyberRiskInstruction.ts",
            vec!["CYBER_RISK_INSTRUCTION"],
        ),
        (
            "src/constants/outputStyles.ts",
            vec!["DEFAULT_OUTPUT_STYLE_NAME"],
        ),
        (
            "src/constants/prompts.ts",
            vec!["SYSTEM_PROMPT_DYNAMIC_BOUNDARY"],
        ),
        ("src/constants/spinnerVerbs.ts", vec!["SPINNER_VERBS"]),
        (
            "src/hooks/useApiKeyVerification.ts",
            vec!["VerificationStatus"],
        ),
        (
            "src/hooks/useDoublePress.ts",
            vec!["DOUBLE_PRESS_TIMEOUT_MS"],
        ),
        ("src/hooks/useMemoryUsage.ts", vec!["MemoryUsageStatus"]),
        (
            "src/hooks/useNotifyAfterTimeout.ts",
            vec!["DEFAULT_INTERACTION_THRESHOLD_MS"],
        ),
        ("src/ink/colorize.ts", vec!["ColorType"]),
        ("src/ink/dom.ts", vec!["ElementNames"]),
        (
            "src/ink/events/event-handlers.ts",
            vec!["HANDLER_FOR_EVENT"],
        ),
        ("src/ink/frame.ts", vec!["FlickerReason"]),
        ("src/ink/hooks/use-tab-status.ts", vec!["TabStatusKind"]),
        ("src/ink/render-border.ts", vec!["CUSTOM_BORDER_STYLES"]),
        ("src/ink/styles.ts", vec!["AnsiColor"]),
        (
            "src/ink/supports-hyperlinks.ts",
            vec!["ADDITIONAL_HYPERLINK_TERMINALS"],
        ),
        (
            "src/ink/terminal-focus-state.ts",
            vec!["TerminalFocusState"],
        ),
        ("src/ink/termio/dec.ts", vec!["DEC"]),
        ("src/keybindings/validate.ts", vec!["KeybindingWarningType"]),
        (
            "src/plugins/builtinPlugins.ts",
            vec!["BUILTIN_MARKETPLACE_NAME"],
        ),
        ("src/tools.ts", vec!["TOOL_PRESETS"]),
        (
            "src/types/generated/google/protobuf/timestamp.ts",
            vec!["Timestamp"],
        ),
        ("src/types/plugin.ts", vec!["PluginComponent"]),
        ("src/utils/api.ts", vec!["CacheScope"]),
        ("src/utils/bash/ParsedCommand.ts", vec!["IParsedCommand"]),
        ("src/utils/bash/parser.ts", vec!["ParsedCommandData"]),
        (
            "src/utils/bash/shellCompletion.ts",
            vec!["ShellCompletionType"],
        ),
        (
            "src/utils/contextSuggestions.ts",
            vec!["SuggestionSeverity"],
        ),
        ("src/utils/debug.ts", vec!["DebugLogLevel"]),
        ("src/utils/doctorDiagnostic.ts", vec!["InstallationType"]),
        ("src/utils/effort.ts", vec!["EFFORT_LEVELS"]),
        ("src/utils/env.ts", vec!["JETBRAINS_IDES"]),
        ("src/utils/errors.ts", vec!["AxiosErrorKind"]),
        ("src/utils/file.ts", vec!["FILE_NOT_FOUND_CWD_NOTE"]),
        ("src/utils/fileRead.ts", vec!["LineEndingType"]),
        (
            "src/utils/fileStateCache.ts",
            vec!["READ_FILE_STATE_CACHE_SIZE"],
        ),
        ("src/utils/fingerprint.ts", vec!["FINGERPRINT_SALT"]),
        ("src/utils/ghPrStatus.ts", vec!["PrReviewState"]),
        ("src/utils/imagePaste.ts", vec!["PASTE_THRESHOLD"]),
        ("src/utils/logoV2Utils.ts", vec!["LayoutMode"]),
        (
            "src/utils/managedEnvConstants.ts",
            vec!["DANGEROUS_SHELL_SETTINGS"],
        ),
        ("src/utils/memoryFileDetection.ts", vec!["MemoryScope"]),
        ("src/utils/model/providers.ts", vec!["APIProvider"]),
        (
            "src/utils/nativeInstaller/download.ts",
            vec!["ARTIFACTORY_REGISTRY_URL"],
        ),
        (
            "src/utils/nativeInstaller/installer.ts",
            vec!["VERSION_RETENTION_COUNT"],
        ),
        (
            "src/utils/nativeInstaller/packageManagers.ts",
            vec!["PackageManager"],
        ),
        (
            "src/utils/plugins/officialMarketplaceStartupCheck.ts",
            vec!["OfficialMarketplaceSkipReason"],
        ),
        (
            "src/utils/preflightChecks.tsx",
            vec!["PreflightCheckResult"],
        ),
        (
            "src/utils/sessionActivity.ts",
            vec!["SessionActivityReason"],
        ),
        ("src/utils/sessionState.ts", vec!["SessionState"]),
        (
            "src/utils/settings/types.ts",
            vec!["CUSTOMIZATION_SURFACES"],
        ),
        ("src/utils/shell/specPrefix.ts", vec!["DEPTH_RULES"]),
        ("src/utils/stats.ts", vec!["StatsDateRange"]),
        (
            "src/utils/statusNoticeDefinitions.tsx",
            vec!["StatusNoticeType"],
        ),
        (
            "src/utils/streamJsonStdoutGuard.ts",
            vec!["STDOUT_GUARD_MARKER"],
        ),
        ("src/utils/swarm/backends/detection.ts", vec!["IT2_COMMAND"]),
        (
            "src/utils/swarm/backends/teammateModeSnapshot.ts",
            vec!["TeammateMode"],
        ),
        ("src/utils/systemTheme.ts", vec!["SystemTheme"]),
        ("src/utils/theme.ts", vec!["THEME_NAMES"]),
        ("src/utils/toolSearch.ts", vec!["ToolSearchMode"]),
        ("src/utils/unaryLogging.ts", vec!["CompletionType"]),
        ("src/utils/warningHandler.ts", vec!["MAX_WARNING_KEYS"]),
        ("src/buddy/useBuddyNotification.tsx", vec![]),
        ("src/cli/handlers/mcp.tsx", vec![]),
        ("src/commands/agents/agents.tsx", vec![]),
        ("src/commands/btw/btw.tsx", vec![]),
        ("src/commands/config/config.tsx", vec![]),
        ("src/commands/context/context.tsx", vec![]),
        ("src/commands/copy/copy.tsx", vec![]),
        ("src/commands/desktop/desktop.tsx", vec![]),
        ("src/commands/diff/diff.tsx", vec![]),
        ("src/commands/doctor/doctor.tsx", vec![]),
        ("src/commands/effort/effort.tsx", vec![]),
        ("src/commands/exit/exit.tsx", vec![]),
        ("src/commands/fast/fast.tsx", vec![]),
        ("src/commands/help/help.tsx", vec![]),
        ("src/commands/hooks/hooks.tsx", vec![]),
        ("src/commands/ide/ide.tsx", vec![]),
        ("src/commands/install.tsx", vec![]),
        ("src/commands/mcp/mcp.tsx", vec![]),
    ]
}
