# tool-tunnel

`tool-tunnel` is a local-first plan for exposing remote stdio MCP servers through peer-to-peer iroh connections while keeping the MCP client-facing surface as normal stdio.

The first build target is one Rust binary with two commands:

- `tool-tunnel local`: a local stdio MCP server that Claude, Cursor, and similar tools launch directly. It reads trusted remote tickets, dials remote exporters over iroh, starts one MCP client session per reachable remote, merges tool lists, prefixes tool names, and forwards calls.
- `tool-tunnel remote`: a remote-side process that launches or connects to a stdio MCP server and exposes its stdin/stdout as a trusted iroh byte pipe to allowlisted local endpoint IDs.

The current repository state is planning and toolchain wiring. Start with [docs/plan.md](docs/plan.md), [docs/architecture.md](docs/architecture.md), and [docs/research.md](docs/research.md).

The primary implementation reference is `grepo/dumbpipe`, pinned with grepo from `https://github.com/n0-computer/dumbpipe.git`. It already implements the iroh ticket, ALPN, endpoint setup, and stream-forwarding shape that `tool-tunnel remote` builds on.

## Development

```sh
direnv allow
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
nix build
```
