import { describe, expect, it } from 'vitest'

import { applicationPathKey, matchesApplication } from './application-proxy'
import { buildApplicationRows } from './application-proxy-model'

const browser: IProxyApplication = {
  name: 'Browser',
  path: '/Applications/Browser.app',
  kind: 'bundle',
}
const helper: IApplicationProxyEntry = {
  name: 'Helper',
  path: browser.path + '/Contents/MacOS/Helper',
  kind: 'executable',
  proxy: 'Exit',
  enabled: true,
}

describe('application traffic aggregation', () => {
  it('merges Windows executable paths across casing, separators and verbatim prefixes', () => {
    const application: IProxyApplication = {
      name: 'Browser',
      path: 'C:\\Apps\\Browser.exe',
      kind: 'executable',
    }
    const configured = {
      ...application,
      path: '\\\\?\\c:\\apps\\browser.exe',
      enabled: true,
      proxy: null,
    }
    const connection = {
      id: 'windows',
      metadata: { processPath: 'c:/APPS/BROWSER.EXE' },
    } as IConnectionsItem
    const rows = buildApplicationRows([application], [configured], [connection])
    expect(rows).toHaveLength(1)
    expect(rows[0].configured).toBe(true)
    expect(rows[0].connections).toEqual([connection])
    expect(applicationPathKey('\\\\?\\UNC\\Server\\Share\\Browser.exe')).toBe(
      '//server/share/browser.exe',
    )
    expect(
      matchesApplication(
        browser,
        '/applications/Browser.app/Contents/MacOS/Browser',
      ),
    ).toBe(false)
  })
  it('puts connected apps first and keeps name ordering when connection counts change', () => {
    const applications: IProxyApplication[] = [
      'Zulu',
      'Alpha',
      'Beta',
      'Idle',
    ].map((name) => ({
      name,
      path: `/Applications/${name}.app`,
      kind: 'bundle',
    }))
    const connection = (name: string, index: number) =>
      ({
        id: `${name}-${index}`,
        metadata: {
          processPath: `/Applications/${name}.app/Contents/MacOS/${name}`,
        },
      }) as IConnectionsItem
    const names = (connections: IConnectionsItem[]) =>
      buildApplicationRows(applications, [], connections).map(
        (row) => row.application.name,
      )
    const connections = [connection('Zulu', 1), connection('Beta', 1)]
    expect(names(connections)).toEqual(['Beta', 'Zulu', 'Alpha', 'Idle'])
    expect(names([...connections, connection('Zulu', 2)])).toEqual([
      'Beta',
      'Zulu',
      'Alpha',
      'Idle',
    ])
    expect(names([connection('Zulu', 1)])).toEqual([
      'Zulu',
      'Alpha',
      'Beta',
      'Idle',
    ])
  })

  it('gives an explicit helper its own connections and scales byte deltas by elapsed time', () => {
    const connections = [
      {
        id: 'helper',
        metadata: { processPath: helper.path },
        curUpload: 200,
        curDownload: 800,
      },
      {
        id: 'browser',
        metadata: { processPath: browser.path + '/Contents/MacOS/Browser' },
        curUpload: 100,
        curDownload: 400,
      },
      { id: 'unknown', metadata: {}, curUpload: 999, curDownload: 999 },
    ] as IConnectionsItem[]
    const rows = buildApplicationRows([browser], [helper], connections, 2000)
    expect(rows).toHaveLength(2)
    const parent = rows.find((row) => row.application.path === browser.path)!
    const child = rows.find((row) => row.application.path === helper.path)!
    expect(parent.connections.map((connection) => connection.id)).toEqual([
      'browser',
    ])
    expect(child.connections.map((connection) => connection.id)).toEqual([
      'helper',
    ])
    expect(parent.uploadSpeed).toBe(50)
    expect(child.downloadSpeed).toBe(400)
    expect(child.enabled).toBe(true)
    expect(parent.enabled).toBe(false)
  })

  it('lists unconfigured installed apps and observed executables without inventing a first-sample speed', () => {
    const rows = buildApplicationRows([browser], [], [
      {
        id: 'git',
        metadata: { process: 'git', processPath: '/usr/local/bin/git' },
        curUpload: 200,
      },
    ] as IConnectionsItem[])
    expect(
      rows.find((row) => row.application.path === browser.path)?.uploadSpeed,
    ).toBe(0)
    expect(
      rows.find((row) => row.application.path === '/usr/local/bin/git')
        ?.uploadSpeed,
    ).toBeNull()
    expect(rows.every((row) => row.proxy === null && !row.enabled)).toBe(true)
  })
})

