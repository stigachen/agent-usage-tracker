"""Native CLI experiment only: synthetic credentials, local OIDC, no chat/session requests."""
import argparse
import hashlib
import json
import os
import platform
import queue
import signal
import subprocess
import tempfile
import threading
import time
import urllib.request
from collections import Counter
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

VERSION = "1.0.46"


def download(root):
    system = "windows" if os.name == "nt" else "macos"
    arch = "aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64"
    name = f"grok-{VERSION}-{system}-{arch}" + (".exe" if os.name == "nt" else "")
    destination = root / ("grok.exe" if os.name == "nt" else "grok")
    for base in ("https://x.ai/cli", "https://storage.googleapis.com/grok-build-public-artifacts/cli"):
        try:
            with urllib.request.urlopen(f"{base}/{name}", timeout=120) as response:
                with destination.open("wb") as output:
                    while chunk := response.read(1024 * 1024):
                        output.write(chunk)
            destination.chmod(0o700)
            return destination
        except Exception:
            if base.endswith("artifacts/cli"):
                raise


def iso(delta):
    return (datetime.now(timezone.utc) + delta).isoformat().replace("+00:00", "Z")


class Mock(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, mode):
        self.mode = mode
        self.events = []
        self.token_count = 0
        self.lock = threading.Lock()
        super().__init__(("127.0.0.1", 0), Handler)
        self.issuer = f"http://127.0.0.1:{self.server_port}"

    def record(self, method, path, **extra):
        with self.lock:
            if path == "/token" and method == "POST":
                self.token_count += 1
                extra["attempt"] = self.token_count
            self.events.append({"method": method, "path": path, **extra})
            return self.token_count


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, status, data):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_CONNECT(self):
        self.server.record("CONNECT", self.path)
        self.reply(502, {"error": "external_network_blocked_by_probe"})

    def do_GET(self):
        path = urlparse(self.path).path
        self.server.record("GET", path)
        issuer = self.server.issuer
        if path == "/.well-known/openid-configuration":
            self.reply(200, {"issuer": issuer, "authorization_endpoint": issuer + "/authorize",
                            "token_endpoint": issuer + "/token", "jwks_uri": issuer + "/jwks",
                            "userinfo_endpoint": issuer + "/userinfo"})
        elif path.endswith("/models"):
            self.reply(200, {"data": []})
        elif path.endswith("/user") or path == "/userinfo":
            self.reply(200, {"userId": "probe-a", "sub": "probe-a", "email": "a@example.invalid", "principalType": "User"})
        elif path.endswith("/settings"):
            self.reply(200, {"subscription_tier_display": "SuperGrok", "telemetry_enabled": False, "disable_codebase_upload": True})
        elif path.endswith("/billing"):
            self.reply(200, {"config": {"creditUsagePercent": 12.5}})
        else:
            self.reply(404, {"error": "probe_route_not_found"})

    def do_POST(self):
        path = urlparse(self.path).path
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        form = parse_qs(body.decode(errors="replace"))
        attempt = self.server.record("POST", path, grant_type=form.get("grant_type", [""])[0])
        mode = self.server.mode
        if path != "/token":
            self.reply(404, {"error": "probe_route_not_found"})
        elif mode == "invalid_grant" or (mode == "slow_strict" and attempt > 1):
            self.reply(400, {"error": "invalid_grant", "error_description": "synthetic refresh token rejected or already used"})
        elif mode == "transient":
            self.reply(503, {"error": "temporarily_unavailable"})
        else:
            if mode in ("slow_strict", "eof", "switch_account"):
                time.sleep(8)
            elif mode == "concurrent":
                time.sleep(1)
            self.reply(200, {"access_token": "probe-new-access", "refresh_token": "probe-new-refresh",
                             "expires_in": 3600, "token_type": "Bearer"})


def read_auth(path, scope):
    try:
        entries = json.loads(path.read_text())
        return entries, entries.get(scope, {})
    except (OSError, json.JSONDecodeError):
        return {}, {}


