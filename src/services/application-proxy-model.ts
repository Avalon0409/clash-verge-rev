import type { ProxyViewV1 } from '@/types/proxy-view'

import { matchesApplication } from './application-proxy'

export interface ApplicationRowModel {
  application: IProxyApplication
  configured: boolean
  enabled: boolean
  routingMode: IApplicationRoutingMode
  proxy: string | null
  connections: IConnectionsItem[]
  uploadSpeed: number | null
  downloadSpeed: number | null
  process?: IApplicationProcess
  children?: ApplicationRowModel[]
  depth?: number
  inheritedFrom?: string
}

export function buildApplicationRows(
  installed: IProxyApplication[],
  configured: IApplicationProxyEntry[],
  connections: IConnectionsItem[],
  sampleIntervalMs?: number,
  snapshot?: IApplicationProcessSnapshot,
): ApplicationRowModel[] {
  const applications = new Map(
    installed.map((application) => [application.path, application]),
  )
  for (const application of configured)
    applications.set(application.path, application)
  const processIndex = new Map(
    snapshot?.processes.map((process) => [process.pid, process]) ?? [],
  )
  const hasApplicationAncestor = (pid?: number) => {
    const visited = new Set<number>()
    let current = pid ? processIndex.get(pid) : undefined
    while (current && !visited.has(current.pid)) {
      visited.add(current.pid)
      if (
        [...applications.values()].some((application) =>
          matchesApplication(application, current!.path),
        )
      )
        return true
      current =
        current.parent_pid === null
          ? undefined
          : processIndex.get(current.parent_pid)
    }
    return false
  }
  for (const connection of connections) {
    const path = connection.metadata.processPath
    if (
      !path ||
      hasApplicationAncestor(connection.metadata.processId) ||
      [...applications.values()].some((application) =>
        matchesApplication(application, path),
      )
    )
      continue
    applications.set(path, {
      name: connection.metadata.process || path.split('/').at(-1) || path,
      path,
      kind: 'executable',
    })
  }
  const saved = new Map(
    configured.map((application) => [application.path, application]),
  )
  const rows: ApplicationRowModel[] = [...applications.values()].map(
    (application) => ({
      application,
      configured: saved.has(application.path),
      enabled: saved.get(application.path)?.enabled ?? false,
      routingMode: saved.get(application.path)?.routing_mode ?? 'rule',
      proxy: saved.get(application.path)?.proxy ?? null,
      connections: [],
      uploadSpeed: 0,
      downloadSpeed: 0,
    }),
  )
  const ownership = [...rows].sort(
    (left, right) =>
      right.application.path.length - left.application.path.length,
  )
  const byPid = new Map(
    snapshot?.processes.map((process) => [process.pid, process]) ?? [],
  )
  const ancestors = (process: IApplicationProcess) => {
    const chain: IApplicationProcess[] = []
    const visited = new Set<number>()
    let current: IApplicationProcess | undefined = process
    while (current && !visited.has(current.pid)) {
      visited.add(current.pid)
      chain.push(current)
      current =
        current.parent_pid === null ? undefined : byPid.get(current.parent_pid)
    }
    return chain
  }
  const settingsByPath = [...configured].sort(
    (left, right) => right.path.length - left.path.length,
  )
  const processRows = new Map<number, ApplicationRowModel>()
  const processOwners = new Map<number, ApplicationRowModel>()
  for (const process of snapshot?.processes ?? []) {
    const chain = ancestors(process)
    const owner = chain
      .map((ancestor) =>
        ownership.find((row) =>
          matchesApplication(row.application, ancestor.path),
        ),
      )
      .find((owner) => !!owner)
    if (!owner) continue
    let enabled = false
    let routingMode: IApplicationRoutingMode = 'rule'
    let proxy: string | null = null
    let inheritedFrom: string | undefined
    let bundleSetting: IApplicationProxyEntry | undefined
    let resolved = false
    for (const ancestor of chain) {
      const override = snapshot?.overrides.find(
        (entry) =>
          entry.pid === ancestor.pid &&
          entry.start_time === ancestor.start_time,
      )
      const executable = settingsByPath.find(
        (entry) =>
          entry.kind === 'executable' &&
          matchesApplication(entry, ancestor.path),
      )
      bundleSetting ??= settingsByPath.find(
        (entry) =>
          entry.kind === 'bundle' && matchesApplication(entry, ancestor.path),
      )
      const setting = override ?? executable
      if (setting) {
        enabled = setting.enabled
        routingMode = setting.routing_mode ?? 'rule'
        proxy = setting.proxy
        inheritedFrom =
          ancestor.pid === process.pid && override
            ? undefined
            : override
              ? `${ancestor.name} · PID ${ancestor.pid}`
              : executable!.name
        resolved = true
        break
      }
    }
    if (!resolved && bundleSetting) {
      enabled = bundleSetting.enabled
      routingMode = bundleSetting.routing_mode ?? 'rule'
      proxy = bundleSetting.proxy
      inheritedFrom = bundleSetting.name
    }
    const ownOverride =
      snapshot?.overrides.some(
        (entry) =>
          entry.pid === process.pid && entry.start_time === process.start_time,
      ) ?? false
    const row: ApplicationRowModel = {
      application: {
        name: process.name,
        path: process.path,
        kind: 'executable',
      },
      configured: ownOverride,
      enabled,
      routingMode,
      proxy,
      connections: [],
      uploadSpeed: 0,
      downloadSpeed: 0,
      process,
      inheritedFrom: ownOverride
        ? undefined
        : (inheritedFrom ?? owner.application.name),
      depth:
        chain.findIndex((ancestor) =>
          matchesApplication(owner.application, ancestor.path),
        ) + 1,
    }
    processRows.set(process.pid, row)
    processOwners.set(process.pid, owner)
  }
  for (const row of processRows.values()) {
    const owner = processOwners.get(row.process!.pid)!
    const parent =
      row.process!.parent_pid === null
        ? undefined
        : processRows.get(row.process!.parent_pid)
    const container =
      parent &&
      processOwners.get(parent.process!.pid) === owner &&
      !ancestors(parent.process!).some(
        (ancestor) => ancestor.pid === row.process!.pid,
      )
        ? parent
        : owner
    ;(container.children ??= []).push(row)
  }
  for (const connection of connections) {
    const pid = connection.metadata.processId
    const processRow = pid ? processRows.get(pid) : undefined
    const identityMatches =
      processRow &&
      processRow.process?.start_time === connection.metadata.processStartTime
    const owner = identityMatches
      ? processOwners.get(pid!)
      : ownership.find((row) =>
          matchesApplication(row.application, connection.metadata.processPath),
        )
    if (owner) owner.connections.push(connection)
    if (identityMatches) processRow.connections.push(connection)
  }
  const compare = (left: ApplicationRowModel, right: ApplicationRowModel) =>
    Number(right.connections.length > 0) -
      Number(left.connections.length > 0) ||
    left.application.name.localeCompare(right.application.name) ||
    (left.process?.pid ?? 0) - (right.process?.pid ?? 0) ||
    left.application.path.localeCompare(right.application.path)
  for (const row of [...rows, ...processRows.values()])
    row.children?.sort(compare)
  for (const row of [...rows, ...processRows.values()]) {
    if (!row.connections.length) continue
    const sampled = typeof sampleIntervalMs === 'number' && sampleIntervalMs > 0
    row.uploadSpeed =
      sampled &&
      row.connections.every((connection) => connection.curUpload !== undefined)
        ? (row.connections.reduce(
            (sum, connection) => sum + Math.max(0, connection.curUpload!),
            0,
          ) *
            1000) /
          sampleIntervalMs
        : null
    row.downloadSpeed =
      sampled &&
      row.connections.every(
        (connection) => connection.curDownload !== undefined,
      )
        ? (row.connections.reduce(
            (sum, connection) => sum + Math.max(0, connection.curDownload!),
            0,
          ) *
            1000) /
          sampleIntervalMs
        : null
  }
  const setDepth = (row: ApplicationRowModel, depth: number) => {
    if (row.process) row.depth = depth
    for (const child of row.children ?? []) setDepth(child, depth + 1)
  }
  for (const row of rows) setDepth(row, 0)
  return rows.sort(compare)
}

export function getApplicationProxyHistory(
  view: ProxyViewV1 | undefined,
  name: string | null,
) {
  const group = view?.groups.find((group) => group.name === name)
  const member = group?.members.find((member) => member.name === group.now)
  const selected =
    member?.kind === 'node' ? view?.records[member.recordId] : undefined
  const node =
    selected ??
    Object.values(view?.records ?? {}).find(
      (node) => node.name === name && node.source.kind === 'core',
    )
  return node?.history.at(-1) ?? group?.history.at(-1)
}
