# AGENTS.MD

For this repo, the planned product is a Rust local-only peer-to-peer MCP remote bridge using iroh. The local-facing MCP side stays stdio; the remote hop uses iroh endpoint identity, explicit peer tickets, allowlisted endpoint IDs, and a generic stdio byte pipe to the remote MCP process.

Use `grepo/dumbpipe` as the primary iroh implementation reference for endpoint setup, tickets, ALPN, and stream forwarding. It is read-only grepo content; copy patterns, do not edit it.

Open item: local startup needs a bounded wait around remote MCP initialization and tool listing. A remote that accepts the iroh stream but never completes MCP setup should be skipped or surfaced without preventing the local stdio server from starting.
