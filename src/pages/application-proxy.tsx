import {
  DragDropProvider,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  type DragEndEvent,
} from '@dnd-kit/react'
import AddRounded from '@mui/icons-material/AddRounded'
import HelpOutlineRounded from '@mui/icons-material/HelpOutlineRounded'
import RefreshRounded from '@mui/icons-material/RefreshRounded'
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  IconButton,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'

import { ApplicationDropZone } from '@/components/application-proxy/application-drop-zone'
import { ApplicationProxyOption } from '@/components/application-proxy/application-proxy-option'
import { ApplicationRow } from '@/components/application-proxy/application-row'
import { BasePage, BaseSearchBox, VirtualList } from '@/components/base'
import { useRuntimeConfig } from '@/hooks/use-clash'
import { useConnectionData } from '@/hooks/use-connection-data'
import { useSystemState } from '@/hooks/use-system-state'
import { useVerge } from '@/hooks/use-verge'
import {
  useAppRefreshers,
  useClashConfigData,
  useProxiesData,
} from '@/providers/app-data-context'
import {
  disconnectApplicationConnections,
  getApplicationProcesses,
  updateApplicationProcess,
  resetApplicationProcess,
  listProxyApplications,
  resolveProxyApplication,
  updateApplicationProxy,
  type ApplicationProxyChange,
} from '@/services/application-proxy'
import {
  buildApplicationRows,
  type ApplicationRowModel,
} from '@/services/application-proxy-model'
import { showNotice } from '@/services/notice-service'
import { useQuery } from '@/services/query-client'

const EMPTY_CONFIGURED: IApplicationProxyEntry[] = []
const applicationPointerSensor = PointerSensor.configure({
  activationConstraints: () => undefined,
})

const ApplicationProxyPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { verge } = useVerge()
  const { data: runtime, refetch: refreshRuntime } = useRuntimeConfig()
  const { clashConfig } = useClashConfigData()
  const { proxyView } = useProxiesData()
  const { isTunModeAvailable } = useSystemState()
  const { refreshClashConfig, refreshRules } = useAppRefreshers()
  const { response } = useConnectionData()
  const {
    data: installed = [],
    error: listError,
    isPending: loadingApplications,
    refetch: refreshInstalled,
  } = useQuery({
    queryKey: ['listProxyApplications'],
    queryFn: listProxyApplications,
    retry: false,
  })
  const { data: processSnapshot, refetch: refreshProcesses } = useQuery({
    queryKey: ['getApplicationProcesses'],
    queryFn: getApplicationProcesses,
    refetchInterval: 2000,
    retry: false,
  })
  const [expandedApplications, setExpandedApplications] = useState<Set<string>>(
    () => new Set(),
  )
  const [search, setSearch] = useState('')
  const [match, setMatch] = useState(() => (_: string) => true)
  const [pending, setPending] = useState(false)
  const [adding, setAdding] = useState(false)
  const [path, setPath] = useState('')
  const [enabling, setEnabling] = useState<ApplicationRowModel | null>(null)
  const [enableOnConfirm, setEnableOnConfirm] = useState(true)
  const [chosenProxy, setChosenProxy] = useState<string | null>(null)
  const [dragging, setDragging] = useState<string | null>(null)
  const applicationMode = verge?.application_proxy?.enabled ?? false
  const configured = verge?.application_proxy?.applications ?? EMPTY_CONFIGURED
  const connections = response.data.activeConnections
  const rows = useMemo(
    () =>
      buildApplicationRows(
        installed,
        configured,
        connections,
        response.data.sampleIntervalMs,
        processSnapshot,
      ),
    [
      installed,
      configured,
      connections,
      response.data.sampleIntervalMs,
      processSnapshot,
    ],
  )
  const targets = useMemo(
    () =>
      [
        ...new Set([
          ...(runtime?.['proxy-groups'] ?? []).map((group) => group.name),
          ...(runtime?.proxies ?? []).map((proxy) => proxy.name),
        ]),
      ].filter(
        (name) =>
          !['DIRECT', 'GLOBAL', 'REJECT', 'REJECT-DROP', 'PASS'].includes(
            name,
          ) && !name.includes(','),
      ),
    [runtime],
  )
  const children = (row: ApplicationRowModel): ApplicationRowModel[] =>
    (row.children ?? []).flatMap((child) => [child, ...children(child)])
  const filtered = rows.filter((row) =>
    [row, ...children(row)].some((item) =>
      match(
        `${item.application.name}\n${item.application.path}\n${item.process?.pid ?? ''}`,
      ),
    ),
  )
  const enabled = filtered.filter((row) => row.enabled)
  const disabled = filtered.filter((row) => !row.enabled)
  const tunActive = isTunModeAvailable && clashConfig?.tun?.enable === true

  const run = async (operation: () => Promise<void>) => {
    setPending(true)
    try {
      await operation()
    } catch (error) {
      showNotice.error(error)
    } finally {
      setPending(false)
    }
  }
  const save = async (change: ApplicationProxyChange) => {
    await updateApplicationProxy(change)
    await Promise.allSettled([
      refreshRuntime(),
      refreshClashConfig(),
      refreshRules(),
    ])
    showNotice.success('rules.applicationProxy.saved')
  }
  const saveProcess = async (
    row: ApplicationRowModel,
    enabled: boolean,
    proxy: string | null,
    routingMode: IApplicationRoutingMode = row.routingMode,
  ) => {
    if (!row.process) return
    await updateApplicationProcess({
      pid: row.process.pid,
      start_time: row.process.start_time,
      enabled,
      routing_mode: routingMode,
      proxy,
    })
    await Promise.allSettled([
      refreshProcesses(),
      refreshRuntime(),
      refreshRules(),
    ])
    showNotice.success('rules.applicationProxy.saved')
  }
  const setProxy = (row: ApplicationRowModel, proxy: string | null) =>
    run(() =>
      row.process
        ? saveProcess(row, row.enabled, proxy)
        : save(
            row.configured
              ? { type: 'set-proxy', path: row.application.path, proxy }
              : {
                  type: 'configure',
                  application: row.application,
                  proxy,
                  enabled: row.enabled,
                  routing_mode: row.routingMode,
                },
          ),
    )
  const setRoutingMode = (
    row: ApplicationRowModel,
    routingMode: IApplicationRoutingMode,
  ) => {
    if (pending || routingMode === row.routingMode) return
    if (
      routingMode === 'proxy' &&
      (!row.proxy || !targets.includes(row.proxy))
    ) {
      setEnabling(row)
      setEnableOnConfirm(row.enabled)
      setChosenProxy(null)
      return
    }
    void run(() =>
      row.process
        ? saveProcess(row, row.enabled, row.proxy, routingMode)
        : save(
            row.configured
              ? {
                  type: 'set-routing-mode',
                  path: row.application.path,
                  routing_mode: routingMode,
                }
              : {
                  type: 'configure',
                  application: row.application,
                  enabled: row.enabled,
                  proxy: row.proxy,
                  routing_mode: routingMode,
                },
          ),
    )
  }
  const toggle = (row: ApplicationRowModel, enabled: boolean) => {
    if (pending || row.enabled === enabled) return
    if (
      enabled &&
      row.routingMode === 'proxy' &&
      (!row.proxy || !targets.includes(row.proxy))
    ) {
      setEnabling(row)
      setEnableOnConfirm(true)
      setChosenProxy(
        row.proxy && targets.includes(row.proxy) ? row.proxy : null,
      )
      return
    }
    void run(() =>
      row.process
        ? saveProcess(row, enabled, row.proxy)
        : save(
            row.configured
              ? { type: 'set-enabled', path: row.application.path, enabled }
              : {
                  type: 'configure',
                  application: row.application,
                  proxy: row.proxy,
                  enabled,
                  routing_mode: row.routingMode,
                },
          ),
    )
  }
  const confirmEnable = () =>
    run(async () => {
      if (!enabling || !chosenProxy) return
      if (enabling.process)
        await saveProcess(enabling, enableOnConfirm, chosenProxy, 'proxy')
      else
        await save({
          type: 'configure',
          application: enabling.application,
          proxy: chosenProxy,
          enabled: enableOnConfirm,
          routing_mode: 'proxy',
        })
      setEnabling(null)
    })
  const add = () =>
    run(async () => {
      const application = await resolveProxyApplication(path)
      if (configured.some((entry) => entry.path === application.path))
        throw new Error(t('rules.applicationProxy.duplicate'))
      await save({ type: 'add', application })
      setAdding(false)
      setPath('')
      setSearch(application.name)
    })
  const drop = (event: DragEndEvent) => {
    setDragging(null)
    if (event.canceled) return
    const { source, target } = event.operation
    if (!source || !target) return
    const row = rows.find((row) => row.application.path === source.id)
    if (row && (target.id === 'enabled' || target.id === 'disabled'))
      toggle(row, target.id === 'enabled')
  }
  const rowActions = (row: ApplicationRowModel) => ({
    onProxy: (proxy: string | null) => setProxy(row, proxy),
    onRoutingMode: (mode: IApplicationRoutingMode) => setRoutingMode(row, mode),
    onToggle: (enabled: boolean) => toggle(row, enabled),
    onReset: () =>
      run(async () => {
        if (row.process) {
          await resetApplicationProcess(row.process)
          await Promise.allSettled([
            refreshProcesses(),
            refreshRuntime(),
            refreshRules(),
          ])
        } else await save({ type: 'remove', path: row.application.path })
      }),
    onDisconnect: () =>
      run(async () => {
        await disconnectApplicationConnections(row.connections)
        showNotice.success('rules.applicationProxy.disconnected')
      }),
  })
  const panel = (roots: ApplicationRowModel[], active: boolean) => {
    const items = roots
    return (
      <ApplicationDropZone
        active={active}
        disabled={pending}
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: active ? 100 : 150,
          flex: active ? '0 1 auto' : '1 1 auto',
          height: active
            ? items.length
              ? Math.min(
                  360,
                  54 +
                    items.length * 110 +
                    (items.some((row) =>
                      expandedApplications.has(row.application.path),
                    )
                      ? 220
                      : 0),
                )
              : 100
            : undefined,
          borderRadius: 2,
          overflow: 'hidden',
        }}
      >
        <Stack
          direction="row"
          style={{ alignItems: 'center' }}
          spacing={1}
          sx={{ px: 2, py: 1.2, flexShrink: 0 }}
        >
          <Box
            sx={{
              width: 7,
              height: 7,
              borderRadius: '50%',
              bgcolor: active ? 'success.main' : 'text.disabled',
            }}
          />
          <Typography variant="subtitle2">
            {t(
              active
                ? 'rules.applicationProxy.enabledArea'
                : 'rules.applicationProxy.disabledArea',
            )}
          </Typography>
          <Chip size="small" label={roots.length} sx={{ height: 21 }} />
          {active && !applicationMode && (
            <Chip
              size="small"
              variant="outlined"
              label={t('rules.applicationProxy.paused')}
              sx={{ height: 21 }}
            />
          )}
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ flex: 1, textAlign: 'right' }}
          >
            {t(
              active
                ? 'rules.applicationProxy.dropEnable'
                : 'rules.applicationProxy.dropDisable',
            )}
          </Typography>
        </Stack>
        <Box
          sx={{
            flex: 1,
            minHeight: 0,
            overflowX: 'auto',
            display: 'flex',
            flexDirection: 'column',
          }}
        >
          {items.length ? (
            <VirtualList
              count={items.length}
              estimateSize={110}
              getItemKey={(index) => items[index].application.path}
              style={{
                flex: 1,
                minHeight: 0,
                padding: '8px 12px 0',
                boxSizing: 'border-box',
              }}
              renderItem={(index) => {
                const row = items[index]
                const descendants = children(row)
                const expanded = expandedApplications.has(row.application.path)
                return (
                  <Box sx={{ pb: 0.75 }}>
                    <ApplicationRow
                      row={row}
                      targets={targets}
                      proxyView={proxyView}
                      pending={pending}
                      applicationMode={applicationMode}
                      {...rowActions(row)}
                      expanded={expanded}
                      processCount={descendants.length}
                      onExpand={() =>
                        setExpandedApplications((previous) => {
                          const next = new Set(previous)
                          if (next.has(row.application.path))
                            next.delete(row.application.path)
                          else next.add(row.application.path)
                          return next
                        })
                      }
                    >
                      {expanded && descendants.length > 0
                        ? descendants.map((processRow) => (
                            <ApplicationRow
                              key={`process:${processRow.process!.pid}:${processRow.process!.start_time}`}
                              row={processRow}
                              targets={targets}
                              proxyView={proxyView}
                              pending={pending}
                              applicationMode={applicationMode}
                              {...rowActions(processRow)}
                            />
                          ))
                        : null}
                    </ApplicationRow>
                  </Box>
                )
              }}
            />
          ) : (
            <Stack
              sx={{
                flex: 1,
                alignItems: 'center',
                justifyContent: 'center',
                minHeight: 65,
                px: 2,
              }}
            >
              <Typography variant="body2" color="text.secondary">
                {t(
                  active
                    ? 'rules.applicationProxy.enabledEmpty'
                    : 'rules.applicationProxy.filteredEmpty',
                )}
              </Typography>
            </Stack>
          )}
        </Box>
      </ApplicationDropZone>
    )
  }

  return (
    <BasePage
      full
      title={t('rules.applicationProxy.title')}
      contentStyle={{ height: '100%', overflow: 'hidden' }}
      header={
        <Stack direction="row" style={{ alignItems: 'center' }} spacing={0.5}>
          <Tooltip title={t('rules.applicationProxy.description')}>
            <IconButton size="small">
              <HelpOutlineRounded fontSize="small" />
            </IconButton>
          </Tooltip>
          <FormControlLabel
            label={t('rules.applicationProxy.enable')}
            control={
              <Switch
                checked={applicationMode}
                disabled={
                  pending || !verge || (!applicationMode && !isTunModeAvailable)
                }
                onChange={(_, enabled) =>
                  run(() => save({ type: 'set-mode', enabled }))
                }
              />
            }
          />
        </Stack>
      }
    >
      <Stack
        spacing={1.5}
        sx={{ p: { xs: 1, sm: 2 }, height: '100%', boxSizing: 'border-box' }}
      >
        <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
          <BaseSearchBox
            value={search}
            placeholder={t('rules.applicationProxy.search')}
            onSearch={(matcher, state) => {
              setSearch(state.text)
              setMatch(() => matcher)
            }}
          />
          <Button
            variant="outlined"
            startIcon={<AddRounded />}
            disabled={pending}
            onClick={() => setAdding(true)}
            sx={{ flexShrink: 0 }}
          >
            {t('rules.applicationProxy.addProcess')}
          </Button>
          <Tooltip title={t('rules.applicationProxy.refresh')}>
            <IconButton
              disabled={pending}
              onClick={() =>
                run(async () => {
                  await refreshInstalled()
                })
              }
            >
              <RefreshRounded />
            </IconButton>
          </Tooltip>
          {(pending || loadingApplications) && <CircularProgress size={20} />}
        </Stack>
        <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
          <Chip
            size="small"
            variant="outlined"
            color={applicationMode && tunActive ? 'success' : 'default'}
            label={t(
              applicationMode
                ? 'rules.applicationProxy.selectedMode'
                : 'rules.applicationProxy.originalMode',
            )}
          />
          <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }}>
            {t('rules.applicationProxy.overview', { count: rows.length })}
          </Typography>
          <Tooltip title={t('rules.applicationProxy.connectionHelp')}>
            <HelpOutlineRounded sx={{ fontSize: 16, color: 'text.disabled' }} />
          </Tooltip>
        </Stack>
        {!isTunModeAvailable && (
          <Alert
            severity="warning"
            action={
              <Button onClick={() => navigate('/settings')}>
                {t('rules.applicationProxy.settings')}
              </Button>
            }
          >
            {t('rules.applicationProxy.serviceRequired')}
          </Alert>
        )}
        {applicationMode && !tunActive && (
          <Alert severity="warning">
            {t('rules.applicationProxy.inactive')}
          </Alert>
        )}
        {listError && (
          <Alert severity="warning">
            {t('rules.applicationProxy.listFailed')}
          </Alert>
        )}
        <DragDropProvider
          sensors={[applicationPointerSensor, KeyboardSensor]}
          onDragStart={(event) => {
            const id = event.operation.source?.id
            setDragging(typeof id === 'string' ? id : null)
          }}
          onDragEnd={drop}
        >
          {panel(enabled, true)}
          {panel(disabled, false)}
          <DragOverlay dropAnimation={null}>
            <Paper sx={{ px: 2, py: 1, boxShadow: 6, pointerEvents: 'none' }}>
              <Typography variant="body2">
                {
                  rows.find((row) => row.application.path === dragging)
                    ?.application.name
                }
              </Typography>
            </Paper>
          </DragOverlay>
        </DragDropProvider>
      </Stack>
      <Dialog
        open={!!enabling}
        onClose={() => {
          if (!pending) setEnabling(null)
        }}
        fullWidth
        maxWidth="xs"
      >
        <DialogTitle>
          {t('rules.applicationProxy.chooseOutbound')} ·{' '}
          {enabling?.application.name}
        </DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            {t('rules.applicationProxy.selectBeforeEnable')}
          </Typography>
          <Autocomplete
            options={targets}
            value={chosenProxy}
            renderOption={({ key, ...props }, option) => (
              <Box component="li" key={key} {...props}>
                <ApplicationProxyOption name={option} proxyView={proxyView} />
              </Box>
            )}
            onChange={(_, value) => setChosenProxy(value)}
            disabled={pending}
            renderInput={(params) => (
              <TextField
                {...params}
                label={t('rules.applicationProxy.outbound')}
              />
            )}
          />
        </DialogContent>
        <DialogActions>
          <Button disabled={pending} onClick={() => setEnabling(null)}>
            {t('rules.applicationProxy.cancel')}
          </Button>
          <Button
            variant="contained"
            disabled={pending || !chosenProxy}
            onClick={confirmEnable}
          >
            {t('rules.applicationProxy.confirmEnable')}
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={adding}
        onClose={() => {
          if (!pending) setAdding(false)
        }}
        fullWidth
        maxWidth="sm"
      >
        <DialogTitle>{t('rules.applicationProxy.addProcess')}</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            {t('rules.applicationProxy.pathHelp')}
          </Typography>
          <TextField
            autoFocus
            fullWidth
            label={t('rules.applicationProxy.path')}
            value={path}
            onChange={(event) => setPath(event.target.value)}
            disabled={pending}
            placeholder="/usr/local/bin/example"
          />
        </DialogContent>
        <DialogActions>
          <Button disabled={pending} onClick={() => setAdding(false)}>
            {t('rules.applicationProxy.cancel')}
          </Button>
          <Button variant="contained" disabled={pending || !path} onClick={add}>
            {t('rules.applicationProxy.add')}
          </Button>
        </DialogActions>
      </Dialog>
    </BasePage>
  )
}

export default ApplicationProxyPage
