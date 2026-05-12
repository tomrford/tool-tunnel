use std::{collections::HashSet, process::Stdio, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use iroh::endpoint::Accepting;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::{process::Command, sync::Semaphore};

use crate::{ExportArgs, config::Config, iroh_link};

const CHILD_INIT_TIMEOUT: Duration = Duration::from_secs(10);
const CHILD_CALL_TIMEOUT: Duration = Duration::from_secs(120);
const STREAM_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct RemoteProxy {
    child: Arc<ChildSession>,
}

struct ChildSession {
    _client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    peer: rmcp::service::Peer<rmcp::RoleClient>,
    tools: Vec<Tool>,
    call_gate: Arc<Semaphore>,
}

pub async fn run(args: ExportArgs) -> Result<()> {
    let config = Config::load(args.config.as_deref())?;
    let export_config = config.export(&args.profile)?;
    let secret = iroh_link::secret_from_file(&export_config.identity).with_context(|| {
        format!(
            "missing export identity; run `tool-tunnel identity init export {}`",
            args.profile
        )
    })?;

    let child = Arc::new(
        connect_child(
            &export_config.command,
            &export_config.args,
            export_config.cwd.as_ref(),
            &export_config.env,
        )
        .await
        .with_context(|| format!("initialize child MCP server for export {:?}", args.profile))?,
    );

    let endpoint = iroh_link::endpoint(secret, vec![iroh_link::ALPN.to_vec()]).await?;
    iroh_link::wait_online(&endpoint).await;

    let ticket = iroh_link::ticket(&endpoint);
    eprintln!("tool-tunnel export {:?} listening", args.profile);
    eprintln!("endpoint id: {}", ticket.endpoint_addr().id);
    eprintln!("ticket: {ticket}");
    eprintln!("tools: {}", child.tools.len());

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
        let allowed = allowed.clone();
        let proxy = RemoteProxy {
            child: Arc::clone(&child),
        };
        tokio::spawn(async move {
            if let Err(error) = handle_connection(accepting, allowed, proxy).await {
                tracing::warn!(?error, "remote connection failed");
            }
        });
    }

    endpoint.close().await;
    Ok(())
}

async fn connect_child(
    command: &str,
    args: &[String],
    cwd: Option<&std::path::PathBuf>,
    env: &std::collections::BTreeMap<String, String>,
) -> Result<ChildSession> {
    let transport = TokioChildProcess::new(Command::new(command).configure(|cmd| {
        cmd.args(args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
    }))
    .with_context(|| format!("spawn export command {command:?}"))?;

    let client = tokio::time::timeout(CHILD_INIT_TIMEOUT, ().serve(transport))
        .await
        .context("child MCP initialize timed out")?
        .context("serve child MCP client")?;
    let tools = tokio::time::timeout(CHILD_INIT_TIMEOUT, client.peer().list_all_tools())
        .await
        .context("child MCP tools/list timed out")?
        .context("list child MCP tools")?;
    let peer = client.peer().clone();

    Ok(ChildSession {
        _client: client,
        peer,
        tools,
        call_gate: Arc::new(Semaphore::new(1)),
    })
}

async fn handle_connection(
    accepting: Accepting,
    allowed: HashSet<String>,
    proxy: RemoteProxy,
) -> Result<()> {
    let connection = accepting.await.context("accept iroh connection")?;
    let remote_id = connection.remote_id().to_string();
    if !allowed.contains(&remote_id) {
        bail!("client endpoint {remote_id} is not allowlisted");
    }

    let (send, mut recv) = tokio::time::timeout(STREAM_ACCEPT_TIMEOUT, connection.accept_bi())
        .await
        .context("accept iroh stream timed out")?
        .context("accept iroh stream")?;
    let mut handshake = vec![0; iroh_link::HANDSHAKE.len()];
    tokio::time::timeout(HANDSHAKE_TIMEOUT, recv.read_exact(&mut handshake))
        .await
        .context("read tunnel handshake timed out")?
        .context("read tunnel handshake")?;
    if handshake != iroh_link::HANDSHAKE {
        bail!("invalid tunnel handshake");
    }

    let running = proxy
        .serve((recv, send))
        .await
        .context("serve remote MCP over iroh")?;
    running
        .waiting()
        .await
        .context("wait for remote MCP session")?;
    Ok(())
}

impl ServerHandler for RemoteProxy {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("tool-tunnel-remote", env!("CARGO_PKG_VERSION")),
        )
    }

    #[allow(clippy::manual_async_fn)]
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + MaybeSendFuture + '_ {
        async move {
            Ok(ListToolsResult {
                tools: self.child.tools.clone(),
                ..Default::default()
            })
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + MaybeSendFuture + '_ {
        async move {
            let _permit = Arc::clone(&self.child.call_gate)
                .acquire_owned()
                .await
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
            tokio::time::timeout(CHILD_CALL_TIMEOUT, self.child.peer.call_tool(request))
                .await
                .map_err(|_| McpError::internal_error("child tool call timed out", None))?
                .map_err(|error| McpError::internal_error(error.to_string(), None))
        }
    }
}
