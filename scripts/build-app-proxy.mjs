import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const env = { ...process.env }
env.CARGO_TARGET_DIR ||= join(root, 'target.noindex')
env.NODE_OPTIONS ||= '--max-old-space-size=4096'
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  const key = join(homedir(), '.config/clash-verge-app-proxy/updater.key')
  if (existsSync(key)) {
    env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(key, 'utf8').trim()
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= ''
  }
}
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  throw new Error(
    'Configure the project release signing key before building an update package',
  )
}
const args = process.argv.slice(2)
if (args[0] === '--') args.shift()
const targetIndex = args.indexOf('--target')
const target = targetIndex < 0 ? [] : [args[targetIndex + 1]]
const command = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
for (const step of [
  ['prebuild', ...target],
  ['build:process-core'],
  [
    'exec',
    'tauri',
    'build',
    '--config',
    'src-tauri/tauri.app-proxy.conf.json',
    ...args,
  ],
]) {
  const result = spawnSync(command, step, {
    cwd: root,
    env,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}
