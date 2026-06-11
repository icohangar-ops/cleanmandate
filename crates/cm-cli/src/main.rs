use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cm_chp::ChpConfig;
use cm_cleanverse::{CleanverseClient, CleanverseConfig};
use cm_core::{default_ledger_path, AgentMandate};
use cm_executor::{default_policy_path, ExecutorConfig, MandateExecutor};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "cleanmandate", about = "Verified AI agent payment mandates (Cleanverse + Monad)")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,

    #[arg(long, global = true, env = "CLEANMANDATE_SIGNING_KEY")]
    signing_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run full mandate pipeline: A-Pass → policy → CCP → CHP → A-Token
    Pay {
        #[arg(long)]
        mandate: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Export signed audit bundle for a mandate
    Export {
        #[arg(long)]
        mandate_id: Uuid,
    },
    /// Validate mandate JSON against local policy (no Cleanverse calls)
    Validate {
        #[arg(long)]
        mandate: PathBuf,
    },
    /// Show Cleanverse client mode and config
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize().unwrap_or(cli.root.clone());

    match cli.command {
        Commands::Pay { mandate, dry_run } => {
            let text = fs::read_to_string(&mandate)
                .with_context(|| format!("read mandate {}", mandate.display()))?;
            let mandate: AgentMandate =
                serde_json::from_str(&text).context("parse mandate JSON")?;

            let cv_config = CleanverseConfig::from_env();
            let executor = MandateExecutor::new(
                ExecutorConfig {
                    policy_path: default_policy_path(&root),
                    ledger_path: default_ledger_path(&root),
                    signing_key: cli.signing_key,
                    dry_run,
                    chp: ChpConfig::default(),
                },
                CleanverseClient::new(cv_config),
            )
            .await?;

            let result = executor.pay(&mandate).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Export { mandate_id } => {
            let cv_config = CleanverseConfig::from_env();
            let executor = MandateExecutor::new(
                ExecutorConfig {
                    policy_path: default_policy_path(&root),
                    ledger_path: default_ledger_path(&root),
                    signing_key: cli.signing_key,
                    dry_run: true,
                    chp: ChpConfig::default(),
                },
                CleanverseClient::new(cv_config),
            )
            .await?;
            let bundle = executor.export_audit(mandate_id)?;
            println!("{}", serde_json::to_string_pretty(&bundle)?);
        }
        Commands::Validate { mandate } => {
            let text = fs::read_to_string(&mandate)?;
            let mandate: AgentMandate = serde_json::from_str(&text)?;
            let policy = cm_policy::MandatePolicy::load(&default_policy_path(&root))?;
            let decision = policy.evaluate(&mandate);
            println!("{}", serde_json::to_string_pretty(&decision)?);
        }
        Commands::Status => {
            let cv = CleanverseConfig::from_env();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "mode": format!("{:?}", cv.mode),
                    "api_base": cv.api_base,
                    "api_key_set": cv.api_key.is_some(),
                    "policy": default_policy_path(&root),
                    "ledger": default_ledger_path(&root),
                }))?
            );
        }
    }

    Ok(())
}
