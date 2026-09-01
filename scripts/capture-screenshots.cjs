// Capture screenshots of each SPA route using puppeteer-core + Edge.
// Self-contained: spawns ui-server.cjs, waits for the port, then drives
// puppeteer against the running app, and tears the server down on exit.
const puppeteer = require('puppeteer-core');
const path = require('path');
const { spawn } = require('child_process');
const net = require('net');
const fs = require('fs');

const EDGE_PATH = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const PORT = 5199;
const BASE_URL = `http://localhost:${PORT}`;
const ROOT = path.join(__dirname, '..');
const OUT_DIR = path.join(ROOT, 'screenshots');
const DIST_DIR = path.join(ROOT, 'dist');
const SERVER_SCRIPT = path.join(__dirname, 'ui-server.cjs');

const PAGES = [
  { name: 'home', path: '/' },
  { name: 'download', path: '/download' },
  { name: 'instances', path: '/instances' },
  { name: 'settings', path: '/settings' },
  { name: 'accounts', path: '/accounts' },
];

/** 轮询端口直到 server 起来,最多 8 秒 */
function waitForPort(port, host, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const socket = net.connect({ port, host });
      let settled = false;
      socket.once('connect', () => {
        settled = true;
        socket.end();
        resolve();
      });
      socket.once('error', () => {
        if (settled) return;
        if (Date.now() > deadline) return reject(new Error(`port ${port} not ready`));
        setTimeout(tryOnce, 200);
      });
    };
    tryOnce();
  });
}

/** Edge 没有 --headless=new 时退回旧 headless,避免 puppeteer-core 25.x 找不到 CDP target */
function pickHeadless() {
  return 'new';
}

async function startServer() {
  if (!fs.existsSync(DIST_DIR)) {
    throw new Error(`dist/ not found at ${DIST_DIR} — run \`npm run build\` first`);
  }
  const child = spawn(process.execPath, [SERVER_SCRIPT], {
    cwd: ROOT,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env },
  });
  child.stdout.on('data', (b) => process.stdout.write(`[ui-server] ${b}`));
  child.stderr.on('data', (b) => process.stderr.write(`[ui-server] ${b}`));
  await waitForPort(PORT, '127.0.0.1', 8000);
  return child;
}

(async () => {
  const server = await startServer();
  let browser = null;
  try {
    browser = await puppeteer.launch({
      executablePath: EDGE_PATH,
      headless: pickHeadless(),
      args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
    });
    const page = await browser.newPage();
    await page.setViewport({ width: 1280, height: 800, deviceScaleFactor: 1 });

    // 注入 Tauri 桥 stub,绕过 TitleBar.tsx 在模块加载时调用 getCurrentWindow() 抛错。
    // 后端 invoke 仍会失败,但会被各页的 try/catch 优雅处理(显示错误态 UI),不会阻塞渲染。
    await page.evaluateOnNewDocument(() => {
      window.__TAURI_INTERNALS__ = {
        metadata: {
          currentWindow: { label: 'main' },
          currentWebview: { label: 'main', windowLabel: 'main' },
        },
        plugins: {},
        invoke: async () => {
          throw new Error('tauri not available in headless capture');
        },
        transformCallback: (cb) => {
          const id = Math.floor(Math.random() * 1e9);
          window[`_${id}`] = cb;
          return id;
        },
      };
    });

    for (const p of PAGES) {
      // Always start from / so SPA mounts fresh, then pushState to the route
      await page.goto(`${BASE_URL}/`, { waitUntil: 'networkidle0', timeout: 20000 });
      await page.evaluate((targetPath) => {
        window.history.pushState({}, '', targetPath);
        window.dispatchEvent(new PopStateEvent('popstate'));
      }, p.path);
      // Wait for React to render the new route (Chakra animations + tauri invoke failures)
      await new Promise((r) => setTimeout(r, 1800));
      const out = path.join(OUT_DIR, `${p.name}.png`);
      await page.screenshot({ path: out, fullPage: false });
      const size = fs.statSync(out).size;
      console.log(`${p.name}: ${out} (${size} bytes)`);
    }

    // Tauri 桌面窗口截图:本机没 build bamcl.exe 时,生成明确标注的占位 PNG,
    // 而不是留下过期的旧截图或空白。PowerShell 脚本在 ExePath 可用时会覆写它们。
    const placeholders = [
      { name: 'app-debug', title: 'app-debug.png', note: '对应 dev 构建窗口的 PrintWindow 抓帧', w: 1280, h: 760 },
      { name: 'app-running', title: 'app-running.png', note: '对应 release 构建窗口的 PrintWindow 抓帧', w: 1280, h: 720 },
    ];
    for (const ph of placeholders) {
      const html = `<!doctype html><html><head><meta charset="utf-8"><style>
        html,body{margin:0;height:100%;background:#fafbfc;font-family:-apple-system,Segoe UI,sans-serif;color:#374151;display:flex;align-items:center;justify-content:center}
        .box{max-width:760px;padding:36px 44px;border:2px dashed #cbd5e1;border-radius:16px;background:#fff;text-align:center;box-shadow:0 1px 3px rgba(0,0,0,.05)}
        h1{font-size:28px;margin:0 0 8px;color:#1e293b}
        .file{font-family:ui-monospace,Consolas,monospace;font-size:15px;color:#2563eb;background:#eff6ff;padding:4px 10px;border-radius:6px;display:inline-block;margin:8px 0}
        .note{font-size:13px;color:#64748b;margin-top:14px;line-height:1.6}
        code{background:#f1f5f9;padding:2px 6px;border-radius:4px;font-size:12px}
        .badge{display:inline-block;background:#fef3c7;color:#92400e;font-size:11px;padding:3px 9px;border-radius:999px;letter-spacing:1px;margin-bottom:18px;font-weight:700}
      </style></head><body>
        <div class="box">
          <div class="badge">PLACEHOLDER · 占位</div>
          <h1>${ph.title}</h1>
          <div class="file">${ph.note}</div>
          <div class="note">
            本机未 build 出 <code>bamcl.exe</code>,无法启动 Tauri 窗口抓帧。<br>
            要生成真截图:先 <code>npm run tauri build</code>,再执行<br>
            <code>pwsh -File scripts/capture-app.ps1 -ExePath src-tauri/target/release/bamcl.exe -ShotName ${ph.name}</code>
          </div>
        </div>
      </body></html>`;
      await page.setViewport({ width: ph.w, height: ph.h, deviceScaleFactor: 1 });
      await page.setContent(html, { waitUntil: 'load' });
      await new Promise((r) => setTimeout(r, 300));
      const out = path.join(OUT_DIR, `${ph.name}.png`);
      await page.screenshot({ path: out, fullPage: false });
      const size = fs.statSync(out).size;
      console.log(`${ph.name}: ${out} (${size} bytes) [placeholder]`);
    }
  } finally {
    if (browser) {
      try { await browser.close(); } catch (_) {}
    }
    try { server.kill(); } catch (_) {}
  }
})().catch((e) => {
  console.error('Screenshot capture failed:', e);
  process.exit(1);
});