import React from 'react'
import { buttonStyle } from '@/styles'

type Props = React.ButtonHTMLAttributes<HTMLButtonElement>

export default function Button({ style, children, className, ...rest }: Props) {
  return (
    <button {...rest} className={['nb-button', className].filter(Boolean).join(' ')} style={{ ...buttonStyle, ...(style || {}) }}>
      {children}
    </button>
  )
}
