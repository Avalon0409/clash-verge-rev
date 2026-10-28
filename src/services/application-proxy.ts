import { invoke } from '@tauri-apps/api/core'
import i18n from 'i18next'
import { closeConnection } from 'tauri-plugin-mihomo-api'

import { mutate } from './mutate'

export const listProxyApplications = () =>
  invoke<IProxyApplication[]>('list_proxy_applications')

export const getApplicationProcesses = () =>
  invoke<IApplicationProcessSnapshot>('get_application_processes')

export const updateApplicationProcess = (
  setting: IApplicationProcessOverride,
) =>
  mutate(() => invoke<void>('update_application_process', { setting }), {
    id: 'patch-verge-config',
    revalidate: [['getApplicationProcesses'], ['getRuntimeConfig']],
    errorNotice: false,
  })

export const resetApplicationProcess = (process: IApplicationProcess) =>
  mutate(
    () =>
      invoke<void>('reset_application_process', {
        pid: process.pid,
        startTime: process.start_time,
      }),
    {
      id: 'patch-verge-config',
      revalidate: [['getApplicationProcesses'], ['getRuntimeConfig']],
      errorNotice: false,
    },
  )

export const resolveProxyApplication = (path: string) =>
  invoke<IProxyApplication>('resolve_proxy_application', { path })

export const getProxyApplicationIcon = (path: string) =>
  invoke<string | null>('get_proxy_application_icon', { path })

export type ApplicationProxyChange =
  | { type: 'set-mode'; enabled: boolean }
  | { type: 'add'; application: IProxyApplication }
  | {
      type: 'configure'
      application: IProxyApplication
      proxy: string | null
      enabled: boolean
      routing_mode?: IApplicationRoutingMode
    }
  | { type: 'set-enabled'; path: string; enabled: boolean }
  | {
      type: 'set-routing-mode'
      path: string
      routing_mode: IApplicationRoutingMode
    }
  | { type: 'set-proxy'; path: string; proxy: string | null }
  | { type: 'remove'; path: string }

export const updateApplicationProxy = (change: ApplicationProxyChange) =>
  mutate(() => invoke<void>('update_application_proxy', { change }), {
    id: 'patch-verge-config',
    revalidate: [['getVergeConfig'], ['getRuntimeConfig'], ['getClashMode']],
    errorNotice: false,
  })

export const matchesApplication = (
  application: IProxyApplication,
  processPath?: string,
) =>
  !!processPath &&
  (application.kind === 'bundle'
    ? processPath.startsWith(application.path + '/')
    : processPath === application.path)

export const disconnectApplication = (
  application: IProxyApplication,
  connections: IConnectionsItem[],
) =>
  disconnectApplicationConnections(
    connections.filter((connection) =>
      matchesApplication(application, connection.metadata.processPath),
    ),
  )

export const disconnectApplicationConnections = (
  connections: IConnectionsItem[],
) =>
  mutate(
    async () => {
      const matching = connections
      let failed = 0
      for (let offset = 0; offset < matching.length; offset += 32) {
        const results = await Promise.allSettled(
          matching
            .slice(offset, offset + 32)
            .map((connection) => closeConnection(connection.id)),
        )
        failed += results.filter(
          (result) => result.status === 'rejected',
        ).length
      }
      if (failed) {
        throw new Error(
          i18n.t(($) => $.rules.applicationProxy.disconnectFailed, {
            count: failed,
          }),
        )
      }
      return matching.length
    },
    { id: 'disconnect-application', errorNotice: false },
  )
