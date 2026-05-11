# Plan

`tool-tunnel` provides private remote MCP access without a hosted account system. The local process remains a normal stdio MCP server from the client point of view. Remote machines run exporters that expose real stdio MCP servers over trusted iroh byte pipes.

The first prototype has two roles.

`tool-tunnel remote` runs beside a real MCP server. It owns a persistent iroh secret key, starts an iroh endpoint, prints or writes a shareable ticket containing its endpoint ID plus relay or direct-address hints, and accepts one application ALPN: `tool-tunnel/stdio/0`. It launches or connects to the configured stdio MCP server and forwards bytes between the iroh stream and the child stdin/stdout. The exporter does not parse MCP messages.

`tool-tunnel local` is configured in Claude, Cursor, or another MCP client as a stdio server. It owns its own persistent iroh key, reads trusted remote tickets from a local config file, dials each exporter, starts one MCP client session over each reachable iroh stdio tunnel, requests tool metadata, prefixes names to avoid collisions, and forwards tool calls to the matching remote session.

Peer trust is explicit. Endpoint IDs are treated like SSH public keys. Exporters allow only configured local endpoint IDs. The local side trusts only configured exporter endpoint IDs. There is no global search, account login, or public index in the first version.

Initial user flow:

1. Start `tool-tunnel remote -- <stdio command>`.
2. Copy the printed ticket to the local machine.
3. Add the ticket to `tool-tunnel local` config.
4. Configure the MCP client to launch `tool-tunnel local`.
5. The client sees remote tools as local MCP tools.

The first code slice should prove one happy path: one local aggregator, one exporter, one wrapped stdio MCP server, one list-tools request, one tool-call request, and rejection of an untrusted peer.

Open decisions:

- Ticket encoding: use `iroh_tickets::endpoint::EndpointTicket` first, with an optional config wrapper once local aliases and trust policy need a portable bundle.
- MCP protocol handling: use `rmcp` for the local outward server and local inward client sessions if its client/server APIs fit the aggregation shape.
- Stream model: one long-lived bidirectional stdio tunnel per connected remote MCP server. Fresh streams require fresh MCP initialization and are not part of the first version.
- Config path: repo-local examples first; XDG config once the CLI contract exists.
