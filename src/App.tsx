import React, { useEffect, useState, lazy, Suspense } from 'react'
import SearchBar from './components/SearchBar'
import ResultsList from './components/ResultsList'
const ReaderPDF = lazy(() => import('./components/readers/ReaderPDF'))
const ReaderEPUB = lazy(() => import('./components/readers/ReaderEPUB'))
const ReaderText = lazy(() => import('./components/readers/ReaderText'))
import ActionsBar from './components/ActionsBar'
import BookmarksList from './components/BookmarksList'
import FolderManager from './components/FolderManager'
import { listWatchedFolders, reindexAll, search, clearExtractCache, type SearchResult } from './lib/ipc'
import { pageStyle } from './styles'
import Button from '@/ui/Button'
import Card from '@/ui/Card'
import useDebouncedValue from '@/hooks/useDebouncedValue'
import type { View } from '@/types/view'

export default function App() {
  const [view, setView] = useState<View>('library')
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SearchResult[]>([])
  const debounced = useDebouncedValue(query, 250)
  const [searching, setSearching] = useState(false)
  const [folders, setFolders] = useState<string[]>([])
  const [openTarget, setOpenTarget] = useState<{ path: string; page?: number; section?: string } | null>(null)
  const [indexing, setIndexing] = useState(false)
  const [clearing, setClearing] = useState(false)

  useEffect(() => {
    listWatchedFolders().then(setFolders).catch(() => setFolders([]))
  }, [])

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

  const handleOpenSelection = (sel: { path: string; page?: number; section?: string }) => {
    const ext = sel.path.toLowerCase().split('.').pop()
    setOpenTarget({ path: sel.path, page: sel.page, section: sel.section })
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
      <div style={{ flex: 1, overflow: 'auto' }}>
        {view === 'library' && (
          <div style={pageStyle}>
            <SearchBar value={query} onChange={setQuery} />
            <Card style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-2)', fontSize: 'var(--fs-sm)' }}>
              <div className="nb-heading nb-h2">{searching ? 'Searching…' : `${results.length} result${results.length === 1 ? '' : 's'}`}</div>
            </Card>
            <ResultsList results={results} onOpen={handleOpen} />
            <div style={{ marginTop: 'auto', display: 'flex', alignItems: 'center', gap: 'var(--sp-3)' }}>
              <div className="nb-meta" style={{ fontSize: 'var(--fs-xs)', flex: 1, fontWeight: 800 }}>
                {indexing ? 'Indexing…' : `Watched folders: ${folders.length ? folders.join(' · ') : 'none'}`}
              </div>
              <Button onClick={() => setView('folders')}>Manage Folders</Button>
              <Button
                onClick={async () => {
                  setClearing(true)
                  try {
                    await clearExtractCache()
                  } finally {
                    setClearing(false)
                  }
                }}
                disabled={clearing || indexing}
              >Clear cache</Button>
              <Button
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
              >Reindex</Button>
            </div>
          </div>
        )}
        {view === 'bookmarks' && <BookmarksList onOpen={handleOpenSelection} />}
        {view === 'folders' && <FolderManager folders={folders} setFolders={setFolders} />}
        <Suspense fallback={<div style={{ padding: 'var(--sp-5)' }} className="nb-meta">Loading reader…</div>}>
          {view === 'pdf' && openTarget && <ReaderPDF target={openTarget} query={query} />}
          {view === 'epub' && openTarget && <ReaderEPUB target={openTarget} query={query} />}
          {view === 'text' && openTarget && <ReaderText target={openTarget} query={query} />}
        </Suspense>
      </div>
    </div>
  )
}
