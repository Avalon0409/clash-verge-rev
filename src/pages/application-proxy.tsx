import {
  DragDropProvider,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  type DragEndEvent,
} from '@dnd-kit/react'
import AddRounded from '@mui/icons-material/AddRounded'
import FilterAltRounded from '@mui/icons-material/FilterAltRounded'
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

import { ApplicationCardList } from '@/components/application-proxy/application-card-list'
import { ApplicationDropZone } from '@/components/application-proxy/application-drop-zone'
import { ApplicationProcessDialog } from '@/components/application-proxy/application-process-dialog'
import { ApplicationProxyOption } from '@/components/application-proxy/application-proxy-option'
import { ApplicationRow } from '@/components/application-proxy/application-row'
import { BasePage, BaseSearchBox } from '@/components/base'
import { useRuntimeConfig } from '@/hooks/use-clash'
import { useConnectionData } from '@/hooks/use-connection-data'
import { useProfiles } from '@/hooks/use-profiles'
import { useSystemState } from '@/hooks/use-system-state'
import { useVerge } from '@/hooks/use-verge'
import {
  useAppRefreshers,
  useClashConfigData,
  useProxiesData,
} from '@/providers/app-data-context'
import { applicationConnectionsUrl } from '@/services/application-connections'
import {
  disconnectApplicationConnections,
  getApplicationProcesses,
  updateApplicationProcess,
  resetApplicationProcess,
  listApplicationProfileOutbounds,
  listProxyApplications,
  resolveProxyApplication,
  updateApplicationProxy,
  type ApplicationProxyChange,
} from '@/services/application-proxy'
import {
  applicationOutboundLabel,
  applicationProfilePrefix,
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
  const { profiles } = useProfiles()
  const subscriptions = (profiles?.items ?? [])
    .filter((item) => item.type === 'remote' || item.type === 'local')
    .map((item) => ({ uid: item.uid, name: item.name ?? item.uid }))
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
  const [processApplication, setProcessApplication] =
    useState<IProxyApplication | null>(null)
  const [search, setSearch] = useState('')
  const [runningOnly, setRunningOnly] = useState(false)
  const [match, setMatch] = useState(() => (_: string) => true)
  const [enabledRowHeight, setEnabledRowHeight] = useState(152)
  const [pending, setPending] = useState(false)
  const [adding, setAdding] = useState(false)
  const [path, setPath] = useState('')
  const [subscriptionEditing, setSubscriptionEditing] =
    useState<ApplicationRowModel | null>(null)
  const [chosenSubscription, setChosenSubscription] = useState<string | null>(
    null,
  )
  const [chosenSubscriptionProxy, setChosenSubscriptionProxy] = useState<
    string | null
  >(null)
  const {
    data: subscriptionOutbounds = [],
    isFetching: subscriptionLoading,
    error: subscriptionError,
  } = useQuery({
    queryKey: ['applicationProfileOutbounds', chosenSubscription],
    queryFn: () => listApplicationProfileOutbounds(chosenSubscription!),
    enabled: !!subscriptionEditing && !!chosenSubscription,
    retry: false,
  })
  const [enabling, setEnabling] = useState<ApplicationRowModel | null>(null)
  const [enableOnConfirm, setEnableOnConfirm] = useState(true)
  const [chosenProxy, setChosenProxy] = useState<string | null>(null)
  const [dragging, setDragging] = useState<string | null>(null)
  const applicationMode = verge?.application_proxy?.enabled ?? false
  const configurationDisabled = pending || !applicationMode
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
  const targetsFor = (row: ApplicationRowModel) =>
    targets.filter((name) =>
      row.profileUid
        ? name.startsWith(applicationProfilePrefix(row.profileUid)) &&
          !['GLOBAL', 'DIRECT', 'REJECT', 'REJECT-DROP', 'PASS'].includes(
            applicationOutboundLabel(name),
          )
        : !name.startsWith('__ap_'),
    )
  const openSubscription = (row: ApplicationRowModel, uid: string | null) => {
    if (configurationDisabled || row.profileUid === uid) return
    setSubscriptionEditing(row)
    setChosenSubscription(uid)
    setChosenSubscriptionProxy(null)
  }
  const confirmSubscription = () =>
    run(async () => {
      if (!subscriptionEditing) return
      const row = subscriptionEditing
      if (row.process)
        await saveProcess(
          row,
          row.enabled,
          chosenSubscriptionProxy,
          row.routingMode,
          chosenSubscription,
        )
      else
        await save({
          type: 'configure',
          application: row.application,
          enabled: row.enabled,
          proxy: chosenSubscriptionProxy,
          routing_mode: row.routingMode,
          profile_uid: chosenSubscription,
        })
      setSubscriptionEditing(null)
    })
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
  const disabled = filtered.filter(
    (row) => !row.enabled && (!runningOnly || !!row.children?.length),
  )
  const tunActive = isTunModeAvailable && clashConfig?.tun?.enable === true

  const run = async (
    operation: () => Promise<void>,
    allowWhenDisabled = false,
  ) => {
    if (!applicationMode && !allowWhenDisabled) return
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
    profileUid: string | null = row.profileUid,
  ) => {
    if (!row.process) return
    await updateApplicationProcess({
      pid: row.process.pid,
      start_time: row.process.start_time,
      enabled,
      routing_mode: routingMode,
      profile_uid: profileUid,
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
                  profile_uid: row.profileUid,
                },
          ),
    )
  const setRoutingMode = (
    row: ApplicationRowModel,
    routingMode: IApplicationRoutingMode,
  ) => {
    if (configurationDisabled || routingMode === row.routingMode) return
    if (
      routingMode === 'proxy' &&
      (!row.proxy || !targetsFor(row).includes(row.proxy))
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
                  profile_uid: row.profileUid,
                },
          ),
    )
  }
  const toggle = (row: ApplicationRowModel, enabled: boolean) => {
    if (configurationDisabled || row.enabled === enabled) return
    if (
      enabled &&
      row.routingMode === 'proxy' &&
      (!row.proxy || !targetsFor(row).includes(row.proxy))
    ) {
      setEnabling(row)
      setEnableOnConfirm(true)
      setChosenProxy(
        row.proxy && targetsFor(row).includes(row.proxy) ? row.proxy : null,
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
                  profile_uid: row.profileUid,
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
          profile_uid: enabling.profileUid,
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
  const processRoot = rows.find(
    (row) => row.application.path === processApplication?.path,
  )
  const rowActions = (row: ApplicationRowModel) => ({
    onViewConnections: () => {
      setProcessApplication(null)
      navigate(applicationConnectionsUrl(row))
    },
    onSubscription: (uid: string | null) => openSubscription(row, uid),
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
        disabled={configurationDisabled}
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: active
            ? items.length
              ? enabledRowHeight + 64
              : 100
            : 150,
          flex: active ? '0 0 auto' : '1 1 auto',
          height: active
            ? items.length
              ? enabledRowHeight + 64
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
          {!active && (
            <Tooltip title={t('rules.applicationProxy.runningOnlyHelp')}>
              <Button
                size="small"
                variant={runningOnly ? 'contained' : 'outlined'}
                startIcon={<FilterAltRounded />}
                aria-pressed={runningOnly}
                disabled={!processSnapshot}
                onClick={() => setRunningOnly((value) => !value)}
                sx={{ flexShrink: 0 }}
              >
                {t('rules.applicationProxy.runningOnly')}
              </Button>
            </Tooltip>
          )}
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
            <ApplicationCardList
              items={items}
              onFirstRowHeight={active ? setEnabledRowHeight : undefined}
              renderCard={(row) => {
                const descendants = children(row)
                return (
                  <Box>
                    <ApplicationRow
                      row={row}
                      targets={targetsFor(row)}
                      subscriptions={subscriptions}
                      proxyView={proxyView}
                      pending={configurationDisabled}
                      applicationMode={applicationMode}
                      {...rowActions(row)}
                      processCount={descendants.length}
                      onExpand={() => setProcessApplication(row.application)}
                    />
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
                    : runningOnly
                      ? 'rules.applicationProxy.runningEmpty'
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
      contentStyle={{ height: '100%', overflow: 'auto' }}
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
                  run(() => save({ type: 'set-mode', enabled }), true)
                }
              />
            }
          />
        </Stack>
      }
    >
      <Stack
        spacing={1.5}
        inert={configurationDisabled}
        sx={{
          p: { xs: 1, sm: 2 },
          height: '100%',
          boxSizing: 'border-box',
          opacity: applicationMode ? 1 : 0.5,
        }}
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
            disabled={configurationDisabled}
            onClick={() => setAdding(true)}
            sx={{ flexShrink: 0 }}
          >
            {t('rules.applicationProxy.addProcess')}
          </Button>
          <Tooltip title={t('rules.applicationProxy.refresh')}>
            <IconButton
              disabled={configurationDisabled}
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
      <ApplicationProcessDialog
        key={processApplication?.path ?? 'closed'}
        application={applicationMode ? processApplication : null}
        rows={processRoot ? children(processRoot) : []}
        pending={configurationDisabled}
        onClose={() => {
          if (!pending) setProcessApplication(null)
        }}
        renderRow={(row) => (
          <ApplicationRow
            row={row}
            targets={targetsFor(row)}
            subscriptions={subscriptions}
            proxyView={proxyView}
            pending={configurationDisabled}
            applicationMode={applicationMode}
            {...rowActions(row)}
          />
        )}
      />
      <Dialog
        open={applicationMode && !!subscriptionEditing}
        onClose={() => {
          if (!pending) setSubscriptionEditing(null)
        }}
        fullWidth
        maxWidth="sm"
      >
        <DialogTitle>
          {t('rules.applicationProxy.subscription')} ·{' '}
          {subscriptionEditing?.application.name}
        </DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 2 }}>
            {t('rules.applicationProxy.subscriptionRulesHint')}
          </Typography>
          {subscriptionError && (
            <Alert severity="error">
              {t('rules.applicationProxy.subscriptionReadFailed')}
            </Alert>
          )}
          <Autocomplete
            options={
              chosenSubscription
                ? subscriptionOutbounds.map((option) => option.value)
                : targets.filter((name) => !name.startsWith('__ap_'))
            }
            getOptionLabel={applicationOutboundLabel}
            value={chosenSubscriptionProxy}
            disabled={configurationDisabled || subscriptionLoading}
            onChange={(_, value) => setChosenSubscriptionProxy(value)}
            renderOption={({ key, ...props }, option) => (
              <Box component="li" key={key} {...props}>
                <ApplicationProxyOption name={option} proxyView={proxyView} />
              </Box>
            )}
            renderInput={(params) => (
              <TextField
                {...params}
                label={t('rules.applicationProxy.chooseOutbound')}
                placeholder={t('rules.applicationProxy.followSubscription')}
              />
            )}
          />
        </DialogContent>
        <DialogActions>
          <Button
            disabled={pending}
            onClick={() => setSubscriptionEditing(null)}
          >
            {t('rules.applicationProxy.cancel')}
          </Button>
          <Button
            disabled={
              configurationDisabled ||
              !!subscriptionError ||
              subscriptionLoading ||
              (subscriptionEditing?.routingMode === 'proxy' &&
                !chosenSubscriptionProxy)
            }
            onClick={confirmSubscription}
          >
            {t('rules.applicationProxy.confirmEnable')}
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={applicationMode && !!enabling}
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
            options={enabling ? targetsFor(enabling) : []}
            getOptionLabel={applicationOutboundLabel}
            value={chosenProxy}
            renderOption={({ key, ...props }, option) => (
              <Box component="li" key={key} {...props}>
                <ApplicationProxyOption name={option} proxyView={proxyView} />
              </Box>
            )}
            onChange={(_, value) => setChosenProxy(value)}
            disabled={configurationDisabled}
            renderInput={(params) => (
              <TextField
                {...params}
                label={t('rules.applicationProxy.outbound')}
              />
            )}
          />
        </DialogContent>
        <DialogActions>
          <Button
            disabled={configurationDisabled}
            onClick={() => setEnabling(null)}
          >
            {t('rules.applicationProxy.cancel')}
          </Button>
          <Button
            variant="contained"
            disabled={configurationDisabled || !chosenProxy}
            onClick={confirmEnable}
          >
            {t('rules.applicationProxy.confirmEnable')}
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={applicationMode && adding}
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
            disabled={configurationDisabled}
            placeholder="/usr/local/bin/example"
          />
        </DialogContent>
        <DialogActions>
          <Button
            disabled={configurationDisabled}
            onClick={() => setAdding(false)}
          >
            {t('rules.applicationProxy.cancel')}
          </Button>
          <Button
            variant="contained"
            disabled={configurationDisabled || !path}
            onClick={add}
          >
            {t('rules.applicationProxy.add')}
          </Button>
        </DialogActions>
      </Dialog>
    </BasePage>
  )
}

export default ApplicationProxyPage
