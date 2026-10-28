import { beforeEach, describe, expect, it, vi } from 'vitest'

const { closeConnection } = vi.hoisted(() => ({ closeConnection: vi.fn() }))
vi.mock('tauri-plugin-mihomo-api', () => ({ closeConnection }))

import { disconnectApplication, matchesApplication } from './application-proxy'

const application: IProxyApplication = {
  name: 'Browser',
  path: '/Applications/Browser.app',
  kind: 'bundle',
}

describe('application connection ownership', () => {
  beforeEach(() => {
    closeConnection.mockReset()
  })

  it('includes bundled helpers without matching similarly named apps or unknown processes', () => {
    expect(
      matchesApplication(
        application,
        '/Applications/Browser.app/Contents/MacOS/Helper',
      ),
    ).toBe(true)
    expect(
      matchesApplication(application, '/Applications/Browser.app.other/Helper'),
    ).toBe(false)
    expect(matchesApplication(application, undefined)).toBe(false)
    const executable = {
      ...application,
      path: '/usr/local/bin/git',
      kind: 'executable' as const,
    }
    expect(matchesApplication(executable, '/usr/local/bin/git')).toBe(true)
    expect(matchesApplication(executable, '/usr/local/bin/git-helper')).toBe(
      false,
    )
  })

  it('closes only this application connections and propagates partial failures', async () => {
    const connections = [
      {
        id: 'owned',
        metadata: {
          processPath: '/Applications/Browser.app/Contents/MacOS/Helper',
        },
      },
      {
        id: 'unrelated',
        metadata: {
          processPath: '/Applications/Other.app/Contents/MacOS/Other',
        },
      },
      { id: 'unknown', metadata: {} },
    ] as IConnectionsItem[]
    closeConnection.mockResolvedValue(undefined)
    await expect(
      disconnectApplication(application, connections),
    ).resolves.toEqual({ ok: true, value: 1 })
    expect(closeConnection.mock.calls).toEqual([['owned']])
    closeConnection.mockReset()
    closeConnection.mockRejectedValue(new Error('connection failure'))
    await expect(
      disconnectApplication(application, connections),
    ).rejects.toThrow()
    expect(closeConnection.mock.calls).toEqual([['owned']])
  })
})
