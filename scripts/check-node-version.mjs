#!/usr/bin/env node
// Keep CI, package metadata, and Docker images aligned with the root .nvmrc.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

process.chdir(fileURLToPath(new URL('../', import.meta.url)));
const major = readFileSync('.nvmrc', 'utf8').trim();
if (!/^\d+$/.test(major)) throw new Error('.nvmrc must contain a Node major version');
const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {
  encoding: 'utf8',
}).split('\0').filter(Boolean);
const errors = [];
for (const file of files) {
  if (file.endsWith('package.json') && !file.endsWith('package-lock.json')) {
    const pkg = JSON.parse(readFileSync(file, 'utf8'));
    const requiresNode = existsSync(join(dirname(file), '.nvmrc'));
    if ((requiresNode || pkg.engines?.node) && pkg.engines?.node !== `>=${major}.0.0 <${Number(major) + 1}`) {
      errors.push(`${file}: engines.node differs from .nvmrc`);
    }
  }
  if (file === '.nvmrc' || file.endsWith('/.nvmrc')) {
    if (readFileSync(file, 'utf8').trim() !== major) errors.push(`${file}: differs from root .nvmrc`);
  }
  if (file.includes('Dockerfile')) {
    for (const match of readFileSync(file, 'utf8').matchAll(/^FROM node:([^\s]+)/gmi)) {
      // An exact pin: node:26.13.0-alpine@sha256:<digest>.
      if (!new RegExp(`^${major}\\.\\d+\\.\\d+-[^@\\s]+@sha256:[0-9a-f]{64}$`).test(match[1])) {
        errors.push(`${file}: node:${match[1]} is not node:${major}.x.y-<variant>@sha256:<digest> (.nvmrc is ${major})`);
      }
    }
  }
  if (/^\.github\/workflows\/.*\.ya?ml$/.test(file)) {
    const text = readFileSync(file, 'utf8');
    if (/^\s*node-version:/m.test(text)) errors.push(`${file}: use node-version-file: '.nvmrc'`);
    for (const match of text.matchAll(/^\s*node-version-file:\s*['"]?([^'"\s]+)/gm)) {
      if (match[1] !== '.nvmrc') errors.push(`${file}: use the root .nvmrc`);
    }
  }
}
if (errors.length) {
  console.error(errors.join('\n'));
  process.exitCode = 1;
} else {
  console.log(`Node ${major}: package engines, Docker images, and workflows agree with .nvmrc`);
}
