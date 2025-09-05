import React, { useEffect } from 'react'
import Card from '@/ui/Card'

type Props = {
  open: boolean
  title?: string
  onClose: () => void
  children: React.ReactNode
}

export default function Modal({ open, title, onClose, children }: Props) {
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  if (!open) return null
  return (
    <div style={{ position: 'fixed', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(0,0,0,0.35)', zIndex: 1000 }} onMouseDown={onClose}>
      <div onMouseDown={(e) => e.stopPropagation()} style={{ minWidth: 360, maxWidth: 560 }}>
        <Card>
          {title ? <div className="nb-heading nb-h2" style={{ marginBottom: 'var(--sp-3)' }}>{title}</div> : null}
          {children}
        </Card>
      </div>
    </div>
  )
}

