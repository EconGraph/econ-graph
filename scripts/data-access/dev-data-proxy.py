#!/usr/bin/env python3
"""Loopback-only fixed-target proxy for a developer's data-only access token."""
import argparse
import base64
import time
import http.server
import ipaddress
import json
import pathlib
import urllib.error
import urllib.parse
import urllib.request

LIMIT = 1024 * 1024


def data_url(value):
    url = urllib.parse.urlsplit(value)
    local = url.hostname == "localhost"
    try:
        local = local or ipaddress.ip_address(url.hostname).is_loopback
    except (ValueError, TypeError):
        pass
    if (url.scheme != "https" and not (url.scheme == "http" and local)) or not url.hostname:
        raise ValueError("data URL requires HTTPS, except loopback test/development services")
    if url.username or url.password or url.query or url.fragment:
        raise ValueError("data URL must not contain credentials, query or fragment")
    return value


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def handler(target, token_file):
    class DataProxy(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass  # Never log caller credentials, query text, or upstream response bodies.

        def reply(self, status, body):
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            if self.headers.get("Host") not in {f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"}:
                self.reply(403, b'{"errors":[{"message":"Invalid local proxy host"}]}')
                return
            if self.headers.get("Content-Type", "").split(";")[0].strip() != "application/json":
                self.reply(415, b'{"errors":[{"message":"JSON required"}]}')
                return
            if self.path not in {"/graphql", "/data/graphql"}:
                self.reply(404, b'{"errors":[{"message":"Not found"}]}')
                return
            try:
                size = int(self.headers.get("Content-Length", "0"))
                if self.headers.get("Transfer-Encoding") or not 0 < size <= LIMIT:
                    self.reply(413, b'{"errors":[{"message":"Invalid body size"}]}')
                    return
                body = self.rfile.read(size)
                if len(body) != size:
                    raise ValueError("incomplete request")
                json.loads(body)
                token = pathlib.Path(token_file).read_text(encoding="utf-8").strip()
                if not token or any(c.isspace() for c in token):
                    raise ValueError("invalid token file")
                # Defense in depth: this local decode only restricts forwarded credentials;
                # the remote service still verifies signature, issuer, audience, expiry and roles.
                payload = token.split(".")[1]
                claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
                audiences = claims.get("aud", [])
                if isinstance(audiences, str): audiences = [audiences]
                if (claims.get("azp") != "econ-graph-dev-readonly" or
                    "econ-graph-data" not in audiences or set(audiences) - {"econ-graph-data", "account"} or
                    not claims.get("roles") or any(not isinstance(r, str) or not r.startswith("catalog.") or not r.endswith(":read") for r in claims["roles"]) or
                    claims.get("exp", 0) <= time.time()):
                    raise ValueError("token is not a current read-only client token")
                request = urllib.request.Request(target, body, {
                    "Content-Type": "application/json", "Accept": "application/json",
                    "Authorization": f"Bearer {token}",
                }, method="POST")
                # A redirect must never carry the developer's bearer token elsewhere.
                opener = urllib.request.build_opener(NoRedirect)
                try:
                    response = opener.open(request, timeout=15)
                except urllib.error.HTTPError as error:
                    response = error
                with response:
                    result = response.read(LIMIT + 1)
                    if len(result) > LIMIT or 300 <= response.code < 400:
                        raise ValueError("invalid upstream response")
                    json.loads(result)
                    self.reply(response.code, result)
            except (ValueError, OSError, IndexError, TypeError, urllib.error.URLError):
                self.reply(502, b'{"errors":[{"message":"Data service unavailable; renew the read-only token if expired"}]}')

    return DataProxy


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-url", required=True, type=data_url)
    parser.add_argument("--token-file", required=True)
    parser.add_argument("--port", type=int, default=9877)
    args = parser.parse_args()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler(args.data_url, args.token_file))
    print(f"http://127.0.0.1:{server.server_port}", flush=True)
    server.serve_forever()
