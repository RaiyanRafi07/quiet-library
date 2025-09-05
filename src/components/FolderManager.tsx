import { addWatchedFolder, removeWatchedFolder } from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/api/dialog';
import React from 'react';
import Button from '@/ui/Button'
import Card from '@/ui/Card'

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
      <h2 className="nb-heading nb-h2">Watched Folders</h2>
      <Button onClick={handleAddFolder}>Add folder</Button>
      {folders.length === 0 && <p>No watched folders yet.</p>}
      <ul style={{ listStyle: 'none', padding: 0 }}>
        {folders.map(f => (
          <li key={f} style={{ marginBottom: 'var(--sp-3)' }}>
            <Card style={{ display: 'flex', alignItems: 'center' }}>
              <span style={{ flex: 1, fontSize: 'var(--fs-sm)' }}>{f}</span>
              <Button onClick={() => handleRemoveFolder(f)}>Remove</Button>
            </Card>
          </li>
        ))}
      </ul>
    </div>
  );
}
