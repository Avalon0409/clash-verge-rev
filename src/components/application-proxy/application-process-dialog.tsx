import AppsRounded from '@mui/icons-material/AppsRounded'
import CloseRounded from '@mui/icons-material/CloseRounded'
import {
  Avatar,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Stack,
  Typography,
} from '@mui/material'
import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseSearchBox, VirtualList } from '@/components/base'
import { getProxyApplicationIcon } from '@/services/application-proxy'
import type { ApplicationRowModel } from '@/services/application-proxy-model'
import { useQuery } from '@/services/query-client'

interface Props {
  application: IProxyApplication | null
  rows: ApplicationRowModel[]
  pending: boolean
  onClose: () => void
  renderRow: (row: ApplicationRowModel) => ReactNode
}

export function ApplicationProcessDialog({
  application,
  rows,
  pending,
  onClose,
  renderRow,
}: Props) {
  const { t } = useTranslation()
  const [search, setSearch] = useState('')
  const [match, setMatch] = useState(() => (_: string) => true)
  const { data: icon } = useQuery({
    queryKey: ['application-icon', application?.path],
    queryFn: () => getProxyApplicationIcon(application!.path),
    enabled: !!application,
    staleTime: Infinity,
    retry: false,
  })
  const filtered = rows.filter((row) =>
    match(
      `${row.application.name}\n${row.application.path}\n${row.process?.pid ?? ''}\n${row.inheritedFrom ?? ''}`,
    ),
  )
  const connected = rows.filter((row) => row.connections.length > 0).length
  const overrides = rows.filter((row) => row.configured).length

  return (
    <Dialog
      open={!!application}
      onClose={onClose}
      fullWidth
      maxWidth="md"
      slotProps={{ paper: { sx: { borderRadius: 3 } } }}
    >
      <DialogTitle component="div" sx={{ px: 2.5, pt: 2, pb: 1 }}>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: 'center' }}>
          <Avatar
            src={icon ?? undefined}
            variant="rounded"
            sx={{ width: 40, height: 40, bgcolor: 'transparent' }}
          >
            <AppsRounded color="disabled" />
          </Avatar>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography
              noWrap
              variant="h6"
              sx={{ fontSize: 17, fontWeight: 600 }}
            >
              {application?.name}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {t('rules.applicationProxy.processDialogHint')}
            </Typography>
          </Box>
          <IconButton
            size="small"
            disabled={pending}
            onClick={onClose}
            aria-label={t('rules.applicationProxy.processDialogClose')}
          >
            <CloseRounded fontSize="small" />
          </IconButton>
        </Stack>
        <Stack
          direction="row"
          spacing={1}
          sx={{ mt: 1.5, flexWrap: 'wrap', rowGap: 0.5 }}
        >
          <Chip
            size="small"
            label={t('rules.applicationProxy.processCount', {
              count: rows.length,
            })}
          />
          <Chip
            size="small"
            color="success"
            variant="outlined"
            label={t('rules.applicationProxy.connectedProcesses', {
              count: connected,
            })}
          />
          <Chip
            size="small"
            color={overrides ? 'warning' : 'default'}
            variant="outlined"
            label={t('rules.applicationProxy.temporaryOverrides', {
              count: overrides,
            })}
          />
        </Stack>
      </DialogTitle>
      <Box sx={{ px: 2.5, pt: 0.5, pb: 1.5 }}>
        <BaseSearchBox
          value={search}
          placeholder={t('rules.applicationProxy.processSearch')}
          onSearch={(matcher, state) => {
            setSearch(state.text)
            setMatch(() => matcher)
          }}
        />
      </Box>
      <DialogContent dividers sx={{ p: 1.5, bgcolor: 'background.default' }}>
        <Box
          sx={{
            display: { xs: 'none', md: 'grid' },
            gridTemplateColumns: 'minmax(150px,0.8fr) minmax(230px,1.3fr) 70px',
            gap: 1,
            px: 1,
            pb: 1,
            color: 'text.secondary',
          }}
        >
          <Typography variant="caption">
            {t('rules.applicationProxy.process')} / PID
          </Typography>
          <Typography variant="caption">
            {t('rules.applicationProxy.outbound')}
          </Typography>
          <Typography variant="caption" sx={{ textAlign: 'right' }}>
            {t('rules.applicationProxy.switchColumn')}
          </Typography>
        </Box>
        {filtered.length ? (
          <VirtualList
            count={filtered.length}
            estimateSize={78}
            getItemKey={(index) =>
              `process:${filtered[index].process!.pid}:${filtered[index].process!.start_time}`
            }
            style={{
              height: Math.min(420, filtered.length * 78),
              maxHeight: '55vh',
            }}
            renderItem={(index) => (
              <Box sx={{ pb: 0.8 }}>{renderRow(filtered[index])}</Box>
            )}
          />
        ) : (
          <Box sx={{ py: 6, textAlign: 'center' }}>
            <Typography variant="body2" color="text.secondary">
              {t(
                rows.length
                  ? 'rules.applicationProxy.noMatchingProcesses'
                  : 'rules.applicationProxy.noProcesses',
              )}
            </Typography>
          </Box>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 2.5, py: 1.25 }}>
        <Button disabled={pending} onClick={onClose}>
          {t('rules.applicationProxy.processDialogClose')}
        </Button>
      </DialogActions>
    </Dialog>
  )
}
