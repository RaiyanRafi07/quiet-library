import React from 'react'
import { addBookmark } from '@/lib/ipc'
import { buttonStyle } from '@/styles'

export default function ActionsBar({
  onBack,
  currentView,
  selection,
  onSwitchView,
}: {
  onBack: () => void
  currentView: 'library' | 'pdf' | 'epub' | 'bookmarks' | 'folders'
  selection?: { path: string; page?: number; section?: string }
  onSwitchView: (view: 'library' | 'bookmarks' | 'folders') => void
}) {
  const handleAddBookmark = async () => {
    if (!selection) return;
    const note = prompt('Add a note to your bookmark:');
    await addBookmark(selection.path, selection.page, selection.section, note || undefined);
  };

  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center', padding: '8px 12px', borderBottom: '1px solid #eee' }}>
      <button onClick={onBack} disabled={currentView === 'library'} style={buttonStyle}>Back</button>
      <button onClick={() => onSwitchView('bookmarks')} disabled={currentView === 'bookmarks'} style={buttonStyle}>Bookmarks</button>
      <div style={{ marginLeft: 'auto', display: 'flex', gap: 8 }}>
        {currentView === 'pdf' || currentView === 'epub' ? (
          <button onClick={handleAddBookmark} style={buttonStyle}>Add bookmark</button>
        ) : null}
      </div>
    </div>
  )
}