it('groups external descendants by app and scopes overrides to the exact process instance', () => {
  const configured: IApplicationProxyEntry[] = [
    { ...browser, enabled: true, proxy: 'Exit', routing_mode: 'proxy' },
  ]
  const processes: IApplicationProcess[] = [
    {
      pid: 10,
      parent_pid: null,
      start_time: 100,
      name: 'Browser',
      path: browser.path + '/Contents/MacOS/Browser',
    },
    {
      pid: 20,
      parent_pid: 10,
      start_time: 200,
      name: 'emulator',
      path: '/sdk/emulator',
    },
    {
      pid: 30,
      parent_pid: 20,
      start_time: 300,
      name: 'qemu',
      path: '/sdk/qemu',
    },
    {
      pid: 31,
      parent_pid: 20,
      start_time: 301,
      name: 'qemu',
      path: '/sdk/qemu',
    },
  ]
  const overrides: IApplicationProcessOverride[] = [
    {
      pid: 30,
      start_time: 300,
      enabled: false,
      proxy: 'Exit',
      routing_mode: 'proxy',
    },
  ]
  const connections = [
    {
      id: 'qemu',
      metadata: {
        processPath: '/sdk/qemu',
        processId: 31,
        processStartTime: 301,
      },
      curUpload: 100,
      curDownload: 200,
    },
  ] as IConnectionsItem[]
  const rows = buildApplicationRows([browser], configured, connections, 1000, {
    processes,
    overrides,
  })
  expect(rows).toHaveLength(1)
  const root = rows[0]
  const emulator = root.children![0].children![0]
  expect(emulator.application.name).toBe('emulator')
  const stopped = emulator.children!.find((row) => row.process!.pid === 30)!
  const active = emulator.children!.find((row) => row.process!.pid === 31)!
  expect(stopped.enabled).toBe(false)
  expect(stopped.configured).toBe(true)
  expect(active.enabled).toBe(true)
  expect(active.configured).toBe(false)
  expect(active.proxy).toBe('Exit')
  expect(active.connections).toHaveLength(1)
  expect(root.connections).toHaveLength(1)
  expect(active.depth).toBe(3)
  const restarted = buildApplicationRows([browser], configured, [], 1000, {
    processes: processes.map((process) =>
      process.pid === 30 ? { ...process, start_time: 999 } : process,
    ),
    overrides,
  })
  const newInstance = restarted[0].children![0].children![0].children!.find(
    (row) => row.process!.pid === 30,
  )!
  expect(newInstance.enabled).toBe(true)
  expect(newInstance.configured).toBe(false)
  expect(configured).toHaveLength(1)
})

it('inherits subscription bindings into child processes', () => {
  const rows = buildApplicationRows(
    [browser],
    [
      {
        ...browser,
        enabled: true,
        routing_mode: 'rule',
        profile_uid: 'subscription-B',
        proxy: 'B-Exit',
      },
    ],
    [],
    1000,
    {
      processes: [
        {
          pid: 10,
          parent_pid: null,
          start_time: 100,
          name: 'Browser',
          path: browser.path + '/Contents/MacOS/Browser',
        },
        {
          pid: 11,
          parent_pid: 10,
          start_time: 101,
          name: 'Worker',
          path: '/external/worker',
        },
      ],
      overrides: [],
    },
  )
  expect(rows[0].profileUid).toBe('subscription-B')
  expect(rows[0].children![0].children![0].profileUid).toBe('subscription-B')
})
