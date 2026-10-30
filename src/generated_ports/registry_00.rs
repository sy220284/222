//! Native source export adapter registry, group 00.
#![allow(dead_code)]
use serde_json::Value;
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    match source {
        "src/constants/figures.ts" => match export {
            "BULLET_OPERATOR" => Some(super::p_7e4e3f10886b7::BULLET_OPERATOR()),
            "TEARDROP_ASTERISK" => Some(super::p_7e4e3f10886b7::TEARDROP_ASTERISK()),
            "UP_ARROW" => Some(super::p_7e4e3f10886b7::UP_ARROW()),
            "DOWN_ARROW" => Some(super::p_7e4e3f10886b7::DOWN_ARROW()),
            "LIGHTNING_BOLT" => Some(super::p_7e4e3f10886b7::LIGHTNING_BOLT()),
            "EFFORT_LOW" => Some(super::p_7e4e3f10886b7::EFFORT_LOW()),
            "EFFORT_MEDIUM" => Some(super::p_7e4e3f10886b7::EFFORT_MEDIUM()),
            "EFFORT_HIGH" => Some(super::p_7e4e3f10886b7::EFFORT_HIGH()),
            "EFFORT_MAX" => Some(super::p_7e4e3f10886b7::EFFORT_MAX()),
            "PLAY_ICON" => Some(super::p_7e4e3f10886b7::PLAY_ICON()),
            "PAUSE_ICON" => Some(super::p_7e4e3f10886b7::PAUSE_ICON()),
            "REFRESH_ARROW" => Some(super::p_7e4e3f10886b7::REFRESH_ARROW()),
            "CHANNEL_ARROW" => Some(super::p_7e4e3f10886b7::CHANNEL_ARROW()),
            "INJECTED_ARROW" => Some(super::p_7e4e3f10886b7::INJECTED_ARROW()),
            "FORK_GLYPH" => Some(super::p_7e4e3f10886b7::FORK_GLYPH()),
            "DIAMOND_OPEN" => Some(super::p_7e4e3f10886b7::DIAMOND_OPEN()),
            "DIAMOND_FILLED" => Some(super::p_7e4e3f10886b7::DIAMOND_FILLED()),
            "REFERENCE_MARK" => Some(super::p_7e4e3f10886b7::REFERENCE_MARK()),
            "FLAG_ICON" => Some(super::p_7e4e3f10886b7::FLAG_ICON()),
            "BLOCKQUOTE_BAR" => Some(super::p_7e4e3f10886b7::BLOCKQUOTE_BAR()),
            "HEAVY_HORIZONTAL" => Some(super::p_7e4e3f10886b7::HEAVY_HORIZONTAL()),
            "BRIDGE_SPINNER_FRAMES" => Some(super::p_7e4e3f10886b7::BRIDGE_SPINNER_FRAMES()),
            "BRIDGE_READY_INDICATOR" => Some(super::p_7e4e3f10886b7::BRIDGE_READY_INDICATOR()),
            "BRIDGE_FAILED_INDICATOR" => Some(super::p_7e4e3f10886b7::BRIDGE_FAILED_INDICATOR()),
            _ => None,
        },
        "src/utils/messages.ts" => match export {
            "INTERRUPT_MESSAGE" => Some(super::p_8882c41a85e4f::INTERRUPT_MESSAGE()),
            "INTERRUPT_MESSAGE_FOR_TOOL_USE" => {
                Some(super::p_8882c41a85e4f::INTERRUPT_MESSAGE_FOR_TOOL_USE())
            }
            "CANCEL_MESSAGE" => Some(super::p_8882c41a85e4f::CANCEL_MESSAGE()),
            "REJECT_MESSAGE" => Some(super::p_8882c41a85e4f::REJECT_MESSAGE()),
            "REJECT_MESSAGE_WITH_REASON_PREFIX" => {
                Some(super::p_8882c41a85e4f::REJECT_MESSAGE_WITH_REASON_PREFIX())
            }
            "SUBAGENT_REJECT_MESSAGE" => Some(super::p_8882c41a85e4f::SUBAGENT_REJECT_MESSAGE()),
            "SUBAGENT_REJECT_MESSAGE_WITH_REASON_PREFIX" => {
                Some(super::p_8882c41a85e4f::SUBAGENT_REJECT_MESSAGE_WITH_REASON_PREFIX())
            }
            "PLAN_REJECTION_PREFIX" => Some(super::p_8882c41a85e4f::PLAN_REJECTION_PREFIX()),
            "NO_RESPONSE_REQUESTED" => Some(super::p_8882c41a85e4f::NO_RESPONSE_REQUESTED()),
            "SYNTHETIC_TOOL_RESULT_PLACEHOLDER" => {
                Some(super::p_8882c41a85e4f::SYNTHETIC_TOOL_RESULT_PLACEHOLDER())
            }
            "SYNTHETIC_MODEL" => Some(super::p_8882c41a85e4f::SYNTHETIC_MODEL()),
            "PLAN_PHASE4_CONTROL" => Some(super::p_8882c41a85e4f::PLAN_PHASE4_CONTROL()),
            "buildClassifierUnavailableMessage" => Some(
                super::p_8882c41a85e4f::buildClassifierUnavailableMessage(args),
            ),
            "isSystemLocalCommandMessage" => {
                Some(super::p_8882c41a85e4f::isSystemLocalCommandMessage(args))
            }
            "wrapInSystemReminder" => Some(super::p_8882c41a85e4f::wrapInSystemReminder(args)),
            "isCompactBoundaryMessage" => {
                Some(super::p_8882c41a85e4f::isCompactBoundaryMessage(args))
            }
            _ => None,
        },
        "src/native-ts/yoga-layout/enums.ts" => match export {
            "Align" => Some(super::p_166b646bc508b::Align()),
            "BoxSizing" => Some(super::p_166b646bc508b::BoxSizing()),
            "Dimension" => Some(super::p_166b646bc508b::Dimension()),
            "Direction" => Some(super::p_166b646bc508b::Direction()),
            "Display" => Some(super::p_166b646bc508b::Display()),
            "Edge" => Some(super::p_166b646bc508b::Edge()),
            "Errata" => Some(super::p_166b646bc508b::Errata()),
            "ExperimentalFeature" => Some(super::p_166b646bc508b::ExperimentalFeature()),
            "FlexDirection" => Some(super::p_166b646bc508b::FlexDirection()),
            "Gutter" => Some(super::p_166b646bc508b::Gutter()),
            "Justify" => Some(super::p_166b646bc508b::Justify()),
            "MeasureMode" => Some(super::p_166b646bc508b::MeasureMode()),
            "Overflow" => Some(super::p_166b646bc508b::Overflow()),
            "PositionType" => Some(super::p_166b646bc508b::PositionType()),
            "Unit" => Some(super::p_166b646bc508b::Unit()),
            "Wrap" => Some(super::p_166b646bc508b::Wrap()),
            _ => None,
        },
        "src/constants/betas.ts" => match export {
            "INTERLEAVED_THINKING_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::INTERLEAVED_THINKING_BETA_HEADER())
            }
            "CONTEXT_1M_BETA_HEADER" => Some(super::p_fa5438ef3e848::CONTEXT_1M_BETA_HEADER()),
            "CONTEXT_MANAGEMENT_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::CONTEXT_MANAGEMENT_BETA_HEADER())
            }
            "STRUCTURED_OUTPUTS_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::STRUCTURED_OUTPUTS_BETA_HEADER())
            }
            "WEB_SEARCH_BETA_HEADER" => Some(super::p_fa5438ef3e848::WEB_SEARCH_BETA_HEADER()),
            "TOOL_SEARCH_BETA_HEADER_1P" => {
                Some(super::p_fa5438ef3e848::TOOL_SEARCH_BETA_HEADER_1P())
            }
            "TOOL_SEARCH_BETA_HEADER_3P" => {
                Some(super::p_fa5438ef3e848::TOOL_SEARCH_BETA_HEADER_3P())
            }
            "EFFORT_BETA_HEADER" => Some(super::p_fa5438ef3e848::EFFORT_BETA_HEADER()),
            "TASK_BUDGETS_BETA_HEADER" => Some(super::p_fa5438ef3e848::TASK_BUDGETS_BETA_HEADER()),
            "PROMPT_CACHING_SCOPE_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::PROMPT_CACHING_SCOPE_BETA_HEADER())
            }
            "FAST_MODE_BETA_HEADER" => Some(super::p_fa5438ef3e848::FAST_MODE_BETA_HEADER()),
            "REDACT_THINKING_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::REDACT_THINKING_BETA_HEADER())
            }
            "TOKEN_EFFICIENT_TOOLS_BETA_HEADER" => {
                Some(super::p_fa5438ef3e848::TOKEN_EFFICIENT_TOOLS_BETA_HEADER())
            }
            "ADVISOR_BETA_HEADER" => Some(super::p_fa5438ef3e848::ADVISOR_BETA_HEADER()),
            _ => None,
        },
        "src/ink/layout/node.ts" => match export {
            "LayoutEdge" => Some(super::p_dbe6d69b9cec8::LayoutEdge()),
            "LayoutGutter" => Some(super::p_dbe6d69b9cec8::LayoutGutter()),
            "LayoutDisplay" => Some(super::p_dbe6d69b9cec8::LayoutDisplay()),
            "LayoutFlexDirection" => Some(super::p_dbe6d69b9cec8::LayoutFlexDirection()),
            "LayoutAlign" => Some(super::p_dbe6d69b9cec8::LayoutAlign()),
            "LayoutJustify" => Some(super::p_dbe6d69b9cec8::LayoutJustify()),
            "LayoutWrap" => Some(super::p_dbe6d69b9cec8::LayoutWrap()),
            "LayoutPositionType" => Some(super::p_dbe6d69b9cec8::LayoutPositionType()),
            "LayoutOverflow" => Some(super::p_dbe6d69b9cec8::LayoutOverflow()),
            "LayoutMeasureMode" => Some(super::p_dbe6d69b9cec8::LayoutMeasureMode()),
            _ => None,
        },
        "src/vim/types.ts" => match export {
            "OPERATORS" => Some(super::p_f4d7ac6b4c8c0::OPERATORS()),
            "TEXT_OBJ_SCOPES" => Some(super::p_f4d7ac6b4c8c0::TEXT_OBJ_SCOPES()),
            "MAX_VIM_COUNT" => Some(super::p_f4d7ac6b4c8c0::MAX_VIM_COUNT()),
            "createInitialVimState" => Some(super::p_f4d7ac6b4c8c0::createInitialVimState(args)),
            "createInitialPersistentState" => {
                Some(super::p_f4d7ac6b4c8c0::createInitialPersistentState(args))
            }
            _ => None,
        },
        "src/ink/termio/ansi.ts" => match export {
            "C0" => Some(super::p_1f1e68eb35633::C0()),
            "ESC" => Some(super::p_1f1e68eb35633::ESC()),
            "BEL" => Some(super::p_1f1e68eb35633::BEL()),
            "SEP" => Some(super::p_1f1e68eb35633::SEP()),
            "ESC_TYPE" => Some(super::p_1f1e68eb35633::ESC_TYPE()),
            "isC0" => Some(super::p_1f1e68eb35633::isC0(args)),
            "isEscFinal" => Some(super::p_1f1e68eb35633::isEscFinal(args)),
            _ => None,
        },
        "src/buddy/types.ts" => match export {
            "RARITIES" => Some(super::p_0e2477efcdf43::RARITIES()),
            "EYES" => Some(super::p_0e2477efcdf43::EYES()),
            "HATS" => Some(super::p_0e2477efcdf43::HATS()),
            "STAT_NAMES" => Some(super::p_0e2477efcdf43::STAT_NAMES()),
            "RARITY_WEIGHTS" => Some(super::p_0e2477efcdf43::RARITY_WEIGHTS()),
            "RARITY_STARS" => Some(super::p_0e2477efcdf43::RARITY_STARS()),
            "RARITY_COLORS" => Some(super::p_0e2477efcdf43::RARITY_COLORS()),
            _ => None,
        },
        "src/utils/config.ts" => match export {
            "PROJECT_CONFIG_KEYS" => Some(super::p_afe390eab4f35::PROJECT_CONFIG_KEYS()),
            "CONFIG_WRITE_DISPLAY_THRESHOLD" => {
                Some(super::p_afe390eab4f35::CONFIG_WRITE_DISPLAY_THRESHOLD())
            }
            _ => None,
        },
        "src/ink/layout/geometry.ts" => match export {
            "ZERO_EDGES" => Some(super::p_bab2fac86d685::ZERO_EDGES()),
            "addEdges" => Some(super::p_bab2fac86d685::addEdges(args)),
            "resolveEdges" => Some(super::p_bab2fac86d685::resolveEdges(args)),
            "withinBounds" => Some(super::p_bab2fac86d685::withinBounds(args)),
            _ => None,
        },
        "src/ink/termio/csi.ts" => match export {
            "CSI_RANGE" => Some(super::p_bc5aed2cde71b::CSI_RANGE()),
            "CSI" => Some(super::p_bc5aed2cde71b::CSI()),
            "ERASE_DISPLAY" => Some(super::p_bc5aed2cde71b::ERASE_DISPLAY()),
            "ERASE_LINE_REGION" => Some(super::p_bc5aed2cde71b::ERASE_LINE_REGION()),
            "CURSOR_STYLES" => Some(super::p_bc5aed2cde71b::CURSOR_STYLES()),
            _ => None,
        },
        "src/types/permissions.ts" => match export {
            "EXTERNAL_PERMISSION_MODES" => {
                Some(super::p_cf19f3fcff0bf::EXTERNAL_PERMISSION_MODES())
            }
            _ => None,
        },
        "src/utils/analyzeContext.ts" => match export {
            "TOOL_TOKEN_COUNT_OVERHEAD" => {
                Some(super::p_a127d290c8efb::TOOL_TOKEN_COUNT_OVERHEAD())
            }
            _ => None,
        },
        "src/utils/sessionStorage.ts" => match export {
            "isTranscriptMessage" => Some(super::p_d6a11c5239e9b::isTranscriptMessage(args)),
            "isChainParticipant" => Some(super::p_d6a11c5239e9b::isChainParticipant(args)),
            "isCustomTitleEnabled" => Some(super::p_d6a11c5239e9b::isCustomTitleEnabled(args)),
            _ => None,
        },
        "src/utils/toolResultStorage.ts" => match export {
            "TOOL_RESULTS_SUBDIR" => Some(super::p_8aa6e76dd0af2::TOOL_RESULTS_SUBDIR()),
            "PERSISTED_OUTPUT_TAG" => Some(super::p_8aa6e76dd0af2::PERSISTED_OUTPUT_TAG()),
            "PERSISTED_OUTPUT_CLOSING_TAG" => {
                Some(super::p_8aa6e76dd0af2::PERSISTED_OUTPUT_CLOSING_TAG())
            }
            "TOOL_RESULT_CLEARED_MESSAGE" => {
                Some(super::p_8aa6e76dd0af2::TOOL_RESULT_CLEARED_MESSAGE())
            }
            "PREVIEW_SIZE_BYTES" => Some(super::p_8aa6e76dd0af2::PREVIEW_SIZE_BYTES()),
            _ => None,
        },
        "src/components/permissions/FilePermissionDialog/ideDiffConfig.ts" => match export {
            "createSingleEditDiffConfig" => {
                Some(super::p_ffb259d974835::createSingleEditDiffConfig(args))
            }
            _ => None,
        },
        "src/ink/termio/types.ts" => match export {
            "defaultStyle" => Some(super::p_506f1ba0cbb2f::defaultStyle(args)),
            _ => None,
        },
        "src/types/textInputTypes.ts" => match export {
            "isValidImagePaste" => Some(super::p_1b3d61a277808::isValidImagePaste(args)),
            _ => None,
        },
        "src/ink/selection.ts" => match export {
            "createSelectionState" => Some(super::p_9b038cbf2e98a::createSelectionState(args)),
            "hasSelection" => Some(super::p_9b038cbf2e98a::hasSelection(args)),
            _ => None,
        },
        "src/ink/termio/osc.ts" => match export {
            "OSC" => Some(super::p_95d00ece00cfd::OSC()),
            "ITERM2" => Some(super::p_95d00ece00cfd::ITERM2()),
            "PROGRESS" => Some(super::p_95d00ece00cfd::PROGRESS()),
            _ => None,
        },
        "src/utils/attachments.ts" => match export {
            "TODO_REMINDER_CONFIG" => Some(super::p_4d95b4cfbf91b::TODO_REMINDER_CONFIG()),
            "PLAN_MODE_ATTACHMENT_CONFIG" => {
                Some(super::p_4d95b4cfbf91b::PLAN_MODE_ATTACHMENT_CONFIG())
            }
            "AUTO_MODE_ATTACHMENT_CONFIG" => {
                Some(super::p_4d95b4cfbf91b::AUTO_MODE_ATTACHMENT_CONFIG())
            }
            "VERIFY_PLAN_REMINDER_CONFIG" => {
                Some(super::p_4d95b4cfbf91b::VERIFY_PLAN_REMINDER_CONFIG())
            }
            _ => None,
        },
        "src/utils/context.ts" => match export {
            "MODEL_CONTEXT_WINDOW_DEFAULT" => {
                Some(super::p_ac10a911dfef7::MODEL_CONTEXT_WINDOW_DEFAULT())
            }
            "COMPACT_MAX_OUTPUT_TOKENS" => {
                Some(super::p_ac10a911dfef7::COMPACT_MAX_OUTPUT_TOKENS())
            }
            "CAPPED_DEFAULT_MAX_TOKENS" => {
                Some(super::p_ac10a911dfef7::CAPPED_DEFAULT_MAX_TOKENS())
            }
            "ESCALATED_MAX_TOKENS" => Some(super::p_ac10a911dfef7::ESCALATED_MAX_TOKENS()),
            _ => None,
        },
        "src/utils/settings/mdm/constants.ts" => match export {
            "WINDOWS_REGISTRY_VALUE_NAME" => {
                Some(super::p_b1f9ce74def72::WINDOWS_REGISTRY_VALUE_NAME())
            }
            "PLUTIL_PATH" => Some(super::p_b1f9ce74def72::PLUTIL_PATH()),
            "PLUTIL_ARGS_PREFIX" => Some(super::p_b1f9ce74def72::PLUTIL_ARGS_PREFIX()),
            "MDM_SUBPROCESS_TIMEOUT_MS" => {
                Some(super::p_b1f9ce74def72::MDM_SUBPROCESS_TIMEOUT_MS())
            }
            _ => None,
        },
        "src/utils/shell/readOnlyCommandValidation.ts" => match export {
            "DOCKER_READ_ONLY_COMMANDS" => {
                Some(super::p_d8621b9191802::DOCKER_READ_ONLY_COMMANDS())
            }
            "RIPGREP_READ_ONLY_COMMANDS" => {
                Some(super::p_d8621b9191802::RIPGREP_READ_ONLY_COMMANDS())
            }
            "EXTERNAL_READONLY_COMMANDS" => {
                Some(super::p_d8621b9191802::EXTERNAL_READONLY_COMMANDS())
            }
            _ => None,
        },
        "src/utils/swarm/backends/types.ts" => match export {
            "isPaneBackend" => Some(super::p_a1327fd797f0d::isPaneBackend(args)),
            _ => None,
        },
        "src/constants/product.ts" => match export {
            "isRemoteSessionStaging" => Some(super::p_9ae508cd79a96::isRemoteSessionStaging(args)),
            "isRemoteSessionLocal" => Some(super::p_9ae508cd79a96::isRemoteSessionLocal(args)),
            _ => None,
        },
        "src/types/ids.ts" => match export {
            "asSessionId" => Some(super::p_87d79f34fe49a::asSessionId(args)),
            "asAgentId" => Some(super::p_87d79f34fe49a::asAgentId(args)),
            _ => None,
        },
        "src/utils/model/agent.ts" => match export {
            "getDefaultSubagentModel" => {
                Some(super::p_64a95397c4c34::getDefaultSubagentModel(args))
            }
            "getAgentModelOptions" => Some(super::p_64a95397c4c34::getAgentModelOptions(args)),
            _ => None,
        },
        "src/utils/proxy.ts" => match export {
            "getProxyUrl" => Some(super::p_e0abda6bc6c38::getProxyUrl(args)),
            "getNoProxy" => Some(super::p_e0abda6bc6c38::getNoProxy(args)),
            _ => None,
        },
        "src/utils/stringUtils.ts" => match export {
            "escapeRegExp" => Some(super::p_33ab1f755c819::escapeRegExp(args)),
            "capitalize" => Some(super::p_33ab1f755c819::capitalize(args)),
            "plural" => Some(super::p_33ab1f755c819::plural(args)),
            "firstLineOf" => Some(super::p_33ab1f755c819::firstLineOf(args)),
            "countCharInString" => Some(super::p_33ab1f755c819::countCharInString(args)),
            "normalizeFullWidthDigits" => Some(super::p_33ab1f755c819::normalizeFullWidthDigits(args)),
            "normalizeFullWidthSpace" => Some(super::p_33ab1f755c819::normalizeFullWidthSpace(args)),
            "safeJoinLines" => Some(super::p_33ab1f755c819::safeJoinLines(args)),
            "truncateToLines" => Some(super::p_33ab1f755c819::truncateToLines(args)),
            _ => None,
        },
        "src/utils/suggestions/commandSuggestions.ts" => match export {
            "isCommandInput" => Some(super::p_955a1746e1a31::isCommandInput(args)),
            "formatCommand" => Some(super::p_955a1746e1a31::formatCommand(args)),
            _ => None,
        },
        "src/vim/motions.ts" => match export {
            "isInclusiveMotion" => Some(super::p_a3cea9130934b::isInclusiveMotion(args)),
            "isLinewiseMotion" => Some(super::p_a3cea9130934b::isLinewiseMotion(args)),
            _ => None,
        },
        "src/keybindings/reservedShortcuts.ts" => match export {
            "NON_REBINDABLE" => Some(super::p_d338e19c29574::NON_REBINDABLE()),
            "TERMINAL_RESERVED" => Some(super::p_d338e19c29574::TERMINAL_RESERVED()),
            "MACOS_RESERVED" => Some(super::p_d338e19c29574::MACOS_RESERVED()),
            _ => None,
        },
        "src/keybindings/schema.ts" => match export {
            "KEYBINDING_CONTEXTS" => Some(super::p_212ef30d7fd2c::KEYBINDING_CONTEXTS()),
            "KEYBINDING_CONTEXT_DESCRIPTIONS" => {
                Some(super::p_212ef30d7fd2c::KEYBINDING_CONTEXT_DESCRIPTIONS())
            }
            "KEYBINDING_ACTIONS" => Some(super::p_212ef30d7fd2c::KEYBINDING_ACTIONS()),
            _ => None,
        },
        "src/utils/fastMode.ts" => match export {
            "FAST_MODE_MODEL_DISPLAY" => Some(super::p_9a3112a5d3add::FAST_MODE_MODEL_DISPLAY()),
            _ => None,
        },
        "src/utils/secureStorage/macOsKeychainHelpers.ts" => match export {
            "CREDENTIALS_SERVICE_SUFFIX" => {
                Some(super::p_b2a6ef22565e7::CREDENTIALS_SERVICE_SUFFIX())
            }
            "KEYCHAIN_CACHE_TTL_MS" => Some(super::p_b2a6ef22565e7::KEYCHAIN_CACHE_TTL_MS()),
            "keychainCacheState" => Some(super::p_b2a6ef22565e7::keychainCacheState()),
            _ => None,
        },
        "src/utils/ultraplan/ccrSession.ts" => match export {
            "ULTRAPLAN_TELEPORT_SENTINEL" => {
                Some(super::p_322c9d928a344::ULTRAPLAN_TELEPORT_SENTINEL())
            }
            _ => None,
        },
        "src/components/PromptInput/PromptInputFooterSuggestions.tsx" => match export {
            "OVERLAY_MAX_ITEMS" => Some(super::p_a680bef86b91f::OVERLAY_MAX_ITEMS()),
            _ => None,
        },
        "src/components/ScrollKeybindingHandler.tsx" => match export {
            "initWheelAccel" => Some(super::p_875b44513014b::initWheelAccel(args)),
            _ => None,
        },
        "src/services/PromptSuggestion/promptSuggestion.ts" => match export {
            "getPromptVariant" => Some(super::p_ac92628c81197::getPromptVariant(args)),
            _ => None,
        },
        "src/utils/advisor.ts" => match export {
            "ADVISOR_TOOL_INSTRUCTIONS" => {
                Some(super::p_f90f0871a04d8::ADVISOR_TOOL_INSTRUCTIONS())
            }
            "isAdvisorBlock" => Some(super::p_f90f0871a04d8::isAdvisorBlock(args)),
            _ => None,
        },
        "src/utils/frontmatterParser.ts" => match export {
            "parseBooleanFrontmatter" => {
                Some(super::p_2e571c4a5ca80::parseBooleanFrontmatter(args))
            }
            _ => None,
        },
        "src/utils/plugins/marketplaceHelpers.ts" => match export {
            "createPluginId" => Some(super::p_191decb55832d::createPluginId(args)),
            _ => None,
        },
        "src/utils/plugins/pluginIdentifier.ts" => match export {
            "SETTING_SOURCE_TO_SCOPE" => Some(super::p_d5db58adb3fd2::SETTING_SOURCE_TO_SCOPE()),
            "buildPluginId" => Some(super::p_d5db58adb3fd2::buildPluginId(args)),
            _ => None,
        },
        "src/utils/powershell/parser.ts" => match export {
            "PARSE_SCRIPT_BODY" => Some(super::p_c4d0f9a46fa38::PARSE_SCRIPT_BODY()),
            "getPipelineSegments" => Some(super::p_c4d0f9a46fa38::getPipelineSegments(args)),
            _ => None,
        },
        "src/utils/statsCache.ts" => match export {
            "STATS_CACHE_VERSION" => Some(super::p_b9afda97bc9a8::STATS_CACHE_VERSION()),
            "isDateBefore" => Some(super::p_b9afda97bc9a8::isDateBefore(args)),
            _ => None,
        },
        "src/utils/swarm/backends/it2Setup.ts" => match export {
            "getPythonApiInstructions" => {
                Some(super::p_e1045bf52f44a::getPythonApiInstructions(args))
            }
            _ => None,
        },
        "src/components/messages/teamMemCollapsed.tsx" => match export {
            "checkHasTeamMemOps" => Some(super::p_7ea337b79dc53::checkHasTeamMemOps(args)),
            _ => None,
        },
        "src/ink/parse-keypress.ts" => match export {
            "DECRPM_STATUS" => Some(super::p_22f64b3a15765::DECRPM_STATUS()),
            "INITIAL_STATE" => Some(super::p_22f64b3a15765::INITIAL_STATE()),
            _ => None,
        },
        "src/services/mockRateLimits.ts" => match export {
            _ => None,
        },
        "src/services/plugins/pluginOperations.ts" => match export {
            "VALID_INSTALLABLE_SCOPES" => Some(super::p_d37d4b8c26098::VALID_INSTALLABLE_SCOPES()),
            "VALID_UPDATE_SCOPES" => Some(super::p_d37d4b8c26098::VALID_UPDATE_SCOPES()),
            _ => None,
        },
        "src/services/voiceStreamSTT.ts" => match export {
            "FINALIZE_TIMEOUTS_MS" => Some(super::p_5bf6ce115b634::FINALIZE_TIMEOUTS_MS()),
            _ => None,
        },
        "src/state/AppStateStore.ts" => match export {
            "IDLE_SPECULATION_STATE" => Some(super::p_4160de63cf190::IDLE_SPECULATION_STATE()),
            _ => None,
        },
        "src/utils/ansiToSvg.ts" => match export {
            "DEFAULT_FG" => Some(super::p_a9e775f18b3e2::DEFAULT_FG()),
            "DEFAULT_BG" => Some(super::p_a9e775f18b3e2::DEFAULT_BG()),
            _ => None,
        },
        "src/utils/computerUse/common.ts" => match export {
            "COMPUTER_USE_MCP_SERVER_NAME" => {
                Some(super::p_94132d04ecb5a::COMPUTER_USE_MCP_SERVER_NAME())
            }
            "CLI_CU_CAPABILITIES" => Some(super::p_94132d04ecb5a::CLI_CU_CAPABILITIES()),
            _ => None,
        },
        "src/utils/concurrentSessions.ts" => match export {
            _ => None,
        },
        "src/utils/ide.ts" => match export {
            _ => None,
        },
        "src/utils/imageResizer.ts" => match export {
            _ => None,
        },
        "src/utils/mcpValidation.ts" => match export {
            "MCP_TOKEN_COUNT_THRESHOLD_FACTOR" => {
                Some(super::p_6287d1dbedede::MCP_TOKEN_COUNT_THRESHOLD_FACTOR())
            }
            "IMAGE_TOKEN_ESTIMATE" => Some(super::p_6287d1dbedede::IMAGE_TOKEN_ESTIMATE()),
            _ => None,
        },
        "src/utils/platform.ts" => match export {
            "SUPPORTED_PLATFORMS" => Some(super::p_dbebe9c759444::SUPPORTED_PLATFORMS()),
            _ => None,
        },
        "src/utils/plugins/fetchTelemetry.ts" => match export {
            _ => None,
        },
        "src/utils/sessionStoragePortable.ts" => match export {
            "LITE_READ_BUF_SIZE" => Some(super::p_64e1431d70c86::LITE_READ_BUF_SIZE()),
            "MAX_SANITIZED_LENGTH" => Some(super::p_64e1431d70c86::MAX_SANITIZED_LENGTH()),
            _ => None,
        },
        "src/utils/settings/constants.ts" => match export {
            "SETTING_SOURCES" => Some(super::p_6a186fe197da0::SETTING_SOURCES()),
            "SOURCES" => Some(super::p_6a186fe197da0::SOURCES()),
            _ => None,
        },
        "src/utils/teleport/api.ts" => match export {
            "CCR_BYOC_BETA" => Some(super::p_1ae7384350177::CCR_BYOC_BETA()),
            _ => None,
        },
        "src/Tool.ts" => match export {
            "toolMatchesName" => Some(super::p_2980dcfa0210a::toolMatchesName(args)),
            _ => None,
        },
        "src/buddy/CompanionSprite.tsx" => match export {
            "MIN_COLS_FOR_FULL_SPRITE" => Some(super::p_ab10542ccfa4c::MIN_COLS_FOR_FULL_SPRITE()),
            _ => None,
        },
        "src/buddy/prompt.ts" => match export {
            "companionIntroText" => Some(super::p_bf68628a27ff8::companionIntroText(args)),
            _ => None,
        },
        "src/components/ChannelDowngradeDialog.tsx" => match export {
            _ => None,
        },
        "src/components/FeedbackSurvey/TranscriptSharePrompt.tsx" => match export {
            _ => None,
        },
        "src/components/LogoV2/Clawd.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/IssueFlagBanner.tsx" => match export {
            "IssueFlagBanner" => Some(super::p_5ca89f5b33dd6::IssueFlagBanner(args)),
            _ => None,
        },
        "src/components/PromptInput/Notifications.tsx" => match export {
            "FOOTER_TEMPORARY_STATUS_TIMEOUT" => {
                Some(super::p_4375034f382f7::FOOTER_TEMPORARY_STATUS_TIMEOUT())
            }
            _ => None,
        },
        "src/components/Spinner/utils.ts" => match export {
            "toRGBColor" => Some(super::p_43185acc13130::toRGBColor(args)),
            _ => None,
        },
        "src/components/StructuredDiff/Fallback.tsx" => match export {
            _ => None,
        },
        "src/components/permissions/FilePermissionDialog/permissionOptions.tsx" => match export {
            _ => None,
        },
        "src/components/permissions/PermissionPrompt.tsx" => match export {
            _ => None,
        },
        "src/components/tasks/taskStatusUtils.tsx" => match export {
            "isTerminalStatus" => Some(super::p_75dd35b9b8459::isTerminalStatus(args)),
            _ => None,
        },
        "src/hooks/useReplBridge.tsx" => match export {
            "BRIDGE_FAILURE_DISMISS_MS" => {
                Some(super::p_c84de04e0085c::BRIDGE_FAILURE_DISMISS_MS())
            }
            _ => None,
        },
        "src/keybindings/resolver.ts" => match export {
            "keystrokesEqual" => Some(super::p_c7d9aa37cc1b1::keystrokesEqual(args)),
            _ => None,
        },
        "src/screens/REPL.tsx" => match export {
            _ => None,
        },
        "src/services/remoteManagedSettings/securityCheck.tsx" => match export {
            _ => None,
        },
        "src/utils/agentContext.ts" => match export {
            "isSubagentContext" => Some(super::p_9029c5407283f::isSubagentContext(args)),
            _ => None,
        },
        "src/utils/autoRunIssue.tsx" => match export {
            _ => None,
        },
        "src/utils/aws.ts" => match export {
            "isAwsCredentialsProviderError" => {
                Some(super::p_6ae88763d0d0f::isAwsCredentialsProviderError(args))
            }
            _ => None,
        },
        "src/utils/cleanup.ts" => match export {
            "addCleanupResults" => Some(super::p_971a6526ff4ab::addCleanupResults(args)),
            _ => None,
        },
        "src/utils/forkedAgent.ts" => match export {
            "createCacheSafeParams" => Some(super::p_c67576a3324b2::createCacheSafeParams(args)),
            _ => None,
        },
        "src/utils/plugins/mcpbHandler.ts" => match export {
            "isMcpbSource" => Some(super::p_ee9950d29aa12::isMcpbSource(args)),
            _ => None,
        },
        "src/utils/plugins/pluginOptionsStorage.ts" => match export {
            "getPluginStorageId" => Some(super::p_3b4c92a0203d8::getPluginStorageId(args)),
            _ => None,
        },
        "src/utils/plugins/pluginStartupCheck.ts" => match export {
            "isPersistableScope" => Some(super::p_6060310ec2cbf::isPersistableScope(args)),
            _ => None,
        },
        "src/utils/plugins/schemas.ts" => match export {
            "isLocalMarketplaceSource" => {
                Some(super::p_e813a809bd066::isLocalMarketplaceSource(args))
            }
            _ => None,
        },
        "src/utils/plugins/zipCache.ts" => match export {
            "isMarketplaceSourceSupportedByZipCache" => {
                Some(super::p_bb3d7b51ba520::isMarketplaceSourceSupportedByZipCache(args))
            }
            _ => None,
        },
        "src/utils/streamlinedTransform.ts" => match export {
            "shouldIncludeInStreamlined" => {
                Some(super::p_8478b958d159a::shouldIncludeInStreamlined(args))
            }
            _ => None,
        },
        "src/utils/suggestions/directoryCompletion.ts" => match export {
            "isPathLikeToken" => Some(super::p_d37e245605bfe::isPathLikeToken(args)),
            _ => None,
        },
        "src/utils/teleport.tsx" => match export {
            _ => None,
        },
        "src/utils/tokens.ts" => match export {
            "getTokenCountFromUsage" => Some(super::p_faf3dbb033a1d::getTokenCountFromUsage(args)),
            _ => None,
        },
        "src/assistant/sessionHistory.ts" => match export {
            "HISTORY_PAGE_SIZE" => Some(super::p_3532178887ef5::HISTORY_PAGE_SIZE()),
            _ => None,
        },
        "src/cli/transports/ccrClient.ts" => match export {
            _ => None,
        },
        "src/components/FeedbackSurvey/submitTranscriptShare.ts" => match export {
            _ => None,
        },
        "src/components/Spinner/teammateSelectHint.ts" => match export {
            "TEAMMATE_SELECT_HINT" => Some(super::p_9cdb888d4eefe::TEAMMATE_SELECT_HINT()),
            _ => None,
        },
        "src/components/TeleportError.tsx" => match export {
            _ => None,
        },
        _ => None,
    }
}
pub fn catalog() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "src/constants/figures.ts",
            vec![
                "BULLET_OPERATOR",
                "TEARDROP_ASTERISK",
                "UP_ARROW",
                "DOWN_ARROW",
                "LIGHTNING_BOLT",
                "EFFORT_LOW",
                "EFFORT_MEDIUM",
                "EFFORT_HIGH",
                "EFFORT_MAX",
                "PLAY_ICON",
                "PAUSE_ICON",
                "REFRESH_ARROW",
                "CHANNEL_ARROW",
                "INJECTED_ARROW",
                "FORK_GLYPH",
                "DIAMOND_OPEN",
                "DIAMOND_FILLED",
                "REFERENCE_MARK",
                "FLAG_ICON",
                "BLOCKQUOTE_BAR",
                "HEAVY_HORIZONTAL",
                "BRIDGE_SPINNER_FRAMES",
                "BRIDGE_READY_INDICATOR",
                "BRIDGE_FAILED_INDICATOR",
            ],
        ),
        (
            "src/utils/messages.ts",
            vec![
                "INTERRUPT_MESSAGE",
                "INTERRUPT_MESSAGE_FOR_TOOL_USE",
                "CANCEL_MESSAGE",
                "REJECT_MESSAGE",
                "REJECT_MESSAGE_WITH_REASON_PREFIX",
                "SUBAGENT_REJECT_MESSAGE",
                "SUBAGENT_REJECT_MESSAGE_WITH_REASON_PREFIX",
                "PLAN_REJECTION_PREFIX",
                "NO_RESPONSE_REQUESTED",
                "SYNTHETIC_TOOL_RESULT_PLACEHOLDER",
                "SYNTHETIC_MODEL",
                "PLAN_PHASE4_CONTROL",
                "buildClassifierUnavailableMessage",
                "isSystemLocalCommandMessage",
                "wrapInSystemReminder",
                "isCompactBoundaryMessage",
            ],
        ),
        (
            "src/native-ts/yoga-layout/enums.ts",
            vec![
                "Align",
                "BoxSizing",
                "Dimension",
                "Direction",
                "Display",
                "Edge",
                "Errata",
                "ExperimentalFeature",
                "FlexDirection",
                "Gutter",
                "Justify",
                "MeasureMode",
                "Overflow",
                "PositionType",
                "Unit",
                "Wrap",
            ],
        ),
        (
            "src/constants/betas.ts",
            vec![
                "INTERLEAVED_THINKING_BETA_HEADER",
                "CONTEXT_1M_BETA_HEADER",
                "CONTEXT_MANAGEMENT_BETA_HEADER",
                "STRUCTURED_OUTPUTS_BETA_HEADER",
                "WEB_SEARCH_BETA_HEADER",
                "TOOL_SEARCH_BETA_HEADER_1P",
                "TOOL_SEARCH_BETA_HEADER_3P",
                "EFFORT_BETA_HEADER",
                "TASK_BUDGETS_BETA_HEADER",
                "PROMPT_CACHING_SCOPE_BETA_HEADER",
                "FAST_MODE_BETA_HEADER",
                "REDACT_THINKING_BETA_HEADER",
                "TOKEN_EFFICIENT_TOOLS_BETA_HEADER",
                "ADVISOR_BETA_HEADER",
            ],
        ),
        (
            "src/ink/layout/node.ts",
            vec![
                "LayoutEdge",
                "LayoutGutter",
                "LayoutDisplay",
                "LayoutFlexDirection",
                "LayoutAlign",
                "LayoutJustify",
                "LayoutWrap",
                "LayoutPositionType",
                "LayoutOverflow",
                "LayoutMeasureMode",
            ],
        ),
        (
            "src/vim/types.ts",
            vec![
                "OPERATORS",
                "TEXT_OBJ_SCOPES",
                "MAX_VIM_COUNT",
                "createInitialVimState",
                "createInitialPersistentState",
                "Operator",
                "FindType",
                "TextObjScope",
            ],
        ),
        (
            "src/ink/termio/ansi.ts",
            vec!["C0", "ESC", "BEL", "SEP", "ESC_TYPE", "isC0", "isEscFinal"],
        ),
        (
            "src/buddy/types.ts",
            vec![
                "RARITIES",
                "EYES",
                "HATS",
                "STAT_NAMES",
                "RARITY_WEIGHTS",
                "RARITY_STARS",
                "RARITY_COLORS",
            ],
        ),
        (
            "src/utils/config.ts",
            vec![
                "PROJECT_CONFIG_KEYS",
                "CONFIG_WRITE_DISPLAY_THRESHOLD",
                "SerializedStructuredHistoryEntry",
                "HistoryEntry",
                "ReleaseChannel",
                "InstallMethod",
                "DiffTool",
            ],
        ),
        (
            "src/ink/layout/geometry.ts",
            vec!["ZERO_EDGES", "addEdges", "resolveEdges", "withinBounds"],
        ),
        (
            "src/ink/termio/csi.ts",
            vec![
                "CSI_RANGE",
                "CSI",
                "ERASE_DISPLAY",
                "ERASE_LINE_REGION",
                "CURSOR_STYLES",
                "CursorStyle",
            ],
        ),
        (
            "src/types/permissions.ts",
            vec![
                "EXTERNAL_PERMISSION_MODES",
                "PermissionBehavior",
                "PermissionRuleSource",
                "PermissionUpdateDestination",
                "ClassifierBehavior",
                "RiskLevel",
            ],
        ),
        (
            "src/utils/analyzeContext.ts",
            vec![
                "TOOL_TOKEN_COUNT_OVERHEAD",
                "DeferredBuiltinTool",
                "SystemToolDetail",
                "SystemPromptSectionDetail",
                "ContextData",
            ],
        ),
        (
            "src/utils/sessionStorage.ts",
            vec![
                "isTranscriptMessage",
                "isChainParticipant",
                "isCustomTitleEnabled",
            ],
        ),
        (
            "src/utils/toolResultStorage.ts",
            vec![
                "TOOL_RESULTS_SUBDIR",
                "PERSISTED_OUTPUT_TAG",
                "PERSISTED_OUTPUT_CLOSING_TAG",
                "TOOL_RESULT_CLEARED_MESSAGE",
                "PREVIEW_SIZE_BYTES",
            ],
        ),
        (
            "src/components/permissions/FilePermissionDialog/ideDiffConfig.ts",
            vec![
                "createSingleEditDiffConfig",
                "FileEdit",
                "IDEDiffConfig",
                "IDEDiffChangeInput",
            ],
        ),
        (
            "src/ink/termio/types.ts",
            vec![
                "defaultStyle",
                "NamedColor",
                "UnderlineStyle",
                "CursorDirection",
            ],
        ),
        (
            "src/types/textInputTypes.ts",
            vec![
                "isValidImagePaste",
                "VimMode",
                "PromptInputMode",
                "QueuePriority",
            ],
        ),
        (
            "src/ink/selection.ts",
            vec!["createSelectionState", "hasSelection", "FocusMove"],
        ),
        (
            "src/ink/termio/osc.ts",
            vec!["OSC", "ITERM2", "PROGRESS", "ClipboardPath"],
        ),
        (
            "src/utils/attachments.ts",
            vec![
                "TODO_REMINDER_CONFIG",
                "PLAN_MODE_ATTACHMENT_CONFIG",
                "AUTO_MODE_ATTACHMENT_CONFIG",
                "VERIFY_PLAN_REMINDER_CONFIG",
            ],
        ),
        (
            "src/utils/context.ts",
            vec![
                "MODEL_CONTEXT_WINDOW_DEFAULT",
                "COMPACT_MAX_OUTPUT_TOKENS",
                "CAPPED_DEFAULT_MAX_TOKENS",
                "ESCALATED_MAX_TOKENS",
            ],
        ),
        (
            "src/utils/settings/mdm/constants.ts",
            vec![
                "WINDOWS_REGISTRY_VALUE_NAME",
                "PLUTIL_PATH",
                "PLUTIL_ARGS_PREFIX",
                "MDM_SUBPROCESS_TIMEOUT_MS",
            ],
        ),
        (
            "src/utils/shell/readOnlyCommandValidation.ts",
            vec![
                "DOCKER_READ_ONLY_COMMANDS",
                "RIPGREP_READ_ONLY_COMMANDS",
                "EXTERNAL_READONLY_COMMANDS",
                "FlagArgType",
            ],
        ),
        (
            "src/utils/swarm/backends/types.ts",
            vec!["isPaneBackend", "BackendType", "PaneBackendType"],
        ),
        (
            "src/constants/product.ts",
            vec!["isRemoteSessionStaging", "isRemoteSessionLocal"],
        ),
        ("src/types/ids.ts", vec!["asSessionId", "asAgentId"]),
        (
            "src/utils/model/agent.ts",
            vec!["getDefaultSubagentModel", "getAgentModelOptions"],
        ),
        ("src/utils/proxy.ts", vec!["getProxyUrl", "getNoProxy"]),
        ("src/utils/stringUtils.ts", vec!["escapeRegExp", "capitalize", "plural", "firstLineOf", "countCharInString", "normalizeFullWidthDigits", "normalizeFullWidthSpace", "safeJoinLines", "truncateToLines"]),
        (
            "src/utils/suggestions/commandSuggestions.ts",
            vec!["isCommandInput", "formatCommand"],
        ),
        (
            "src/vim/motions.ts",
            vec!["isInclusiveMotion", "isLinewiseMotion"],
        ),
        (
            "src/keybindings/reservedShortcuts.ts",
            vec!["NON_REBINDABLE", "TERMINAL_RESERVED", "MACOS_RESERVED"],
        ),
        (
            "src/keybindings/schema.ts",
            vec![
                "KEYBINDING_CONTEXTS",
                "KEYBINDING_CONTEXT_DESCRIPTIONS",
                "KEYBINDING_ACTIONS",
            ],
        ),
        (
            "src/utils/fastMode.ts",
            vec![
                "FAST_MODE_MODEL_DISPLAY",
                "CooldownReason",
                "FastModeDisabledReason",
            ],
        ),
        (
            "src/utils/secureStorage/macOsKeychainHelpers.ts",
            vec![
                "CREDENTIALS_SERVICE_SUFFIX",
                "KEYCHAIN_CACHE_TTL_MS",
                "keychainCacheState",
            ],
        ),
        (
            "src/utils/ultraplan/ccrSession.ts",
            vec![
                "ULTRAPLAN_TELEPORT_SENTINEL",
                "PollFailReason",
                "UltraplanPhase",
            ],
        ),
        (
            "src/components/PromptInput/PromptInputFooterSuggestions.tsx",
            vec!["OVERLAY_MAX_ITEMS", "SuggestionType"],
        ),
        (
            "src/components/ScrollKeybindingHandler.tsx",
            vec!["initWheelAccel", "ModalPagerAction"],
        ),
        (
            "src/services/PromptSuggestion/promptSuggestion.ts",
            vec!["getPromptVariant", "PromptVariant"],
        ),
        (
            "src/utils/advisor.ts",
            vec!["ADVISOR_TOOL_INSTRUCTIONS", "isAdvisorBlock"],
        ),
        (
            "src/utils/frontmatterParser.ts",
            vec!["parseBooleanFrontmatter", "FrontmatterShell"],
        ),
        (
            "src/utils/plugins/marketplaceHelpers.ts",
            vec!["createPluginId", "EmptyMarketplaceReason"],
        ),
        (
            "src/utils/plugins/pluginIdentifier.ts",
            vec!["SETTING_SOURCE_TO_SCOPE", "buildPluginId"],
        ),
        (
            "src/utils/powershell/parser.ts",
            vec!["PARSE_SCRIPT_BODY", "getPipelineSegments"],
        ),
        (
            "src/utils/statsCache.ts",
            vec!["STATS_CACHE_VERSION", "isDateBefore"],
        ),
        (
            "src/utils/swarm/backends/it2Setup.ts",
            vec!["getPythonApiInstructions", "PythonPackageManager"],
        ),
        (
            "src/components/messages/teamMemCollapsed.tsx",
            vec!["checkHasTeamMemOps"],
        ),
        (
            "src/ink/parse-keypress.ts",
            vec!["DECRPM_STATUS", "INITIAL_STATE"],
        ),
        (
            "src/services/mockRateLimits.ts",
            vec!["MockHeaderKey", "MockScenario"],
        ),
        (
            "src/services/plugins/pluginOperations.ts",
            vec!["VALID_INSTALLABLE_SCOPES", "VALID_UPDATE_SCOPES"],
        ),
        (
            "src/services/voiceStreamSTT.ts",
            vec!["FINALIZE_TIMEOUTS_MS", "FinalizeSource"],
        ),
        (
            "src/state/AppStateStore.ts",
            vec!["IDLE_SPECULATION_STATE", "FooterItem"],
        ),
        ("src/utils/ansiToSvg.ts", vec!["DEFAULT_FG", "DEFAULT_BG"]),
        (
            "src/utils/computerUse/common.ts",
            vec!["COMPUTER_USE_MCP_SERVER_NAME", "CLI_CU_CAPABILITIES"],
        ),
        (
            "src/utils/concurrentSessions.ts",
            vec!["SessionKind", "SessionStatus"],
        ),
        (
            "src/utils/ide.ts",
            vec!["IdeType", "IDEExtensionInstallationStatus"],
        ),
        (
            "src/utils/imageResizer.ts",
            vec!["ResizeResult", "ImageBlockWithDimensions"],
        ),
        (
            "src/utils/mcpValidation.ts",
            vec!["MCP_TOKEN_COUNT_THRESHOLD_FACTOR", "IMAGE_TOKEN_ESTIMATE"],
        ),
        (
            "src/utils/platform.ts",
            vec!["SUPPORTED_PLATFORMS", "Platform"],
        ),
        (
            "src/utils/plugins/fetchTelemetry.ts",
            vec!["PluginFetchSource", "PluginFetchOutcome"],
        ),
        (
            "src/utils/sessionStoragePortable.ts",
            vec!["LITE_READ_BUF_SIZE", "MAX_SANITIZED_LENGTH"],
        ),
        (
            "src/utils/settings/constants.ts",
            vec!["SETTING_SOURCES", "SOURCES"],
        ),
        (
            "src/utils/teleport/api.ts",
            vec!["CCR_BYOC_BETA", "SessionStatus"],
        ),
        ("src/Tool.ts", vec!["toolMatchesName"]),
        (
            "src/buddy/CompanionSprite.tsx",
            vec!["MIN_COLS_FOR_FULL_SPRITE"],
        ),
        ("src/buddy/prompt.ts", vec!["companionIntroText"]),
        (
            "src/components/ChannelDowngradeDialog.tsx",
            vec!["ChannelDowngradeChoice"],
        ),
        (
            "src/components/FeedbackSurvey/TranscriptSharePrompt.tsx",
            vec!["TranscriptShareResponse"],
        ),
        ("src/components/LogoV2/Clawd.tsx", vec!["ClawdPose"]),
        (
            "src/components/PromptInput/IssueFlagBanner.tsx",
            vec!["IssueFlagBanner"],
        ),
        (
            "src/components/PromptInput/Notifications.tsx",
            vec!["FOOTER_TEMPORARY_STATUS_TIMEOUT"],
        ),
        ("src/components/Spinner/utils.ts", vec!["toRGBColor"]),
        (
            "src/components/StructuredDiff/Fallback.tsx",
            vec!["LineObject"],
        ),
        (
            "src/components/permissions/FilePermissionDialog/permissionOptions.tsx",
            vec!["FileOperationType"],
        ),
        (
            "src/components/permissions/PermissionPrompt.tsx",
            vec!["FeedbackType"],
        ),
        (
            "src/components/tasks/taskStatusUtils.tsx",
            vec!["isTerminalStatus"],
        ),
        (
            "src/hooks/useReplBridge.tsx",
            vec!["BRIDGE_FAILURE_DISMISS_MS"],
        ),
        ("src/keybindings/resolver.ts", vec!["keystrokesEqual"]),
        ("src/screens/REPL.tsx", vec!["Screen"]),
        (
            "src/services/remoteManagedSettings/securityCheck.tsx",
            vec!["SecurityCheckResult"],
        ),
        ("src/utils/agentContext.ts", vec!["isSubagentContext"]),
        ("src/utils/autoRunIssue.tsx", vec!["AutoRunIssueReason"]),
        ("src/utils/aws.ts", vec!["isAwsCredentialsProviderError"]),
        ("src/utils/cleanup.ts", vec!["addCleanupResults"]),
        ("src/utils/forkedAgent.ts", vec!["createCacheSafeParams"]),
        ("src/utils/plugins/mcpbHandler.ts", vec!["isMcpbSource"]),
        (
            "src/utils/plugins/pluginOptionsStorage.ts",
            vec!["getPluginStorageId"],
        ),
        (
            "src/utils/plugins/pluginStartupCheck.ts",
            vec!["isPersistableScope"],
        ),
        (
            "src/utils/plugins/schemas.ts",
            vec!["isLocalMarketplaceSource"],
        ),
        (
            "src/utils/plugins/zipCache.ts",
            vec!["isMarketplaceSourceSupportedByZipCache"],
        ),
        (
            "src/utils/streamlinedTransform.ts",
            vec!["shouldIncludeInStreamlined"],
        ),
        (
            "src/utils/suggestions/directoryCompletion.ts",
            vec!["isPathLikeToken"],
        ),
        ("src/utils/teleport.tsx", vec!["TeleportProgressStep"]),
        ("src/utils/tokens.ts", vec!["getTokenCountFromUsage"]),
        ("src/assistant/sessionHistory.ts", vec!["HISTORY_PAGE_SIZE"]),
        ("src/cli/transports/ccrClient.ts", vec!["CCRInitFailReason"]),
        (
            "src/components/FeedbackSurvey/submitTranscriptShare.ts",
            vec!["TranscriptShareTrigger"],
        ),
        (
            "src/components/Spinner/teammateSelectHint.ts",
            vec!["TEAMMATE_SELECT_HINT"],
        ),
        (
            "src/components/TeleportError.tsx",
            vec!["TeleportLocalErrorType"],
        ),
    ]
}
