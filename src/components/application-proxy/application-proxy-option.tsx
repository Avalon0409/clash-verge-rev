import { Box, Typography } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { getApplicationProxyHistory } from '@/services/application-proxy-model'
import type { ProxyViewV1 } from '@/types/proxy-view'

interface Props {
  name: string
  proxyView?: ProxyViewV1
  missing?: boolean
}

export function ApplicationProxyOption({
  name,
  proxyView,
  missing = false,
}: Props) {
  const { t } = useTranslation()
  const history = getApplicationProxyHistory(proxyView, name)
  const delay = missing
    ? t('rules.applicationProxy.missing')
    : history
      ? history.delay > 0
        ? `${history.delay} ms`
        : t('rules.applicationProxy.testFailed')
      : t('rules.applicationProxy.untested')

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1.5,
        width: '100%',
        minWidth: 0,
      }}
    >
      <Typography
        variant="body2"
        sx={{ flex: 1, overflowWrap: 'anywhere', fontSize: 12.5 }}
      >
        {name}
      </Typography>
      <Typography
        variant="caption"
        sx={{
          flexShrink: 0,
          fontVariantNumeric: 'tabular-nums',
          color:
            missing || history?.delay === 0 ? 'error.main' : 'text.secondary',
        }}
      >
        {delay}
      </Typography>
    </Box>
  )
}
