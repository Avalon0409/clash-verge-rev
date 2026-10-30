import { matchesApplication } from './application-proxy'
import type { ApplicationRowModel } from './application-proxy-model'

export type ApplicationConnectionFilter =
  | { kind: 'application'; application: IProxyApplication }
  | { kind: 'process'; pid: number; startTime: number; name: string }

export const APPLICATION_CONNECTION_PARAMS = [
  'applicationPath',
  'applicationKind',
  'applicationName',
  'processId',
  'processStartTime',
  'processName',
] as const

export function applicationConnectionsUrl(row: ApplicationRowModel): string {
  const params = new URLSearchParams()
  if (row.process) {
    params.set('processId', String(row.process.pid))
    params.set('processStartTime', String(row.process.start_time))
    params.set('processName', row.application.name)
  } else {
    params.set('applicationPath', row.application.path)
    params.set('applicationKind', row.application.kind)
    params.set('applicationName', row.application.name)
  }
  return `/connections?${params}`
}

export function readApplicationConnectionFilter(
  params: URLSearchParams,
): ApplicationConnectionFilter | null {
  if (params.has('processId')) {
    const pid = Number(params.get('processId'))
    const startTime = Number(params.get('processStartTime'))
    if (
      !Number.isInteger(pid) ||
      pid <= 0 ||
      pid > 0xffffffff ||
      !Number.isSafeInteger(startTime) ||
      startTime <= 0
    )
      return null
    return {
      kind: 'process',
      pid,
      startTime,
      name: params.get('processName') ?? '',
    }
  }
  const path = params.get('applicationPath')
  const kind = params.get('applicationKind')
  if (!path || (kind !== 'bundle' && kind !== 'executable')) return null
  return {
    kind: 'application',
    application: { path, kind, name: params.get('applicationName') ?? path },
  }
}

export function createApplicationConnectionMatcher(
  filter: ApplicationConnectionFilter,
  processes: IApplicationProcess[] = [],
) {
  const byPid = new Map(processes.map((process) => [process.pid, process]))
  return (connection: IConnectionsItem): boolean => {
    const metadata = connection.metadata
    if (filter.kind === 'process')
      return (
        metadata.processId === filter.pid &&
        metadata.processStartTime === filter.startTime
      )
    const application = filter.application
    if (
      matchesApplication(application, metadata.processPath) ||
      metadata.processAncestors?.some((ancestor) =>
        matchesApplication(application, ancestor.path),
      )
    )
      return true
    let process = metadata.processId ? byPid.get(metadata.processId) : undefined
    if (!process || process.start_time !== metadata.processStartTime)
      return false
    const visited = new Set<number>()
    while (process && !visited.has(process.pid)) {
      visited.add(process.pid)
      if (matchesApplication(application, process.path)) return true
      process =
        process.parent_pid === null ? undefined : byPid.get(process.parent_pid)
    }
    return false
  }
}
