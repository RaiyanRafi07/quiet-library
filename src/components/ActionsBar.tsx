import React, { useState } from 'react'
import { addBookmark } from '@/lib/ipc'
import Button from '@/ui/Button'
import Toolbar from '@/ui/Toolbar'
import BookmarkModal from '@/components/BookmarkModal'
import type { View } from '@/types/view'

export default function ActionsBar({
  onBack,
  currentView,
  selection,
  onSwitchView,
}: {
  onBack: () => void
  currentView: View
  selection?: { path: string; page?: number; section?: string }
  onSwitchView: (view: Extract<View, 'library' | 'bookmarks' | 'folders'>) => void
}) {
  const [open, setOpen] = useState(false)
  const handleAddBookmark = () => {
    if (!selection) return
    setOpen(true)
  }

  return (
    <>
      <Toolbar>
        <Button onClick={onBack} disabled={currentView === 'library'}>Back</Button>
        <Button onClick={() => onSwitchView('bookmarks')} disabled={currentView === 'bookmarks'}>Bookmarks</Button>
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 8 }}>
          {currentView === 'pdf' || currentView === 'epub' ? (
            <Button onClick={handleAddBookmark} disabled={!selection}>Add bookmark</Button>
          ) : null}
        </div>
      </Toolbar>
      {selection && (
        <BookmarkModal
          open={open}
          selection={selection}
          onCancel={() => setOpen(false)}
          onSave={async (note?: string) => {
            await addBookmark(selection.path, selection.page, selection.section, note)
            setOpen(false)
          }}
        />
      )}
    </>
  )
}
