#!/usr/bin/env python3
"""Sign in through PKCE and save a short-lived, data-only development access token."""
import argparse
import base64
import hashlib
import http.server
import json
import os
import pathlib
import secrets
import time
import urllib.parse
import urllib.request
import webbrowser

CLIENT = "econ-graph-dev-readonly"
REDIRECT = "http://127.0.0.1:8765/callback"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--issuer", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--no-browser", action="store_true")
    args = parser.parse_args()
    issuer = urllib.parse.urlsplit(args.issuer)
    if (issuer.scheme != "https" and not (issuer.scheme == "http" and issuer.hostname in {"localhost", "127.0.0.1"})) or not issuer.hostname or issuer.username or issuer.password or issuer.query or issuer.fragment:
        parser.error("issuer requires HTTPS (loopback HTTP is allowed for local Keycloak)")
    state, verifier = secrets.token_urlsafe(32), secrets.token_urlsafe(64)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    result = {}

    class Callback(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            if self.headers.get("Host") != "127.0.0.1:8765":
                self.send_error(403, "Invalid callback host")
                return
            query = urllib.parse.urlsplit(self.path)
            values = urllib.parse.parse_qs(query.query)
            if query.path != "/callback" or values.get("state") != [state] or len(values.get("code", [])) != 1:
                self.send_error(400, "Invalid sign-in callback")
                return
            result["code"] = values["code"][0]
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(b"Signed in. You may close this window.")

    url = args.issuer.rstrip("/") + "/protocol/openid-connect/auth?" + urllib.parse.urlencode({
        "client_id": CLIENT, "redirect_uri": REDIRECT, "response_type": "code", "scope": "openid",
        "state": state, "code_challenge": challenge, "code_challenge_method": "S256",
    })
    with http.server.HTTPServer(("127.0.0.1", 8765), Callback) as server:
        server.timeout = 1
        print("Open this sign-in URL:", url, flush=True)
        if not args.no_browser:
            webbrowser.open(url)
        deadline = time.monotonic() + 180
        while not result and time.monotonic() < deadline:
            server.handle_request()
    if not result:
        parser.exit(1, "Sign-in timed out\n")
    request = urllib.request.Request(args.issuer.rstrip("/") + "/protocol/openid-connect/token",
        urllib.parse.urlencode({"grant_type": "authorization_code", "client_id": CLIENT,
            "code": result["code"], "redirect_uri": REDIRECT, "code_verifier": verifier}).encode())
    # The issuer is explicit, and redirects may not forward the authorization code.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args):
            return None
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=15) as response:
            token = json.load(response)["access_token"]
        # The data service cryptographically validates this token; never use this local decode
        # as authentication. Check that the requested client was configured read-only.
        payload = token.split(".")[1]
        claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
        audiences = claims.get("aud", [])
        if isinstance(audiences, str):
            audiences = [audiences]
        if "econ-graph-data" not in audiences or set(audiences) - {"econ-graph-data", "account"} or claims.get("azp") != CLIENT:
            raise ValueError("client did not receive a data-only token")
        if not claims.get("roles") or any(not isinstance(r, str) or not r.startswith("catalog.") or not r.endswith(":read") for r in claims["roles"]):
            raise ValueError("client did not receive only catalog read roles")
        # Refuse replacement and symlink traversal; renewal is an explicit remove/re-auth step.
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as output:
            output.write(token + "\n")
    except (OSError, ValueError, KeyError, IndexError) as error:
        parser.exit(1, f"Could not save read-only token: {type(error).__name__}\n")
    print(f"Saved short-lived read-only access token to {args.output}; renew when it expires.")


if __name__ == "__main__":
    main()
