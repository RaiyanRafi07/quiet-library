#!/usr/bin/env node
// Starts Vite dev server only if port 5500 is not already serving.
// If server is already up, exits 0 so Tauri continues. Otherwise, runs `npm run dev` and keeps it attached.

const http = require('http');
const { spawn } = require('child_process');

const DEV_URL = process.env.VITE_DEV_URL || 'http://localhost:5500';

function checkUp(url) {
  return new Promise((resolve) => {
    try {
      const req = http.get(url, (res) => {
        res.resume();
        resolve(true);
      });
      req.on('error', () => resolve(false));
      req.setTimeout(1500, () => {
        req.destroy(new Error('timeout'));
        resolve(false);
      });
    } catch (_) {
      resolve(false);
    }
  });
}

(async () => {
  const up = await checkUp(DEV_URL);
  if (up) {
    console.log(`[ensure-dev] Dev server already running at ${DEV_URL}`);
    process.exit(0);
  }
  console.log('[ensure-dev] Starting Vite dev server...');
  const child = spawn('npm', ['run', 'dev'], { stdio: 'inherit', shell: true });
  child.on('exit', (code) => process.exit(code ?? 0));
})();