def environment(directory, issuer):
    env = {k: v for k, v in os.environ.items()
           if not k.upper().startswith(("GROK_", "XAI_", "OPENAI_", "ANTHROPIC_", "GH_", "GITHUB_"))}
    env.update({"GROK_HOME": str(directory), "GROK_LOCAL_AUTH": "1", "GROK_OAUTH2_ISSUER": issuer,
                "GROK_OAUTH2_CLIENT_ID": "probe-client", "GROK_CLI_CHAT_PROXY_BASE_URL": issuer + "/v1",
                "GROK_XAI_API_BASE_URL": issuer + "/api", "GROK_AUTH_EARLY_INVALIDATION_SECS": "300",
                "RUST_LOG": "warn", "OTEL_SDK_DISABLED": "true"})
    for key in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"):
        env[key] = issuer
    for key in ("NO_PROXY", "no_proxy"):
        env[key] = "127.0.0.1,localhost"
    return env


def stop(process):
    if not process.stdin.closed:
        process.stdin.close()
    try:
        process.wait(timeout=3)
        return False
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, timeout=10)
        else:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            if os.name != "nt":
                os.killpg(process.pid, signal.SIGKILL)
            process.kill()
            process.wait(timeout=3)
        return True


def run_case(exe, root, method, mode):
    case = root / f"{method}-{mode}"
    directory = case / "isolated grok 用户"
    cwd = case / "empty workspace"
    directory.mkdir(parents=True)
    cwd.mkdir()
    server = Mock(mode)
    threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.1}, daemon=True).start()
    issuer = server.issuer
    scope = issuer + "::probe-client"
    auth = {"key": "probe-old-access", "refresh_token": "probe-old-refresh", "auth_mode": "oidc",
            "oidc_issuer": issuer, "oidc_client_id": "probe-client", "user_id": "probe-a",
            "email": "a@example.invalid", "principal_type": "User", "principal_id": "probe-a",
            "create_time": iso(timedelta(hours=-2)), "expires_at": iso(timedelta(hours=-1)),
            "coding_data_retention_opt_out": True}
    if mode == "valid":
        auth["expires_at"] = iso(timedelta(hours=1))
    elif mode == "near_expiry":
        auth["expires_at"] = iso(timedelta(seconds=120))
    elif mode == "no_refresh":
        del auth["refresh_token"]
    path = directory / "auth.json"
    if mode != "missing":
        path.write_text(json.dumps({scope: auth}))
        path.chmod(0o600)
    (directory / "config.toml").write_text('[cli]\nuse_leader=false\nauto_update=false\n[grok_com_config.oauth2]\nissuer="' + issuer + '"\nclient_id="probe-client"\n')
    env = environment(directory, issuer)
    cli_command = method in ("version", "models", "inspect")
    args = [str(exe), "--no-auto-update", method] if cli_command else [
        str(exe), "--no-auto-update", "agent", "--no-leader", "--cli-chat-proxy-base-url", issuer + "/v1",
        "--xai-api-base-url", issuer + "/api", "stdio"]
    processes, errors, readers, replies, rpc_sent, rpc_skipped = [], [], [], [], set(), set()
    lines = queue.Queue()
    start = time.monotonic()
    for index in range(2 if mode == "concurrent" else 1):
        error = (case / f"stderr-{index}.txt").open("wb")
        errors.append(error)
        process = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=error,
                                   cwd=cwd, env=env, start_new_session=os.name != "nt",
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        processes.append(process)
        def reader(proc=process, idx=index):
            for line in proc.stdout:
                try:
                    message = json.loads(line)
                    if isinstance(message, dict):
                        lines.put((idx, message))
                except (ValueError, UnicodeDecodeError):
                    pass
        thread = threading.Thread(target=reader, daemon=True)
        thread.start()
        readers.append(thread)
        if not cli_command:
            process.stdin.write((json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                                            "params": {"protocolVersion": 1, "clientCapabilities": {}}}) + "\n").encode())
            process.stdin.flush()
    eof_sent = False
    switched = False
    budget = 14 if mode in ("slow_strict", "eof", "switch_account") else 6
    while time.monotonic() - start < budget:
        while not lines.empty():
            index, response = lines.get()
            replies.append((index, response))
            if response.get("id") == 1 and "result" in response and index not in rpc_sent and method in ("authenticate", "bearer"):
                advertised = {item["id"] for item in response["result"].get("authMethods", [])}
                if method == "authenticate" and "cached_token" not in advertised:
                    # Follow the official ACP contract: never fall through into an interactive login.
                    rpc_skipped.add(index)
                    continue
                params = {"methodId": "cached_token", "_meta": {"headless": True}} if method == "authenticate" else {}
                request_method = "authenticate" if method == "authenticate" else "_x.ai/auth/getBearerToken"
                process = processes[index]
                if process.poll() is None:
                    process.stdin.write((json.dumps({"jsonrpc": "2.0", "id": 2, "method": request_method, "params": params}) + "\n").encode())
                    process.stdin.flush()
                rpc_sent.add(index)
        if server.token_count and mode == "eof" and not eof_sent:
            for process in processes:
                process.stdin.close()
            eof_sent = True
        if server.token_count and mode == "switch_account" and not switched:
            other = {**auth, "key": "probe-other-access", "refresh_token": "probe-other-refresh",
                     "user_id": "probe-b", "email": "b@example.invalid", "principal_id": "probe-b",
                     "expires_at": iso(timedelta(hours=1))}
            replacement = path.with_suffix(".replacement")
            replacement.write_text(json.dumps({scope: other}))
            replacement.replace(path)
            switched = True
        entries, stored = read_auth(path, scope)
        completed = len({index for index, reply in replies if reply.get("id") == (2 if method in ("authenticate", "bearer") else 1)} | rpc_skipped) == len(processes)
        if all(process.poll() is not None for process in processes):
            break
        if completed and time.monotonic() - start > 1:
            if stored.get("key") == "probe-new-access" or mode in ("valid", "missing", "no_refresh", "invalid_grant"):
                break
            if mode == "slow_strict" and server.token_count > 1 and not entries:
                break
        time.sleep(0.025)
    forced = sum(stop(process) for process in processes)
    for thread in readers:
        thread.join(timeout=1)
    for error in errors:
        error.close()
    server.shutdown()
    server.server_close()
    entries, stored = read_auth(path, scope)
    events = server.events.copy()
    result = {"method": method, "mode": mode, "seconds": round(time.monotonic() - start, 3),
              "exit_codes": [process.returncode for process in processes], "forced_terminations": forced,
              "token_requests": server.token_count, "access_rotated": stored.get("key") == "probe-new-access",
              "refresh_rotated": stored.get("refresh_token") == "probe-new-refresh", "entry_count": len(entries),
              "other_account_preserved": stored.get("key") == "probe-other-access" and stored.get("user_id") == "probe-b",
              "request_counts": dict(Counter(event["method"] + " " + event["path"] for event in events)),
              "authenticate_skipped_not_advertised": sorted(rpc_skipped),
              "rpc": [{"process": index, "id": reply.get("id"), "error": reply.get("error"),
                       "auth_methods": [item["id"] for item in reply.get("result", {}).get("authMethods", [])] if isinstance(reply.get("result"), dict) else [],
                       "result_keys": list(reply.get("result", {})) if isinstance(reply.get("result"), dict) else []}
                      for index, reply in replies if "id" in reply],
              "session_files": [str(item.relative_to(directory)) for item in (directory / "sessions").rglob("*") if item.is_file()]}
    result["chat_requests"] = [event for event in events if event["method"] == "POST" and any(
        part in event["path"] for part in ("/chat", "/responses", "/completions", "/messages"))]
    print(json.dumps(result, ensure_ascii=False), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--quick", action="store_true")
    parser.add_argument("--method", choices=("initialize", "authenticate", "bearer", "version", "models", "inspect"))
    options = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix="grok-renewal-probe-"))
    exe = options.exe.resolve() if options.exe else download(root)
    version_dir = root / "version"
    version_dir.mkdir()
    version = subprocess.run([str(exe), "--no-auto-update", "--version"], env=environment(version_dir, "http://127.0.0.1:9"),
                             cwd=version_dir, capture_output=True, text=True, timeout=10)
    metadata = {"platform": platform.system(), "machine": platform.machine(), "version": version.stdout.strip(),
                "binary_sha256": hashlib.sha256(exe.read_bytes()).hexdigest(), "probe_root": str(root)}
    print(json.dumps(metadata), flush=True)
    cases = [(method, mode) for method in ("initialize", "authenticate", "bearer") for mode in ("success", "slow_strict")]
    if not options.quick:
        cases = [(method, "success") for method in ("version", "models", "inspect")] + cases + [
            ("initialize", mode) for mode in ("valid", "near_expiry", "missing", "no_refresh", "invalid_grant", "transient", "concurrent", "eof", "switch_account")]
    if options.method:
        cases = [(method, mode) for method, mode in cases if method == options.method]
    results = [run_case(exe, root, method, mode) for method, mode in cases]
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps({"metadata": metadata, "results": results}, indent=2), encoding="utf-8")
    assert all(not result["chat_requests"] for result in results), "Unexpected chat request"


if __name__ == "__main__":
    main()
