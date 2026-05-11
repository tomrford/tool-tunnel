use std::{borrow::Cow, collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};

use crate::{ClientArgs, config::Config, iroh_link};

struct Aggregator {
    remotes: Arc<HashMap<String, RemoteSession>>,
    tools: Arc<Vec<Tool>>,
}

struct RemoteSession {
    _client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    peer: rmcp::service::Peer<rmcp::RoleClient>,
}

pub async fn run(args: ClientArgs) -> Result<()> {
    let config = Config::load(args.config.as_deref())?;
    let client_config = config.client(&args.profile)?;
    let secret = iroh_link::secret_from_file(&client_config.identity).with_context(|| {
        format!(
            "missing client identity; run `tool-tunnel identity init client {}`",
            args.profile
        )
    })?;
    let endpoint = iroh_link::endpoint(secret, vec![]).await?;
    iroh_link::wait_online(&endpoint).await;

    let mut remotes = HashMap::new();
    let mut published_tools = Vec::new();

    for (alias, import_config) in client_config.imports {
        match connect_remote(&endpoint, &alias, &import_config.ticket).await {
            Ok((session, tools)) => {
                for tool in tools {
                    published_tools.push(prefix_tool(&alias, tool));
                }
                remotes.insert(alias.clone(), session);
            }
            Err(error) => {
                eprintln!("warning: skipping import {alias:?}: {error:#}");
            }
        }
    }

    let service = Aggregator {
        remotes: Arc::new(remotes),
        tools: Arc::new(published_tools),
    };

    let running = service
        .serve(stdio())
        .await
        .context("serve local MCP stdio")?;
    running
        .waiting()
        .await
        .context("wait for local MCP service")?;
    endpoint.close().await;
    Ok(())
}

async fn connect_remote(
    endpoint: &iroh::Endpoint,
    alias: &str,
    ticket: &str,
) -> Result<(RemoteSession, Vec<Tool>)> {
    let ticket = iroh_link::parse_ticket(ticket)?;
    let addr = ticket.endpoint_addr();
    let connection = endpoint
        .connect(addr.clone(), iroh_link::ALPN)
        .await
        .with_context(|| format!("connect to remote {alias:?}"))?;
    let (mut send, recv) = connection.open_bi().await.context("open iroh stream")?;
    send.write_all(iroh_link::HANDSHAKE)
        .await
        .context("write tunnel handshake")?;

    let client = ().serve((recv, send)).await.context("initialize remote MCP session")?;
    let tools = client
        .peer()
        .list_tools(Default::default())
        .await
        .context("list remote tools")?
        .tools;

    let peer = client.peer().clone();
    Ok((
        RemoteSession {
            _client: client,
            peer,
        },
        tools,
    ))
}

fn prefix_tool(alias: &str, mut tool: Tool) -> Tool {
    let original = tool.name.to_string();
    tool.name = Cow::Owned(format!("{alias}__{original}"));
    if tool.title.is_none() {
        tool.title = Some(format!("{alias}: {original}"));
    }
    tool
}

fn split_tool_name(name: &str) -> Option<(&str, &str)> {
    name.split_once("__")
}

impl ServerHandler for Aggregator {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("tool-tunnel", env!("CARGO_PKG_VERSION")),
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
                tools: self.tools.as_ref().clone(),
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
            let (alias, original_name) = split_tool_name(&request.name)
                .map(|(alias, name)| (alias.to_owned(), name.to_owned()))
                .ok_or_else(|| McpError::invalid_params("tool name must be prefixed", None))?;
            let remote = self
                .remotes
                .get(&alias)
                .ok_or_else(|| McpError::invalid_params("unknown remote alias", None))?;
            let mut forwarded = request;
            forwarded.name = Cow::Owned(original_name);
            remote
                .peer
                .call_tool(forwarded)
                .await
                .map_err(|error| McpError::internal_error(error.to_string(), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::JsonObject;

    #[test]
    fn prefixes_tool_name() {
        let tool = Tool::new("status", "status", Arc::new(JsonObject::default()));
        let prefixed = prefix_tool("remote_a", tool);

        assert_eq!(prefixed.name, "remote_a__status");
    }

    #[test]
    fn splits_prefixed_name() {
        assert_eq!(
            split_tool_name("remote_a__status"),
            Some(("remote_a", "status"))
        );
    }
}
