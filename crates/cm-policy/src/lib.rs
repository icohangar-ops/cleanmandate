use cm_core::AgentMandate;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandatePolicy {
    pub version: String,
    pub max_single_transfer_usd: f64,
    pub max_daily_agent_spend_usd: f64,
    pub allowed_assets: Vec<String>,
    pub allowed_chains: Vec<String>,
    pub recipient_allowlist: Vec<String>,
    pub require_travel_rule: bool,
}

impl Default for MandatePolicy {
    fn default() -> Self {
        Self {
            version: "1.0".into(),
            max_single_transfer_usd: 10_000.0,
            max_daily_agent_spend_usd: 25_000.0,
            allowed_assets: vec!["A-USDC".into(), "USDC".into()],
            allowed_chains: vec!["monad".into(), "monad-testnet".into()],
            recipient_allowlist: vec![],
            require_travel_rule: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub violations: Vec<PolicyViolation>,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

impl MandatePolicy {
    pub fn load(path: &Path) -> Result<Self, PolicyError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(serde_yaml::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub fn evaluate(&self, mandate: &AgentMandate) -> PolicyDecision {
        let mut violations = Vec::new();
        let amount: f64 = mandate.amount.parse().unwrap_or(0.0);

        if amount > self.max_single_transfer_usd {
            violations.push(PolicyViolation {
                rule_id: "max-single-transfer".into(),
                message: format!("amount ${amount} exceeds max ${}", self.max_single_transfer_usd),
            });
        }

        if amount > mandate.daily_cap_usd {
            violations.push(PolicyViolation {
                rule_id: "mandate-daily-cap".into(),
                message: format!("amount exceeds mandate daily cap ${}", mandate.daily_cap_usd),
            });
        }

        if !self
            .allowed_assets
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&mandate.asset))
        {
            violations.push(PolicyViolation {
                rule_id: "asset-allowlist".into(),
                message: format!("asset {} not allowed", mandate.asset),
            });
        }

        if !self
            .allowed_chains
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&mandate.chain))
        {
            violations.push(PolicyViolation {
                rule_id: "chain-allowlist".into(),
                message: format!("chain {} not allowed", mandate.chain),
            });
        }

        if !self.recipient_allowlist.is_empty()
            && !self
                .recipient_allowlist
                .iter()
                .any(|r| r.eq_ignore_ascii_case(&mandate.recipient_wallet))
        {
            violations.push(PolicyViolation {
                rule_id: "recipient-allowlist".into(),
                message: "recipient not on allowlist".into(),
            });
        }

        if self.require_travel_rule {
            if mandate.travel_rule.originator_name.is_empty()
                || mandate.travel_rule.beneficiary_name.is_empty()
            {
                violations.push(PolicyViolation {
                    rule_id: "travel-rule-metadata".into(),
                    message: "Travel Rule originator/beneficiary names required".into(),
                });
            }
        }

        PolicyDecision {
            allowed: violations.is_empty(),
            violations,
        }
    }
}
