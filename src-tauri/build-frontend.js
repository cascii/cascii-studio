#!/usr/bin/env node
// Cross-platform script to build frontend with trunk
// This handles the NO_COLOR environment variable issue

const { spawn } = require('child_process');
const path = require('path');

// Remove NO_COLOR from environment to prevent it being passed as --no-color 1 to trunk
const env = { ...process.env };
delete env.NO_COLOR;

// Run trunk build
const trunk = spawn('trunk', ['build'], {
  stdio: 'inherit',
  env: env,
  cwd: path.dirname(__filename)
});

trunk.on('close', (code) => {
  process.exit(code);
});

trunk.on('error', (err) => {
  console.error('Failed to start trunk:', err);
  process.exit(1);
});
