#!/usr/bin/env python3
"""Opt-in real Keycloak PKCE and signed HTTP acceptance, entirely disposable/local.

Build backend/crawler binaries first. Requires local Docker, Node, the admin frontend
Playwright dependency and Chromium. Never selects a saved Docker context, existing realm,
external database, host cluster, or production endpoint.
"""

import argparse
import base64
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
# Verified against the official 26.8.1 OCI index. Keep aligned with docker-compose.yml.
# https://www.keycloak.org/2026/10/keycloak-2681-released
KC_IMAGE = "quay.io/keycloak/keycloak:26.8.1@sha256:a78d6b79572168cb25886b80a8b5b8d4ed4390743e3eecb810538683e209dd6b"
OWNER_LABEL = "econgraph.independent-access.owner"


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def local_opener(*handlers):
    return urllib.request.build_opener(urllib.request.ProxyHandler({}), *handlers)


def http(url, *, data=None, headers=None, opener=None):
    target = urllib.parse.urlsplit(url)
    assert (
        target.scheme == "http"
        and target.hostname == "127.0.0.1"
        and not target.username
    ), "acceptance HTTP is restricted to owned loopback endpoints"
    request = urllib.request.Request(url, data=data, headers=headers or {})
    try:
        response = (opener or local_opener(NoRedirect)).open(request, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return response.code, response.headers, response.read(2 * 1024 * 1024)


def form_post(url, values, *, opener=None):
    status, headers, body = http(
        url, data=urllib.parse.urlencode(values).encode(), opener=opener
    )
    return status, headers, body


def json_body(body):
    return json.loads(body.decode())


def claims(token):
    payload = token.split(".")[1]
    return json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))


def authorization_code(
    issuer, client, redirect, username, password, playwright_module, work
):
    browser_temp = work / "browser-temporary"
    browser_temp.mkdir(mode=0o700, exist_ok=True)
    browser_env = os.environ.copy()
    browser_env["TMPDIR"] = str(browser_temp)
    response = subprocess.check_output(
        ["node", str(ROOT / "scripts/data-access/keycloak-pkce-login.cjs")],
        input=json.dumps(
            {
                "issuer": issuer,
                "client": client,
                "redirect": redirect,
                "username": username,
                "password": password,
                "playwrightModule": str(playwright_module),
            }
        ).encode(),
        env=browser_env,
        timeout=90,
    )
    result = json.loads(response)
    return result["code"], result["verifier"]


def exchange(issuer, client, redirect, code, verifier):
    status, _, body = form_post(
        issuer + "/protocol/openid-connect/token",
        {
            "grant_type": "authorization_code",
            "client_id": client,
            "redirect_uri": redirect,
            "code": code,
            "code_verifier": verifier,
        },
        opener=local_opener(NoRedirect),
    )
    return status, json_body(body)


