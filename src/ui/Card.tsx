import React from 'react'
import { cardStyle } from '@/styles'

type Props = React.HTMLAttributes<HTMLDivElement>

export default function Card({ style, children, ...rest }: Props) {
  return (
    <div {...rest} style={{ ...cardStyle, ...(style || {}) }}>
      {children}
    </div>
  )
}

