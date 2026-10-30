import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, mkdtempSync, rmSync, copyFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const source = JSON.parse(
  readFileSync(
    resolve(root, 'patches/mihomo-application-process.json'),
    'utf8',
  ),
)
if (process.platform !== 'darwin')
  throw new Error('Process instance routing currently targets macOS')
const host =
  process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'
const work = mkdtempSync(resolve(tmpdir(), 'mihomo-process-'))
try {
  const archive = resolve(work, 'source.tar.gz')
  execFileSync(
    'curl',
    [
      '--fail',
      '--location',
      '--connect-timeout',
      '10',
      '--max-time',
      '120',
      `https://codeload.github.com/MetaCubeX/mihomo/tar.gz/refs/tags/${source.version}`,
      '-o',
      archive,
    ],
    { stdio: 'inherit' },
  )
  const hash = createHash('sha256').update(readFileSync(archive)).digest('hex')
  if (hash !== source.sha256)
    throw new Error('Mihomo source archive checksum mismatch')
  execFileSync('tar', ['-xzf', archive, '--strip-components=1', '-C', work])
  execFileSync(
    'git',
    ['apply', resolve(root, 'patches/mihomo-application-process.patch')],
    { cwd: work },
  )
  const env = { ...process.env, CGO_ENABLED: '0' }
  execFileSync('go', ['test', './rules/common', './component/process', './adapter/outboundgroup', './adapter/provider'], {
    cwd: work,
    env,
    stdio: 'inherit',
  })
  execFileSync(
    'go',
    [
      'build',
      '-tags',
      'with_gvisor',
      '-trimpath',
      '-ldflags',
      `-s -w -X github.com/metacubex/mihomo/constant.Version=${source.version}-app-cross-subscription-1`,
      '-o',
      resolve(root, `src-tauri/sidecar/verge-mihomo-${host}`),
    ],
    { cwd: work, env, stdio: 'inherit' },
  )
  copyFileSync(
    resolve(root, `src-tauri/sidecar/verge-mihomo-${host}`),
    resolve(root, `src-tauri/sidecar/verge-mihomo-alpha-${host}`),
  )
} finally {
  rmSync(work, { recursive: true, force: true })
}
