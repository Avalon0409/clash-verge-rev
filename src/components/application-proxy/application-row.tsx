import { useDraggable } from '@dnd-kit/react'
import AppsRounded from '@mui/icons-material/AppsRounded'
import ChevronRightRounded from '@mui/icons-material/ChevronRightRounded'
import DragIndicatorRounded from '@mui/icons-material/DragIndicatorRounded'
import MoreHorizRounded from '@mui/icons-material/MoreHorizRounded'
import {
  Autocomplete,
  Avatar,
  Box,
  Button,
  Chip,
  IconButton,
  Menu,
  MenuItem,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { getProxyApplicationIcon } from '@/services/application-proxy'
import {
  applicationOutboundLabel,
  getApplicationProxyHistory,
  type ApplicationRowModel,
} from '@/services/application-proxy-model'
import { useQuery } from '@/services/query-client'
import type { ProxyViewV1 } from '@/types/proxy-view'
import parseTraffic from '@/utils/parse-traffic'

import { ApplicationProxyOption } from './application-proxy-option'

interface Props {
  row: ApplicationRowModel
  targets: string[]
  proxyView?: ProxyViewV1
  pending: boolean
  applicationMode: boolean
  subscriptions: { uid: string; name: string }[]
  onSubscription: (uid: string | null) => void
  onRoutingMode: (mode: IApplicationRoutingMode) => void
  onProxy: (proxy: string | null) => void
  onToggle: (enabled: boolean) => void
  onDisconnect: () => void
  onViewConnections: () => void
  onReset: () => void
  onExpand?: () => void
  processCount?: number
}

export function ApplicationRow({
  row,
  targets,
  proxyView,
  pending,
  applicationMode,
  onProxy,
  onRoutingMode,
  subscriptions,
  onSubscription,
  onToggle,
  onDisconnect,
  onViewConnections,
  onReset,
  onExpand,
  processCount,
}: Props) {
  const { t } = useTranslation()
  const [menu, setMenu] = useState<HTMLElement | null>(null)
  const { application } = row
  const { ref, handleRef, isDragging } = useDraggable({
    id: row.process
      ? `process:${row.process.pid}:${row.process.start_time}`
      : application.path,
    disabled: pending || !!row.process,
  })
  const { data: icon } = useQuery({
    queryKey: ['application-icon', application.path],
    queryFn: () => getProxyApplicationIcon(application.path),
    staleTime: Infinity,
    retry: false,
  })
  const group = proxyView?.groups.find((group) => group.name === row.proxy)
  const history = getApplicationProxyHistory(proxyView, row.proxy)
  const missing = !!row.proxy && !targets.includes(row.proxy)
  const status = !row.proxy
    ? t(
        row.routingMode === 'rule'
          ? 'rules.applicationProxy.followSubscription'
          : 'rules.applicationProxy.notConfigured',
      )
    : missing
      ? t('rules.applicationProxy.missing')
      : history
        ? history.delay > 0
          ? `${history.delay} ms`
          : t('rules.applicationProxy.testFailed')
        : t('rules.applicationProxy.untested')
  const routes = [
    ...new Set(
      row.connections.map((connection) =>
        connection.chains.map(applicationOutboundLabel).reverse().join(' / '),
      ),
    ),
  ].filter(Boolean)
  const speed = (value: number | null) =>
    value === null
      ? t('rules.applicationProxy.sampling')
      : `${parseTraffic(value).join(' ')}/s`
  const options = row.proxy && missing ? [row.proxy, ...targets] : targets

  const identity = (
    <Tooltip title={application.path} placement="top-start">
      <Stack
        direction="row"
        spacing={0.8}
        ref={handleRef}
        tabIndex={pending || row.process ? -1 : 0}
        role={row.process ? undefined : 'button'}
        aria-label={application.name}
        sx={{
          alignItems: 'center',
          minWidth: 0,
          cursor: pending || row.process ? 'default' : 'grab',
          touchAction: 'none',
          userSelect: 'none',
        }}
      >
        {!row.process && (
          <DragIndicatorRounded sx={{ fontSize: 16, color: 'text.disabled' }} />
        )}
        <Avatar
          src={icon ?? undefined}
          slotProps={{ img: { draggable: false } }}
          variant="rounded"
          sx={{
            width: row.process ? 22 : 36,
            height: row.process ? 22 : 36,
            bgcolor: 'transparent',
            flexShrink: 0,
          }}
        >
          <AppsRounded color="disabled" />
        </Avatar>
        <Box sx={{ minWidth: 0 }}>
          <Typography
            noWrap
            variant={row.process ? 'body2' : 'subtitle1'}
            sx={{ fontWeight: 600, fontSize: row.process ? 12 : 14 }}
          >
            {application.name}
          </Typography>
          <Tooltip
            title={
              row.process
                ? `PID ${row.process.pid} · ${t(row.configured ? 'rules.applicationProxy.temporary' : 'rules.applicationProxy.inherited', { name: row.inheritedFrom })}`
                : application.path
            }
          >
            <Typography
              noWrap
              variant="caption"
              color="text.secondary"
              sx={{
                display: 'block',
                fontSize: row.process ? 10 : undefined,
                lineHeight: row.process ? 1.3 : undefined,
              }}
            >
              {row.process
                ? `PID ${row.process.pid} · ${t(row.configured ? 'rules.applicationProxy.temporary' : 'rules.applicationProxy.inherited', { name: row.inheritedFrom })}`
                : t(
                    application.kind === 'bundle'
                      ? 'rules.applicationProxy.application'
                      : 'rules.applicationProxy.process',
                  )}
            </Typography>
          </Tooltip>
        </Box>
      </Stack>
    </Tooltip>
  )
  const actions = (
    <Stack direction="row" sx={{ alignItems: 'center', flexShrink: 0 }}>
      <Tooltip
        title={
          row.enabled && !applicationMode
            ? t('rules.applicationProxy.pausedHelp')
            : t('rules.applicationProxy.proxy')
        }
      >
        <Switch
          size="small"
          checked={row.enabled}
          disabled={pending}
          onChange={(_, enabled) => onToggle(enabled)}
          slotProps={{
            input: {
              'aria-label': `${application.name} ${t('rules.applicationProxy.proxy')}`,
            },
          }}
        />
      </Tooltip>
      <IconButton
        size="small"
        disabled={pending}
        onClick={(event) => setMenu(event.currentTarget)}
        aria-label={t('rules.applicationProxy.actions')}
      >
        <MoreHorizRounded fontSize="small" />
      </IconButton>
      <Menu anchorEl={menu} open={!!menu} onClose={() => setMenu(null)}>
        <MenuItem
          onClick={() => {
            setMenu(null)
            onViewConnections()
          }}
        >
          {t('rules.applicationProxy.viewConnections')}
        </MenuItem>
        <MenuItem
          disabled={!row.connections.length}
          onClick={() => {
            setMenu(null)
            onDisconnect()
          }}
        >
          {t('rules.applicationProxy.disconnect')}
        </MenuItem>
        <MenuItem
          disabled={!row.configured}
          onClick={() => {
            setMenu(null)
            onReset()
          }}
        >
          {t(
            row.process
              ? 'rules.applicationProxy.followParent'
              : 'rules.applicationProxy.reset',
          )}
        </MenuItem>
      </Menu>
    </Stack>
  )
  const controls = (
    <Box
      sx={{
        display: 'grid',
        gridTemplateColumns: {
          xs: '76px minmax(0, 1fr)',
          sm: '82px minmax(0, 1fr)',
        },
        gap: 0.75,
        minWidth: 0,
      }}
    >
      <TextField
        select
        size="small"
        sx={{
          '& .MuiInputBase-root': {
            height: row.process ? 28 : 32,
            fontSize: row.process ? 11.5 : 12.5,
          },
          '& .MuiSelect-select': { py: 0.5, pl: 1, pr: '24px !important' },
          '& .MuiSelect-icon': { fontSize: 18, right: 4 },
        }}
        value={row.routingMode}
        disabled={pending}
        onChange={(event) =>
          onRoutingMode(event.target.value as IApplicationRoutingMode)
        }
        slotProps={{
          htmlInput: {
            'aria-label': `${application.name} ${t('rules.applicationProxy.routingMode')}`,
          },
        }}
      >
        <MenuItem value="rule">{t('rules.applicationProxy.ruleMode')}</MenuItem>
        <MenuItem value="proxy">
          {t('rules.applicationProxy.proxyMode')}
        </MenuItem>
      </TextField>
      <Autocomplete
        sx={{
          minWidth: 0,
          '& .MuiOutlinedInput-root': {
            minHeight: row.process ? 28 : 32,
            fontSize: row.process ? 11.5 : 12.5,
            padding: row.process ? '3px 8px !important' : '4px 8px !important',
            paddingRight:
              row.proxy && !(row.enabled && row.routingMode === 'proxy')
                ? '58px !important'
                : '32px !important',
          },
          '& .MuiAutocomplete-input': {
            minWidth: 0,
            width: '0 !important',
            flexGrow: 1,
            padding: '0 !important',
          },
          '& .MuiAutocomplete-popupIndicator, & .MuiAutocomplete-clearIndicator':
            { padding: '2px' },
        }}
        size="small"
        options={options}
        value={row.proxy}
        getOptionLabel={applicationOutboundLabel}
        disabled={pending}
        disableClearable={row.enabled && row.routingMode === 'proxy'}
        onChange={(_, value) => onProxy(value)}
        renderOption={({ key, ...props }, option) => (
          <Box component="li" key={key} {...props}>
            <ApplicationProxyOption
              name={option}
              proxyView={proxyView}
              missing={!targets.includes(option)}
            />
          </Box>
        )}
        renderInput={(params) => (
          <TextField
            {...params}
            placeholder={t(
              row.routingMode === 'rule'
                ? 'rules.applicationProxy.followSubscription'
                : 'rules.applicationProxy.chooseOutbound',
            )}
            error={missing}
          />
        )}
      />
    </Box>
  )
  const metrics = (
    <Stack
      direction="row"
      spacing={1}
      sx={{
        alignItems: 'center',
        flexWrap: 'wrap',
        rowGap: 0.5,
        fontVariantNumeric: 'tabular-nums',
      }}
    >
      <Tooltip title={t('rules.applicationProxy.speedHelp')}>
        <Stack direction="row" spacing={1}>
          <Typography
            variant="caption"
            sx={{
              color: 'warning.main',
              fontSize: row.process ? 10 : undefined,
            }}
          >
            ↑ {speed(row.uploadSpeed)}
          </Typography>
          <Typography
            variant="caption"
            sx={{ color: 'info.main', fontSize: row.process ? 10 : undefined }}
          >
            ↓ {speed(row.downloadSpeed)}
          </Typography>
        </Stack>
      </Tooltip>
      <Chip
        size="small"
        variant="outlined"
        onClick={onViewConnections}
        label={t('rules.applicationProxy.connections', {
          count: row.connections.length,
        })}
        sx={{ height: row.process ? 18 : 22, fontSize: row.process ? 10 : 11 }}
      />
      <Tooltip
        title={
          history
            ? `${t('rules.applicationProxy.lastTest')} ${history.time}`
            : status
        }
      >
        <Typography
          variant="caption"
          sx={{
            fontSize: row.process ? 10 : undefined,
            color:
              missing || history?.delay === 0 ? 'error.main' : 'text.secondary',
          }}
        >
          {status}
        </Typography>
      </Tooltip>
    </Stack>
  )
  const route = (
    <Tooltip
      title={
        routes.length
          ? routes.join('; ')
          : t('rules.applicationProxy.noConnections')
      }
    >
      <Typography
        noWrap
        variant="caption"
        color="text.secondary"
        sx={{ display: 'block', mt: 0.25, fontSize: row.process ? 10 : 11 }}
      >
        {routes.length
          ? routes.join('; ')
          : group?.now
            ? `${t('rules.applicationProxy.selectedNode')} ${applicationOutboundLabel(group.now)}`
            : t('rules.applicationProxy.noConnections')}
      </Typography>
    </Tooltip>
  )

  return (
    <Box
      ref={ref}
      sx={{
        opacity: isDragging ? 0.4 : 1,
        border: '1px solid',
        borderColor: 'divider',
        borderRadius: row.process ? 1.5 : 2.5,
        bgcolor: 'background.paper',
        overflow: 'hidden',
        boxShadow: row.process ? undefined : '0 2px 8px rgba(0,0,0,0.03)',
        transition: 'border-color 150ms',
        '&:hover': { borderColor: row.process ? 'divider' : 'primary.main' },
      }}
    >
      {row.process ? (
        <Box
          data-process-row
          sx={{
            p: 0.75,
            ml: Math.min(Math.max((row.depth ?? 1) - 1, 0), 3),
            borderLeft: '2px solid',
            borderColor: 'divider',
            borderBottom: '1px solid',
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) auto',
            alignItems: 'center',
            gap: 0.5,
          }}
        >
          <Box sx={{ minWidth: 0 }}>{identity}</Box>
          <Box
            sx={{
              gridColumn: '1 / -1',
              gridRow: 2,
            }}
          >
            <TextField
              select
              size="small"
              fullWidth
              value={row.profileUid ?? ''}
              disabled={pending}
              onChange={(event) => onSubscription(event.target.value || null)}
              sx={{
                mb: 0.5,
                '& .MuiInputBase-root': { height: 32, fontSize: 12.5 },
              }}
              slotProps={{
                htmlInput: {
                  'aria-label': `${application.name} ${t('rules.applicationProxy.subscription')}`,
                },
                select: { displayEmpty: true },
              }}
            >
              <MenuItem value="">
                {t('rules.applicationProxy.globalSubscription')}
              </MenuItem>
              {row.profileUid &&
                !subscriptions.some((item) => item.uid === row.profileUid) && (
                  <MenuItem value={row.profileUid}>
                    {t('rules.applicationProxy.missingSubscription')}
                  </MenuItem>
                )}
              {subscriptions.map((item) => (
                <MenuItem key={item.uid} value={item.uid}>
                  {item.name}
                </MenuItem>
              ))}
            </TextField>
            {controls}
          </Box>
          {actions}
          <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
            {metrics}
            {routes.length > 0 && route}
          </Box>
        </Box>
      ) : (
        <Box
          sx={{
            p: 1,
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1fr) auto',
            alignItems: 'center',
            columnGap: 1.25,
            rowGap: 0.5,
          }}
        >
          <Box sx={{ minWidth: 0, gridRow: 1 }}>
            {identity}
            {!!processCount && onExpand && (
              <Button
                size="small"
                onClick={onExpand}
                aria-haspopup="dialog"
                startIcon={<ChevronRightRounded />}
                sx={{
                  py: 0,
                  mt: 0.2,
                  ml: 2.5,
                  fontSize: 11,
                  minHeight: 20,
                  '& .MuiButton-startIcon': { mr: 0.25 },
                }}
              >
                {t('rules.applicationProxy.processCount', {
                  count: processCount,
                })}
              </Button>
            )}
          </Box>
          <Box
            sx={{
              minWidth: 0,
              gridColumn: '1 / -1',
              gridRow: 2,
            }}
          >
            <TextField
              select
              size="small"
              fullWidth
              value={row.profileUid ?? ''}
              disabled={pending}
              onChange={(event) => onSubscription(event.target.value || null)}
              sx={{
                mb: 0.5,
                '& .MuiInputBase-root': { height: 32, fontSize: 12.5 },
              }}
              slotProps={{
                htmlInput: {
                  'aria-label': `${application.name} ${t('rules.applicationProxy.subscription')}`,
                },
                select: { displayEmpty: true },
              }}
            >
              <MenuItem value="">
                {t('rules.applicationProxy.globalSubscription')}
              </MenuItem>
              {row.profileUid &&
                !subscriptions.some((item) => item.uid === row.profileUid) && (
                  <MenuItem value={row.profileUid}>
                    {t('rules.applicationProxy.missingSubscription')}
                  </MenuItem>
                )}
              {subscriptions.map((item) => (
                <MenuItem key={item.uid} value={item.uid}>
                  {item.name}
                </MenuItem>
              ))}
            </TextField>
            {controls}
          </Box>
          <Box
            sx={{
              gridColumn: 2,
              gridRow: 1,
            }}
          >
            {actions}
          </Box>
          <Box sx={{ minWidth: 0, gridColumn: '1 / -1' }}>
            {metrics}
            {route}
          </Box>
        </Box>
      )}
    </Box>
  )
}
