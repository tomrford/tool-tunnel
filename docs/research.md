# Research

## Chat Summary

The shared ChatGPT plan chooses iroh over Tailscale, libp2p, and NATS for the local/private prototype. Tailscale is production-proven but requires a tailnet and auth/admin model. libp2p is broader and more complex than needed. NATS is straightforward with a broker, but then the backend becomes the control plane instead of a private peer-to-peer hop.

The recommended shape is a remote stdio exporter plus local MCP aggregator. The exporter wraps a stdio MCP server and exposes its stdin/stdout over an iroh ALPN. The local aggregator is the only process configured in the MCP client; it reads remote tickets, dials exporters, starts MCP client sessions over the tunnels, fetches tool manifests, prefixes tools, and forwards calls. Trust is endpoint-ID based, with allowlists on both ends.

The chat also separates the local/private product from a SaaS bridge. A SaaS bridge for web AI clients needs a public HTTPS MCP endpoint because web clients cannot dial arbitrary iroh peers.

## Iroh

Current docs.rs shows `iroh` 0.97.0. The crate describes iroh as peer-to-peer QUIC connectivity with direct connection establishment backed by hole punching and relay servers. The central API is `Endpoint`; dialing requires an `EndpointAddr`, an endpoint ID, addressing information such as a relay URL or direct addresses, and an ALPN protocol name. Connections expose cheap QUIC streams.

The current docs also mark `iroh-net` as renamed to `iroh`, so new code should depend on `iroh`, not `iroh-net`.

Iroh discovery docs describe endpoint discovery as publishing addressing information through configured discovery services, usually the home relay. Relay docs describe relays as temporary encrypted traffic routers until a direct path is available; when direct connectivity is not available, encrypted traffic can remain relayed.

Sources:

- https://docs.rs/iroh/latest/iroh/
- https://docs.rs/iroh-net/latest/iroh_net/endpoint/
- https://docs.iroh.computer/concepts/discovery
- https://docs.iroh.computer/concepts/relays
- https://www.iroh.computer/docs/concepts/router

## Dumbpipe

`grepo/dumbpipe` is the closest concrete reference. It is maintained by the iroh team and describes itself as a QUIC pipe between machines using iroh for hole punching, NAT traversal, relay fallback, endpoint IDs, and encryption.

The pinned snapshot is `n0-computer/dumbpipe` `main` at commit `ffa3d1f322bcaba4ec063289cd21e341eed93bab`. Its package version is `0.37.0`, using `iroh = "=1.0.0-rc.0"`, `iroh-tickets = "=1.0.0-rc.0"`, and `noq = "=1.0.0-rc.0"`.

Important code points:

- `src/lib.rs` exports `EndpointTicket`, `ALPN = b"DUMBPIPEV0"`, and a fixed connecting-side `hello` handshake.
- `create_endpoint` builds `Endpoint::builder(presets::N0).secret_key(secret_key).alpns(alpns)`.
- listeners wait for `endpoint.online()`, create a ticket from `endpoint.addr()`, and print tickets on stderr to avoid corrupting stdout.
- connect commands parse a ticket, dial its `EndpointAddr`, open a bidirectional stream, send the handshake, and copy bytes both ways.
- TCP and Unix socket modes show how to multiplex multiple local connections over repeated iroh bidirectional streams.

`tool-tunnel` is not a wrapper around the dumbpipe CLI. It should copy the endpoint/ticket/stream lifecycle patterns for the remote exporter and put MCP-specific behavior in the local aggregator: stdio JSON-RPC discipline, MCP client sessions over remote tunnels, tool-list aggregation, name prefixing, trusted-peer config, and structured errors.

## MCP Transport

The MCP spec defines stdio and Streamable HTTP as standard transports. Stdio is process-local: the client launches the server, JSON-RPC messages are newline-delimited UTF-8 on stdin/stdout, stderr is available for logging, and stdout must contain only valid MCP messages. Streamable HTTP is the remote standard transport and requires local HTTP servers to bind carefully and validate origins.

Custom transports are allowed, but the practical compatibility path is to keep the client-facing side stdio. `tool-tunnel local` is therefore a stdio MCP server even though it uses iroh internally.

The remote exporter does not define a new MCP transport. It provides a byte-equivalent stdio path to the child MCP server. The local aggregator is the MCP client for every remote tunnel.

Source:

- https://modelcontextprotocol.io/specification/2025-06-18/basic/transports

## Alternatives

Tailscale/tsnet gives an embeddable Go library for joining a tailnet and exposing virtual private services. It is a strong option when a tailnet account and admin model are acceptable.

libp2p offers a general modular peer-to-peer networking stack with QUIC, identify, discovery/routing, and relay protocols. It is broader than this prototype needs and likely shifts effort into network-stack design.

NATS is a good brokered architecture for a hosted product. It simplifies routing and observability, but the transport is no longer peer-to-peer.

Executor-style catalogs and Cloudflare codemode-style public surfaces favor the same layer split: a bridge or facade owns catalog, auth, policy, search, and execution UX, while connectors provide access to execution sites. This keeps the V1 remote exporter useful as a generic stdio pipe even if the public face later becomes Streamable HTTP, `search`/`execute`, or a single `code` tool.

Syncthing, Magic Wormhole, and Reticulum are useful reference points for private sharing, pairing, and mesh networking, but they do not directly match the MCP stdio-to-remote-tool bridge shape.
