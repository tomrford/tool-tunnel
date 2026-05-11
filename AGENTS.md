# AGENTS.MD

For this repo, the product is a Rust local-only peer-to-peer MCP remote bridge using iroh. The local-facing MCP side stays stdio; the remote hop uses iroh endpoint identity, explicit peer tickets, allowlisted endpoint IDs, and MCP messages over an iroh stream. The export side owns one shared child stdio MCP client session and proxies tools semantically.

Use `grepo/dumbpipe` as the primary iroh implementation reference for endpoint setup, tickets, ALPN, handshake, and stream opening. It is read-only grepo content; copy patterns, do not edit it.
