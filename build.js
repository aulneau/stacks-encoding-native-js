/**
 * Script to build the native NodeJS addon for the current host platform.
 * Useful so a universal `npm install` command can work in development environments,
 * and to reduce the amount of CI/CD config needed to generate artifacts.
 */

const path = require('path');
const fs = require('fs');
const cargoCpArtifact = require('cargo-cp-artifact');

const targetMap = {
  'darwin-x64': ['darwin-x64.node', 'x86_64-apple-darwin'],
  'darwin-arm64': ['darwin-arm64.node', 'aarch64-apple-darwin'],
  'win32-x64': ['win32-x64.node', 'x86_64-pc-windows-msvc'],
  'linux-x64-glibc': ['linux-x64-glibc.node', 'x86_64-unknown-linux-gnu'],
  'linux-x64-musl': ['linux-x64-musl.node', 'x86_64-unknown-linux-musl'],
  'linux-arm64-glibc': ['linux-arm64-glibc.node', 'aarch64-unknown-linux-gnu'],
  'linux-arm64-musl': ['linux-arm64-musl.node', 'aarch64-unknown-linux-musl'],
};

const targetPlatform = process.env['TARGET_PLATFORM'] ?? process.platform;
const targetArch = process.env['TARGET_ARCH'] ?? process.arch;

let configuredTarget = `${targetPlatform}-${targetArch}`;

if (targetPlatform === 'linux') {
  const targetLibc = process.env['TARGET_LIBC'] ?? require('detect-libc').familySync() ?? 'glibc';
  configuredTarget += '-' + targetLibc;
  if (targetLibc === 'musl') {
    // Required in order to compile cdylib crates on musl targets
    process.env['RUSTFLAGS'] = '-C target-feature=-crt-static';
  }
}

const [outputFileName, cargoTarget] = targetMap[configuredTarget]

if (!outputFileName || !cargoTarget) {
  throw new Error(`Unsupported target ${configuredTarget}`);
}

const outputFilePath = path.join('native', outputFileName);
if (fs.existsSync(outputFilePath)) {
  fs.unlinkSync(outputFilePath);
}
let runArgs = [
  '-nc', outputFilePath, '--',
  'cargo', 'build', '--message-format=json-render-diagnostics', '--release',
  '--target', cargoTarget
];
cargoCpArtifact(runArgs, process.env);
