import { Box } from '@mui/material'
import { useEffect, useRef, useState, type ReactNode } from 'react'

import { VirtualList } from '@/components/base'
import type { ApplicationRowModel } from '@/services/application-proxy-model'

interface Props {
  items: ApplicationRowModel[]
  renderCard: (row: ApplicationRowModel) => ReactNode
  onFirstRowHeight?: (height: number) => void
}

export function ApplicationCardList({
  items,
  renderCard,
  onFirstRowHeight,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [columns, setColumns] = useState(1)
  const [firstRow, setFirstRow] = useState<HTMLDivElement | null>(null)

  useEffect(() => {
    const element = containerRef.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => {
      setColumns(entry.contentRect.width >= 640 ? 2 : 1)
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!firstRow || !onFirstRowHeight) return
    const measure = () =>
      onFirstRowHeight(Math.ceil(firstRow.getBoundingClientRect().height))
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(firstRow)
    return () => observer.disconnect()
  }, [firstRow, onFirstRowHeight])

  return (
    <Box ref={containerRef} sx={{ flex: 1, minHeight: 0, display: 'flex' }}>
      <VirtualList
        count={Math.ceil(items.length / columns)}
        estimateSize={152}
        getItemKey={(index) =>
          JSON.stringify(
            items
              .slice(index * columns, (index + 1) * columns)
              .map((row) => row.application.path),
          )
        }
        style={{
          flex: 1,
          minHeight: 0,
          padding: '8px 12px 0',
          boxSizing: 'border-box',
        }}
        renderItem={(index) => (
          <Box
            ref={index === 0 ? setFirstRow : undefined}
            sx={{
              display: 'grid',
              gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
              alignItems: 'start',
              gap: 1,
              pb: 1,
            }}
          >
            {items.slice(index * columns, (index + 1) * columns).map((row) => (
              <Box key={row.application.path} sx={{ minWidth: 0 }}>
                {renderCard(row)}
              </Box>
            ))}
          </Box>
        )}
      />
    </Box>
  )
}
