// Exercise the OAuth provider and MCP client exported by the installed
// Gemini CLI bundle. The Windows runner supplies the browser separately.
import { createRequire, syncBuiltinESMExports } from "node:module";
import { pathToFileURL } from "node:url";
import assert from "node:assert/strict";

const require = createRequire(import.meta.url);
const childProcess = require("node:child_process");
const originalExecFile = childProcess.execFile;
childProcess.execFile = (program, args, options, callback) => {
  if (program === "powershell.exe" && args.some((value) => value.includes("/oauth/authorize?"))) {
    return originalExecFile(process.execPath, ["-e", ""], options, callback);
  }
  return originalExecFile(program, args, options, callback);
};
syncBuiltinESMExports();

const [bundle, resource, requestedMode, target = 'fixture', requestedTool, rawArgs] = process.argv.slice(2);
const {
  Client, CoreEvent, MCPOAuthProvider, MCPOAuthTokenStorage, StreamableHTTPClientTransport, coreEvents,
} = await import(pathToFileURL(bundle).href);
const storage = new MCPOAuthTokenStorage();
const provider = new MCPOAuthProvider(storage);
const name = "horizon_fixture";
const game = requestedMode.endsWith("-game");
const mode = game ? requestedMode.slice(0, -5) : requestedMode;
const scopes = game
  ? ["launcher.read", "launcher.manage", "game.observe", "game.control"]
  : ["launcher.read"];
const config = { enabled: true, allowPrivateNetwork: true, scopes };
if (mode === "pair") {
  coreEvents.on(CoreEvent.ConsentRequest, ({ onConfirm }) => onConfirm(true));
  coreEvents.on(CoreEvent.UserFeedback, ({ message }) => {
    const url = message?.match(/https:\/\/[^\s]+\/oauth\/authorize\?[^\s]+/);
    if (url) console.log("AUTHORIZE " + url[0]);
  });
  await provider.authenticate(name, config, resource);
} else if (mode === "refresh") {
  const saved = await storage.getCredentials(name);
  assert(saved?.token.refreshToken);
  // Simulate elapsed client time in the isolated fixture credential cache.
  // The CLI provider performs the refresh and writes the rotated grant.
  await storage.saveToken(name, { ...saved.token, expiresAt: 1 }, saved.clientId,
    saved.tokenUrl, saved.mcpServerUrl);
}

const token = await provider.getValidToken(name, config);
assert(token, "Gemini did not obtain an access token");
const client = new Client({ name: "gemini-fixture", version: "1.0" });
const transport = new StreamableHTTPClientTransport(new URL(resource), {
  requestInit: { headers: { Authorization: "Bearer " + token } },
});
try {
  await client.connect(transport);
  const list = await client.listTools();
  const tool = requestedTool || "versions";
  assert(list.tools.some((item) => item.name === tool), "Tool missing from the current catalog");
  const result = await client.callTool({
    name: tool, arguments: rawArgs ? JSON.parse(rawArgs) : target === "cafe" && tool === "versions" ? { installed: true } : {},
  });
  assert(!result.isError, `Tool ${tool} returned an error: ${result.content?.[0]?.text ?? "unknown"}`);
  if (target === "fixture") {
    assert.equal(result.content[0].text, "Horizon Wii U HTTPS fixture");
  } else {
    console.log("RESULT " + JSON.stringify(result));
  }
} finally {
  await client.close();
}
await new Promise((done) => process.stdout.write("PASS " + mode + "\n", done));
process.exit(0);
