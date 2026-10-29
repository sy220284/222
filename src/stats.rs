//! Workspace-local usage statistics; data comes from durable Rust sessions
//! and model-reported token counters, never from guessed token prices.
use crate::{
    cost::{CostLedger, ModelCost},
    protocol::TokenUsage,
    session::Session,
    session_control,
};
use anyhow::Result;
use std::{
    collections::BTreeMap,
    fmt::Write,
    path::Path,
};

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub sessions: usize,
    pub branches: usize,
    pub recorded_messages: u64,
    pub user_messages: u64,
    pub tool_calls: u64,
    pub tokens: TokenUsage,
    pub known_cost_usd: f64,
    pub has_unpriced_usage: bool,
    pub api_duration_ms: u64,
    pub tool_duration_ms: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub earliest_update_ms: u128,
    pub latest_update_ms: u128,
    pub legacy_branches_without_baseline: usize,
    pub models: BTreeMap<(String, String), ModelCost>,
}

fn usage_delta(current: TokenUsage, baseline: Option<TokenUsage>) -> TokenUsage {
    let old = baseline.unwrap_or_default();
    TokenUsage {
        input_tokens: current.input_tokens.saturating_sub(old.input_tokens),
        output_tokens: current.output_tokens.saturating_sub(old.output_tokens),
        total_tokens: current.total_tokens.saturating_sub(old.total_tokens),
        cache_read_input_tokens: current.cache_read_input_tokens
            .saturating_sub(old.cache_read_input_tokens),
        cache_creation_input_tokens: current.cache_creation_input_tokens
            .saturating_sub(old.cache_creation_input_tokens),
        web_search_requests: current.web_search_requests
            .saturating_sub(old.web_search_requests),
    }
}

fn cost_delta(current: &CostLedger, old: Option<&CostLedger>) -> CostLedger {
    let mut result = CostLedger::default();
    let default = CostLedger::default();
    let baseline = old.unwrap_or(&default);
    result.total_api_duration_ms = current.total_api_duration_ms
        .saturating_sub(baseline.total_api_duration_ms);
    result.total_tool_duration_ms = current.total_tool_duration_ms
        .saturating_sub(baseline.total_tool_duration_ms);
    result.total_wall_duration_ms = current.total_wall_duration_ms
        .saturating_sub(baseline.total_wall_duration_ms);
    result.total_lines_added = current.total_lines_added
        .saturating_sub(baseline.total_lines_added);
    result.total_lines_removed = current.total_lines_removed
        .saturating_sub(baseline.total_lines_removed);
    for model in &current.models {
        let prior = baseline.models.iter().find(|m| {
            m.provider == model.provider && m.model == model.model
        });
        let previous = prior.map(|m| m.usage);
        let usage = usage_delta(model.usage, previous);
        let known_cost_usd = (
            model.known_cost_usd -
                prior.map_or(0.0, |m| m.known_cost_usd)
        ).max(0.0);
        if usage.total_tokens == 0 && usage.web_search_requests == 0
            && known_cost_usd == 0.0 {
            continue;
        }
        result.models.push(ModelCost {
            provider: model.provider.clone(),
            model: model.model.clone(),
            usage,
            known_cost_usd,
            // The historical ledger cannot unmark an unpriced token; this is
            // conservative instead of inventing an exact model price.
            has_unpriced_usage: model.has_unpriced_usage,
        });
    }
    result
}

impl Snapshot {
    fn include(&mut self, session: Session, modified_ms: u128) {
        self.sessions += 1;
        if self.sessions == 1 {
            self.earliest_update_ms = modified_ms;
            self.latest_update_ms = modified_ms;
        } else {
            self.earliest_update_ms = self.earliest_update_ms.min(modified_ms);
            self.latest_update_ms = self.latest_update_ms.max(modified_ms);
        }
        if session.forked_from.is_some() {
            self.branches += 1;
            if session.fork_baseline_usage.is_none()
                || session.fork_baseline_cost.is_none()
                || session.fork_baseline_messages.is_none()
            {
                // Legacy forks without a recorded baseline cannot be counted
                // without charging the same historical turns twice.
                self.legacy_branches_without_baseline += 1;
                return;
            }
        }
        let start = session.fork_baseline_messages.unwrap_or(0);
        let new_messages = session.messages.get(start..).unwrap_or(&[]);
        self.recorded_messages += new_messages.len() as u64;
        self.user_messages += new_messages.iter().filter(|m|m.role=="user").count() as u64;
        self.tool_calls += new_messages.iter()
            .map(|m|m.tool_calls.as_ref().map_or(0,Vec::len) as u64)
            .sum::<u64>();
        self.tokens.accumulate(usage_delta(session.usage, session.fork_baseline_usage));
        let cost = cost_delta(&session.cost, session.fork_baseline_cost.as_ref());
        self.api_duration_ms=self.api_duration_ms.saturating_add(cost.total_api_duration_ms);
        self.tool_duration_ms=self.tool_duration_ms.saturating_add(cost.total_tool_duration_ms);
        self.lines_added=self.lines_added.saturating_add(cost.total_lines_added);
        self.lines_removed=self.lines_removed.saturating_add(cost.total_lines_removed);
        for model in cost.models {
            self.known_cost_usd += model.known_cost_usd;
            self.has_unpriced_usage |= model.has_unpriced_usage;
            let key = (model.provider.clone(), model.model.clone());
            let entry=self.models.entry(key).or_insert_with(|| ModelCost {
                provider: model.provider.clone(),
                model: model.model.clone(),
                ..Default::default()
            });
            entry.usage.accumulate(model.usage);
            entry.known_cost_usd += model.known_cost_usd;
            entry.has_unpriced_usage |= model.has_unpriced_usage;
        }
    }

