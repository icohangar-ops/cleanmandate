use cm_core::TravelRulePayload;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApassVerification {
    pub wallet: String,
    pub verified: bool,
    pub identity_id: Option<String>,
    pub kyc_tier: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcpPreCheckRequest {
    pub mandate_id: Uuid,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount: String,
    pub asset: String,
    pub chain: String,
    pub travel_rule: TravelRulePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcpPreCheckResult {
    pub passed: bool,
    pub ccp_reference: String,
    pub travel_rule_status: String,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransferRequest {
    pub mandate_id: Uuid,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount: String,
    pub asset: String,
    pub chain: String,
    pub ccp_reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransferResult {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub a_token_reference: String,
    pub message: String,
}
