#!/usr/bin/env python3
import json
import sys


TOOLS = [
    {
        "name": "status",
        "description": "Return fixture status.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    }
]


def write(message):
    sys.stdout.write(json.dumps(message, separators=(",", ":")) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if "id" not in message:
        continue

    if method == "initialize":
        write(
            {
                "jsonrpc": "2.0",
                "id": message["id"],
                "result": {
                    "protocolVersion": message.get("params", {}).get(
                        "protocolVersion", "2025-11-25"
                    ),
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "fixture", "version": "0.1.0"},
                },
            }
        )
    elif method == "tools/list":
        write({"jsonrpc": "2.0", "id": message["id"], "result": {"tools": TOOLS}})
    elif method == "tools/call":
        params = message.get("params", {})
        write(
            {
                "jsonrpc": "2.0",
                "id": message["id"],
                "result": {
                    "content": [
                        {
                            "type": "text",
                            "text": f"called {params.get('name')} with {params.get('arguments')}",
                        }
                    ],
                    "isError": False,
                },
            }
        )
    else:
        write(
            {
                "jsonrpc": "2.0",
                "id": message["id"],
                "error": {"code": -32601, "message": f"unknown method {method}"},
            }
        )
