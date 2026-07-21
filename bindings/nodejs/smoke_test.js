#!/usr/bin/env node
/**
 * Smoke test: load guardd, verify digest, checkedLoad valid flag.
 * Exit 0 on success, 2 if native deps/lib unavailable (CI skip), 1 on failure.
 */
'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

function skip(msg) {
  console.log(`SKIP: ${msg}`);
  process.exitCode = 2;
}

function fail(msg) {
  console.error(`FAIL: ${msg}`);
  process.exitCode = 1;
}

function pass(msg) {
  console.log(`PASS: ${msg}`);
  process.exitCode = 0;
}

let testFile = null;

function cleanup() {
  if (!testFile) return;
  try {
    if (fs.existsSync(testFile)) fs.unlinkSync(testFile);
  } catch (_) {
    /* ignore */
  }
}

try {
  require('koffi');
} catch (err) {
  skip(`koffi not installable/available (${err.message})`);
  process.exit(process.exitCode);
}

let createGuard;
try {
  ({ createGuard } = require('./node_guard'));
} catch (err) {
  skip(`node_guard load failed (${err.message})`);
  process.exit(process.exitCode);
}

let guard;
try {
  guard = createGuard();
} catch (err) {
  skip(`libguardd not found or failed to load (${err.message})`);
  process.exit(process.exitCode);
}

testFile = path.join(os.tmpdir(), `mag-smoke-${process.pid}-${Date.now()}.bin`);
try {
  const payload = Buffer.from('model-asset-guard-node-smoke');
  fs.writeFileSync(testFile, payload);
  const digest = guard.computeDigest(testFile);
  if (!Buffer.isBuffer(digest) || digest.length !== 32) {
    fail('computeDigest must return a 32-byte Buffer');
  } else if (!guard.verifyDigest(testFile, digest)) {
    fail('verifyDigest returned false for matching digest');
  } else {
    const bad = Buffer.alloc(32, 0);
    if (guard.verifyDigest(testFile, bad) !== false) {
      fail('verifyDigest must return false on mismatch');
    } else {
      const ok = guard.checkedLoad(testFile, digest);
      if (!ok || !ok.valid) {
        fail('checkedLoad.valid expected true');
      } else {
        const badLoad = guard.checkedLoad(testFile, bad);
        if (!badLoad || badLoad.valid) {
          fail('checkedLoad.valid must be false on digest mismatch');
        } else {
          pass(`node smoke (lib=${guard.libPath})`);
        }
      }
    }
  }
} catch (err) {
  fail(err.message || String(err));
} finally {
  cleanup();
}

process.exit(process.exitCode ?? 0);
