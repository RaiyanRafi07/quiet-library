import React, { useEffect, useState } from 'react'
import SearchBar from './components/SearchBar'
import ResultsList from './components/ResultsList'
import ReaderPDF from './components/ReaderPDF'
import ReaderEPUB from './components/ReaderEPUB'
import ReaderText from './components/ReaderText'
import ActionsBar from './components/ActionsBar'
import BookmarksList from './components/BookmarksList'
import FolderManager from './components/FolderManager'
import { addWatchedFolder, listWatchedFolders, removeWatchedFolder, reindexAll, search, clearExtractCache, type SearchResult } from './lib/ipc'
import { open as openDialog } from '@tauri-apps/api/dialog'
import { buttonStyle } from './styles'

type View = 'library' | 'pdf' | 'epub' | 'bookmarks' | 'folders' | 'text'

export default function App() {
  const [view, setView] = useState<View>('library')
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SearchResult[]>([])
  const [debounced, setDebounced] = useState('')
  const [searching, setSearching] = useState(false)
  const [folders, setFolders] = useState<string[]>([])
  const [openTarget, setOpenTarget] = useState<{ path: string; page?: number; section?: string } | null>(null)
  const [indexing, setIndexing] = useState(false)
  const [clearing, setClearing] = useState(false)

  useEffect(() => {
    listWatchedFolders().then(setFolders).catch(() => setFolders([]))
  }, [])

  // Debounce query input to reduce backend calls
  useEffect(() => {
    const id = setTimeout(() => setDebounced(query), 250)
    return () => clearTimeout(id)
  }, [query])

  useEffect(() => {
    let cancelled = false
    const run = async () => {
      if (!debounced.trim()) { setResults([]); return }
      setSearching(true)
      try {
        const r = await search(debounced, 50)
        if (!cancelled) setResults(r)
      } finally {
        if (!cancelled) setSearching(false)
      }
    }
    run()
    return () => { cancelled = true }
  }, [debounced])

  const handleOpen = async (r: SearchResult) => {
    const ext = r.path.toLowerCase().split('.').pop()
    setOpenTarget({ path: r.path, page: r.page, section: r.section })
    if (ext === 'pdf') {
      setView('pdf')
    } else if (ext === 'epub') {
      setView('epub')
    } else if (ext === 'txt' || ext === 'md' || ext === 'markdown' || ext === 'html' || ext === 'htm') {
      setView('text')
    } else {
      alert(`Unsupported file type: ${ext}`)
    }
  }

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <ActionsBar
        onBack={() => setView('library')}
        currentView={view}
        selection={openTarget ?? undefined}
        onSwitchView={setView}
      />
      {view === 'library' && (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}>
          <SearchBar value={query} onChange={setQuery} />
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: '#666' }}>
            <div>{searching ? 'Searching…' : `${results.length} result${results.length === 1 ? '' : 's'}`}</div>
          </div>
          <ResultsList results={results} onOpen={handleOpen} />
          <div style={{ marginTop: 'auto', display: 'flex', alignItems: 'center', gap: 8 }}>
            <div style={{ fontSize: 12, opacity: 0.7, flex: 1 }}>
              {indexing ? 'Indexing…' : `Watched folders: ${folders.length ? folders.join(' · ') : 'none'}`}
            </div>
            <button onClick={() => setView('folders')} style={buttonStyle}>Manage Folders</button>
            <button
              onClick={async () => {
                setClearing(true)
                try {
                  await clearExtractCache()
                } finally {
                  setClearing(false)
                }
              }}
              disabled={clearing || indexing}
              style={buttonStyle}
            >Clear cache</button>
            <button
              onClick={async () => {
                setIndexing(true)
                try {
                  await reindexAll()
                  if (debounced.trim()) {
                    const r = await search(debounced, 50)
                    setResults(r)
                  }
                } finally {
                  setIndexing(false)
                }
              }}
              disabled={indexing}
              style={buttonStyle}
            >Reindex</button>
          </div>
        </div>
      )}
      {view === 'bookmarks' && <BookmarksList />}
      {view === 'folders' && <FolderManager folders={folders} setFolders={setFolders} />}
      {view === 'pdf' && openTarget && <ReaderPDF target={openTarget} query={query} />}
      {view === 'epub' && openTarget && <ReaderEPUB target={openTarget} query={query} />}
      {view === 'text' && openTarget && <ReaderText target={openTarget} query={query} />}
    </div>
  )
}