    pub fn render(&self) -> String {
        let mut out=String::new();
        let _ = writeln!(out,"Local conversation statistics (all available history)");
        let _ = writeln!(out,"Sessions: {} ({} forks)",self.sessions,self.branches);
        let _ = writeln!(out,"Recorded messages: {} (user {}, tool calls {})",
            self.recorded_messages,self.user_messages,self.tool_calls);
        let _ = writeln!(out,"Model-reported tokens: input {}, output {}, total {}",
            self.tokens.input_tokens,self.tokens.output_tokens,self.tokens.total_tokens);
        let _ = writeln!(out,"Cache read: {}  cache write: {}  web searches: {}",
            self.tokens.cache_read_input_tokens,
            self.tokens.cache_creation_input_tokens,self.tokens.web_search_requests);
        let _ = writeln!(out,"Known model cost: {}",crate::cost::format_cost(self.known_cost_usd,4));
        if self.has_unpriced_usage {
            let _ = writeln!(out,"Some model usage is unpriced; actual cost may be higher.");
        }
        let _ = writeln!(out,"Tracked code changes: +{} -{}",self.lines_added,self.lines_removed);
        let _ = writeln!(out,"Measured API time: {} ms; tool time: {} ms",
            self.api_duration_ms,self.tool_duration_ms);
        if self.sessions > 0 {
            let _ = writeln!(out,"Session last-modified range: {} .. {} (Unix ms)",
                self.earliest_update_ms,self.latest_update_ms);
        }
        if self.legacy_branches_without_baseline > 0 {
            let _ = writeln!(out,"{} legacy forks omitted from usage totals (no saved fork baseline).",
                self.legacy_branches_without_baseline);
        }
        if !self.models.is_empty() {
            let _=writeln!(out,"Model usage:");
            for ((provider,model),usage) in &self.models {
                let _=writeln!(out,"  {provider}/{model}: in {} out {}, known {}{}",
                    usage.usage.input_tokens,
                    usage.usage.output_tokens,
                    crate::cost::format_cost(usage.known_cost_usd,4),
                    if usage.has_unpriced_usage { " + unpriced" } else { "" });
            }
        }
        out
    }
}

pub fn collect(root: &Path) -> Result<Snapshot> {
    let mut summary=Snapshot::default();
    for item in session_control::list(root)? {
        let session=Session::load(root,&item.id.to_string())?;
        summary.include(session,item.modified_ms);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Message;

    #[test]
    fn totals_use_actual_usage_and_do_not_invent_pricing() {
        let root=tempfile::tempdir().unwrap();
        let mut parent=Session::fresh();
        parent.messages.push(Message::text("user","hello"));
        parent.messages.push(Message::text("assistant","world"));
        parent.usage=TokenUsage {
            input_tokens:250,output_tokens:120,total_tokens:370,
            ..Default::default()
        };
        parent.cost.models.push(ModelCost {
            provider:"local".into(),model:"test".into(),usage:parent.usage,
            known_cost_usd:0.125,has_unpriced_usage:true,
        });
        parent.save(root.path()).unwrap();
        let stats=collect(root.path()).unwrap();
        assert_eq!(stats.sessions,1);
        assert_eq!(stats.recorded_messages,2);
        assert_eq!(stats.user_messages,1);
        assert_eq!(stats.tokens.total_tokens,370);
        assert!(stats.has_unpriced_usage);
        assert!(stats.render().contains("known $0.1250 + unpriced"));
    }

    #[test]
    fn branches_only_charge_incremental_messages_and_tokens() {
        let root=tempfile::tempdir().unwrap();
        let mut parent=Session::fresh();
        parent.messages.push(Message::text("user","parent"));
        parent.usage=TokenUsage {
            input_tokens:100,output_tokens:50,total_tokens:150,
            ..Default::default()
        };
        parent.cost.models.push(ModelCost {
            provider:"provider".into(),model:"model".into(),
            usage:parent.usage, known_cost_usd:0.1,
            has_unpriced_usage:false
        });
        parent.save(root.path()).unwrap();
        let mut fork=parent.clone();
        fork.id=uuid::Uuid::new_v4();
        fork.forked_from=Some(parent.id);
        fork.fork_baseline_messages=Some(parent.messages.len());
        fork.fork_baseline_usage=Some(parent.usage);
        fork.fork_baseline_cost=Some(parent.cost.clone());
        fork.messages.push(Message::text("user","fork"));
        fork.usage.accumulate(TokenUsage{
            input_tokens:30,output_tokens:10,total_tokens:40,..Default::default()
        });
        let model=fork.cost.models.get_mut(0).unwrap();
        model.usage.accumulate(TokenUsage{
            input_tokens:30,output_tokens:10,total_tokens:40,..Default::default()
        });
        model.known_cost_usd += 0.03;
        fork.save(root.path()).unwrap();
        let stats=collect(root.path()).unwrap();
        assert_eq!(stats.sessions,2);
        assert_eq!(stats.branches,1);
        assert_eq!(stats.recorded_messages,2);
        assert_eq!(stats.tokens.total_tokens,190);
        assert!((stats.known_cost_usd-0.13).abs()<1e-9);
        assert_eq!(stats.models.len(),1);
    }

    #[test]
    fn legacy_forks_are_reported_without_double_counting() {
        let root=tempfile::tempdir().unwrap();
        let mut session=Session::fresh();
        session.forked_from=Some(uuid::Uuid::new_v4());
        session.messages.push(Message::text("user","inherited"));
        session.save(root.path()).unwrap();
        let result=collect(root.path()).unwrap();
        assert_eq!(result.sessions,1);
        assert_eq!(result.legacy_branches_without_baseline,1);
        assert_eq!(result.recorded_messages,0);
    }
}
