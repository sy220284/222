//! Native source export adapter registry, group 03.
#![allow(dead_code)]
use serde_json::Value;
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    match source {
        "src/components/MarkdownTable.tsx" => match export {
            _ => None,
        },
        "src/components/MemoryUsageIndicator.tsx" => match export {
            _ => None,
        },
        "src/components/Message.tsx" => match export {
            _ => None,
        },
        "src/components/MessageModel.tsx" => match export {
            "view" => serde_json::to_value(super::p_0ae9a8bbc4878::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/MessageResponse.tsx" => match export {
            _ => None,
        },
        "src/components/MessageRow.tsx" => match export {
            _ => None,
        },
        "src/components/MessageSelector.tsx" => match export {
            _ => None,
        },
        "src/components/MessageTimestamp.tsx" => match export {
            "view" => serde_json::to_value(super::p_ce48428997d07::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/Messages.tsx" => match export {
            _ => None,
        },
        "src/components/ModelPicker.tsx" => match export {
            _ => None,
        },
        "src/components/NativeAutoUpdater.tsx" => match export {
            _ => None,
        },
        "src/components/NotebookEditToolUseRejectedMessage.tsx" => match export {
            "view" => serde_json::to_value(super::p_969c9d9b904d0::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/OffscreenFreeze.tsx" => match export {
            _ => None,
        },
        "src/components/OutputStylePicker.tsx" => match export {
            _ => None,
        },
        "src/components/PackageManagerAutoUpdater.tsx" => match export {
            _ => None,
        },
        "src/components/PrBadge.tsx" => match export {
            "view" => serde_json::to_value(super::p_6da763d87b5ad::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/PressEnterToContinue.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/HistorySearchInput.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInput.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputFooter.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputFooterLeftSide.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputHelpMenu.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputModeIndicator.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputQueuedCommands.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/PromptInputStashNotice.tsx" => match export {
            "view" => serde_json::to_value(super::p_f6470f6cbfbdc::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/PromptInput/SandboxPromptFooterHint.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/ShimmeredInput.tsx" => match export {
            _ => None,
        },
        "src/components/PromptInput/VoiceIndicator.tsx" => match export {
            _ => None,
        },
        "src/components/QuickOpenDialog.tsx" => match export {
            _ => None,
        },
        "src/components/RemoteEnvironmentDialog.tsx" => match export {
            _ => None,
        },
        "src/components/ResumeTask.tsx" => match export {
            _ => None,
        },
        "src/components/SandboxViolationExpandedView.tsx" => match export {
            _ => None,
        },
        "src/components/SearchBox.tsx" => match export {
            _ => None,
        },
        "src/components/SessionBackgroundHint.tsx" => match export {
            _ => None,
        },
        "src/components/SessionPreview.tsx" => match export {
            _ => None,
        },
        "src/components/Settings/Config.tsx" => match export {
            _ => None,
        },
        "src/components/Settings/Settings.tsx" => match export {
            _ => None,
        },
        "src/components/Settings/Status.tsx" => match export {
            _ => None,
        },
        "src/components/Settings/Usage.tsx" => match export {
            _ => None,
        },
        "src/components/ShowInIDEPrompt.tsx" => match export {
            _ => None,
        },
        "src/components/SkillImprovementSurvey.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/FlashingChar.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/GlimmerMessage.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/ShimmerChar.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/SpinnerAnimationRow.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/SpinnerGlyph.tsx" => match export {
            _ => None,
        },
        "src/components/Spinner/TeammateSpinnerLine.tsx" => match export {
            "view" => serde_json::to_value(super::p_09647f4245ece::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/Spinner/TeammateSpinnerTree.tsx" => match export {
            "view" => serde_json::to_value(super::p_024038719ca35::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/StatusLine.tsx" => match export {
            _ => None,
        },
        "src/components/StatusNotices.tsx" => match export {
            _ => None,
        },
        "src/components/StructuredDiff.tsx" => match export {
            _ => None,
        },
        "src/components/StructuredDiffList.tsx" => match export {
            _ => None,
        },
        "src/components/TagTabs.tsx" => match export {
            _ => None,
        },
        "src/components/TaskListV2.tsx" => match export {
            _ => None,
        },
        "src/components/TeammateViewHeader.tsx" => match export {
            "view" => serde_json::to_value(super::p_00d240667df39::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/TeleportProgress.tsx" => match export {
            _ => None,
        },
        "src/components/TeleportResumeWrapper.tsx" => match export {
            _ => None,
        },
        "src/components/TeleportStash.tsx" => match export {
            _ => None,
        },
        "src/components/TextInput.tsx" => match export {
            _ => None,
        },
        "src/components/ThemePicker.tsx" => match export {
            _ => None,
        },
        "src/components/ThinkingToggle.tsx" => match export {
            _ => None,
        },
        "src/components/TokenWarning.tsx" => match export {
            "view" => serde_json::to_value(super::p_236973c85ed82::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/ToolUseLoader.tsx" => match export {
            "view" => serde_json::to_value(super::p_02c961fae3b16::render_source_view(
                args.first().unwrap_or(&Value::Null),
            ))
            .ok(),
            _ => None,
        },
        "src/components/TrustDialog/TrustDialog.tsx" => match export {
            _ => None,
        },
        "src/components/ValidationErrorsList.tsx" => match export {
            _ => None,
        },
        "src/components/VimTextInput.tsx" => match export {
            _ => None,
        },
        "src/components/VirtualMessageList.tsx" => match export {
            _ => None,
        },
        "src/components/WorkflowMultiselectDialog.tsx" => match export {
            _ => None,
        },
        "src/components/WorktreeExitDialog.tsx" => match export {
            _ => None,
        },
        "src/components/agents/AgentDetail.tsx" => match export {
            _ => None,
        },
        "src/components/agents/AgentEditor.tsx" => match export {
            _ => None,
        },
        "src/components/agents/AgentNavigationFooter.tsx" => match export {
            _ => None,
        },
        "src/components/agents/AgentsList.tsx" => match export {
            _ => None,
        },
        "src/components/agents/AgentsMenu.tsx" => match export {
            _ => None,
        },
        "src/components/agents/ColorPicker.tsx" => match export {
            _ => None,
        },
        "src/components/agents/ModelSelector.tsx" => match export {
            _ => None,
        },
        "src/components/agents/ToolSelector.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/CreateAgentWizard.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/ColorStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/ConfirmStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/ConfirmStepWrapper.tsx" => {
            match export {
                _ => None,
            }
        }
        "src/components/agents/new-agent-creation/wizard-steps/DescriptionStep.tsx" => match export
        {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/GenerateStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/LocationStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/MemoryStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/MethodStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/ModelStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/PromptStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/ToolsStep.tsx" => match export {
            _ => None,
        },
        "src/components/agents/new-agent-creation/wizard-steps/TypeStep.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/Byline.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/Dialog.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/Divider.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/FuzzyPicker.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/KeyboardShortcutHint.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/ListItem.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/LoadingState.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/Pane.tsx" => match export {
            _ => None,
        },
        "src/components/design-system/ProgressBar.tsx" => match export {
            _ => None,
        },
        _ => None,
    }
}
pub fn catalog() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("src/components/MarkdownTable.tsx", vec![]),
        ("src/components/MemoryUsageIndicator.tsx", vec![]),
        ("src/components/Message.tsx", vec![]),
        ("src/components/MessageModel.tsx", vec!["view"]),
        ("src/components/MessageResponse.tsx", vec![]),
        ("src/components/MessageRow.tsx", vec![]),
        ("src/components/MessageSelector.tsx", vec![]),
        ("src/components/MessageTimestamp.tsx", vec!["view"]),
        ("src/components/Messages.tsx", vec![]),
        ("src/components/ModelPicker.tsx", vec![]),
        ("src/components/NativeAutoUpdater.tsx", vec![]),
        (
            "src/components/NotebookEditToolUseRejectedMessage.tsx",
            vec!["view"],
        ),
        ("src/components/OffscreenFreeze.tsx", vec![]),
        ("src/components/OutputStylePicker.tsx", vec![]),
        ("src/components/PackageManagerAutoUpdater.tsx", vec![]),
        ("src/components/PrBadge.tsx", vec!["view"]),
        ("src/components/PressEnterToContinue.tsx", vec![]),
        ("src/components/PromptInput/HistorySearchInput.tsx", vec![]),
        ("src/components/PromptInput/PromptInput.tsx", vec![]),
        ("src/components/PromptInput/PromptInputFooter.tsx", vec![]),
        (
            "src/components/PromptInput/PromptInputFooterLeftSide.tsx",
            vec![],
        ),
        ("src/components/PromptInput/PromptInputHelpMenu.tsx", vec![]),
        (
            "src/components/PromptInput/PromptInputModeIndicator.tsx",
            vec![],
        ),
        (
            "src/components/PromptInput/PromptInputQueuedCommands.tsx",
            vec![],
        ),
        (
            "src/components/PromptInput/PromptInputStashNotice.tsx",
            vec!["view"],
        ),
        (
            "src/components/PromptInput/SandboxPromptFooterHint.tsx",
            vec![],
        ),
        ("src/components/PromptInput/ShimmeredInput.tsx", vec![]),
        ("src/components/PromptInput/VoiceIndicator.tsx", vec![]),
        ("src/components/QuickOpenDialog.tsx", vec![]),
        ("src/components/RemoteEnvironmentDialog.tsx", vec![]),
        ("src/components/ResumeTask.tsx", vec![]),
        ("src/components/SandboxViolationExpandedView.tsx", vec![]),
        ("src/components/SearchBox.tsx", vec![]),
        ("src/components/SessionBackgroundHint.tsx", vec![]),
        ("src/components/SessionPreview.tsx", vec![]),
        ("src/components/Settings/Config.tsx", vec![]),
        ("src/components/Settings/Settings.tsx", vec![]),
        ("src/components/Settings/Status.tsx", vec![]),
        ("src/components/Settings/Usage.tsx", vec![]),
        ("src/components/ShowInIDEPrompt.tsx", vec![]),
        ("src/components/SkillImprovementSurvey.tsx", vec![]),
        ("src/components/Spinner.tsx", vec![]),
        ("src/components/Spinner/FlashingChar.tsx", vec![]),
        ("src/components/Spinner/GlimmerMessage.tsx", vec![]),
        ("src/components/Spinner/ShimmerChar.tsx", vec![]),
        ("src/components/Spinner/SpinnerAnimationRow.tsx", vec![]),
        ("src/components/Spinner/SpinnerGlyph.tsx", vec![]),
        (
            "src/components/Spinner/TeammateSpinnerLine.tsx",
            vec!["view"],
        ),
        (
            "src/components/Spinner/TeammateSpinnerTree.tsx",
            vec!["view"],
        ),
        ("src/components/StatusLine.tsx", vec![]),
        ("src/components/StatusNotices.tsx", vec![]),
        ("src/components/StructuredDiff.tsx", vec![]),
        ("src/components/StructuredDiffList.tsx", vec![]),
        ("src/components/TagTabs.tsx", vec![]),
        ("src/components/TaskListV2.tsx", vec![]),
        ("src/components/TeammateViewHeader.tsx", vec!["view"]),
        ("src/components/TeleportProgress.tsx", vec![]),
        ("src/components/TeleportResumeWrapper.tsx", vec![]),
        ("src/components/TeleportStash.tsx", vec![]),
        ("src/components/TextInput.tsx", vec![]),
        ("src/components/ThemePicker.tsx", vec![]),
        ("src/components/ThinkingToggle.tsx", vec![]),
        ("src/components/TokenWarning.tsx", vec!["view"]),
        ("src/components/ToolUseLoader.tsx", vec!["view"]),
        ("src/components/TrustDialog/TrustDialog.tsx", vec![]),
        ("src/components/ValidationErrorsList.tsx", vec![]),
        ("src/components/VimTextInput.tsx", vec![]),
        ("src/components/VirtualMessageList.tsx", vec![]),
        ("src/components/WorkflowMultiselectDialog.tsx", vec![]),
        ("src/components/WorktreeExitDialog.tsx", vec![]),
        ("src/components/agents/AgentDetail.tsx", vec![]),
        ("src/components/agents/AgentEditor.tsx", vec![]),
        ("src/components/agents/AgentNavigationFooter.tsx", vec![]),
        ("src/components/agents/AgentsList.tsx", vec![]),
        ("src/components/agents/AgentsMenu.tsx", vec![]),
        ("src/components/agents/ColorPicker.tsx", vec![]),
        ("src/components/agents/ModelSelector.tsx", vec![]),
        ("src/components/agents/ToolSelector.tsx", vec![]),
        (
            "src/components/agents/new-agent-creation/CreateAgentWizard.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/ColorStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/ConfirmStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/ConfirmStepWrapper.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/DescriptionStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/GenerateStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/LocationStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/MemoryStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/MethodStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/ModelStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/PromptStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/ToolsStep.tsx",
            vec![],
        ),
        (
            "src/components/agents/new-agent-creation/wizard-steps/TypeStep.tsx",
            vec![],
        ),
        ("src/components/design-system/Byline.tsx", vec![]),
        ("src/components/design-system/Dialog.tsx", vec![]),
        ("src/components/design-system/Divider.tsx", vec![]),
        ("src/components/design-system/FuzzyPicker.tsx", vec![]),
        (
            "src/components/design-system/KeyboardShortcutHint.tsx",
            vec![],
        ),
        ("src/components/design-system/ListItem.tsx", vec![]),
        ("src/components/design-system/LoadingState.tsx", vec![]),
        ("src/components/design-system/Pane.tsx", vec![]),
        ("src/components/design-system/ProgressBar.tsx", vec![]),
    ]
}

#[cfg(test)]
mod source_view_dispatch_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn message_pr_views_are_reachable_via_source_inspection() {
        let assistant = json!({"type":"assistant","timestamp":"2026-10-29T14:00:00Z","message":{"model":"gpt-6","content":[{"type":"text","text":"hello"}]}});
        for (path, data, text) in [
            (
                "src/components/MessageModel.tsx",
                json!({"message":assistant,"isTranscriptMode":true}),
                "gpt-6",
            ),
            (
                "src/components/MessageTimestamp.tsx",
                json!({"message":assistant,"isTranscriptMode":true}),
                ":",
            ),
            (
                "src/components/PrBadge.tsx",
                json!({"number":22,"url":"https://github.com/example/repo/pull/22","reviewState":"approved"}),
                "#22",
            ),
        ] {
            let wire = inspect(path, "view", &[data]).expect("view must be registered");
            let node: super::super::ui::VNode = serde_json::from_value(wire).unwrap();
            assert_eq!(node.unresolved_node_count(), 0, "{path}");
            assert!(node.text_content().contains(text), "{path}");
            assert!(
                catalog()
                    .iter()
                    .any(|(source, exports)| *source == path && exports.contains(&"view")),
                "{path} catalog"
            );
        }
    }
}

#[cfg(test)]
mod connected_teammate_view_tests {
    use super::*;
    use crate::generated_ports::ui::VNode;
    use serde_json::json;

    #[test]
    fn registry_dispatches_live_teammate_rows_and_composed_tree() {
        let tree_path = "src/components/Spinner/TeammateSpinnerTree.tsx";
        let line_path = "src/components/Spinner/TeammateSpinnerLine.tsx";
        let entries = catalog();
        for path in [tree_path, line_path] {
            assert!(entries
                .iter()
                .any(|(source, exports)| *source == path && exports.contains(&"view")));
        }
        let props = json!({
            "tasks":{
                "worker":{"id":"worker","type":"in_process_teammate","status":"running",
                    "identity":{"agentName":"worker"},"progress":{"toolUseCount":2,"tokenCount":1200},
                    "messages":[{"type":"assistant","message":{"content":[{"type":"text","text":"Review ready"}]}}]}
            },
            "columns":120,
            "showTeammateMessagePreview":true
        });
        let tree: VNode = serde_json::from_value(
            inspect(tree_path, "view", &[props]).expect("tree registry must resolve"),
        )
        .unwrap();
        assert_eq!(tree.unresolved_node_count(), 0);
        assert!(tree.text_content().contains("@worker"));
        assert!(tree.text_content().contains("2 tool uses"));
        assert!(tree.text_content().contains("Review ready"));

        let single: VNode = serde_json::from_value(
            inspect(
                line_path,
                "view",
                &[json!({"teammate":{
                "identity":{"agentName":"worker"}, "progress":{"toolUseCount":2,"tokenCount":1200}
            },"columns":120})],
            )
            .expect("row registry must resolve"),
        )
        .unwrap();
        assert_eq!(single.unresolved_node_count(), 0);
        assert!(single.text_content().contains("@worker"));
        assert!(inspect(tree_path, "missing", &[]).is_none());
    }
}
