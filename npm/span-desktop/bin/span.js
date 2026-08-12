#!/usr/bin/env node

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

const packageRoot = path.resolve(__dirname, '..');
const binaryDir = process.env.SPAN_VENDOR_DIR || path.join(packageRoot, 'vendor');
const cliName = process.platform === 'win32' ? 'span.exe' : 'span';
const guiName = process.platform === 'win32' ? 'span-gui.exe' : 'span-gui';
const cliPath = process.env.SPAN_BINARY_PATH || path.join(binaryDir, cliName);
const guiPath = process.env.SPAN_GUI_BINARY_PATH || path.join(binaryDir, guiName);

for (const [label, file] of [['CLI', cliPath], ['GUI', guiPath]]) {
  if (!fs.existsSync(file)) {
    console.error(`Span ${label} binary is not installed: ${file}`);
    console.error('Try reinstalling the package, or set SPAN_LOCAL_BINARY and SPAN_LOCAL_GUI_BINARY.');
    process.exit(1);
  }
}

if (process.platform !== 'win32') {
  for (const file of [cliPath, guiPath]) {
    try { fs.chmodSync(file, 0o755); } catch (_) {}
  }
}

const forwardedArgs = process.argv.slice(2);
const opensGui = forwardedArgs.length === 0 || forwardedArgs[0] === 'gui' || forwardedArgs[0] === 'ui';
if (opensGui && process.env.SPAN_FOREGROUND_GUI !== '1') {
  const child = spawn(guiPath, [], {
    detached: true,
    stdio: 'ignore',
    windowsHide: true,
    env: process.env,
  });
  child.unref();
  process.exit(0);
}

const result = spawnSync(cliPath, forwardedArgs, {
  stdio: 'inherit',
  windowsHide: true,
  env: process.env,
});
if (result.error) {
  console.error(`Unable to start Span: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  console.error(`Span exited because of signal ${result.signal}`);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
