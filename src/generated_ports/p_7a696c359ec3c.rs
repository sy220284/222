//! Native Rust port derived from original: src/commands/install-github-app/CreatingStep.tsx
//!
//! GitHub App install progress: the two pure cores are the progress-step
//! list builder (skipWorkflow branch, existing-secret branch, workflow
//! file singular/plural) and the step status classifier
//! (completed/in-progress/pending against the current step index). The
//! bordered pane view renders those over the ui VNode primitives.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Completed,
    InProgress,
    Pending,
}

/// Progress steps for the current install shape (original `progressSteps`).
pub fn progress_steps(
    secret_exists: bool,
    use_existing_secret: bool,
    secret_name: &str,
    skip_workflow: bool,
    workflow_count: usize,
) -> Vec<String> {
    let secret_step = if secret_exists && use_existing_secret {
        "Using existing API key secret".to_string()
    } else {
        format!("Setting up {secret_name} secret")
    };
    if skip_workflow {
        vec!["Getting repository information".to_string(), secret_step]
    } else {
        vec![
            "Getting repository information".to_string(),
            "Creating branch".to_string(),
            if workflow_count > 1 {
                "Creating workflow files".to_string()
            } else {
                "Creating workflow file".to_string()
            },
            secret_step,
            "Opening pull request page".to_string(),
        ]
    }
}

/// Status of one step against the current install index.
pub fn step_status(index: usize, current_step: usize) -> StepStatus {
    if index < current_step {
        StepStatus::Completed
    } else if index == current_step {
        StepStatus::InProgress
    } else {
        StepStatus::Pending
    }
}

/// One rendered progress line (text + color role), original semantics.
pub fn render_step_line(step_text: &str, status: StepStatus) -> (String, &'static str) {
    match status {
        StepStatus::Completed => (format!("\u{2713} {step_text}"), "success"),
        StepStatus::InProgress => (format!("{step_text}\u{2026}"), "warning"),
        StepStatus::Pending => (step_text.to_string(), "default"),
    }
}

/// Builds the full step-line list for the current install state.
pub fn build_step_lines(
    secret_exists: bool,
    use_existing_secret: bool,
    secret_name: &str,
    skip_workflow: bool,
    workflow_count: usize,
    current_step: usize,
) -> Vec<(String, &'static str)> {
    progress_steps(
        secret_exists,
        use_existing_secret,
        secret_name,
        skip_workflow,
        workflow_count,
    )
    .iter()
    .enumerate()
    .map(|(i, text)| render_step_line(text, step_status(i, current_step)))
    .collect()
}

/// Bordered pane with title block and progress lines.
pub fn render_source_view(props: &Value) -> VNode {
    let current = props
        .get("currentWorkflowInstallStep")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let lines = build_step_lines(
        props
            .get("secretExists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        props
            .get("useExistingSecret")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        props
            .get("secretName")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        props
            .get("skipWorkflow")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        props
            .get("selectedWorkflows")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        current,
    );
    let mut children = vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: [("bold".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text("Install GitHub App".into())],
        },
        VNode::Element {
            tag: "Text".into(),
            attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text("Create GitHub Actions workflow".into())],
        },
    ];
    for (text, color) in lines {
        children.push(VNode::Element {
            tag: "Text".into(),
            attrs: [("color".into(), json!(color))].into_iter().collect(),
            children: vec![VNode::Text(text)],
        });
    }
    VNode::Element {
        tag: "Box".into(),
        attrs: [
            ("flexDirection".into(), json!("column")),
            ("borderStyle".into(), json!("round")),
            ("paddingX".into(), json!(1)),
        ]
        .into_iter()
        .collect(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_flow_has_five_steps() {
        let steps = progress_steps(false, false, "vendor", false, 1);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[1], "Creating branch");
        assert_eq!(steps[2], "Creating workflow file"); // singular
        assert_eq!(steps[4], "Opening pull request page");
    }

    #[test]
    fn skip_flow_has_two_steps() {
        let steps = progress_steps(true, false, "MY_SECRET", true, 3);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1], "Setting up MY_SECRET secret");
    }

    #[test]
    fn workflow_plural_and_existing_secret() {
        let steps = progress_steps(true, true, "KEY", false, 3);
        assert_eq!(steps[2], "Creating workflow files"); // plural
        assert_eq!(steps[3], "Using existing API key secret");
    }

    #[test]
    fn status_classifier_and_line_render() {
        assert_eq!(step_status(0, 1), StepStatus::Completed);
        assert_eq!(step_status(1, 1), StepStatus::InProgress);
        assert_eq!(step_status(2, 1), StepStatus::Pending);
        let (text, color) = render_step_line("Creating branch", StepStatus::Completed);
        assert_eq!(text, "\u{2713} Creating branch");
        assert_eq!(color, "success");
        let (text2, color2) = render_step_line("Creating branch", StepStatus::InProgress);
        assert_eq!(text2, "Creating branch\u{2026}");
        assert_eq!(color2, "warning");
    }

    #[test]
    fn step_lines_count_matches_progress() {
        let lines = build_step_lines(false, false, "K", false, 2, 1);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].1, "success");
        assert_eq!(lines[1].1, "warning");
    }
}
