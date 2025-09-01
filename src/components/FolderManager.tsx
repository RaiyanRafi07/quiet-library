import { addWatchedFolder, removeWatchedFolder } from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/api/dialog';
import React from 'react';
import { buttonStyle } from '@/styles';

export default function FolderManager({ folders, setFolders }: { folders: string[], setFolders: (folders: string[]) => void }) {

  const handleAddFolder = async () => {
    const picked = await openDialog({ directory: true, multiple: false });
    if (typeof picked === 'string') {
      await addWatchedFolder(picked);
      setFolders([...folders, picked]);
    }
  };

  const handleRemoveFolder = async (folder: string) => {
    await removeWatchedFolder(folder);
    setFolders(folders.filter(f => f !== folder));
  };

  return (
    <div style={{ padding: 12 }}>
      <h2>Watched Folders</h2>
      <button onClick={handleAddFolder} style={buttonStyle}>Add folder</button>
      {folders.length === 0 && <p>No watched folders yet.</p>}
      <ul style={{ listStyle: 'none', padding: 0 }}>
        {folders.map(f => (
          <li key={f} style={{ display: 'flex', alignItems: 'center', marginBottom: 8, padding: 8, border: '1px solid #eee', borderRadius: 4 }}>
            <span style={{ flex: 1 }}>{f}</span>
            <button onClick={() => handleRemoveFolder(f)} style={buttonStyle}>Remove</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
