#!/usr/bin/env node

'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const packageRoot = path.resolve(__dirname, '..');
const vendorDir = path.join(packageRoot, 'vendor');
const cliName = process.platform === 'win32' ? 'span.exe' : 'span';
const guiName = process.platform === 'win32' ? 'span-gui.exe' : 'span-gui';
const cliPath = path.join(vendorDir, cliName);
const guiPath = path.join(vendorDir, guiName);
const repository = process.env.SPAN_REPOSITORY || 'mrone0/rs';
const packageVersion = require(path.join(packageRoot, 'package.json')).version;
const releaseVersion = process.env.SPAN_VERSION || packageVersion;

function log(message) {
  console.log(`[span] ${message}`);
}

function fail(message) {
  console.error(`[span] install failed: ${message}`);
  process.exit(1);
}

function platformAsset() {
  if (process.platform === 'darwin' && process.arch === 'arm64') return 'span-macos-arm64.tar.gz';
  if (process.platform === 'darwin' && process.arch === 'x64') return 'span-macos-x64.tar.gz';
  if (process.platform === 'linux' && process.arch === 'x64') return 'span-linux-x64.tar.gz';
  if (process.platform === 'win32' && process.arch === 'x64') return 'span-windows-x64.zip';
  fail(`unsupported platform: ${process.platform}/${process.arch}`);
}

function sha256(data) {
  return crypto.createHash('sha256').update(data).digest('hex');
}

function installFile(source, destination) {
  fs.mkdirSync(vendorDir, { recursive: true });
  fs.copyFileSync(source, destination);
  if (process.platform !== 'win32') fs.chmodSync(destination, 0o755);
}

function copyLocalBinaries() {
  const cliSource = process.env.SPAN_LOCAL_BINARY;
  const guiSource = process.env.SPAN_LOCAL_GUI_BINARY;
  if (!cliSource || !fs.existsSync(path.resolve(cliSource))) {
    fail(`SPAN_LOCAL_BINARY does not exist: ${cliSource || '(not set)'}`);
  }
  if (!guiSource || !fs.existsSync(path.resolve(guiSource))) {
    fail(`SPAN_LOCAL_GUI_BINARY does not exist: ${guiSource || '(not set)'}`);
  }
  installFile(path.resolve(cliSource), cliPath);
  installFile(path.resolve(guiSource), guiPath);
  log(`using local CLI: ${cliSource}`);
  log(`using local GUI: ${guiSource}`);
  log(`installed: ${cliPath}`);
  log(`installed: ${guiPath}`);
}

async function download(url) {
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) throw new Error(`${response.status} ${response.statusText} (${url})`);
  return Buffer.from(await response.arrayBuffer());
}

function extractArchive(archivePath, destination) {
  fs.mkdirSync(destination, { recursive: true });
  if (archivePath.endsWith('.tar.gz')) {
    execFileSync('tar', ['-xzf', archivePath, '-C', destination], { stdio: 'inherit' });
    return;
  }
  if (process.platform === 'win32') {
    execFileSync('powershell.exe', [
      '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
      '-Command', `Expand-Archive -LiteralPath '${archivePath.replaceAll("'", "''")}' -DestinationPath '${destination.replaceAll("'", "''")}' -Force`,
    ], { stdio: 'inherit' });
    return;
  }
  execFileSync('unzip', ['-q', archivePath, '-d', destination], { stdio: 'inherit' });
}

async function installReleaseBinaries() {
  if (process.env.SPAN_SKIP_DOWNLOAD === '1') {
    log('download skipped (SPAN_SKIP_DOWNLOAD=1)');
    return;
  }

  const asset = platformAsset();
  const tag = releaseVersion.startsWith('v') ? releaseVersion : `v${releaseVersion}`;
  const baseUrl = process.env.SPAN_RELEASE_BASE_URL || `https://github.com/${repository}/releases/download/${tag}`;
  log(`downloading ${asset} from ${tag}`);

  const archive = await download(`${baseUrl.replace(/\/$/, '')}/${asset}`);
  if (process.env.SPAN_SHA256 && sha256(archive) !== process.env.SPAN_SHA256.toLowerCase()) {
    fail(`SHA-256 mismatch for ${asset}`);
  }

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'span-npm-'));
  const archivePath = path.join(tempDir, asset);
  const extractedDir = path.join(tempDir, 'extracted');
  try {
    fs.writeFileSync(archivePath, archive);
    extractArchive(archivePath, extractedDir);
    const find = (name) => {
      const result = [];
      const visit = (current) => {
        for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
          const full = path.join(current, entry.name);
          if (entry.isDirectory()) visit(full);
          else if (entry.name === name) result.push(full);
        }
      };
      visit(extractedDir);
      return result[0];
    };
    const extractedCli = find(cliName);
    const extractedGui = find(guiName);
    if (!extractedCli || !extractedGui) {
      fail(`archive must contain both ${cliName} and ${guiName}`);
    }
    installFile(extractedCli, cliPath);
    installFile(extractedGui, guiPath);
    log(`installed: ${cliPath}`);
    log(`installed: ${guiPath}`);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

(async () => {
  try {
    if (process.env.SPAN_LOCAL_BINARY) copyLocalBinaries();
    else await installReleaseBinaries();
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
})();
