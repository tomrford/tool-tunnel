#![allow(clippy::print_stderr)]

mod config;
mod iroh_link;
mod local;
mod remote;

use std::{io::Write, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{IdentityRole, config_base_dir, identity_path};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a client-facing stdio MCP aggregator profile.
    Client(ClientArgs),
    /// Run an exported stdio MCP server profile.
    Export(ExportArgs),
    /// Manage tool-tunnel endpoint identities.
    Identity(IdentityArgs),
}

#[derive(Parser, Debug, Clone)]
struct ClientArgs {
    /// Client profile name from config.
    #[arg(default_value = "default")]
    profile: String,
    /// JSON config with client imports and export profiles.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Parser, Debug, Clone)]
struct ExportArgs {
    /// Export profile name from config.
    profile: String,
    /// JSON config with client imports and export profiles.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Parser, Debug, Clone)]
struct IdentityArgs {
    #[command(subcommand)]
    command: IdentityCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum IdentityCommand {
    /// Create the private identity for a client or export profile.
    Init(IdentityProfileArgs),
    /// Print the public endpoint ID for a client or export profile.
    Show(IdentityProfileArgs),
}

#[derive(Parser, Debug, Clone)]
struct IdentityProfileArgs {
    /// Profile role.
    #[arg(value_enum)]
    role: IdentityRoleArg,
    /// Profile name.
    profile: String,
    /// JSON config location used to resolve the identity directory.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum IdentityRoleArg {
    Client,
    Export,
}

impl From<IdentityRoleArg> for IdentityRole {
    fn from(value: IdentityRoleArg) -> Self {
        match value {
            IdentityRoleArg::Client => Self::Client,
            IdentityRoleArg::Export => Self::Export,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    match args.command {
        Some(Command::Client(args)) => local::run(args).await,
        Some(Command::Export(args)) => remote::run(args).await,
        Some(Command::Identity(args)) => run_identity(args),
        None => {
            local::run(ClientArgs {
                profile: "default".to_owned(),
                config: None,
            })
            .await
        }
    }
}

fn run_identity(args: IdentityArgs) -> Result<()> {
    match args.command {
        IdentityCommand::Init(args) => {
            let path = identity_arg_path(&args);
            iroh_link::init_identity(&path)?;
            eprintln!(
                "created {} identity {:?} at {}",
                IdentityRole::from(args.role).command_name(),
                args.profile,
                path.display()
            );
            Ok(())
        }
        IdentityCommand::Show(args) => {
            let path = identity_arg_path(&args);
            let public = iroh_link::public_key_from_file(&path)?;
            writeln!(std::io::stdout(), "{public}")?;
            Ok(())
        }
    }
}

fn identity_arg_path(args: &IdentityProfileArgs) -> PathBuf {
    let base_dir = config_base_dir(args.config.as_deref());
    identity_path(&base_dir, args.role.into(), &args.profile)
}

fn default_config_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("tool-tunnel")
            .join("config.json");
    }
    PathBuf::from("tool-tunnel.json")
}
