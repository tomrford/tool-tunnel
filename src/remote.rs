use std::{collections::HashSet, process::Stdio};

use anyhow::{Context, Result, bail};
use iroh::endpoint::Accepting;
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{ExportArgs, config::Config, iroh_link};

pub async fn run(args: ExportArgs) -> Result<()> {
    let config = Config::load(args.config.as_deref())?;
    let export_config = config.export(&args.profile)?;
    let secret = iroh_link::secret_from_file(&export_config.identity).with_context(|| {
        format!(
            "missing export identity; run `tool-tunnel identity init export {}`",
            args.profile
        )
    })?;
    let endpoint = iroh_link::endpoint(secret, vec![iroh_link::ALPN.to_vec()]).await?;
    iroh_link::wait_online(&endpoint).await;

    let ticket = iroh_link::ticket(&endpoint);
    eprintln!("tool-tunnel remote listening");
    eprintln!("endpoint id: {}", ticket.endpoint_addr().id);
    eprintln!("ticket: {ticket}");
    let allowed = export_config
        .allow_clients
        .into_iter()
        .collect::<HashSet<_>>();
    loop {
        let incoming = tokio::select! {
            incoming = endpoint.accept() => incoming,
            _ = tokio::signal::ctrl_c() => {
                eprintln!("received ctrl-c, shutting down");
                break;
            }
        };
        let Some(incoming) = incoming else { break };
        let Ok(accepting) = incoming.accept() else {
            continue;
        };
        let command = export_config.command.clone();
        let args = export_config.args.clone();
        let cwd = export_config.cwd.clone();
        let env = export_config.env.clone();
        let allowed = allowed.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(accepting, command, args, cwd, env, allowed).await
            {
                tracing::warn!(?error, "remote connection failed");
            }
        });
    }

    endpoint.close().await;
    Ok(())
}

async fn handle_connection(
    accepting: Accepting,
    command: String,
    args: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    env: std::collections::BTreeMap<String, String>,
    allowed: HashSet<String>,
) -> Result<()> {
    let connection = accepting.await.context("accept iroh connection")?;
    let remote_id = connection.remote_id().to_string();
    if !allowed.contains(&remote_id) {
        bail!("client endpoint {remote_id} is not allowlisted");
    }

    let (mut send, mut recv) = connection.accept_bi().await.context("accept iroh stream")?;
    let mut handshake = vec![0; iroh_link::HANDSHAKE.len()];
    recv.read_exact(&mut handshake)
        .await
        .context("read tunnel handshake")?;
    if handshake != iroh_link::HANDSHAKE {
        bail!("invalid tunnel handshake");
    }

    let mut child_command = Command::new(&command);
    child_command
        .args(args)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(cwd) = cwd {
        child_command.current_dir(cwd);
    }
    let mut child = child_command
        .spawn()
        .with_context(|| format!("spawn export command {command:?}"))?;

    let mut child_stdin = child.stdin.take().context("child stdin unavailable")?;
    let mut child_stdout = child.stdout.take().context("child stdout unavailable")?;

    let to_child = tokio::spawn(async move {
        let copied = tokio::io::copy(&mut recv, &mut child_stdin).await?;
        child_stdin.shutdown().await?;
        std::io::Result::Ok(copied)
    });
    let from_child = tokio::spawn(async move {
        let copied = tokio::io::copy(&mut child_stdout, &mut send).await?;
        send.shutdown().await?;
        std::io::Result::Ok(copied)
    });

    let _ = tokio::try_join!(to_child, from_child)?;
    let status = child.wait().await.context("wait for remote command")?;
    if !status.success() {
        bail!("remote command exited with {status}");
    }

    Ok(())
}
