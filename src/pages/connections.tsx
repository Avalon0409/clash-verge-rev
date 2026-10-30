import {
  DeleteForeverRounded,
  TableChartRounded,
  TableRowsRounded,
  ViewColumnRounded,
} from '@mui/icons-material'
import {
  Box,
  Button,
  ButtonGroup,
  Chip,
  Fab,
  IconButton,
  MenuItem,
  Tooltip,
  Typography,
  Zoom,
} from '@mui/material'
import { useLockFn } from 'ahooks'
import { useCallback, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { closeAllConnections } from 'tauri-plugin-mihomo-api'

import {
  BaseEmpty,
  BasePage,
  BaseSearchBox,
  BaseStyledSelect,
  type SearchState,
  VirtualList,
} from '@/components/base'
import {
  ConnectionDetail,
  ConnectionDetailRef,
} from '@/components/connection/connection-detail'
import { ConnectionRowItem } from '@/components/connection/connection-row-item'
import {
  getConnectionStartTime,
  useConnectionRowViews,
} from '@/components/connection/connection-row-view'
import { ConnectionTable } from '@/components/connection/connection-table'
import { useConnectionData } from '@/hooks/use-connection-data'
import { useConnectionSetting } from '@/hooks/use-connection-setting'
import { useTrafficData } from '@/hooks/use-traffic-data'
import { useVisibility } from '@/hooks/use-visibility'
import {
  APPLICATION_CONNECTION_PARAMS,
  createApplicationConnectionMatcher,
  readApplicationConnectionFilter,
} from '@/services/application-connections'
import {
  disconnectApplicationConnections,
  getApplicationProcesses,
} from '@/services/application-proxy'
import { mutate } from '@/services/mutate'
import { useQuery } from '@/services/query-client'
import parseTraffic from '@/utils/parse-traffic'

type OrderFunc = (list: IConnectionsItem[]) => IConnectionsItem[]

const ORDER_OPTIONS = [
  {
    id: 'default',
    labelKey: 'connections.components.order.default',
    fn: (list: IConnectionsItem[]) =>
      list.sort(
        (a, b) => getConnectionStartTime(b) - getConnectionStartTime(a),
      ),
  },
  {
    id: 'uploadSpeed',
    labelKey: 'connections.components.order.uploadSpeed',
    fn: (list: IConnectionsItem[]) =>
      list.sort((a, b) => (b.curUpload ?? 0) - (a.curUpload ?? 0)),
  },
  {
    id: 'downloadSpeed',
    labelKey: 'connections.components.order.downloadSpeed',
    fn: (list: IConnectionsItem[]) =>
      list.sort((a, b) => (b.curDownload ?? 0) - (a.curDownload ?? 0)),
  },
] as const

type OrderKey = (typeof ORDER_OPTIONS)[number]['id']

const orderFunctionMap = ORDER_OPTIONS.reduce<Record<OrderKey, OrderFunc>>(
  (acc, option) => {
    acc[option.id] = option.fn
    return acc
  },
  {} as Record<OrderKey, OrderFunc>,
)

const EMPTY_CONNECTIONS: IConnectionsItem[] = []
const ConnectionsPage = () => {
  const { t } = useTranslation()
  const pageVisible = useVisibility()
  const [searchParams, setSearchParams] = useSearchParams()
  const applicationFilter = useMemo(
    () => readApplicationConnectionFilter(searchParams),
    [searchParams],
  )
  const { data: processSnapshot } = useQuery({
    queryKey: ['getApplicationProcesses'],
    queryFn: getApplicationProcesses,
    enabled: pageVisible && applicationFilter?.kind === 'application',
    refetchInterval: 2000,
    retry: false,
  })
  const appMatcher = useMemo(
    () =>
      applicationFilter
        ? createApplicationConnectionMatcher(
            applicationFilter,
            processSnapshot?.processes,
          )
        : null,
    [applicationFilter, processSnapshot?.processes],
  )
  const clearApplicationFilter = () =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous)
        for (const key of APPLICATION_CONNECTION_PARAMS) next.delete(key)
        return next
      },
      { replace: true },
    )
  const [match, setMatch] = useState<(input: string) => boolean>(
    () => () => true,
  )
  const [hasSearch, setHasSearch] = useState(false)
  const [curOrderOpt, setCurOrderOpt] = useState<OrderKey>('default')
  const [connectionsType, setConnectionsType] = useState<'active' | 'closed'>(
    'active',
  )

  const {
    response: { data: connections },
    clearClosedConnections,
  } = useConnectionData({ enabled: pageVisible })
  const {
    response: { data: traffic },
  } = useTrafficData({ enabled: pageVisible })

  const [setting, setSetting] = useConnectionSetting()

  const isTableLayout = setting.layout === 'table'

  const [isColumnManagerOpen, setIsColumnManagerOpen] = useState(false)

  const scopedConnections = useMemo(() => {
    const active = connections?.activeConnections ?? EMPTY_CONNECTIONS
    const closed = connections?.closedConnections ?? EMPTY_CONNECTIONS
    return appMatcher
      ? { active: active.filter(appMatcher), closed: closed.filter(appMatcher) }
      : { active, closed }
  }, [
    connections?.activeConnections,
    connections?.closedConnections,
    appMatcher,
  ])
  const selectedConnections =
    connectionsType === 'active'
      ? scopedConnections.active
      : scopedConnections.closed

  const filterConn = useMemo(() => {
    const orderFunc = orderFunctionMap[curOrderOpt]

    if (isTableLayout && !hasSearch) return selectedConnections
    if (!hasSearch) return orderFunc([...selectedConnections])

    const matchConns = selectedConnections.filter((conn) => {
      const { host, destinationIP, process } = conn.metadata
      return (
        match(host || '') || match(destinationIP || '') || match(process || '')
      )
    })

    return orderFunc ? orderFunc(matchConns) : matchConns
  }, [selectedConnections, isTableLayout, hasSearch, match, curOrderOpt])

  const displayRows = useConnectionRowViews(
    isTableLayout ? EMPTY_CONNECTIONS : filterConn,
  )

  const detailRef = useRef<ConnectionDetailRef>(null!)

  const selectConnectionsType = useCallback(
    (type: 'active' | 'closed') => {
      if (type === connectionsType) return
      detailRef.current?.close()
      setIsColumnManagerOpen(false)
      setConnectionsType(type)
    },
    [connectionsType],
  )

  const showDetailById = useCallback(
    (id: string) => {
      const connection = filterConn.find((item) => item.id === id)
      if (connection) {
        detailRef.current?.open(connection, connectionsType === 'closed')
      }
    },
    [connectionsType, filterConn],
  )

  const onCloseAll = useLockFn(async () => {
    if (applicationFilter) await disconnectApplicationConnections(filterConn)
    else
      await mutate(() => closeAllConnections(), {
        id: 'close-all-connections',
        errorNotice: false,
      })
  })

  const handleSearch = useCallback(
    (match: (content: string) => boolean, state: SearchState) => {
      setMatch(() => match)
      setHasSearch(state.text.length > 0)
    },
    [],
  )
  const hasTableData = filterConn.length > 0
  const displayedTraffic = useMemo(
    () =>
      filterConn.reduce(
        (total, connection) => ({
          up: total.up + connection.upload,
          down: total.down + connection.download,
        }),
        { up: 0, down: 0 },
      ),
    [filterConn],
  )

  return (
    <BasePage
      full
      title={
        <span style={{ whiteSpace: 'nowrap' }}>
          {t('connections.page.title')}
        </span>
      }
      contentStyle={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        borderRadius: '8px',
        minHeight: 0,
      }}
      header={
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <Box sx={{ mx: 1 }}>
            {t(
              applicationFilter
                ? 'connections.page.visibleDownloaded'
                : 'shared.labels.downloaded',
            )}
            :{' '}
            {parseTraffic(
              applicationFilter
                ? displayedTraffic.down
                : traffic?.downTotal || 0,
            )}
          </Box>
          <Box sx={{ mx: 1 }}>
            {t(
              applicationFilter
                ? 'connections.page.visibleUploaded'
                : 'shared.labels.uploaded',
            )}
            :{' '}
            {parseTraffic(
              applicationFilter ? displayedTraffic.up : traffic?.upTotal || 0,
            )}
          </Box>
          <IconButton
            color="inherit"
            size="small"
            onClick={() =>
              setSetting((o) =>
                o?.layout !== 'table'
                  ? { ...o, layout: 'table' }
                  : { ...o, layout: 'list' },
              )
            }
          >
            {isTableLayout ? (
              <TableRowsRounded titleAccess={t('shared.actions.listView')} />
            ) : (
              <TableChartRounded titleAccess={t('shared.actions.tableView')} />
            )}
          </IconButton>
          <Button
            size="small"
            variant="contained"
            onClick={onCloseAll}
            disabled={
              !!applicationFilter &&
              (connectionsType !== 'active' || !filterConn.length)
            }
          >
            <span style={{ whiteSpace: 'nowrap' }}>
              {t(
                applicationFilter
                  ? 'connections.page.closeFiltered'
                  : 'shared.actions.closeAll',
              )}
            </span>
          </Button>
        </Box>
      }
    >
      {applicationFilter && (
        <Box
          sx={{
            mx: '10px',
            mt: 1,
            display: 'flex',
            alignItems: 'center',
            gap: 1,
          }}
        >
          <Chip
            size="small"
            color="primary"
            variant="outlined"
            onDelete={clearApplicationFilter}
            label={
              applicationFilter.kind === 'application'
                ? applicationFilter.application.name
                : `${applicationFilter.name} · PID ${applicationFilter.pid}`
            }
            sx={{ maxWidth: 360 }}
          />
          <Typography variant="caption" color="text.secondary">
            {t(
              applicationFilter.kind === 'application'
                ? 'connections.page.includingChildren'
                : 'connections.page.processInstanceOnly',
            )}
          </Typography>
        </Box>
      )}
      <Box
        sx={{
          pt: 1,
          mb: 0.5,
          mx: '10px',
          minHeight: '36px',
          display: 'flex',
          alignItems: 'center',
          gap: 1,
          userSelect: 'text',
          position: 'sticky',
          top: 0,
          zIndex: 2,
        }}
      >
        <ButtonGroup sx={{ mr: 1, flexBasis: 'content' }}>
          <Button
            size="small"
            variant={connectionsType === 'active' ? 'contained' : 'outlined'}
            onClick={() => selectConnectionsType('active')}
          >
            {t('connections.components.actions.active')}{' '}
            {scopedConnections.active.length}
          </Button>
          <Button
            size="small"
            variant={connectionsType === 'closed' ? 'contained' : 'outlined'}
            onClick={() => selectConnectionsType('closed')}
          >
            {t('connections.components.actions.closed')}{' '}
            {scopedConnections.closed.length}
          </Button>
        </ButtonGroup>
        {!isTableLayout && (
          <BaseStyledSelect
            value={curOrderOpt}
            onChange={(e) => setCurOrderOpt(e.target.value as OrderKey)}
          >
            {ORDER_OPTIONS.map((option) => (
              <MenuItem key={option.id} value={option.id}>
                <span style={{ fontSize: 14 }}>{t(option.labelKey)}</span>
              </MenuItem>
            ))}
          </BaseStyledSelect>
        )}
        <Box
          sx={{
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            '& > *': {
              flex: 1,
            },
          }}
        >
          <BaseSearchBox onSearch={handleSearch} />
        </Box>
        {isTableLayout && hasTableData && (
          <Tooltip title={t('connections.components.columnManager.title')}>
            <IconButton
              size="small"
              aria-label={t('connections.components.columnManager.title')}
              onClick={() => setIsColumnManagerOpen(true)}
              sx={{ flex: '0 0 auto' }}
            >
              <ViewColumnRounded fontSize="small" />
            </IconButton>
          </Tooltip>
        )}
      </Box>

      {!hasTableData ? (
        <BaseEmpty />
      ) : isTableLayout ? (
        <ConnectionTable
          connections={filterConn}
          onShowDetail={showDetailById}
          columnManagerOpen={isColumnManagerOpen}
          onCloseColumnManager={() => setIsColumnManagerOpen(false)}
        />
      ) : (
        <VirtualList
          key={connectionsType}
          count={displayRows.length}
          estimateSize={56}
          renderItem={(i) => (
            <ConnectionRowItem
              row={displayRows[i]}
              closed={connectionsType === 'closed'}
              onShowDetail={showDetailById}
            />
          )}
          style={{
            flex: 1,
            borderRadius: '8px',
            WebkitOverflowScrolling: 'touch',
            overscrollBehavior: 'contain',
          }}
        />
      )}
      <ConnectionDetail ref={detailRef} />
      <Zoom
        in={
          !applicationFilter &&
          connectionsType === 'closed' &&
          filterConn.length > 0
        }
        unmountOnExit
      >
        <Fab
          size="medium"
          variant="extended"
          sx={{
            position: 'absolute',
            right: 16,
            bottom: isTableLayout ? 70 : 16,
          }}
          color="primary"
          onClick={() => clearClosedConnections()}
        >
          <DeleteForeverRounded sx={{ mr: 1 }} fontSize="small" />
          {t('shared.actions.clear')}
        </Fab>
      </Zoom>
    </BasePage>
  )
}

export default ConnectionsPage
