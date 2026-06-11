use cm_chp::{ChpConfig, ChpGate, ChpLock};
use cm_cleanverse::{
    CleanverseClient, CcpPreCheckRequest, TokenTransferRequest,
};
use cm_core::{
    AgentMandate, AuditLedger, AuditPhase, ComplianceBundle, LedgerConfig, MandateStatus,
};
use cm_policy::MandatePolicy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub policy_path: PathBuf,
    pub ledger_path: PathBuf,
    pub signing_key: Option<String>,
    pub dry_run: bool,
    pub chp: ChpConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayResult {
    pub mandate_id: Uuid,
    pub status: MandateStatus,
    pub tx_hash: Option<String>,
    pub ccp_reference: Option<String>,
    pub bundle: ComplianceBundle,
    pub chp_lock: Option<ChpLock>,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("audit error: {0}")]
    Audit(#[from] cm_core::audit::AuditError),
    #[error("policy error: {0}")]
    Policy(#[from] cm_policy::PolicyError),
    #[error("cleanverse error: {0}")]
    Cleanverse(#[from] cm_cleanverse::CleanverseError),
    #[error("chp error: {0}")]
    Chp(#[from] cm_chp::ChpError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Blocked(String),
}

pub struct MandateExecutor {
    policy: MandatePolicy,
    ledger: AuditLedger,
    cleanverse: CleanverseClient,
    chp: ChpGate,
    config: ExecutorConfig,
}

impl MandateExecutor {
    pub async fn new(config: ExecutorConfig, cleanverse: CleanverseClient) -> Result<Self, ExecutorError> {
        let policy = MandatePolicy::load(&config.policy_path)?;
        let ledger = AuditLedger::new(LedgerConfig {
            signing_key: config.signing_key.clone(),
            ledger_path: config.ledger_path.clone(),
        });
        let chp = ChpGate::new(config.chp.clone());
        Ok(Self {
            policy,
            ledger,
            cleanverse,
            chp,
            config,
        })
    }

    pub async fn pay(&self, mandate: &AgentMandate) -> Result<PayResult, ExecutorError> {
        let mut audit_ids = Vec::new();

        let ingest = self.ledger.record(
            AuditPhase::Ingest,
            &mandate.agent_id,
            "mandate_received",
            Some(mandate.id),
            serde_json::json!({ "amount": mandate.amount, "asset": mandate.asset }),
        )?;
        audit_ids.push(ingest.id);

        let apass = self
            .cleanverse
            .verify_apass(&mandate.principal_wallet)
            .await?;
        let apass_evt = self.ledger.record(
            AuditPhase::Apass,
            "cleanverse-apass",
            if apass.verified { "verified" } else { "failed" },
            Some(mandate.id),
            serde_json::to_value(&apass)?,
        )?;
        audit_ids.push(apass_evt.id);
        if !apass.verified {
            return Err(ExecutorError::Blocked("A-Pass verification failed".into()));
        }

        let policy_decision = self.policy.evaluate(mandate);
        let policy_evt = self.ledger.record(
            AuditPhase::Policy,
            "cm-policy",
            if policy_decision.allowed {
                "passed"
            } else {
                "failed"
            },
            Some(mandate.id),
            serde_json::to_value(&policy_decision)?,
        )?;
        audit_ids.push(policy_evt.id);
        if !policy_decision.allowed {
            return Err(ExecutorError::Blocked(format!(
                "policy violations: {:?}",
                policy_decision.violations
            )));
        }

        let ccp_req = CcpPreCheckRequest {
            mandate_id: mandate.id,
            from_wallet: mandate.principal_wallet.clone(),
            to_wallet: mandate.recipient_wallet.clone(),
            amount: mandate.amount.clone(),
            asset: mandate.asset.clone(),
            chain: mandate.chain.clone(),
            travel_rule: mandate.travel_rule.clone(),
        };
        let ccp = self.cleanverse.ccp_pre_check(&ccp_req).await?;
        let ccp_evt = self.ledger.record(
            AuditPhase::Ccp,
            "cleanverse-ccp",
            if ccp.passed { "cleared" } else { "blocked" },
            Some(mandate.id),
            serde_json::to_value(&ccp)?,
        )?;
        audit_ids.push(ccp_evt.id);
        if !ccp.passed {
            return Err(ExecutorError::Blocked(
                ccp.blocked_reason.unwrap_or_else(|| "CCP blocked".into()),
            ));
        }

        let chp_decision = self.chp.evaluate(mandate, true)?;
        let chp_evt = self.ledger.record(
            AuditPhase::Chp,
            "cm-chp",
            if chp_decision.allowed {
                "locked"
            } else {
                "awaiting_principal"
            },
            Some(mandate.id),
            serde_json::to_value(&chp_decision.lock)?,
        )?;
        audit_ids.push(chp_evt.id);

        if chp_decision.requires_human && !chp_decision.allowed {
            let message = chp_decision.lock.reason.clone();
            let bundle = ComplianceBundle {
                mandate_id: mandate.id,
                a_pass_verified: true,
                ccp_passed: true,
                policy_passed: true,
                chp_locked: false,
                travel_rule: mandate.travel_rule.clone(),
                audit_event_ids: audit_ids,
                export_ready: false,
            };
            return Ok(PayResult {
                mandate_id: mandate.id,
                status: MandateStatus::ChpReview,
                tx_hash: None,
                ccp_reference: Some(ccp.ccp_reference),
                bundle,
                chp_lock: Some(chp_decision.lock),
                message,
            });
        }

        if self.config.dry_run {
            let bundle = ComplianceBundle {
                mandate_id: mandate.id,
                a_pass_verified: true,
                ccp_passed: true,
                policy_passed: true,
                chp_locked: true,
                travel_rule: mandate.travel_rule.clone(),
                audit_event_ids: audit_ids,
                export_ready: true,
            };
            let _ = self.ledger.record(
                AuditPhase::Execute,
                "cm-executor",
                "dry_run",
                Some(mandate.id),
                serde_json::json!({ "ccp_reference": ccp.ccp_reference }),
            )?;
            return Ok(PayResult {
                mandate_id: mandate.id,
                status: MandateStatus::Completed,
                tx_hash: None,
                ccp_reference: Some(ccp.ccp_reference),
                bundle,
                chp_lock: Some(chp_decision.lock),
                message: "dry-run: all gates passed, transfer skipped".into(),
            });
        }

        let transfer_req = TokenTransferRequest {
            mandate_id: mandate.id,
            from_wallet: mandate.principal_wallet.clone(),
            to_wallet: mandate.recipient_wallet.clone(),
            amount: mandate.amount.clone(),
            asset: mandate.asset.clone(),
            chain: mandate.chain.clone(),
            ccp_reference: ccp.ccp_reference.clone(),
        };
        let transfer = self.cleanverse.transfer_a_token(&transfer_req).await?;
        let exec_evt = self.ledger.record(
            AuditPhase::Execute,
            "cleanverse-atoken",
            if transfer.success { "completed" } else { "failed" },
            Some(mandate.id),
            serde_json::to_value(&transfer)?,
        )?;
        audit_ids.push(exec_evt.id);

        let bundle = ComplianceBundle {
            mandate_id: mandate.id,
            a_pass_verified: true,
            ccp_passed: true,
            policy_passed: true,
            chp_locked: true,
            travel_rule: mandate.travel_rule.clone(),
            audit_event_ids: audit_ids.clone(),
            export_ready: transfer.success,
        };

        let export_evt = self.ledger.record(
            AuditPhase::Export,
            "cm-executor",
            "bundle_ready",
            Some(mandate.id),
            serde_json::to_value(&bundle)?,
        )?;
        audit_ids.push(export_evt.id);

        Ok(PayResult {
            mandate_id: mandate.id,
            status: if transfer.success {
                MandateStatus::Completed
            } else {
                MandateStatus::Failed
            },
            tx_hash: transfer.tx_hash,
            ccp_reference: Some(ccp.ccp_reference),
            bundle,
            chp_lock: Some(chp_decision.lock),
            message: transfer.message,
        })
    }

    pub fn export_audit(&self, mandate_id: Uuid) -> Result<serde_json::Value, ExecutorError> {
        let events: Vec<_> = self
            .ledger
            .read_all()?
            .into_iter()
            .filter(|e| e.mandate_id == Some(mandate_id))
            .collect();
        Ok(serde_json::json!({
            "mandate_id": mandate_id,
            "events": events,
            "exported_at": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

pub fn default_policy_path(root: &Path) -> PathBuf {
    root.join("policies").join("mandate.yaml")
}
