//! CLI エントリポイント — `kernel-cli` バイナリ
//!
//! DD-10 §1.2.6 / DD-13 §F-03 (Agent Mode) に向けて、最小限の CLI を提供。
//! サブコマンド:
//! - `session create --actor <kind>`  : 新規 Session 作成
//! - `echo --session <id> '<json>'`   : echo Capability 呼び出し
//! - `sessions list`                  : 現在の Session 一覧
//! - `caps list`                      : 登録済み Capability 一覧
//! - `event-bus replay`               : Durable Event を Replay

#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use kernel_capability_registry::SideEffectLevel;
use kernel_cmd_bus::CommandBuilder;
use kernel_core::Microkernel;
use kernel_session::{ActorKind, PermissionSet, ResourceBudget};
use kernel_tracing::TracingConfig;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "kernel-cli", version, about = "Pure Rust AI Native CLI Development Kernel")]
struct Cli {
    /// ログレベル (trace / debug / info / warn / error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// JSON 形式でログ出力
    #[arg(long)]
    log_json: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 新規 Session を作成
    SessionCreate {
        #[arg(long, value_enum)]
        actor: ActorArg,
    },
    /// Session 一覧
    SessionsList,
    /// 登録済み Capability 一覧
    CapsList,
    /// echo Capability を呼び出し (MVP-1 受入基準)
    Echo {
        #[arg(long)]
        session: String,
        /// JSON 引数 (例: `{"text":"hello"}`)
        args: String,
    },
    /// Durable Event を Replay
    EventBusReplay {
        #[arg(long, default_value_t = 1)]
        from: u64,
        #[arg(long)]
        to: Option<u64>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
enum ActorArg {
    Human,
    LangGraph,
    Agent,
    Ci,
    Plugin,
    Automation,
}

impl From<ActorArg> for ActorKind {
    fn from(a: ActorArg) -> Self {
        match a {
            ActorArg::Human => Self::Human,
            ActorArg::LangGraph => Self::LangGraph,
            ActorArg::Agent => Self::Agent,
            ActorArg::Ci => Self::Ci,
            ActorArg::Plugin => Self::Plugin,
            ActorArg::Automation => Self::Automation,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    kernel_tracing::init_tracing(&TracingConfig {
        level: cli.log_level.clone(),
        json: cli.log_json,
        target: "stdout".into(),
    })
    .context("failed to init tracing")?;

    let kernel = Microkernel::new();
    kernel.register_echo_capability().await?;

    match cli.cmd {
        Commands::SessionCreate { actor } => {
            let perm = match actor {
                ActorArg::Human => PermissionSet::human_editor(),
                _ => PermissionSet::agent_default(),
            };
            let s = kernel
                .session_manager
                .create(actor.into(), perm, ResourceBudget::default())
                .await?;
            println!("{}", s.id.0);
        }
        Commands::SessionsList => {
            println!("session_count={}", kernel.session_manager.count());
        }
        Commands::CapsList => {
            for m in kernel.capability_registry.list() {
                println!(
                    "{}\t{}\t{:?}\tv{}",
                    m.capability_id, m.description, m.side_effect_level, m.version
                );
            }
        }
        Commands::Echo { session, args } => {
            let sid = uuid::Uuid::parse_str(&session)
                .map_err(|e| anyhow::anyhow!("invalid session uuid: {e}"))?;
            let args_json: serde_json::Value = serde_json::from_str(&args)
                .with_context(|| format!("invalid JSON args: {args}"))?;
            let cmd = CommandBuilder::new("echo", kernel_session::SessionId(sid))
                .args(args_json)
                .build();
            let result = kernel.command_bus.execute(cmd).await?;
            match result.status {
                kernel_cmd_bus::CommandStatus::Ok => {
                    println!("{}", serde_json::to_string_pretty(&result.data.unwrap())?);
                }
                _ => {
                    eprintln!(
                        "command failed: status={:?} error={:?}",
                        result.status, result.error
                    );
                    std::process::exit(1);
                }
            }
        }
        Commands::EventBusReplay { from, to } => {
            let replayed = kernel.event_bus.replay(from, to).await?;
            for e in replayed {
                println!(
                    "seq={} type={} corr={} at={}",
                    e.sequence, e.event_type, e.correlation_id, e.occurred_at
                );
            }
        }
    }

    Ok(())
}

// SideEffectLevel への re-export 防止: 使われないがビルドを通すため
#[allow(dead_code)]
fn _force_link(_: SideEffectLevel, _: Arc<()>) {}