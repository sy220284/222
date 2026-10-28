use crate::protocol::TokenUsage;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{fmt::Write,time::Duration};

/// Prices must be user-supplied. Unknown model prices are never guessed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub provider: String,
    pub model: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
    #[serde(default)]
    pub cache_read_per_million: f64,
    #[serde(default)]
    pub cache_write_per_million: f64,
    #[serde(default)]
    pub web_search_per_request: f64,
}
impl ModelPrice {
    pub fn validate(&self) -> Result<()> {
        if self.provider.trim().is_empty() || self.model.trim().is_empty() {
            bail!("Model price requires a provider and model");
        }
        for value in [self.input_per_million, self.output_per_million,
            self.cache_read_per_million, self.cache_write_per_million,
            self.web_search_per_request] {
            if !value.is_finite() || value < 0.0 {
                bail!("Prices must be finite, nonnegative USD rates");
            }
        }
        Ok(())
    }
    pub fn format_model_pricing(&self) -> String {
        fn fmt_price(value:f64)->String{
            if value.fract()==0.0 {format!("{}{:.0}", "$", value)}
            else {format!("{}{:.2}", "$", value)}
        }
        format!("{}/{} per Mtok",
            fmt_price(self.input_per_million),fmt_price(self.output_per_million))
    }
    pub fn cost_from_tokens(&self,input:u64,output:u64,cache_read:u64,cache_write:u64)->f64{
        self.cost(TokenUsage{
            input_tokens:input,output_tokens:output,
            total_tokens:input.saturating_add(output),
            cache_read_input_tokens:cache_read,
            cache_creation_input_tokens:cache_write,
            ..Default::default()
        })
    }
    pub fn cost(&self, usage: TokenUsage) -> f64 {
        let uncached = usage.input_tokens.saturating_sub(usage.cache_read_input_tokens);
        (uncached as f64 * self.input_per_million
            + usage.output_tokens as f64 * self.output_per_million
            + usage.cache_read_input_tokens as f64 * self.cache_read_per_million
            + usage.cache_creation_input_tokens as f64 * self.cache_write_per_million)
            / 1_000_000.0
            + usage.web_search_requests as f64 * self.web_search_per_request
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelCost {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub usage: TokenUsage,
    #[serde(default)]
    pub known_cost_usd: f64,
    #[serde(default)]
    pub has_unpriced_usage: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CostLedger {
    #[serde(default)]
    pub models: Vec<ModelCost>,
    #[serde(default)]
    pub total_api_duration_ms: u64,
    #[serde(default)]
    pub total_tool_duration_ms: u64,
    #[serde(default)]
    pub total_wall_duration_ms: u64,
    #[serde(default)]
    pub total_lines_added: u64,
    #[serde(default)]
    pub total_lines_removed: u64,
}
impl CostLedger {
    pub fn merge(&mut self,other:Self){
        for incoming in other.models {
            if let Some(existing)=self.models.iter_mut().find(|m|
                m.provider==incoming.provider && m.model==incoming.model
            ){
                existing.usage.accumulate(incoming.usage);
                existing.known_cost_usd+=incoming.known_cost_usd;
                existing.has_unpriced_usage|=incoming.has_unpriced_usage;
            } else {
                self.models.push(incoming);
            }
        }
        self.total_api_duration_ms=self.total_api_duration_ms.saturating_add(other.total_api_duration_ms);
        self.total_tool_duration_ms=self.total_tool_duration_ms.saturating_add(other.total_tool_duration_ms);
        self.total_wall_duration_ms=self.total_wall_duration_ms.saturating_add(other.total_wall_duration_ms);
        self.total_lines_added=self.total_lines_added.saturating_add(other.total_lines_added);
        self.total_lines_removed=self.total_lines_removed.saturating_add(other.total_lines_removed);
    }
    pub fn add_api_duration(&mut self,elapsed:Duration){
        self.total_api_duration_ms=self.total_api_duration_ms.saturating_add(elapsed.as_millis() as u64);
    }
    pub fn add_tool_duration(&mut self,elapsed:Duration){
        self.total_tool_duration_ms=self.total_tool_duration_ms.saturating_add(elapsed.as_millis() as u64);
    }
    pub fn add_wall_duration(&mut self,elapsed:Duration){
        self.total_wall_duration_ms=self.total_wall_duration_ms.saturating_add(elapsed.as_millis() as u64);
    }
    pub fn set_changed_lines(&mut self,added:u64,removed:u64){
        self.total_lines_added=added;
        self.total_lines_removed=removed;
    }
    pub fn model_pricing<'a>(prices:&'a[ModelPrice],provider:&str,model:&str)->Option<&'a ModelPrice>{
        prices.iter().find(|p|p.provider==provider && p.model==model)
    }
    pub fn record(&mut self, provider: &str, model: &str, usage: TokenUsage, prices: &[ModelPrice]) {
        let index = self.models.iter().position(|m| m.provider == provider && m.model == model)
            .unwrap_or_else(|| {
                self.models.push(ModelCost {
                    provider: provider.into(), model: model.into(), ..Default::default()
                });
                self.models.len() - 1
            });
        let row = &mut self.models[index];
        row.usage.accumulate(usage);
        if let Some(rate) = prices.iter().find(|r| r.provider == provider && r.model == model) {
            row.known_cost_usd += rate.cost(usage);
        } else if usage.total_tokens > 0 || usage.input_tokens > 0 ||
            usage.output_tokens > 0 || usage.cache_read_input_tokens > 0 ||
            usage.cache_creation_input_tokens > 0 || usage.web_search_requests > 0 {
            row.has_unpriced_usage = true;
        }
    }
    pub fn total_known_cost_usd(&self) -> f64 {
        self.models.iter().map(|m| m.known_cost_usd).sum()
    }
    pub fn has_unknown_prices(&self) -> bool {
        self.models.iter().any(|m| m.has_unpriced_usage)
    }
    pub fn format_summary(&self) -> String {
        let mut summary = format!("Total cost (priced usage): {}{:.4}", "$", self.total_known_cost_usd());
        if self.has_unknown_prices() {
            summary.push_str(" (incomplete: unknown model prices)");
        }
        let _=write!(summary,
            "\nTotal duration (API): {} ms\nTotal duration (tools): {} ms\nTotal duration (wall): {} ms\nTotal code changes: {} lines added, {} lines removed",
            self.total_api_duration_ms,self.total_tool_duration_ms,
            self.total_wall_duration_ms,self.total_lines_added,self.total_lines_removed);
        if self.models.is_empty() {
            summary.push_str("\nUsage: 0 input, 0 output, 0 cache read, 0 cache write");
        }
        for model in &self.models {
            let _ = write!(summary,
                "\n{}/{}: {} input, {} output, {} cache read, {} cache write, {} web search ({}{:.4}{})",
                model.provider, model.model, model.usage.input_tokens, model.usage.output_tokens,
                model.usage.cache_read_input_tokens, model.usage.cache_creation_input_tokens,
                model.usage.web_search_requests, "$", model.known_cost_usd,
                if model.has_unpriced_usage {", price missing"} else {""});
        }
        summary
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn price() -> ModelPrice {
        ModelPrice {
            provider: "a".into(), model: "m".into(), input_per_million: 3.0,
            output_per_million: 15.0, cache_read_per_million: 0.3,
            cache_write_per_million: 3.75, web_search_per_request: 0.01
        }
    }
    #[test]
    fn cached_tokens_are_not_double_charged() {
        let usage = TokenUsage {
            input_tokens: 1_000_000, output_tokens: 100_000, total_tokens: 1_100_000,
            cache_read_input_tokens: 400_000, cache_creation_input_tokens: 200_000,
            web_search_requests: 2,
        };
        assert!((price().cost(usage) - 4.19).abs() < 1e-9);
        let mut ledger = CostLedger::default();
        ledger.record("a", "m", usage, &[price()]);
        assert_eq!(ledger.models[0].usage, usage);
        assert!(!ledger.has_unknown_prices());
        let copy: CostLedger = serde_json::from_str(&serde_json::to_string(&ledger).unwrap()).unwrap();
        assert_eq!(copy, ledger);
    }
    #[test]
    fn model_switch_and_unknown_prices_preserve_separate_usage() {
        let usage = TokenUsage {input_tokens: 10, output_tokens: 20,
            total_tokens: 30, ..Default::default()};
        let mut ledger = CostLedger::default();
        ledger.record("a", "m", usage, &[price()]);
        ledger.record("b", "m", usage, &[price()]);
        ledger.record("a", "m", usage, &[price()]);
        assert_eq!(ledger.models.len(), 2);
        assert_eq!(ledger.models[0].usage.input_tokens, 20);
        assert!(ledger.has_unknown_prices());
        assert!(ledger.format_summary().contains("price missing"));
    }
    #[test]
    fn merging_child_model_costs_preserves_models_unknown_prices_and_durations(){
        let sample=TokenUsage{input_tokens:11,output_tokens:5,total_tokens:16,
            ..Default::default()};
        let mut main=CostLedger::default();
        main.record("a","m",sample,&[price()]);
        main.add_api_duration(Duration::from_millis(8));
        let mut child=CostLedger::default();
        child.record("a","m",sample,&[price()]);
        child.record("b","n",sample,&[]);
        child.add_api_duration(Duration::from_millis(5));
        child.add_tool_duration(Duration::from_millis(2));
        child.set_changed_lines(3,1);
        main.merge(child);
        assert_eq!(main.models.len(),2);
        assert_eq!(main.models[0].usage.input_tokens,22);
        assert!(main.models[1].has_unpriced_usage);
        assert!(main.has_unknown_prices());
        assert_eq!(main.total_api_duration_ms,13);
        assert_eq!(main.total_tool_duration_ms,2);
        assert_eq!(main.total_lines_added,3);
        assert!((main.models[0].known_cost_usd-price().cost(sample)*2.0).abs()<1e-12);
    }
    #[test]
    fn pricing_strings_and_side_query_costs_match_public_generic_contract(){
        let mut p=price();
        assert_eq!(p.format_model_pricing(),"$3/$15 per Mtok");
        p.input_per_million=0.8;
        assert_eq!(p.format_model_pricing(),"$0.80/$15 per Mtok");
        assert!((p.cost_from_tokens(1_000_000,10_000,0,0)-0.95).abs()<1e-9);
        let prices=vec![p];
        assert!(CostLedger::model_pricing(&prices,"other","m").is_none());
        assert!(CostLedger::model_pricing(&prices,"a","m").is_some());
        let mut ledger=CostLedger::default();
        ledger.add_api_duration(Duration::from_millis(20));
        ledger.add_tool_duration(Duration::from_millis(5));
        ledger.add_wall_duration(Duration::from_millis(30));
        ledger.set_changed_lines(3,1);
        let restored:CostLedger=serde_json::from_str(&serde_json::to_string(&ledger).unwrap()).unwrap();
        assert_eq!(restored,ledger);
        assert!(ledger.format_summary().contains("3 lines added, 1 lines removed"));
    }
    #[test]
    fn invalid_rates_and_old_data() {
        for invalid in [f64::NAN, f64::INFINITY, -0.1] {
            let mut p = price(); p.input_per_million = invalid;
            assert!(p.validate().is_err());
        }
        let restored: CostLedger = serde_json::from_str("{}").unwrap();
        assert!(restored.models.is_empty());
        let legacy: TokenUsage = serde_json::from_str(
            r#"{"input_tokens":1,"output_tokens":2,"total_tokens":3}"#).unwrap();
        assert_eq!(legacy.cache_read_input_tokens, 0);
    }
}
