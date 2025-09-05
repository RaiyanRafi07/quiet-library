import { listBookmarks, removeBookmark, type Bookmark } from '@/lib/ipc';
import React, { useEffect, useState } from 'react';
import Button from '@/ui/Button'
import Card from '@/ui/Card'
import { pageStyle } from '@/styles'

export default function BookmarksList({ onOpen }: { onOpen: (sel: { path: string; page?: number; section?: string }) => void }) {
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);

  useEffect(() => {
    listBookmarks().then(setBookmarks);
  }, []);

  const handleRemove = async (id: string) => {
    await removeBookmark(id);
    setBookmarks(bookmarks.filter(b => b.id !== id));
  };

  return (
    <div style={pageStyle}>
      <Card style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div className="nb-heading nb-h2">Bookmarks</div>
        <div className="nb-meta">{bookmarks.length} saved</div>
      </Card>

      {bookmarks.length === 0 && (
        <Card style={{ color: 'var(--nb-card-text)' }}>
          <div className="nb-heading nb-h2" style={{ marginBottom: 'var(--sp-2)' }}>No bookmarks yet</div>
          <div className="nb-meta">Add one from the reader using “Add bookmark”.</div>
        </Card>
      )}

      <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
        {bookmarks.map(b => {
          const name = b.path.split(/[/\\]/).pop()
          return (
            <li key={b.id}>
              <Card>
                <div className="nb-heading nb-h2" style={{ marginBottom: 'var(--sp-2)' }}>{name}</div>
                <div className="nb-meta" style={{ marginBottom: 'var(--sp-1)', color: 'var(--nb-muted)' }}>{b.path}</div>
                <div className="nb-meta" style={{ marginBottom: b.note ? 'var(--sp-2)' : 0 }}>
                  {b.page ? `Page ${b.page}` : ''}{b.page && b.section ? ' · ' : ''}{b.section ? `Section ${b.section}` : ''}
                </div>
                {b.note && <div style={{ fontSize: 'var(--fs-sm)', marginBottom: 'var(--sp-2)' }}>{b.note}</div>}
                <div style={{ display: 'flex', gap: 'var(--sp-2)', justifyContent: 'flex-end' }}>
                  <Button onClick={() => onOpen({ path: b.path, page: b.page, section: b.section })}>Open</Button>
                  <Button onClick={() => handleRemove(b.id)}>Remove</Button>
                </div>
              </Card>
            </li>
          )
        })}
      </ul>
    </div>
  );
}
