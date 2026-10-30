import { describe, expect, it } from 'vitest'

import {
  applicationConnectionsUrl,
  createApplicationConnectionMatcher,
  readApplicationConnectionFilter,
} from './application-connections'
import type { ApplicationRowModel } from './application-proxy-model'

const application: IProxyApplication = {
  name: 'IDE',
  path: '/Applications/IDE.app',
  kind: 'bundle',
}
const connection = (metadata: IConnectionsItem['metadata']) =>
  ({ metadata }) as IConnectionsItem

describe('application connection navigation', () => {
  it('round-trips application paths and process instance filters', () => {
    const row = { application } as ApplicationRowModel
    const url = applicationConnectionsUrl(row)
    expect(
      readApplicationConnectionFilter(new URLSearchParams(url.split('?')[1])),
    ).toEqual({ kind: 'application', application })
    row.application = { name: 'qemu', path: '/sdk/qemu', kind: 'executable' }
    row.process = {
      pid: 12,
      start_time: 345,
      parent_pid: 10,
      name: 'qemu',
      path: '/sdk/qemu',
    }
    const processUrl = applicationConnectionsUrl(row)
    expect(
      readApplicationConnectionFilter(
        new URLSearchParams(processUrl.split('?')[1]),
      ),
    ).toEqual({ kind: 'process', pid: 12, startTime: 345, name: 'qemu' })
    expect(
      readApplicationConnectionFilter(
        new URLSearchParams('processId=12&processStartTime=invalid'),
      ),
    ).toBeNull()
  })

  it('matches bundle helpers and preserved ancestry after processes exit', () => {
    const match = createApplicationConnectionMatcher({
      kind: 'application',
      application,
    })
    expect(
      match(
        connection({
          processPath: application.path + '/Contents/MacOS/Helper',
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(true)
    expect(
      match(
        connection({
          processPath: '/sdk/qemu',
          processAncestors: [
            {
              pid: 10,
              startTime: 123,
              path: application.path + '/Contents/MacOS/IDE',
            },
          ],
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(true)
    expect(
      match(
        connection({
          processPath: '/Applications/IDE.app.other/Helper',
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(false)
  })

  it('uses a live parent tree only when the socket process identity agrees', () => {
    const match = createApplicationConnectionMatcher(
      { kind: 'application', application },
      [
        {
          pid: 10,
          start_time: 123,
          parent_pid: null,
          name: 'IDE',
          path: application.path + '/Contents/MacOS/IDE',
        },
        {
          pid: 12,
          start_time: 345,
          parent_pid: 10,
          name: 'qemu',
          path: '/sdk/qemu',
        },
      ],
    )
    expect(
      match(
        connection({
          processId: 12,
          processStartTime: 345,
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(true)
    expect(
      match(
        connection({
          processId: 12,
          processStartTime: 346,
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(false)
  })

  it('keeps a process filter separate from other instances of the same executable', () => {
    const match = createApplicationConnectionMatcher({
      kind: 'process',
      pid: 12,
      startTime: 345,
      name: 'qemu',
    })
    expect(
      match(
        connection({
          processId: 12,
          processStartTime: 345,
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(true)
    expect(
      match(
        connection({
          processId: 13,
          processStartTime: 345,
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(false)
    expect(
      match(
        connection({
          processId: 12,
          processStartTime: 346,
        } as IConnectionsItem['metadata']),
      ),
    ).toBe(false)
  })
})
