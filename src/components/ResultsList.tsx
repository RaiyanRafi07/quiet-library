import React, { useState } from 'react'
import type { SearchResult } from '@/lib/ipc'
import Card from '@/ui/Card'
import { subtleLink } from '@/styles'

export default function ResultsList({ results, onOpen }: { results: SearchResult[]; onOpen: (r: SearchResult) => void }) {
  const [expanded, setExpanded] = useState<number | null>(null)

  const toggleExpanded = (i: number) => {
    setExpanded(expanded === i ? null : i)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
      {results.map((r, i) => (
        <Card key={i}>
          <button onClick={() => onOpen(r)} style={{ textAlign: 'left', background: 'none', border: 'none', padding: 0, cursor: 'pointer' }}>
            <div className="nb-heading nb-h2" style={{ fontSize: 'var(--fs-lg)' }}>{r.title || r.path.split(/[\\/]/).pop()}</div>
          </button>
          <div style={{ fontSize: 'var(--fs-sm)', color: 'var(--nb-card-text)', marginTop: 'var(--sp-2)', whiteSpace: 'pre-wrap', lineHeight: 'var(--lh-normal)', maxHeight: expanded === i ? 'none' : 120, overflow: 'hidden' }}>
            {r.snippet || '-'}
          </div>
          {r.section && (
            <div className="nb-meta" style={{ fontSize: 'var(--fs-xs)', color: 'var(--nb-muted)', marginTop: 'var(--sp-1)' }}>
              [{r.section}]
            </div>
          )}
          <div className="nb-meta" style={{ fontSize: 'var(--fs-xs)', color: 'var(--nb-muted)', marginTop: 'var(--sp-2)' }}>
            {r.page ? `p.${r.page}` : r.section ? r.section : ''} • {r.path}
          </div>
          {r.snippet && r.snippet.length > 220 && (
            <button onClick={() => toggleExpanded(i)} style={{ ...subtleLink, fontSize: 'var(--fs-xs)', marginTop: 'var(--sp-2)' }}>
              {expanded === i ? 'Show less' : 'Show more'}
            </button>
          )}
        </Card>
      ))}
      {!results.length && (
        <Card style={{ color: 'var(--nb-muted)' }}>
          <div className="nb-heading nb-h2">No results</div>
        </Card>
      )}
    </div>
  )
}
