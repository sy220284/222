use crate::protocol::TokenUsage;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

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
}
impl CostLedger {
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
