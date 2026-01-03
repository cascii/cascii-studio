#!/usr/bin/env node
// Cross-platform script to build frontend with trunk
// This handles the NO_COLOR environment variable issue

const { spawn } = require('child_process');
const path = require('path');

// Remove NO_COLOR from environment
const env = { ...process.env };
delete env.NO_COLOR;

// Run trunk build from project root (parent directory of scripts/)
const projectRoot = path.resolve(__dirname, '../..');
const trunk = spawn('trunk', ['build'], {
  stdio: 'inherit',
  env: env,
  cwd: projectRoot
});

trunk.on('close', (code) => {
  process.exit(code);
});

trunk.on('error', (err) => {
  console.error('Failed to start trunk:', err);
  process.exit(1);
});
