import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  readFileSync,
  mkdtempSync,
  rmSync,
  copyFileSync,
  writeFileSync,
} from 'node:fs'
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
if (!['darwin', 'win32'].includes(process.platform))
  throw new Error('Process instance routing supports macOS and Windows')
const architecture = { arm64: 'aarch64', x64: 'x86_64', ia32: 'i686' }[
  process.arch
]
if (!architecture) throw new Error('Unsupported process routing architecture')
const host =
  process.platform === 'win32'
    ? `${architecture}-pc-windows-msvc.exe`
    : `${architecture}-apple-darwin`
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
  execFileSync(
    'go',
    [
      'test',
      './rules/common',
      './component/process',
      './adapter/outboundgroup',
      './adapter/provider',
    ],
    {
      cwd: work,
      env,
      stdio: 'inherit',
    },
  )
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
  if (process.platform === 'win32') {
    const digest = createHash('sha256')
      .update(
        readFileSync(resolve(root, `src-tauri/sidecar/verge-mihomo-${host}`)),
      )
      .digest('hex')
    writeFileSync(
      resolve(root, 'src-tauri/packages/windows/core-hashes.nsh'),
      `!define MIHOMO_SHA256 "${digest}"\n!define MIHOMO_ALPHA_SHA256 "${digest}"\n`,
    )
  }
} finally {
  rmSync(work, { recursive: true, force: true })
}