class Lab:
    def __init__(self):
        self.owner = "econgraph-readonly-" + uuid.uuid4().hex[:12]
        self.containers = []
        self.network = None
        self.processes = []
        self.files = []
        self.env = os.environ.copy()
        for name in [
            "DOCKER_HOST",
            "DOCKER_CONTEXT",
            "DOCKER_TLS",
            "DOCKER_TLS_VERIFY",
            "DOCKER_CERT_PATH",
        ]:
            self.env.pop(name, None)

    def docker(self, *args):
        return subprocess.check_output(
            ["docker", "--host=unix:///var/run/docker.sock", *args],
            env=self.env,
            text=True,
        ).strip()

    def create_network(self):
        self.network = self.docker(
            "network",
            "create",
            "--opt",
            "com.docker.network.bridge.enable_ip_masquerade=false",
            "--label",
            f"{OWNER_LABEL}={self.owner}",
            self.owner,
        )

    def run(self, name, image, *args, command=()):
        container = self.docker(
            "run",
            "--detach",
            "--name",
            f"{self.owner}-{name}",
            "--label",
            f"{OWNER_LABEL}={self.owner}",
            "--network",
            self.network,
            "--cpus=2",
            "--pids-limit=256",
            "--log-opt=max-size=2m",
            "--log-opt=max-file=1",
            *args,
            image,
            *command,
        )
        self.containers.append(container)
        return container

    def port(self, container, port):
        published = self.docker("port", container, port + "/tcp")
        assert re.fullmatch(
            r"127\.0\.0\.1:\d+", published
        ), "container was not published only on random loopback"
        return int(published.rsplit(":", 1)[1])

    def sql(self, pg, database, query):
        return self.docker(
            "exec",
            pg,
            "psql",
            "-U",
            "postgres",
            "-d",
            database,
            "-v",
            "ON_ERROR_STOP=1",
            "-Atc",
            query,
        )

    def start_backend(
        self, binary, work, app_url, data_url, issuer, plane, data_service=None
    ):
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = os.environ.copy()
        for name in list(env):
            if name.startswith(
                ("OIDC_", "TIMESERIES_", "ICEBERG_", "DATA_GRAPHQL_")
            ) or name in {"DATABASE_URL", "APP_DATABASE_URL", "DATA_DATABASE_URL"}:
                env.pop(name, None)
        env.update(
            {
                "APP_DATABASE_URL": app_url,
                "GRAPHQL_SERVICE_PLANE": plane,
                "OIDC_ISSUER": issuer,
                "OIDC_AUDIENCE": (
                    "econ-graph-data" if plane == "data" else "econ-graph-api"
                ),
                "OIDC_REQUIRED": "true",
                "BACKEND_PORT": str(port),
                "SERVER_HOST": "127.0.0.1",
                "REFERENCE_DATA_DIR": str(ROOT / "backend/crates/econ-graph-core/data"),
                "RUST_LOG": "warn",
                "TOKIO_WORKER_THREADS": "2",
                "NO_PROXY": env.get("NO_PROXY", "") + ",127.0.0.1,localhost",
            }
        )
        if data_url:
            env["DATA_DATABASE_URL"] = data_url
        if data_service:
            env.update(
                {
                    "DATA_GRAPHQL_URL": data_service + "/graphql",
                    "DATA_GRAPHQL_FORWARD_AUTHORIZATION": "true",
                    "DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION": "true",
                }
            )
        log = open(work / f"{plane}-backend.log", "wb")
        self.files.append(log)
        process = subprocess.Popen(
            [str(binary)],
            cwd=ROOT / "backend",
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        self.processes.append(process)
        base = f"http://127.0.0.1:{port}"
        wait_http(base + "/readyz", process=process)
        return base

    def cleanup(self):
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
        for log in self.files:
            log.close()
        for container in reversed(self.containers):
            labels = json.loads(
                self.docker("inspect", "--format", "{{json .Config.Labels}}", container)
            )
            assert (
                labels.get(OWNER_LABEL) == self.owner
            ), "refusing to clean another owner's container"
            self.docker("rm", "--force", container)
        if self.network:
            labels = json.loads(
                self.docker(
                    "network", "inspect", "--format", "{{json .Labels}}", self.network
                )
            )
            assert (
                labels.get(OWNER_LABEL) == self.owner
            ), "refusing to clean another owner's network"
            self.docker("network", "rm", self.network)


def wait_http(url, *, process=None):
    deadline = time.monotonic() + 150
    while time.monotonic() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError("disposable backend exited before readiness")
        try:
            if http(url)[0] == 200:
                return
        except (OSError, urllib.error.URLError):
            pass
        time.sleep(0.5)
    raise TimeoutError("disposable local service did not become ready")


def gql(base, token, query):
    status, _, body = http(
        base + "/graphql",
        data=json.dumps({"query": query}).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {token}",
        },
    )
    return status, json_body(body)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--backend", type=Path, default=ROOT / "backend/target/debug/econ-graph-backend"
    )
    parser.add_argument(
        "--crawler", type=Path, default=ROOT / "backend/target/debug/crawler"
    )
    parser.add_argument("--evidence", type=Path)
    parser.add_argument(
        "--playwright-module",
        type=Path,
        default=ROOT / "admin-frontend/node_modules/@playwright/test",
    )
    args = parser.parse_args()
    assert (
        args.backend.is_file() and args.crawler.is_file()
    ), "build backend and crawler binaries before running this explicit integration test"
    pinned = re.search(
        r"image: (quay.io/keycloak/keycloak:[\d.]+)",
        (ROOT / "docker-compose.yml").read_text(),
    ).group(1)
    assert (
        KC_IMAGE.split("@")[0] == pinned
    ), "update the test's verified immutable image to the repository Keycloak pin"
    lab = Lab()
    with tempfile.TemporaryDirectory(prefix=lab.owner + "-") as directory:
        work = Path(directory)
        try:
            lab.create_network()
            realm_name, username, password = (
                lab.owner,
                "staff-fixture",
                secrets.token_urlsafe(24),
            )
            spec = importlib.util.spec_from_file_location(
                "realm_overlay", ROOT / "scripts/keycloak/independent-data-realm.py"
            )
            overlay = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(overlay)
            realm = overlay.render(
                json.loads((ROOT / "config/keycloak/econ-graph-realm.json").read_text())
            )
            realm["realm"] = realm_name
            realm["users"].append(
                {
                    "id": str(uuid.uuid4()),
                    "username": username,
                    "enabled": True,
                    "email": "fixture@invalid.example",
                    "emailVerified": True,
                    "firstName": "Local",
                    "lastName": "Fixture",
                    "realmRoles": ["admin"],
                    "requiredActions": [],
                    "credentials": [
                        {"type": "password", "value": password, "temporary": False}
                    ],
                }
            )
            template = work / "template.json"
            template.write_text(json.dumps(realm))
            # Never copy an inherited production broker secret into this disposable realm.
            env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8"}
            env.update(
                {
                    "KC_REALM_SSL_REQUIRED": "none",
                    "KC_WEB_BASE_URL": "http://127.0.0.1:8765",
                    "KC_WEB_E2E_BASE_URL": "http://127.0.0.1:8765",
                    "KC_WEB_DIRECT_GRANTS": "false",
                    "KC_GOOGLE_ENABLED": "false",
                    "KC_FACEBOOK_ENABLED": "false",
                    "KC_ANONYMOUS_CLIENT_SECRET": secrets.token_urlsafe(24),
                }
            )
            rendered = subprocess.check_output(
                ["bash", str(ROOT / "scripts/keycloak/render-realm.sh"), str(template)],
                env=env,
            )
            import_file = work / (realm_name + "-realm.json")
            import_file.write_bytes(rendered)
            # The image can augment its immutable application distribution in its own writable
            # container layer; all realm/database data is bounded tmpfs and no host secret mounts.
            kc = lab.run(
                "keycloak",
                KC_IMAGE,
                "--memory=1536m",
                "--publish=127.0.0.1::8080",
                "--tmpfs=/opt/keycloak/data:rw,size=256m,uid=1000,gid=0",
                "--env=KC_DB=dev-file",
                "--env=KC_HOSTNAME_STRICT=false",
                "--env=JAVA_OPTS_KC_HEAP=-Xms64m -Xmx512m",
                "--env=KC_BOOTSTRAP_ADMIN_USERNAME=local-test-owner",
                "--env=KC_BOOTSTRAP_ADMIN_PASSWORD=" + secrets.token_urlsafe(24),
                "--mount",
                f"type=bind,src={import_file},dst=/opt/keycloak/data/import/{import_file.name},readonly",
                "--entrypoint=/opt/keycloak/bin/kc.sh",
                command=("start-dev", "--import-realm", "--cache=local"),
            )
            issuer = f"http://127.0.0.1:{lab.port(kc, '8080')}/realms/{realm_name}"
            wait_http(issuer + "/.well-known/openid-configuration")
            readonly_client, callback = (
                "econ-graph-dev-readonly",
                "http://127.0.0.1:8765/callback",
            )
            code, verifier = authorization_code(
                issuer,
                readonly_client,
                callback,
                username,
                password,
                args.playwright_module.resolve(),
                work,
            )
            status, read_response = exchange(
                issuer, readonly_client, callback, code, verifier
            )
            assert (
                status == 200 and "access_token" in read_response
            ), "real read-only PKCE exchange failed"
            readonly_token = read_response["access_token"]
            readonly = claims(readonly_token)
            audience = (
                readonly["aud"]
                if isinstance(readonly["aud"], list)
                else [readonly["aud"]]
            )
            expected_roles = {
                r["name"]
                for r in realm["roles"]["client"]["econ-graph-api"]
                if r["name"].startswith("catalog.") and r["name"].endswith(":read")
            }
            assert "econ-graph-data" in audience and "econ-graph-api" not in audience
            assert (
                readonly["azp"] == readonly_client
                and set(readonly["roles"]) == expected_roles
            )
            web_redirect = "http://127.0.0.1:8765/auth/callback"
            code, verifier = authorization_code(
                issuer,
                "econ-graph-web",
                web_redirect,
                username,
                password,
                args.playwright_module.resolve(),
                work,
            )
            status, web_response = exchange(
                issuer, "econ-graph-web", web_redirect, code, verifier
            )
            assert status == 200
            web_token = web_response["access_token"]
            web = claims(web_token)
            assert (
                "admin.crawlers:manage" in web["roles"]
                and "annotation:create" in web["roles"]
            ), "fixture user was not actually privileged"
            assert {"econ-graph-api", "econ-graph-data"} <= set(web["aud"])
            for grant in [
                {"grant_type": "password", "username": username, "password": password},
                {"grant_type": "client_credentials"},
            ]:
                status, _, body = form_post(
                    issuer + "/protocol/openid-connect/token",
                    dict(grant, client_id=readonly_client),
                )
                assert status >= 400 and "access_token" not in json_body(
                    body
                ), "read-only client enabled a forbidden grant"
            code, _ = authorization_code(
                issuer,
                readonly_client,
                callback,
                username,
                password,
                args.playwright_module.resolve(),
                work,
            )
            assert (
                exchange(
                    issuer, readonly_client, callback, code, secrets.token_urlsafe(64)
                )[0]
                >= 400
            ), "PKCE verifier was not enforced"
            print(
                "Real Keycloak import, privileged-user role clipping, PKCE and forbidden grants passed",
                flush=True,
            )
            evidence = {
                "keycloak_image": KC_IMAGE,
                "realm": realm_name,
                "readonly_audiences": audience,
                "readonly_roles": sorted(readonly["roles"]),
                "pkce": True,
                "password_and_service_grants_rejected": True,
            }
            pg_password = secrets.token_urlsafe(24)
            pg_images = json.loads(
                lab.docker(
                    "image",
                    "inspect",
                    "postgres:18",
                    "--format",
                    "{{json .RepoDigests}}",
                )
            )
            pg_image = next(
                image for image in pg_images if image.startswith("postgres@sha256:")
            )
            pg = lab.run(
                "postgres",
                pg_image,
                "--memory=512m",
                "--publish=127.0.0.1::5432",
                "--tmpfs=/var/lib/postgresql:rw,size=512m",
                "--env=POSTGRES_PASSWORD=" + pg_password,
            )
            for _ in range(120):
                try:
                    lab.docker("exec", pg, "pg_isready", "--quiet", "-U", "postgres")
                    break
                except subprocess.CalledProcessError:
                    time.sleep(0.5)
            else:
                raise TimeoutError("disposable PostgreSQL startup failed")
            pg_port = lab.port(pg, "5432")
            for database in ["readonly_data", "readonly_auth", "fresh_app"]:
                lab.sql(pg, "postgres", f"CREATE DATABASE {database}")
            url = (
                lambda database: f"postgres://postgres:{pg_password}@127.0.0.1:{pg_port}/{database}"
            )
            migration_env = os.environ.copy()
            for name in list(migration_env):
                if name.startswith(("TIMESERIES_", "ICEBERG_")) or name in {
                    "DATABASE_URL",
                    "APP_DATABASE_URL",
                    "DATA_DATABASE_URL",
                }:
                    migration_env.pop(name, None)
            migration_env["DATA_DATABASE_URL"] = url("readonly_data")
            with open(work / "data-migrate.log", "wb") as log:
                subprocess.run(
                    [str(args.crawler.resolve()), "migrate"],
                    cwd=ROOT / "backend",
                    env=migration_env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    check=True,
                    timeout=60,
                )
            series, dataset = uuid.uuid4(), uuid.uuid4()
            lab.sql(
                pg,
                "readonly_data",
                f"INSERT INTO datasets(id,source_id,code,name) SELECT '{dataset}',id,'readonly-fixture','Local fixture' FROM data_sources WHERE name='Federal Reserve Economic Data (FRED)'; INSERT INTO economic_series(id,source_id,dataset_id,external_id,title,frequency,end_date) SELECT '{series}',source_id,id,'PKCE-FIXTURE','Read-only fixture','Monthly','2024-01-01' FROM datasets WHERE id='{dataset}'; INSERT INTO data_points(series_id,date,revision_date,value) VALUES('{series}','2024-01-01','2024-01-15',12.5)",
            )
            data_service = lab.start_backend(
                args.backend.resolve(),
                work,
                url("readonly_auth"),
                url("readonly_data"),
                issuer,
                "data",
            )
            app_service = lab.start_backend(
                args.backend.resolve(),
                work,
                url("fresh_app"),
                None,
                issuer,
                "app",
                data_service,
            )
            query = f'{{seriesData(seriesId:"{series}"){{totalCount nodes{{value}}}}}}'
            status, answer = gql(data_service, readonly_token, query)
            assert status == 200 and answer.get("data", {}).get("seriesData") == {
                "totalCount": 1,
                "nodes": [{"value": "12.5"}],
            }, "real Keycloak token did not authorize the actual data backend"
            assert (
                gql(app_service, readonly_token, "{__typename}")[0] == 401
            ), "data-only token gained app API access"
            mutation = f'mutation{{createAnnotation(input:{{seriesId:"{series}",annotationDate:"2024-01-01",annotationValue:"12.5",title:"Local note",content:"Fresh app fixture",annotationType:"note",isPublic:true}}){{id}}}}'
            for token in [readonly_token, web_token]:
                for operation in [
                    mutation,
                    'mutation{triggerCrawl(input:{source:"FRED"}){enqueuedCount}}',
                    "{crawlerJobs{id}}",
                ]:
                    status, denied = gql(data_service, token, operation)
                    assert (
                        status == 403
                        and denied["errors"][0]["extensions"]["code"]
                        == "DATA_READ_ONLY"
                    ), "data service allowed app/control work"
            status, app_answer = gql(app_service, web_token, mutation)
            assert status == 200 and app_answer.get("data", {}).get(
                "createAnnotation", {}
            ).get(
                "id"
            ), "fresh app could not validate its target through the independently authorized data service"
            assert (
                lab.sql(
                    pg, "fresh_app", "SELECT to_regclass('public.data_points') IS NULL"
                )
                == "t"
            )
            assert (
                lab.sql(
                    pg,
                    "readonly_data",
                    "SELECT to_regclass('public.chart_annotations') IS NULL",
                )
                == "t"
            )
            assert (
                lab.sql(pg, "fresh_app", "SELECT count(*) FROM chart_annotations")
                == "1"
            )
            assert (
                lab.sql(pg, "readonly_auth", "SELECT count(*) FROM chart_annotations")
                == "0"
            )
            assert (
                lab.sql(pg, "readonly_data", "SELECT count(*) FROM crawl_queue") == "0"
            )
            assert (
                lab.sql(pg, "readonly_data", "SELECT value::text FROM data_points")
                == "12.5"
            )
            print(
                "Real signed-token data read, app audience denial, no recrawl/write, and fresh-app annotation passed",
                flush=True,
            )
            evidence.update(
                {
                    "postgres_image": pg_image,
                    "actual_backend_signed_read": True,
                    "readonly_app_audience_rejected": True,
                    "readonly_and_staff_data_mutations_rejected": True,
                    "fresh_app_remote_target_and_annotation": True,
                    "queue_and_data_unchanged": True,
                    "scope": "loopback HTTP and disposable local realm; production issuer/TLS/platform not exercised",
                }
            )
            if args.evidence:
                args.evidence.write_text(json.dumps(evidence, indent=2) + "\n")
        except Exception:
            for path in work.glob("*.log"):
                print(
                    path.name + ": " + path.read_text(errors="replace")[-4000:],
                    flush=True,
                )
            for container in lab.containers:
                print(
                    lab.docker(
                        "inspect",
                        "--format",
                        "{{json .State}} {{json .HostConfig.PortBindings}} {{json .NetworkSettings.Ports}}",
                        container,
                    ),
                    flush=True,
                )
                print(lab.docker("logs", "--tail", "25", container), flush=True)
            raise
        finally:
            lab.cleanup()


if __name__ == "__main__":
    main()
