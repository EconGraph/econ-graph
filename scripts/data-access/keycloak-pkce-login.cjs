// Only the disposable issuer and loopback callback may receive browser requests.
// Secrets arrive on stdin and the authorization response is consumed by the parent harness.
const { randomBytes, createHash } = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");

let stage = "read input";
(async () => {
  const input = JSON.parse(fs.readFileSync(0, "utf8"));
  const { chromium } = require(input.playwrightModule);
  const state = randomBytes(32).toString("base64url");
  const verifier = randomBytes(64).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  const issuer = new URL(input.issuer);
  const callback = new URL(input.redirect);
  if (issuer.hostname !== "127.0.0.1" || callback.hostname !== "127.0.0.1")
    throw new Error("Loopback endpoints required");
  const authorization = new URL(input.issuer + "/protocol/openid-connect/auth");
  authorization.search = new URLSearchParams({
    client_id: input.client,
    redirect_uri: input.redirect,
    response_type: "code",
    scope: "openid",
    state,
    code_challenge: challenge,
    code_challenge_method: "S256",
  });
  stage = "bind loopback callback";
  const listener = http.createServer((request, response) => {
    const target = new URL(request.url, callback.origin);
    if (
      request.method !== "GET" ||
      request.headers.host !== callback.host ||
      target.pathname !== callback.pathname ||
      target.searchParams.get("state") !== state
    ) {
      response.writeHead(400).end("Invalid test callback");
      return;
    }
    response
      .writeHead(200, {
        "Content-Type": "text/plain",
        "Cache-Control": "no-store",
      })
      .end("Local PKCE callback");
  });
  await new Promise((resolve, reject) => {
    listener.once("error", reject);
    listener.listen(Number(callback.port), "127.0.0.1", resolve);
  });
  let browser;
  try {
    stage = "launch Chromium";
    browser = await chromium.launch({
      headless: true,
      timeout: 10000,
      proxy: { server: callback.origin, bypass: "127.0.0.1,localhost" },
    });
  } catch (error) {
    listener.closeAllConnections();
    listener.close();
    throw error;
  }
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(10000);
    page.setDefaultNavigationTimeout(10000);
    await page.route("**/*", async (route) => {
      const url = new URL(route.request().url());
      if (url.origin === issuer.origin) await route.continue();
      else if (
        url.origin === callback.origin &&
        url.pathname === callback.pathname
      )
        await route.continue();
      else await route.abort();
    });
    stage = "open local authorization endpoint";
    await page.goto(authorization.toString());
    stage = "fill local login form";
    await page.locator("#username").fill(input.username);
    await page.locator("#password").fill(input.password);
    stage = "submit local login and receive callback";
    await Promise.all([
      page.waitForURL(
        (url) =>
          url.origin === callback.origin && url.pathname === callback.pathname,
      ),
      page.locator("#kc-login").click(),
    ]);
    stage = "validate local callback";
    const returned = new URL(page.url());
    if (
      returned.searchParams.get("state") !== state ||
      !returned.searchParams.get("code")
    )
      throw new Error("Invalid authorization response");
    process.stdout.write(
      JSON.stringify({ code: returned.searchParams.get("code"), verifier }),
    );
  } finally {
    await browser.close();
    listener.closeAllConnections();
    await new Promise((resolve) => listener.close(resolve));
  }
})().catch((error) => {
  // Do not print browser URLs, authorization codes, cookies or credentials on a failed run.
  const message = error.message
    .split("\n")[0]
    .replace(/https?:\/\/\S+/g, "[local URL]");
  process.stderr.write(
    `Disposable PKCE browser failed (${stage}): ${error.name}: ${message}\n`,
  );
  process.exitCode = 1;
});
