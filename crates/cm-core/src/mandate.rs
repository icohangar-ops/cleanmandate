use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MandateStatus {
    Draft,
    PolicyCheck,
    CcpPending,
    ChpReview,
    AwaitingPrincipal,
    Executing,
    Completed,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelRulePayload {
    pub originator_name: String,
    pub originator_wallet: String,
    pub beneficiary_name: String,
    pub beneficiary_wallet: String,
    pub originator_vasp: Option<String>,
    pub beneficiary_vasp: Option<String>,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMandate {
    pub id: Uuid,
    pub principal_wallet: String,
    pub agent_id: String,
    pub recipient_wallet: String,
    pub amount: String,
    pub asset: String,
    pub chain: String,
    pub daily_cap_usd: f64,
    pub travel_rule: TravelRulePayload,
    pub memo: String,
    pub issued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferIntent {
    pub mandate_id: Uuid,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount: String,
    pub asset_symbol: String,
    pub chain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceBundle {
    pub mandate_id: Uuid,
    pub a_pass_verified: bool,
    pub ccp_passed: bool,
    pub policy_passed: bool,
    pub chp_locked: bool,
    pub travel_rule: TravelRulePayload,
    pub audit_event_ids: Vec<Uuid>,
    pub export_ready: bool,
}

impl AgentMandate {
    pub fn to_transfer_intent(&self) -> TransferIntent {
        TransferIntent {
            mandate_id: self.id,
            from_wallet: self.principal_wallet.clone(),
            to_wallet: self.recipient_wallet.clone(),
            amount: self.amount.clone(),
            asset_symbol: self.asset.clone(),
            chain: self.chain.clone(),
        }
    }
}
