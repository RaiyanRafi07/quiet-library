import React, { useEffect, useRef, useState } from 'react'
import type { SearchResult } from '@/lib/ipc'
import Card from '@/ui/Card'
import { subtleLink } from '@/styles'
import { prewarmPdfDocument, setupWorker } from '@/lib/pdfDocCache'
import { prewarmPageText } from '@/lib/pdfTextCache'
// Vite URL for the worker; ensure pdf.js worker is configured even before the reader mounts
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
import workerSrc from 'pdfjs-dist/build/pdf.worker.min.js?url'
setupWorker(workerSrc as string)

export default function ResultsList({ results, onOpen }: { results: SearchResult[]; onOpen: (r: SearchResult) => void }) {
  const [expanded, setExpanded] = useState<number | null>(null)
  const prewarmedRef = useRef<Set<string>>(new Set())

  const toggleExpanded = (i: number) => {
    setExpanded(expanded === i ? null : i)
  }

  // Prewarm the first visible PDF result after a short delay
  useEffect(() => {
    const id = setTimeout(() => {
      const firstPdf = results.find(r => r.path.toLowerCase().endsWith('.pdf'))
      if (firstPdf && !prewarmedRef.current.has(firstPdf.path)) {
        prewarmedRef.current.add(firstPdf.path)
        prewarmPdfDocument(firstPdf.path)
        if (firstPdf.page) prewarmPageText(firstPdf.path, firstPdf.page)
      }
    }, 250)
    return () => clearTimeout(id)
  }, [results])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
      {results.map((r, i) => (
        <Card key={i}>
          <button
            onClick={() => onOpen(r)}
            onMouseEnter={() => {
              if (r.path.toLowerCase().endsWith('.pdf') && !prewarmedRef.current.has(r.path)) {
                prewarmedRef.current.add(r.path)
                prewarmPdfDocument(r.path)
                if (r.page) prewarmPageText(r.path, r.page)
              }
            }}
            onFocus={() => {
              if (r.path.toLowerCase().endsWith('.pdf') && !prewarmedRef.current.has(r.path)) {
                prewarmedRef.current.add(r.path)
                prewarmPdfDocument(r.path)
                if (r.page) prewarmPageText(r.path, r.page)
              }
            }}
            style={{ textAlign: 'left', background: 'none', border: 'none', padding: 0, cursor: 'pointer' }}
          >
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
