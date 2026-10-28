import { useDraggable } from '@dnd-kit/react'
import AppsRounded from '@mui/icons-material/AppsRounded'
import ChevronRightRounded from '@mui/icons-material/ChevronRightRounded'
import DragIndicatorRounded from '@mui/icons-material/DragIndicatorRounded'
import ExpandMoreRounded from '@mui/icons-material/ExpandMoreRounded'
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
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { getProxyApplicationIcon } from '@/services/application-proxy'
import {
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
  onRoutingMode: (mode: IApplicationRoutingMode) => void
  onProxy: (proxy: string | null) => void
  onToggle: (enabled: boolean) => void
  onDisconnect: () => void
  onReset: () => void
  onExpand?: () => void
  expanded?: boolean
  processCount?: number
  children?: ReactNode
}

export function ApplicationRow({
  row,
  targets,
  proxyView,
  pending,
  applicationMode,
  onProxy,
  onRoutingMode,
  onToggle,
  onDisconnect,
  onReset,
  onExpand,
  expanded,
  processCount,
  children,
}: Props) {
  const { t } = useTranslation()
  const [menu, setMenu] = useState<HTMLElement | null>(null)
  const processListRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!expanded) return
    const frame = requestAnimationFrame(() => {
      processListRef.current?.firstElementChild?.scrollIntoView({
        block: 'nearest',
      })
    })
    return () => cancelAnimationFrame(frame)
  }, [expanded])
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
        [...connection.chains].reverse().join(' / '),
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
            width: row.process ? 24 : 36,
            height: row.process ? 24 : 36,
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
              sx={{ display: 'block' }}
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
          '& .MuiInputBase-root': { height: 32, fontSize: 12.5 },
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
            minHeight: 32,
            fontSize: 12.5,
            padding: '4px 8px !important',
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
          <Typography variant="caption" sx={{ color: 'warning.main' }}>
            ↑ {speed(row.uploadSpeed)}
          </Typography>
          <Typography variant="caption" sx={{ color: 'info.main' }}>
            ↓ {speed(row.downloadSpeed)}
          </Typography>
        </Stack>
      </Tooltip>
      <Chip
        size="small"
        variant="outlined"
        label={t('rules.applicationProxy.connections', {
          count: row.connections.length,
        })}
        sx={{ height: 22, fontSize: 11 }}
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
        sx={{ display: 'block', mt: 0.25, fontSize: 11 }}
      >
        {routes.length
          ? routes.join('; ')
          : group?.now
            ? `${t('rules.applicationProxy.selectedNode')} ${group.now}`
            : t('rules.applicationProxy.noConnections')}
      </Typography>
    </Tooltip>
  )

  return (
    <Box
      ref={ref}
      sx={{
        opacity: isDragging ? 0.4 : 1,
        border: row.process ? 0 : '1px solid',
        borderColor: 'divider',
        borderRadius: row.process ? 0 : 2.5,
        bgcolor: row.process ? 'transparent' : 'background.paper',
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
            gridTemplateColumns: {
              xs: 'minmax(0,1fr) auto',
              md: 'minmax(150px,0.8fr) minmax(230px,1.3fr) auto',
            },
            alignItems: 'center',
            gap: 1,
          }}
        >
          <Box sx={{ minWidth: 0 }}>{identity}</Box>
          <Box
            sx={{
              gridColumn: { xs: '1 / -1', md: 'auto' },
              gridRow: { xs: 2, md: 'auto' },
            }}
          >
            {controls}
          </Box>
          {actions}
          <Box sx={{ gridColumn: '1 / -1', minWidth: 0 }}>
            {metrics}
            {route}
          </Box>
        </Box>
      ) : (
        <Box
          sx={{
            p: 1,
            display: 'grid',
            gridTemplateColumns: {
              xs: 'minmax(0, 1fr) auto',
              md: 'minmax(150px, 0.8fr) minmax(260px, 1.6fr) auto',
            },
            alignItems: 'center',
            columnGap: 1.25,
            rowGap: 0.5,
          }}
        >
          <Box sx={{ minWidth: 0, gridRow: { md: '1 / span 2' } }}>
            {identity}
            {!!processCount && onExpand && (
              <Button
                size="small"
                onClick={onExpand}
                aria-expanded={!!expanded}
                startIcon={
                  expanded ? <ExpandMoreRounded /> : <ChevronRightRounded />
                }
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
              gridColumn: { xs: '1 / -1', md: 2 },
              gridRow: { xs: 2, md: 1 },
            }}
          >
            {controls}
          </Box>
          <Box
            sx={{
              gridColumn: { xs: 2, md: 3 },
              gridRow: { xs: 1, md: '1 / span 2' },
            }}
          >
            {actions}
          </Box>
          <Box sx={{ minWidth: 0, gridColumn: { xs: '1 / -1', md: 2 } }}>
            {metrics}
            {route}
          </Box>
        </Box>
      )}
      {children && (
        <Box
          sx={{
            borderTop: '1px solid',
            borderColor: 'divider',
            bgcolor: 'action.hover',
          }}
        >
          <Typography
            variant="caption"
            sx={{ display: 'block', px: 2, py: 0.8, color: 'text.secondary' }}
          >
            {application.name} ·{' '}
            {t('rules.applicationProxy.processCount', { count: processCount })}
          </Typography>
          <Box
            ref={processListRef}
            sx={{ maxHeight: 280, overflowY: 'auto', px: 1, pb: 1 }}
          >
            {children}
          </Box>
        </Box>
      )}
    </Box>
  )
}
