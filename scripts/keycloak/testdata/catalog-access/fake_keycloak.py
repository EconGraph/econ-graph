#!/usr/bin/env python3
"""A fake Keycloak admin API for scripts/keycloak/test-set-catalog-access.sh.

Usage: fake_keycloak.py STATE.json PORT_FILE WRITES_LOG

Serves the requests scripts/keycloak/catalog-access.py makes, from STATE.json, which it reads
on every request and rewrites on every change, so a test can set it up and read it back:

  {"realm": true, "roles": ["catalog.fred:read", ...],       # econ-graph-api client roles
   "realm_roles": ["catalog-public", "admin"],
   "composites": {"catalog-public": [...], "admin": [...]},  # their econ-graph-api roles
   "anonymous": true, "scope": [...],                        # the reader's scope mapping
   "reader_direct": [...],                                   # roles mapped to the reader
   "reader_scopes": {"basic": [...]},                        # its client scopes' mappings,
                                                             # composites expanded
   "scope_realm": [...],                                     # catalog reads realm roles
                                                             # mapped on its scope admit
   "fail_write": N,                                          # optional: the Nth write answers 500
   "ignore_writes_to": "admin"}                              # optional: writes to it are dropped

Only the password `test-admin-password` signs in. Each change is appended to WRITES_LOG as
"METHOD PATH ROLE". It listens on 127.0.0.1 on a free port, written to PORT_FILE.
"""
import json
import os
import re
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, HTTPServer

STATE, PORT_FILE, WRITES = sys.argv[1:4]
PREFIX = "/admin/realms/econ-graph"


def load():
    with open(STATE, encoding="utf-8") as f:
        return json.load(f)


def save(state):
    with open(STATE, "w", encoding="utf-8") as f:
        json.dump(state, f)


def rep(name):
    return {"id": "id-" + name, "name": name, "clientRole": True, "containerId": "api"}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, code, body=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length)

    def handle_any(self, method):
        url = urllib.parse.urlsplit(self.path)
        path = urllib.parse.unquote(url.path)
        query = urllib.parse.parse_qs(url.query)
        if path == "/realms/master/protocol/openid-connect/token" and method == "POST":
            form = urllib.parse.parse_qs(self.body().decode())
            if form.get("password") == ["test-admin-password"] and form.get("username") == ["admin"]:
                return self.reply(200, {"access_token": "fake-token"})
            return self.reply(401, {"error": "invalid_grant"})
        if self.headers.get("Authorization") != "Bearer fake-token":
            return self.reply(401)
        state = load()
        if not path.startswith(PREFIX) or not state["realm"]:
            return self.reply(404)
        path = path[len(PREFIX):]
        if path == "" and method == "GET":
            return self.reply(200, {"realm": "econ-graph"})
        if path == "/clients" and method == "GET":
            client = query.get("clientId", [""])[0]
            ids = {"econ-graph-api": "api", "econgraph-anonymous": "anon" if state["anonymous"] else None}
            found = ids.get(client)
            return self.reply(200, [{"id": found, "clientId": client}] if found else [])
        m = re.fullmatch(r"/clients/api/roles/(.+)", path)
        if m and method == "GET":
            return self.reply(200, rep(m.group(1))) if m.group(1) in state["roles"] else self.reply(404)
        m = re.fullmatch(r"/roles/([^/]+)(/composites(/clients/api)?)?", path)
        if m:
            name = m.group(1)
            if name not in state["realm_roles"]:
                return self.reply(404)
            if not m.group(2):
                return self.reply(200, {"name": name})
            return self.edit(state, method, state["composites"].setdefault(name, []), path)
        if path == "/clients/anon/scope-mappings/clients/api/composite" and method == "GET" and state["anonymous"]:
            return self.reply(200, [rep(n) for n in sorted(set(state["scope"]) | set(state.get("scope_realm", [])))])
        if path == "/clients/anon/scope-mappings/clients/api" and state["anonymous"]:
            return self.edit(state, method, state["scope"], path)
        if path == "/clients/anon/default-client-scopes" and method == "GET" and state["anonymous"]:
            return self.reply(200, [{"id": n, "name": n} for n in sorted(state.get("reader_scopes", {}))])
        if path == "/clients/anon/optional-client-scopes" and method == "GET" and state["anonymous"]:
            return self.reply(200, [])
        m = re.fullmatch(r"/client-scopes/([^/]+)/scope-mappings/clients/api/composite", path)
        if m and method == "GET" and m.group(1) in state.get("reader_scopes", {}):
            return self.reply(200, [rep(n) for n in state["reader_scopes"][m.group(1)]])
        if path == "/clients/anon/service-account-user" and state["anonymous"]:
            return self.reply(200, {"id": "reader"})
        if path == "/users/reader/role-mappings/clients/api/composite" and method == "GET":
            granted = set(state.get("reader_direct", []))
            if "catalog-public" in state.get("reader_realm", ["catalog-public"]):
                granted |= set(state["composites"].get("catalog-public", []))
            return self.reply(200, [rep(n) for n in sorted(granted)])
        return self.reply(404)

    def edit(self, state, method, edges, path):
        if method == "GET":
            return self.reply(200, [rep(n) for n in edges])
        state["writes"] = state.get("writes", 0) + 1
        if state["writes"] == state.get("fail_write"):
            save(state)
            return self.reply(500, {"error": "injected"})
        names = [r["name"] for r in json.loads(self.body())]
        if state.get("ignore_writes_to") and path.startswith("/roles/%s/" % state["ignore_writes_to"]):
            return self.reply(204)
        for name in names:
            if method == "POST" and name not in edges:
                edges.append(name)
            elif method == "DELETE" and name in edges:
                edges.remove(name)
            with open(WRITES, "a", encoding="utf-8") as log:
                log.write("%s %s %s\n" % (method, path, name))
        save(state)
        return self.reply(204)

    def do_GET(self):
        self.handle_any("GET")

    def do_POST(self):
        self.handle_any("POST")

    def do_DELETE(self):
        self.handle_any("DELETE")


server = HTTPServer(("127.0.0.1", 0), Handler)
with open(PORT_FILE + ".tmp", "w", encoding="utf-8") as f:
    f.write(str(server.server_address[1]))
os.rename(PORT_FILE + ".tmp", PORT_FILE)
server.serve_forever()
