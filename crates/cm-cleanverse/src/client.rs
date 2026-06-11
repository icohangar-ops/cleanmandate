use crate::types::{
    ApassVerification, CcpPreCheckRequest, CcpPreCheckResult, TokenTransferRequest,
    TokenTransferResult,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanverseMode {
    Mock,
    Sandbox,
}

#[derive(Debug, Clone)]
pub struct CleanverseConfig {
    pub mode: CleanverseMode,
    pub api_base: String,
    pub api_key: Option<String>,
}

impl CleanverseConfig {
    pub fn from_env() -> Self {
        let mode = match std::env::var("CLEANVERSE_MODE")
            .unwrap_or_else(|_| "mock".into())
            .to_lowercase()
            .as_str()
        {
            "sandbox" | "live" => CleanverseMode::Sandbox,
            _ => CleanverseMode::Mock,
        };
        Self {
            mode,
            api_base: std::env::var("CLEANVERSE_API_BASE")
                .unwrap_or_else(|_| "https://sandbox.api.cleanverse.com/v3".into()),
            api_key: std::env::var("CLEANVERSE_API_KEY").ok(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CleanverseError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api error: {0}")]
    Api(String),
}

pub struct CleanverseClient {
    config: CleanverseConfig,
    http: Client,
}

impl CleanverseClient {
    pub fn new(config: CleanverseConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    pub fn mode(&self) -> CleanverseMode {
        self.config.mode
    }

    pub async fn verify_apass(&self, wallet: &str) -> Result<ApassVerification, CleanverseError> {
        if self.config.mode == CleanverseMode::Mock {
            return Ok(ApassVerification {
                wallet: wallet.to_string(),
                verified: wallet.starts_with("0x"),
                identity_id: Some(format!("apass-mock-{}", &wallet[..wallet.len().min(10)])),
                kyc_tier: Some("tier2".into()),
                message: "mock A-Pass verification".into(),
            });
        }

        #[derive(Serialize)]
        struct Req<'a> {
            wallet: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            verified: bool,
            identity_id: Option<String>,
            kyc_tier: Option<String>,
            message: Option<String>,
        }

        let resp: Resp = self
            .post("/apass/verify", &Req { wallet })
            .await?;
        Ok(ApassVerification {
            wallet: wallet.to_string(),
            verified: resp.verified,
            identity_id: resp.identity_id,
            kyc_tier: resp.kyc_tier,
            message: resp.message.unwrap_or_else(|| "A-Pass verified".into()),
        })
    }

    pub async fn ccp_pre_check(
        &self,
        req: &CcpPreCheckRequest,
    ) -> Result<CcpPreCheckResult, CleanverseError> {
        if self.config.mode == CleanverseMode::Mock {
            let blocked = req.to_wallet.eq_ignore_ascii_case("0xdead000000000000000000000000000000000001");
            return Ok(CcpPreCheckResult {
                passed: !blocked,
                ccp_reference: format!("ccp-mock-{}", Uuid::new_v4()),
                travel_rule_status: if blocked {
                    "blocked".into()
                } else {
                    "cleared".into()
                },
                blocked_reason: if blocked {
                    Some("sanctions match (mock)".into())
                } else {
                    None
                },
            });
        }

        let resp: CcpPreCheckResult = self.post("/ccp/pre-check", req).await?;
        Ok(resp)
    }

    pub async fn transfer_a_token(
        &self,
        req: &TokenTransferRequest,
    ) -> Result<TokenTransferResult, CleanverseError> {
        if self.config.mode == CleanverseMode::Mock {
            return Ok(TokenTransferResult {
                success: true,
                tx_hash: Some(format!("0xmock{:064x}", Uuid::new_v4().as_u128())),
                a_token_reference: format!("atoken-mock-{}", Uuid::new_v4()),
                message: "mock A-Token transfer on Monad testnet".into(),
            });
        }

        let resp: TokenTransferResult = self.post("/atoken/transfer", req).await?;
        Ok(resp)
    }

    async fn post<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, CleanverseError> {
        let url = format!("{}{}", self.config.api_base.trim_end_matches('/'), path);
        let mut req = self.http.post(&url).json(body);
        if let Some(key) = &self.config.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(CleanverseError::Api(format!("{status}: {text}")));
        }
        Ok(resp.json().await?)
    }
}
