#!/usr/bin/env node
// Capture frames of a replay scenario from the web build in headless Chrome.
//
//   node tools/web/capture_frames.mjs <site-dir> <scenario> <t1,t2,...> <out-dir>
//
// <site-dir> is a private web build (tools/build_web.sh --private, which
// copies replays/ into it). Serves it on a local port, opens
// index.html?replay=<scenario>&ticks=<ticks> in headless Chrome (Chrome
// DevTools Protocol over Node's built-in WebSocket; no npm packages), waits
// for the game to finish the capture and writes each frame as
// frame_<tick>_<w>x<h>.rgba (raw RGBA, top-down), the format the native
// frame-capture mode writes (SKIFREE_REPLAY / SKIFREE_FRAMES).
//
// Chrome: $CHROME, else the usual install locations.

import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const [siteDir, scenario, ticks, outDir] = process.argv.slice(2);
if (!outDir) {
    console.error("usage: capture_frames.mjs <site-dir> <scenario> <t1,t2,...> <out-dir>");
    process.exit(2);
}
const TIMEOUT_MS = 180_000;

const MIME = {
    ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm",
    ".json": "application/json", ".txt": "text/plain",
};

function serve(root) {
    const server = http.createServer((req, res) => {
        const rel = decodeURIComponent(new URL(req.url, "http://x").pathname).replace(/^\/+/, "");
        const file = path.resolve(root, rel || "index.html");
        if (!file.startsWith(path.resolve(root)) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
            res.writeHead(404).end();
            return;
        }
        res.writeHead(200, { "Content-Type": MIME[path.extname(file)] || "application/octet-stream" });
        fs.createReadStream(file).pipe(res);
    });
    return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

function chromePath() {
    const candidates = [
        process.env.CHROME,
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    const found = candidates.find((p) => p && fs.existsSync(p));
    if (!found) throw new Error("Chrome not found; set CHROME");
    return found;
}

function launchChrome() {
    const profile = fs.mkdtempSync(path.join(os.tmpdir(), "skifree-chrome-"));
    const chrome = spawn(chromePath(), [
        "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
        "--no-first-run", "--no-default-browser-check", "--mute-audio", "about:blank",
    ], { stdio: ["ignore", "ignore", "pipe"] });
    const ws = new Promise((resolve, reject) => {
        let err = "";
        chrome.stderr.on("data", (d) => {
            err += d;
            const m = err.match(/DevTools listening on (ws:\/\/\S+)/);
            if (m) resolve(m[1]);
        });
        chrome.on("exit", (code) => reject(new Error(`Chrome exited (${code}): ${err}`)));
    });
    return { chrome, profile, ws };
}

class Cdp {
    constructor(url) {
        this.socket = new WebSocket(url);
        this.next = 1;
        this.pending = new Map();
        this.socket.onmessage = (e) => {
            const msg = JSON.parse(e.data);
            const p = this.pending.get(msg.id);
            if (!p) return;
            this.pending.delete(msg.id);
            if (msg.error) p.reject(new Error(msg.error.message));
            else p.resolve(msg.result);
        };
        this.open = new Promise((resolve, reject) => {
            this.socket.onopen = resolve;
            this.socket.onerror = reject;
        });
    }

    send(method, params = {}, sessionId) {
        const id = this.next++;
        this.socket.send(JSON.stringify({ id, method, params, sessionId }));
        return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
    }
}

async function main() {
    const server = await serve(siteDir);
    const { chrome, profile, ws } = launchChrome();
    try {
        const cdp = new Cdp(await ws);
        await cdp.open;
        const port = server.address().port;
        const url = `http://127.0.0.1:${port}/index.html?replay=${encodeURIComponent(scenario)}&ticks=${ticks}&nosound`;
        const { targetId } = await cdp.send("Target.createTarget", { url });
        const { sessionId } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
        const evaluate = async (expression) => {
            const r = await cdp.send("Runtime.evaluate", { expression, returnByValue: true }, sessionId);
            if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
            return r.result.value;
        };

        const start = Date.now();
        while (!(await evaluate("!!(window.skifreeCapture && window.skifreeCapture.done)"))) {
            if (Date.now() - start > TIMEOUT_MS) throw new Error("timed out waiting for the capture");
            await new Promise((r) => setTimeout(r, 250));
        }

        fs.mkdirSync(outDir, { recursive: true });
        let written = 0;
        for (const tick of ticks.split(",").map((t) => t.trim()).filter(Boolean)) {
            const frame = await evaluate(`(() => {
                const f = window.skifreeCapture.frames[${Number(tick)}];
                return f ? { w: f.w, h: f.h, data: window.skifreeCapture.base64(${Number(tick)}) } : null;
            })()`);
            if (!frame) {
                console.error(`tick ${tick}: no frame`);
                continue;
            }
            fs.writeFileSync(path.join(outDir, `frame_${tick}_${frame.w}x${frame.h}.rgba`), Buffer.from(frame.data, "base64"));
            written++;
        }
        console.log(`${scenario}: ${written} frame(s) -> ${outDir}`);
        cdp.socket.close();
    } finally {
        const exited = new Promise((r) => chrome.once("exit", r));
        chrome.kill();
        await Promise.race([exited, new Promise((r) => setTimeout(r, 5000))]);
        server.close();
        try {
            fs.rmSync(profile, { recursive: true, force: true, maxRetries: 3 });
        } catch (e) {
            // a leftover temp profile is harmless
        }
    }
}

main().catch((e) => {
    console.error(e.message);
    process.exit(1);
});
