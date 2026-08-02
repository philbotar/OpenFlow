#!/usr/bin/env python3
"""Deterministic local MCP stdio fixture. No network, secrets, or package installs."""

import json
import sys


def result(request_id, payload):
    return {"jsonrpc": "2.0", "id": request_id, "result": payload}


def handle(message):
    request_id = message.get("id")
    method = message.get("method")
    params = message.get("params") or {}
    if request_id is None:
        return None
    if method == "initialize":
        return result(
            request_id,
            {
                "protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {
                    "tools": {},
                    "resources": {"subscribe": False, "listChanged": False},
                    "prompts": {"listChanged": False},
                },
                "serverInfo": {"name": "openflow-live-fixture", "version": "1.0.0"},
            },
        )
    if method == "tools/list":
        return result(
            request_id,
            {
                "tools": [
                    {
                        "name": "echo",
                        "description": "Echo a message",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"message": {"type": "string"}},
                            "required": ["message"],
                        },
                    }
                ]
            },
        )
    if method == "tools/call":
        message_value = (params.get("arguments") or {}).get("message", "")
        return result(
            request_id,
            {"content": [{"type": "text", "text": f"echo:{message_value}"}], "isError": False},
        )
    if method == "resources/list":
        return result(
            request_id,
            {
                "resources": [
                    {
                        "uri": "fixture://status",
                        "name": "Fixture status",
                        "description": "Deterministic local fixture resource",
                        "mimeType": "text/plain",
                    }
                ]
            },
        )
    if method == "resources/read":
        return result(
            request_id,
            {
                "contents": [
                    {
                        "uri": params.get("uri", "fixture://status"),
                        "mimeType": "text/plain",
                        "text": "fixture-ready",
                    }
                ]
            },
        )
    if method == "prompts/list":
        return result(
            request_id,
            {
                "prompts": [
                    {
                        "name": "fixture_prompt",
                        "description": "Deterministic local fixture prompt",
                        "arguments": [],
                    }
                ]
            },
        )
    if method == "prompts/get":
        return result(
            request_id,
            {
                "description": "Deterministic local fixture prompt",
                "messages": [
                    {"role": "user", "content": {"type": "text", "text": "fixture prompt"}}
                ],
            },
        )
    if method == "ping":
        return result(request_id, {})
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": -32601, "message": "Method not found"},
    }


for line in sys.stdin:
    try:
        response = handle(json.loads(line))
        if response is not None:
            sys.stdout.write(json.dumps(response, separators=(",", ":")) + "\n")
            sys.stdout.flush()
    except Exception as error:  # Fixture must return protocol-safe errors, not crash silently.
        sys.stderr.write(f"fixture error: {type(error).__name__}\n")
        sys.stderr.flush()

