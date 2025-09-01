import { listBookmarks, removeBookmark, type Bookmark } from '@/lib/ipc';
import { buttonStyle } from '@/styles';
import React, { useEffect, useState } from 'react';

export default function BookmarksList() {
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);

  useEffect(() => {
    listBookmarks().then(setBookmarks);
  }, []);

  const handleRemove = async (id: string) => {
    await removeBookmark(id);
    setBookmarks(bookmarks.filter(b => b.id !== id));
  };

  return (
    <div style={{ padding: 12 }}>
      <h2>Bookmarks</h2>
      {bookmarks.length === 0 && <p>No bookmarks yet.</p>}
      <ul style={{ listStyle: 'none', padding: 0 }}>
        {bookmarks.map(b => (
          <li key={b.id} style={{ marginBottom: 8, padding: 8, border: '1px solid #eee', borderRadius: 4 }}>
            <div><strong>{b.path}</strong></div>
            {b.page && <div>Page: {b.page}</div>}
            {b.section && <div>Section: {b.section}</div>}
            {b.note && <p>{b.note}</p>}
            <button onClick={() => handleRemove(b.id)} style={buttonStyle}>Remove</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
