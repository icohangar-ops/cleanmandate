use cm_core::AgentMandate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChpState {
    Open,
    Locked,
    Released,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChpLock {
    pub lock_id: Uuid,
    pub mandate_id: Uuid,
    pub state: ChpState,
    pub quorum_required: u8,
    pub approvals: u8,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChpDecision {
    pub allowed: bool,
    pub lock: ChpLock,
    pub requires_human: bool,
}

#[derive(Debug, Error)]
pub enum ChpError {
    #[error("mandate rejected by CHP: {0}")]
    Rejected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChpConfig {
    pub quorum_required: u8,
    pub human_threshold_usd: f64,
    pub auto_approve_below_usd: f64,
}

impl Default for ChpConfig {
    fn default() -> Self {
        Self {
            quorum_required: 1,
            human_threshold_usd: 5_000.0,
            auto_approve_below_usd: 1_000.0,
        }
    }
}

pub struct ChpGate {
    config: ChpConfig,
}

impl ChpGate {
    pub fn new(config: ChpConfig) -> Self {
        Self { config }
    }

    pub fn evaluate(&self, mandate: &AgentMandate, policy_passed: bool) -> Result<ChpDecision, ChpError> {
        if !policy_passed {
            return Err(ChpError::Rejected("policy failed".into()));
        }

        let amount: f64 = mandate.amount.parse().unwrap_or(0.0);
        let requires_human = amount >= self.config.human_threshold_usd;
        let auto = amount <= self.config.auto_approve_below_usd;

        let (state, approvals, allowed, reason) = if auto && !requires_human {
            (
                ChpState::Locked,
                self.config.quorum_required,
                true,
                "auto-approved under CHP threshold".into(),
            )
        } else if requires_human {
            (
                ChpState::Open,
                0,
                false,
                format!("human approval required for ${amount}"),
            )
        } else {
            (
                ChpState::Locked,
                self.config.quorum_required,
                true,
                "CHP quorum satisfied".into(),
            )
        };

        Ok(ChpDecision {
            allowed,
            requires_human,
            lock: ChpLock {
                lock_id: Uuid::new_v4(),
                mandate_id: mandate.id,
                state,
                quorum_required: self.config.quorum_required,
                approvals,
                reason,
            },
        })
    }

    pub fn principal_approve(&self, lock: &mut ChpLock) -> ChpDecision {
        if lock.state == ChpState::Rejected {
            return ChpDecision {
                allowed: false,
                requires_human: true,
                lock: lock.clone(),
            };
        }
        lock.approvals = lock.approvals.saturating_add(1);
        if lock.approvals >= lock.quorum_required {
            lock.state = ChpState::Locked;
            lock.reason = "principal approved CHP lock".into();
        }
        ChpDecision {
            allowed: lock.state == ChpState::Locked,
            requires_human: lock.state != ChpState::Locked,
            lock: lock.clone(),
        }
    }
}
