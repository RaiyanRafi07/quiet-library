import React, { useState } from 'react'
import type { SearchResult } from '@/lib/ipc'

export default function ResultsList({ results, onOpen }: { results: SearchResult[]; onOpen: (r: SearchResult) => void }) {
  const [expanded, setExpanded] = useState<number | null>(null)

  const toggleExpanded = (i: number) => {
    setExpanded(expanded === i ? null : i)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      {results.map((r, i) => (
        <div key={i} style={{ padding: 10, borderRadius: 8, border: '1px solid #ddd', background: 'white' }}>
          <button onClick={() => onOpen(r)} style={{ textAlign: 'left', background: 'none', border: 'none', padding: 0, cursor: 'pointer' }}>
            <div style={{ fontWeight: 600 }}>{r.title || r.path.split(/[\\/]/).pop()}</div>
          </button>
          <div style={{ fontSize: 12, color: '#444', marginTop: 4, whiteSpace: 'pre-wrap', lineHeight: 1.4, maxHeight: expanded === i ? 'none' : 100, overflow: 'hidden' }}>
            {r.snippet || '-'}
          </div>
          {r.section && (
            <div style={{ fontSize: 11, color: '#777', marginTop: 2 }}>
              [{r.section}]
            </div>
          )}
          <div style={{ fontSize: 11, color: '#777', marginTop: 4 }}>
            {r.page ? `p.${r.page}` : r.section ? r.section : ''} • {r.path}
          </div>
          {r.snippet && r.snippet.length > 220 && (
            <button onClick={() => toggleExpanded(i)} style={{ fontSize: 11, color: 'blue', background: 'none', border: 'none', padding: 0, cursor: 'pointer', marginTop: 4 }}>
              {expanded === i ? 'Show less' : 'Show more'}
            </button>
          )}
        </div>
      ))}
      {!results.length && <div style={{ color: '#777' }}>No results</div>}
    </div>
  )
}
