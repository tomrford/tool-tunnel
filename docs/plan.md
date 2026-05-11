# Plan

`tool-tunnel` provides private remote MCP access without a hosted account system. The local process remains a normal stdio MCP server from the outer client point of view. Remote machines run exporters that expose one managed child stdio MCP session over trusted iroh MCP sessions.

The runtime has two roles.

`tool-tunnel export <profile>` runs beside a real MCP server. It owns a persistent iroh secret key, starts an iroh endpoint, prints a shareable ticket containing its endpoint ID and reachability hints, and accepts one application ALPN: `tool-tunnel/stdio/0`. It launches the configured child stdio MCP server once, initializes it once, caches `tools/list`, serves that catalog to allowlisted local adapters, and forwards `tools/call` into the shared child session. Calls are serialized by default.

`tool-tunnel client <profile>` is configured in Claude, Cursor, Codex, or another MCP client as a stdio server. It owns its own persistent iroh key, reads trusted import tickets from config, verifies endpoint ID pins, dials each exporter, starts one MCP client session over each reachable iroh stream, requests tool metadata, prefixes names to avoid collisions, and forwards tool calls to the matching remote session.

Peer trust is explicit. Endpoint IDs are treated like SSH public keys. Exporters allow only configured local endpoint IDs. The local side trusts only configured exporter endpoint IDs. There is no global search, account login, public index, temp identity, or open export mode.

Config is human-editable JSON plus tool-owned identity files under the config directory:

```text
~/.config/tool-tunnel/
  config.json
  identities/
    clients/default
    exports/mini-tools
```

User flow:

1. Run `tool-tunnel identity init client default` and share `tool-tunnel identity show client default` with the export machine.
2. Run `tool-tunnel identity init export mini-tools` on the export machine.
3. Configure the export profile command, args, cwd, env, and `allowClients`.
4. Run `tool-tunnel export mini-tools`, then copy its ticket and endpoint ID into the client profile imports.
5. Configure the outer MCP app to launch `tool-tunnel client default`.

Open implementation work:

- Restart and cache-refresh behavior for child MCP process failure or tool-list changes.
- Cancellation propagation from local callers into the shared child session.
- Per-tool or per-server concurrency policy after real tool safety is known.
