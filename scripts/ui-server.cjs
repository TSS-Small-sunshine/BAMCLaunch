// Minimal static SPA server for screenshot capture
// Serves dist/ on a fixed port. SPA fallback → index.html
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..', 'dist');
const PORT = 5199;

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.json': 'application/json; charset=utf-8',
  '.ico': 'image/x-icon',
};

const server = http.createServer((req, res) => {
  // Parse URL — for SPA, treat any path without a file extension as /
  const url = new URL(req.url, `http://localhost:${PORT}`);
  let pathname = decodeURIComponent(url.pathname);

  // Find file
  let filepath = path.join(ROOT, pathname);
  if (!fs.existsSync(filepath) || fs.statSync(filepath).isDirectory()) {
    // SPA fallback
    filepath = path.join(ROOT, 'index.html');
  }

  const content = fs.readFileSync(filepath);
  const ext = path.extname(filepath).toLowerCase();
  res.setHeader('Content-Type', MIME[ext] || 'application/octet-stream');
  res.end(content);
});

server.listen(PORT, () => {
  console.log(`UI server: http://localhost:${PORT}`);
});