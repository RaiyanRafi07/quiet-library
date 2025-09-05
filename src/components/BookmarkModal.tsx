import React, { useEffect, useRef, useState } from 'react'
import Modal from '@/ui/Modal'
import Button from '@/ui/Button'
import { inputStyle } from '@/styles'

type Selection = { path: string; page?: number; section?: string }

export default function BookmarkModal({
  open,
  selection,
  onCancel,
  onSave,
}: {
  open: boolean
  selection: Selection
  onCancel: () => void
  onSave: (note?: string) => Promise<void> | void
}) {
  const [note, setNote] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (open) {
      setNote('')
      setTimeout(() => inputRef.current?.focus(), 0)
    }
  }, [open, selection?.path])

  return (
    <Modal open={open} onClose={onCancel} title="Add bookmark">
      <div className="nb-meta" style={{ marginBottom: 'var(--sp-2)' }}>{selection.path}</div>
      <input
        ref={inputRef}
        placeholder="Optional note"
        value={note}
        onChange={(e) => setNote(e.target.value)}
        style={{ ...inputStyle, marginBottom: 'var(--sp-3)' }}
      />
      <div style={{ display: 'flex', gap: 'var(--sp-2)', justifyContent: 'flex-end' }}>
        <Button onClick={onCancel}>Cancel</Button>
        <Button onClick={() => onSave(note || undefined)}>Save</Button>
      </div>
    </Modal>
  )
}

