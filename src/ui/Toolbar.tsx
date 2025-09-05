import React from 'react'
import { toolbarStyle } from '@/styles'

type Props = React.HTMLAttributes<HTMLDivElement>

export default function Toolbar({ style, children, ...rest }: Props) {
  return (
    <div {...rest} style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', padding: 'var(--sp-3) var(--sp-5)', color: 'var(--nb-card-text)', ...toolbarStyle, ...(style || {}) }}>
      {children}
    </div>
  )
}
