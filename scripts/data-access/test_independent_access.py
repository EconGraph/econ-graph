#!/usr/bin/env python3
"""Offline realm checks and real loopback HTTP proxy boundary tests."""
import base64
import importlib.util
import json
import pathlib
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


proxy = load("proxy", ROOT / "scripts/data-access/dev-data-proxy.py")
realm = load("realm", ROOT / "scripts/keycloak/independent-data-realm.py")


class AccessTests(unittest.TestCase):
    def test_realm_overlay_is_explicit_catalog_only_and_has_no_password_or_service_grant(self):
        original = json.loads((ROOT / "config/keycloak/econ-graph-realm.json").read_text())
        rendered = realm.render(original)
        self.assertNotIn("econ-graph-data", [c["clientId"] for c in original["clients"]])
        client = next(c for c in rendered["clients"] if c["clientId"] == "econ-graph-dev-readonly")
        self.assertFalse(client["fullScopeAllowed"])
        self.assertFalse(client["directAccessGrantsEnabled"])
        self.assertFalse(client["serviceAccountsEnabled"])
        self.assertEqual(client["attributes"]["pkce.code.challenge.method"], "S256")
        self.assertNotIn("roles", client["defaultClientScopes"])
        audiences = [m["config"]["included.client.audience"] for m in client["protocolMappers"] if m["protocolMapper"] == "oidc-audience-mapper"]
        self.assertEqual(audiences, ["econ-graph-data"])
        scope = next(m for m in rendered["clientScopeMappings"]["econ-graph-api"] if m["client"] == client["clientId"])
        self.assertTrue(scope["roles"])
        self.assertTrue(all(r.startswith("catalog.") and r.endswith(":read") for r in scope["roles"]))
        web = next(c for c in rendered["clients"] if c["clientId"] == "econ-graph-web")
        self.assertEqual({m["config"]["included.client.audience"] for m in web["protocolMappers"] if m["protocolMapper"] == "oidc-audience-mapper"}, {"econ-graph-api", "econ-graph-data"})
        with self.assertRaises(ValueError):
            realm.render(rendered)

    def test_data_url_refuses_insecure_remote_urls_and_url_credentials(self):
        for url in ["http://remote.example/graphql", "https://u:p@remote.example/graphql", "https://remote.example/graphql?token=x", "https://remote.example/graphql#x"]:
            with self.assertRaises(ValueError): proxy.data_url(url)
        self.assertEqual(proxy.data_url("https://remote.example/graphql"), "https://remote.example/graphql")
        self.assertEqual(proxy.data_url("http://127.0.0.1:123/graphql"), "http://127.0.0.1:123/graphql")

    def test_loopback_proxy_pins_credentials_paths_and_redirects(self):
        seen = []
        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *_): pass
            def do_POST(self):
                seen.append((self.path, self.headers.get("Authorization"), self.headers.get("Cookie")))
                self.rfile.read(int(self.headers["Content-Length"]))
                if self.path == "/redirect":
                    self.send_response(302)
                    self.send_header("Location", f"http://127.0.0.1:{self.server.server_port}/leak")
                    self.end_headers()
                else:
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(b'{"data":{"ok":true}}')
        upstream = ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
        threading.Thread(target=upstream.serve_forever, daemon=True).start()
        with tempfile.TemporaryDirectory() as directory:
            token_file = pathlib.Path(directory) / "token"
            claims = {"aud":["econ-graph-data"], "azp":"econ-graph-dev-readonly", "roles":["catalog.fred:read"], "exp":int(time.time())+300}
            def token(changes=None):
                payload = dict(claims, **(changes or {}))
                result = "header." + base64.urlsafe_b64encode(json.dumps(payload).encode()).decode().rstrip("=") + ".signature"
                token_file.write_text(result)
                return result
            credential = token()
            def start(path):
                server=ThreadingHTTPServer(("127.0.0.1",0),proxy.handler(f"http://127.0.0.1:{upstream.server_port}/{path}",token_file))
                threading.Thread(target=server.serve_forever,daemon=True).start()
                self.addCleanup(server.server_close)
                self.addCleanup(server.shutdown)
                return server
            server = start("graphql")
            def post(path="/graphql", headers=None):
                req=urllib.request.Request(f"http://127.0.0.1:{server.server_port}{path}",b'{"query":"{__typename}"}',dict({"Content-Type":"application/json", "Authorization":"Bearer caller-token", "Cookie":"session=do-not-forward"},**(headers or {})))
                try:
                    with urllib.request.urlopen(req,timeout=3) as response: return response.status
                except urllib.error.HTTPError as error:
                    with error: return error.code
            self.assertEqual(post(),200)
            self.assertEqual(seen,[("/graphql",f"Bearer {credential}",None)])
            self.assertEqual(post("/mcp"),404)
            self.assertEqual(post(headers={"Host":"rebind.example"}),403)
            self.assertEqual(post(headers={"Content-Type":"text/plain"}),415)
            for change in [{"aud":["econ-graph-data","econ-graph-api"]},{"roles":["admin.crawlers:manage"]},{"exp":1},{"azp":"econ-graph-web"}]:
                token(change)
                self.assertEqual(post(),502)
            self.assertEqual(len(seen),1)
            token()
            server=start("redirect")
            self.assertEqual(post(),502)
            self.assertEqual([path for path,_,_ in seen],["/graphql","/redirect"])
        upstream.shutdown()
        upstream.server_close()


if __name__ == "__main__":
    unittest.main()
