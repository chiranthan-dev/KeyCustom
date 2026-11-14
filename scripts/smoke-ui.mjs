// Smoke test for the release build on Windows: KeyForge must be started with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222. Connects
// to its WebView2 windows and checks that the main window renders the app
// (not a loading or error screen) and that the cheat-sheet window loaded.
import { chromium } from "playwright-core";

const endpoint = process.env.KEYFORGE_CDP ?? "http://127.0.0.1:9222";
const screenshot = process.argv[2] ?? "smoke.png";

async function connect() {
  const deadline = Date.now() + 30_000;
  for (;;) {
    try {
      return await chromium.connectOverCDP(endpoint);
    } catch (e) {
      if (Date.now() > deadline) throw new Error(`could not reach WebView2 at ${endpoint}: ${e.message}`);
      await new Promise((r) => setTimeout(r, 500));
    }
  }
}

const browser = await connect();
const pages = () => browser.contexts().flatMap((c) => c.pages());
const deadline = Date.now() + 30_000;
while (pages().length < 2 && Date.now() < deadline) await new Promise((r) => setTimeout(r, 250));
console.log("windows:", pages().map((p) => p.url()));

// Tauri tells each page which window it is in.
const label = (page) => page.evaluate(() => window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label).catch(() => undefined);
let main = null;
let overlay = null;
while ((!main || !overlay) && Date.now() < deadline + 10_000) {
  for (const page of pages()) {
    const l = await label(page);
    if (l === "main") main = page;
    if (l === "overlay") overlay = page;
  }
  if (!main || !overlay) await new Promise((r) => setTimeout(r, 250));
}
if (!main) throw new Error("main window not found");

try {
  await main.waitForSelector('nav[aria-label="Main"]', { timeout: 20_000 });
} catch {
  const text = await main.evaluate(() => document.body.innerText);
  await main.screenshot({ path: screenshot });
  throw new Error(`main window did not render the app. It shows: ${JSON.stringify(text.slice(0, 300))}`);
}
const text = await main.evaluate(() => document.body.innerText);
for (const bad of ["state not managed", "could not start", "Loading…"]) {
  if (text.includes(bad)) {
    await main.screenshot({ path: screenshot });
    throw new Error(`main window shows "${bad}": ${JSON.stringify(text.slice(0, 300))}`);
  }
}
await main.screenshot({ path: screenshot });
if (!overlay) throw new Error("cheat-sheet window was not created");
console.log("main window rendered; cheat-sheet window present");
await browser.close();
