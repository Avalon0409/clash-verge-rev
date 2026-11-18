import { createHash, createPublicKey, verify } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))

export function verifyUpdateArtifact(
  bytes,
  encodedSignature,
  encodedKey,
  version,
) {
  const keyLines = Buffer.from(encodedKey, 'base64')
    .toString('utf8')
    .trim()
    .split(/\r?\n/)
  const signatureLines = Buffer.from(encodedSignature, 'base64')
    .toString('utf8')
    .trim()
    .split(/\r?\n/)
  const key = Buffer.from(keyLines[1] ?? '', 'base64')
  const signature = Buffer.from(signatureLines[1] ?? '', 'base64')
  const globalSignature = Buffer.from(signatureLines[3] ?? '', 'base64')
  const commentPrefix = 'trusted comment: '
  const commentLine = signatureLines[2] ?? ''
  if (
    key.length !== 42 ||
    signature.length !== 74 ||
    globalSignature.length !== 64 ||
    !commentLine.startsWith(commentPrefix)
  ) {
    throw new Error('Invalid updater signature format')
  }
  if (
    signature.subarray(0, 2).toString('ascii') !== 'ED' ||
    !key.subarray(2, 10).equals(signature.subarray(2, 10))
  ) {
    throw new Error('Updater signature does not use the project signing key')
  }
  const comment = commentLine.slice(commentPrefix.length)
  if (!comment.split('\t').includes(`version:${version}`)) {
    throw new Error(
      'Updater signature is bound to a different application version',
    )
  }
  const publicKey = createPublicKey({
    key: Buffer.concat([
      Buffer.from('302a300506032b6570032100', 'hex'),
      key.subarray(10),
    ]),
    format: 'der',
    type: 'spki',
  })
  const payloadSignature = signature.subarray(10)
  const hashed = createHash('blake2b512').update(bytes).digest()
  if (
    !verify(null, hashed, publicKey, payloadSignature) ||
    !verify(
      null,
      Buffer.concat([payloadSignature, Buffer.from(comment)]),
      publicKey,
      globalSignature,
    )
  ) {
    throw new Error('Updater artifact signature verification failed')
  }
}

export function buildUpdateManifest(directory, tag, notes = '') {
  const { version } = JSON.parse(
    readFileSync(join(root, 'package.json'), 'utf8'),
  )
  if (!tag.startsWith(`app-proxy-${version}-`)) {
    throw new Error('Release tag must match the application version')
  }
  const config = JSON.parse(
    readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'),
  )
  const platforms = {}
  const assets = [
    [
      'windows-x86_64',
      'nsis',
      `Clash.Verge.App.Proxy_${version}_windows-x64-setup.exe`,
    ],
    [
      'darwin-aarch64',
      'app',
      `Clash.Verge.App.Proxy_${version}_macos-arm64.app.tar.gz`,
    ],
  ]
  for (const [platform, installer, filename] of assets) {
    const signature = readFileSync(
      join(directory, `${filename}.sig`),
      'utf8',
    ).trim()
    verifyUpdateArtifact(
      readFileSync(join(directory, filename)),
      signature,
      config.plugins.updater.pubkey,
      version,
    )
    const artifact = {
      signature,
      url: `https://github.com/Avalon0409/clash-verge-rev/releases/download/${tag}/${filename}`,
    }
    platforms[platform] = artifact
    platforms[`${platform}-${installer}`] = artifact
  }
  return {
    version,
    notes,
    pub_date: new Date().toISOString(),
    platforms,
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [directory, tag, notesFile] = process.argv.slice(2)
  if (!directory || !tag)
    throw new Error(
      'Usage: pnpm updater:app-proxy <asset-directory> <release-tag> [notes-file]',
    )
  const notes = notesFile ? readFileSync(notesFile, 'utf8') : ''
  const manifest = buildUpdateManifest(directory, tag, notes)
  writeFileSync(
    join(directory, 'latest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
  )
  console.log(`Verified signed updater artifacts for ${manifest.version}`)
}
