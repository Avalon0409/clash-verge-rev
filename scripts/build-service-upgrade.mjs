import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { appendFileSync, copyFileSync, mkdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

if (process.platform !== 'darwin')
  throw new Error('Build the macOS service installer on macOS')
const root = fileURLToPath(new URL('../', import.meta.url))
const source = JSON.parse(
  readFileSync(
    join(root, 'patches/service-install-owner-release.json'),
    'utf8',
  ),
)
const dependency = readFileSync(join(root, 'src-tauri/Cargo.toml'), 'utf8')
  .split(/\r?\n/)
  .find((line) => line.startsWith('clash_verge_service_ipc ='))
if (!dependency?.includes(`version = "${source.version}"`))
  throw new Error('The service upgrade patch must match the IPC dependency')
const target =
  process.argv[2] ||
  execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(
    /^host: (.+)$/m,
  )?.[1]
if (!target?.endsWith('-apple-darwin'))
  throw new Error('A macOS target is required for the service installer')
const work = join(root, 'node_modules/.verge/service-owner-release')
const archive = join(work, 'source.tar.gz')
const directory = join(work, source.revision)
mkdirSync(directory, { recursive: true })
execFileSync(
  'curl',
  [
    '--fail',
    '--location',
    '--connect-timeout',
    '10',
    '--max-time',
    '120',
    `https://codeload.github.com/clash-verge-rev/clash-verge-service-ipc/tar.gz/${source.revision}`,
    '--output',
    archive,
  ],
  { stdio: 'inherit' },
)
if (
  createHash('sha256').update(readFileSync(archive)).digest('hex') !==
  source.sha256
)
  throw new Error('Service installer source checksum mismatch')
execFileSync('tar', ['-xzf', archive, '--strip-components=1', '-C', directory])
appendFileSync(join(directory, 'Cargo.toml'), '\n[workspace]\n')
execFileSync(
  'git',
  ['apply', join(root, 'patches/service-install-owner-release.patch')],
  {
    cwd: directory,
  },
)
const targetDirectory = join(
  process.env.CARGO_TARGET_DIR || join(root, 'target.noindex'),
  'bundled-service',
)
const args = [
  '--manifest-path',
  join(directory, 'Cargo.toml'),
  '--target-dir',
  targetDirectory,
  '--target',
  target,
  '--features',
  'standalone,client',
  '--bin',
  'clash-verge-service-install',
  '--locked',
]
execFileSync(
  'cargo',
  ['test', ...args, 'owner_release_tests', '--', '--test-threads=1'],
  {
    cwd: directory,
    stdio: 'inherit',
  },
)
execFileSync('cargo', ['build', '--release', ...args], {
  cwd: directory,
  stdio: 'inherit',
})
const executable = join(
  targetDirectory,
  target,
  'release/clash-verge-service-install',
)
if (!readFileSync(executable).includes(Buffer.from(source.marker)))
  throw new Error(
    'The built service installer is missing the owner-release fix',
  )
copyFileSync(
  executable,
  join(root, 'src-tauri/resources/clash-verge-service-install'),
)
