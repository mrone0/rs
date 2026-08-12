#!/usr/bin/env node

'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const packageRoot = path.resolve(__dirname, '..');
const binaryName = process.platform === 'win32' ? 'span.exe' : 'span';
const binaryPath = process.env.SPAN_BINARY_PATH || path.join(packageRoot, 'vendor', binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(`Span binary is not installed: ${binaryPath}`);
  console.error('Try reinstalling the package, or set SPAN_LOCAL_BINARY for a local build.');
  process.exit(1);
}

if (process.platform !== 'win32') {
  try {
    fs.chmodSync(binaryPath, 0o755);
  } catch (_) {
    // The child process will report a useful error if chmod is unavailable.
  }
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
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
