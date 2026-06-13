use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditPhase {
    Ingest,
    Apass,
    Policy,
    Ccp,
    Chp,
    Principal,
    Execute,
    Export,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub phase: AuditPhase,
    pub agent: String,
    pub action: String,
    pub mandate_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub content_hash: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LedgerConfig {
    pub signing_key: Option<String>,
    pub ledger_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit ledger integrity check failed for event {0}: {1}")]
    Tampered(Uuid, String),
}

pub struct AuditLedger {
    config: LedgerConfig,
}

impl AuditLedger {
    pub fn new(config: LedgerConfig) -> Self {
        Self { config }
    }

    pub fn record(
        &self,
        phase: AuditPhase,
        agent: &str,
        action: &str,
        mandate_id: Option<Uuid>,
        details: serde_json::Value,
    ) -> Result<TraceEvent, AuditError> {
        let timestamp = Utc::now();
        let canonical =
            canonical_payload(phase, agent, action, mandate_id, &details, timestamp);
        let content_hash = hash_payload(&canonical);
        let signature = self
            .config
            .signing_key
            .as_ref()
            .map(|k| sign_payload(k, &content_hash));

        let event = TraceEvent {
            id: Uuid::new_v4(),
            timestamp,
            phase,
            agent: agent.to_string(),
            action: action.to_string(),
            mandate_id,
            details,
            content_hash,
            signature,
        };
        self.append(&event)?;
        Ok(event)
    }

    pub fn read_all(&self) -> Result<Vec<TraceEvent>, AuditError> {
        if !self.config.ledger_path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&self.config.ledger_path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                let event: TraceEvent = serde_json::from_str(&line)?;
                self.verify_event(&event)?;
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Re-derive the content hash and (when a signing key is configured)
    /// re-check the stored HMAC signature. This prevents a tampered or forged
    /// ledger line from being presented as legitimate compliance evidence.
    fn verify_event(&self, event: &TraceEvent) -> Result<(), AuditError> {
        let canonical = canonical_payload(
            event.phase,
            &event.agent,
            &event.action,
            event.mandate_id,
            &event.details,
            event.timestamp,
        );
        let expected_hash = hash_payload(&canonical);
        if expected_hash != event.content_hash {
            return Err(AuditError::Tampered(
                event.id,
                "content hash mismatch".into(),
            ));
        }

        if let Some(key) = &self.config.signing_key {
            let signature = event.signature.as_ref().ok_or_else(|| {
                AuditError::Tampered(event.id, "missing signature".into())
            })?;
            let expected_sig = sign_payload(key, &event.content_hash);
            // Constant-time-ish comparison via fixed-length hex strings.
            if !bool::from(constant_time_eq(
                expected_sig.as_bytes(),
                signature.as_bytes(),
            )) {
                return Err(AuditError::Tampered(
                    event.id,
                    "signature mismatch".into(),
                ));
            }
        }
        Ok(())
    }

    fn append(&self, event: &TraceEvent) -> Result<(), AuditError> {
        if let Some(parent) = self.config.ledger_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.ledger_path)?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }
}

/// Build the exact canonical JSON payload that is hashed (and then signed).
/// Both `record` and `verify_event` MUST go through this so a verified hash is
/// guaranteed to match what was written.
fn canonical_payload(
    phase: AuditPhase,
    agent: &str,
    action: &str,
    mandate_id: Option<Uuid>,
    details: &serde_json::Value,
    timestamp: DateTime<Utc>,
) -> serde_json::Value {
    serde_json::json!({
        "phase": phase,
        "agent": agent,
        "action": action,
        "mandate_id": mandate_id,
        "details": details,
        "timestamp": timestamp.to_rfc3339(),
    })
}

/// Length-aware constant-time byte comparison to avoid leaking signature bytes
/// via early-exit timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> subtle_eq::Choice {
    subtle_eq::ct_eq(a, b)
}

mod subtle_eq {
    pub struct Choice(u8);
    impl From<Choice> for bool {
        fn from(c: Choice) -> bool {
            c.0 == 1
        }
    }
    pub fn ct_eq(a: &[u8], b: &[u8]) -> Choice {
        if a.len() != b.len() {
            return Choice(0);
        }
        let mut diff: u8 = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        Choice(((diff == 0) as u8) & 1)
    }
}

pub fn hash_payload(value: &serde_json::Value) -> String {
    hex::encode(Sha256::digest(serde_json::to_vec(value).unwrap_or_default()))
}

fn sign_payload(key: &str, hash: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("hmac");
    mac.update(hash.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn default_ledger_path(root: &Path) -> PathBuf {
    root.join(".cleanmandate").join("audit.jsonl")
}
