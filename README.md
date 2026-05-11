# tool-tunnel

`tool-tunnel` exposes remote stdio MCP servers through peer-to-peer iroh connections while keeping the MCP client-facing surface as normal stdio.

The first build target is one Rust binary with two commands:

- `tool-tunnel client`: a local stdio MCP server that Claude, Cursor, and similar tools launch directly. It reads trusted remote tickets, dials remote exporters over iroh, starts one MCP client session per reachable remote, merges tool lists, prefixes tool names, and forwards calls.
- `tool-tunnel export`: a remote-side process that launches one stdio MCP child, keeps one managed MCP client session to it, and exposes its cached tools/call surface as an MCP server over iroh to allowlisted local endpoint IDs.

Start with [docs/architecture.md](docs/architecture.md) for the runtime shape and [docs/plan.md](docs/plan.md) for the product contract.

The primary iroh reference is `grepo/dumbpipe`, pinned with grepo from `https://github.com/n0-computer/dumbpipe.git`. It provides endpoint, ticket, ALPN, handshake, and stream setup patterns; MCP lifecycle and scheduling live in `tool-tunnel`.

## Development

```sh
direnv allow
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
nix build
```
