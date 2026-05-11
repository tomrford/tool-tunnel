# Architecture

The design keeps MCP transport boundaries simple. MCP clients already know how to launch stdio servers, so `tool-tunnel client` presents stdio locally and hides the remote hop. Remote exporters expose an MCP server over iroh while keeping one managed MCP client session to the wrapped stdio MCP server.

The local machine runs the only process an MCP client knows about: `tool-tunnel client`. It behaves like a normal stdio MCP server on its client-facing side, but internally it reads trusted remote tickets, dials each configured exporter, initializes one MCP client session per remote, aggregates tool metadata, and publishes prefixed tool names back to the original client.

The network boundary is an encrypted iroh QUIC stream selected by the `tool-tunnel/stdio/0` ALPN. Tickets carry the remote endpoint identity and reachability hints; iroh decides whether traffic can move directly or needs relay assistance. Endpoint IDs are the peer identities used for explicit trust decisions on both sides.

The remote machine runs `tool-tunnel export` beside the real stdio MCP server. The exporter admits only allowlisted local endpoint IDs, accepts selected iroh streams, checks the fixed tunnel handshake, and serves normal MCP messages to each local adapter. It initializes the child stdio MCP server once, caches `tools/list`, and forwards `tools/call` into the single child session. Remote child stderr and exporter logs stay on stderr so neither side injects non-MCP bytes into stdout.

```text
MCP client
  stdio
tool-tunnel client
  MCP-over-iroh client sessions
  iroh ALPN tool-tunnel/stdio/0
tool-tunnel export
  one MCP client session over stdio
real MCP server
```

Iroh supplies process-level identity and encrypted connectivity. Each process persists an iroh secret key. The public endpoint ID is the stable peer identity. Tickets carry endpoint identity plus reachability hints, usually a relay URL and optionally direct addresses. Relays help rendezvous and NAT traversal; the encrypted data path moves direct when iroh can establish a direct path and remains relayed when it cannot.

The iroh connection setup is intentionally close to dumbpipe:

- ALPN selects `tool-tunnel/stdio/0`.
- The connecting side sends a small fixed handshake so the listener can reject accidental streams.
- After the handshake, `rmcp` runs over the iroh bidirectional stream.
- Child stderr and exporter logs stay on exporter stderr; local stdout remains reserved for local MCP messages.

`grepo/dumbpipe` is the closest iroh implementation reference. It demonstrates endpoint creation with `Endpoint::builder(presets::N0)`, `EndpointTicket` creation from `endpoint.addr()`, stderr-only ticket output, custom ALPN parsing, a fixed connecting-side handshake, and `open_bi`/`accept_bi`. `tool-tunnel` reuses those patterns while putting MCP semantics in the proxy layer.

The local process owns aggregation. It starts one MCP client session over each connected remote, initializes the remote server, fetches tool lists, prefixes tool names with the configured remote alias, maps calls back to the original remote tool name, and keeps per-remote connection state observable. The exporter owns endpoint admission, the shared child MCP lifecycle, cached tool metadata, serialized child calls, and call timeouts.

Security baseline:

- no unauthenticated peers;
- no ambient local network trust;
- endpoint-ID pinning on both sides;
- explicit command config for wrapped stdio servers;
- stderr logs only, never non-MCP bytes on local stdout;
- no automatic remote discovery in the first version.

The future hosted shape keeps the same layers. The agent-facing facade can become Streamable HTTP, a codemode-style `search`/`execute` surface, or an Executor-style catalog and policy layer. The transport to execution sites can remain iroh, move to Tailscale/TCP for testing, or become an outbound hosted tunnel. The remote proxy stays semantic because scheduling, cancellation, restart, cache invalidation, and session repair belong beside the shared child MCP session.
