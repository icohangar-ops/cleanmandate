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
        let canonical = serde_json::json!({
            "phase": phase,
            "agent": agent,
            "action": action,
            "mandate_id": mandate_id,
            "details": details,
            "timestamp": timestamp.to_rfc3339(),
        });
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
                events.push(serde_json::from_str(&line)?);
            }
        }
        Ok(events)
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
