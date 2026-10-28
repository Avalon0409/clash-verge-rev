import { useDroppable } from '@dnd-kit/react'
import { Paper, type PaperProps } from '@mui/material'

interface Props extends PaperProps {
  active: boolean
  disabled: boolean
}

export function ApplicationDropZone({ active, disabled, sx, ...props }: Props) {
  const { ref, isDropTarget } = useDroppable({
    id: active ? 'enabled' : 'disabled',
    disabled,
  })

  return (
    <Paper
      {...props}
      ref={ref}
      variant="outlined"
      sx={[
        {
          borderColor: isDropTarget
            ? 'primary.main'
            : active
              ? 'success.main'
              : 'divider',
          bgcolor: isDropTarget ? 'action.hover' : 'background.paper',
          boxShadow: isDropTarget ? 3 : undefined,
        },
        ...(Array.isArray(sx) ? sx : [sx]),
      ]}
    />
  )
}
