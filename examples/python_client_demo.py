#!/usr/bin/env python3
"""
ide-shell-protocol Python client demo.

演示 AI agent 用 ide-shell-protocol JSON-RPC over stdio / HTTP / WS
控制 IDE buffer (per Cursor / VSCode 的 AI Edit 协议).

3 protocols supported by ide-shell-protocol-server:
- stdio: 单连接, 命令行 AI agent 集成
- HTTP: 多 client REST
- WS: 双向 push events

Examples:
  python python_client_demo.py stdio /path/to/ide-shell-protocol-server
  python python_client_demo.py http  http://127.0.0.1:7777
"""
import sys
import json
import argparse
import urllib.request
import urllib.error
import socket
import base64
import os
import struct
import hashlib
import time



class StdioSession:
    """Long-lived stdio session — one server process, many requests."""
    def __init__(self, server_path, mode="stdio"):
        import subprocess, threading, queue
        self.proc = subprocess.Popen(
            [server_path, mode],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        self._q = queue.Queue()
        threading.Thread(target=self._read_loop, daemon=True).start()

    def _read_loop(self):
        for line in self.proc.stdout:
            line = line.decode("utf-8", errors="replace").strip()
            if not line: continue
            try: self._q.put(json.loads(line))
            except Exception: self._q.put({"error": "parse failed", "raw": line})

    def request(self, req, timeout=5.0):
        self.proc.stdin.write((json.dumps(req) + "\n").encode("utf-8"))
        self.proc.stdin.flush()
        try: return self._q.get(timeout=timeout)
        except Exception: return {"error": "queue timeout"}

    def close(self):
        try: self.proc.stdin.close(); self.proc.wait(timeout=2)
        except Exception: self.proc.kill()


def send_request_stdio(server_path: str, req: dict, timeout: float = 5.0, mode: str = "stdio") -> dict:
    """Send single JSON request via stdin, read single response. Keep stdin open until response."""
    import subprocess
    import threading
    payload = json.dumps(req) + "\n"
    proc = subprocess.Popen(
        [server_path, mode],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    def writer():
        try:
            proc.stdin.write(payload.encode("utf-8"))
            proc.stdin.flush()
        except Exception:
            pass
    wt = threading.Thread(target=writer, daemon=True)
    wt.start()
    out_lines = []
    try:
        for _ in range(100):
            line = proc.stdout.readline()
            if not line:
                break
            out_lines.append(line.decode("utf-8", errors="replace").rstrip())
            if out_lines[-1].startswith("{") and out_lines[-1].endswith("}"):
                break
    finally:
        proc.stdin.close()
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            proc.kill()
    if not out_lines:
        return {"error": "empty response"}
    return json.loads(out_lines[-1])


def send_request_http(base_url: str, req: dict, timeout: float = 5.0) -> dict:
    """POST JSON-RPC request to HTTP server."""
    url = base_url.rstrip("/") + "/rpc"
    data = json.dumps(req).encode("utf-8")
    req_obj = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req_obj, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.URLError as e:
        return {"error": str(e)}


def send_request_ws(url: str, req: dict, timeout: float = 5.0) -> dict:
    """Send via WebSocket — minimal client (no ws library)."""
    import socket
    import base64
    import os
    # ws:// → socket connect
    if not url.startswith("ws://"):
        return {"error": "ws URL required"}
    url = url[5:]
    host, _, rest = url.partition("/")
    host, _, port = host.partition(":")
    port = int(port) if port else 80
    path = "/" + rest
    sock = socket.create_connection((host, port), timeout=timeout)
    key = base64.b64encode(os.urandom(16)).decode()
    handshake = (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        f"Upgrade: websocket\r\n"
        f"Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        f"Sec-WebSocket-Version: 13\r\n\r\n"
    ).encode()
    sock.sendall(handshake)
    # Read handshake response
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = sock.recv(4096)
        if not chunk:
            break
        buf += chunk
    # Send WS frame (client → server, opcode 1 = text)
    payload = json.dumps(req).encode("utf-8")
    mask = os.urandom(4)
    masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    header = bytes([0x81])  # FIN + text
    if len(payload) < 126:
        header += bytes([0x80 | len(payload)])
    elif len(payload) < 65536:
        header += bytes([0x80 | 126]) + struct.pack(">H", len(payload))
    else:
        header += bytes([0x80 | 127]) + struct.pack(">Q", len(payload))
    sock.sendall(header + mask + masked)
    # Read response frame (server → client, no mask)
    sock.settimeout(timeout)
    resp_buf = b""
    while True:
        chunk = sock.recv(65536)
        if not chunk:
            break
        resp_buf += chunk
        if len(resp_buf) >= 2:
            op = resp_buf[0] & 0x0F
            plen = resp_buf[1] & 0x7F
            if plen < 126 and len(resp_buf) >= 2 + plen:
                break
    payload_len = resp_buf[1] & 0x7F
    payload_start = 2
    payload_data = resp_buf[payload_start:payload_start + payload_len]
    sock.close()
    return json.loads(payload_data.decode("utf-8"))


def demo_stdio(server_path: str, mode: str = "stdio"):
    print(f"=== stdio (持久 session) → {server_path} ===\n")
    s = StdioSession(server_path, mode)
    try:
        r = s.request({"jsonrpc": "2.0", "id": 1, "method": "kernel_info", "params": {}})
        api_list = r.get("result", {}).get("api", [])
        print(f"kernel_info: api={len(api_list)} 个")

        r = s.request({"jsonrpc": "2.0", "id": 2, "method": "open_buffer", "params": {"path": None}})
        buf_id = r.get("result", {}).get("id", 0)
        print(f"open_buffer → id={buf_id}")

        r = s.request({"jsonrpc": "2.0", "id": 3, "method": "insert_at_cursor",
                       "params": {"id": buf_id, "text": "fn greet() { println!(\"Hello AI\"); }\n"}})
        print(f"insert_at_cursor: {r.get('result', {}).get('text', r.get('error', {}))}")

        r = s.request({"jsonrpc": "2.0", "id": 4, "method": "get_buffer", "params": {"id": buf_id}})
        text = r.get("result", {}).get("text", "")
        print(f"get_buffer: {text}")

        r = s.request({"jsonrpc": "2.0", "id": 5, "method": "edit_via_provider",
                       "params": {"id": buf_id, "range": {"start_row": 0, "start_col": 3, "end_row": 0, "end_col": 8}, "instruction": "rename to hello_world"}})
        print(f"\nedit_via_provider: {r.get('result', {}).get('text', r.get('error', {}))}")

        r = s.request({"jsonrpc": "2.0", "id": 6, "method": "chat",
                       "params": {"messages": [{"role": "user", "content": "分析上面的 Rust 代码"}], "tools": []}})
        print(f"\nchat: {r.get('result', {}).get('message', r.get('error', {}))}")
    finally:
        s.close()


def demo_http(base_url: str):
    print(f"=== HTTP 模式 → {base_url} ===\n")
    # Same calls via HTTP
    r = send_request_http(base_url, {
        "jsonrpc": "2.0", "id": 1, "method": "kernel_info", "params": {}
    })
    print(f"kernel_info: {json.dumps(r, ensure_ascii=False)[:300]}")


def demo_ws(url: str):
    print(f"=== WebSocket 模式 → {url} ===\n")
    r = send_request_ws(url, {
        "jsonrpc": "2.0", "id": 1, "method": "kernel_info", "params": {}
    })
    print(f"kernel_info: {json.dumps(r, ensure_ascii=False)[:300]}")


def main():
    parser = argparse.ArgumentParser(description="ide-shell-protocol Python client demo")
    sub = parser.add_subparsers(dest="mode", required=True)

    p1 = sub.add_parser("stdio", help="stdio mode (subprocess)")
    p1.add_argument("server", help="path to ide-shell-protocol-server binary")

    p2 = sub.add_parser("http", help="HTTP mode")
    p2.add_argument("base_url", help="e.g. http://127.0.0.1:7777")

    p3 = sub.add_parser("ws", help="WebSocket mode")
    p3.add_argument("url", help="e.g. ws://127.0.0.1:7777/rpcws")

    args = parser.parse_args()

    if args.mode == "stdio":
        demo_stdio(args.server)
    elif args.mode == "http":
        demo_http(args.base_url)
    elif args.mode == "ws":
        demo_ws(args.url)


if __name__ == "__main__":
    main()