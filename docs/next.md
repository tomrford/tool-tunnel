# Next

- Decide first dependency set after a quick crate health check: `iroh`, `tokio`, `clap`, `serde`, `serde_json`, `toml`, `tracing`, and a possible MCP protocol crate.
- Align iroh dependencies with `grepo/dumbpipe`: `iroh = "=1.0.0-rc.0"` and `iroh-tickets = "=1.0.0-rc.0"` unless a current upstream check shows a better stable release. Add `noq` only if iroh stream copying requires it directly.
- Split the placeholder binary into local-by-default mode and a `remote` subcommand.
- Add ticket and config types with round-trip tests, using `iroh_tickets::endpoint::EndpointTicket` as the first ticket format.
- Build an iroh smoke test with one `tool-tunnel/stdio/0` ALPN, one local endpoint, one exporter endpoint, and pinned peer IDs.
- Add exporter-side byte forwarding between one accepted iroh stream and one child MCP server stdin/stdout.
- Add local-side MCP client sessions over remote stdio tunnels for `initialize`, `tools/list`, and `tools/call`.
- Add local-side `rmcp` server handling with dynamic tool publication from the connected remotes.
- Add an example wrapped MCP server for integration tests.
