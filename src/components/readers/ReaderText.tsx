import React, { useEffect, useMemo, useRef, useState } from 'react'
import Card from '@/ui/Card'
import { readTextFile } from '@tauri-apps/api/fs'

type Props = { target: { path: string }; query: string }

function escapeHtml(s: string) {
  return s.replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch] as string))
}

function buildHighlightedHtml(text: string, query: string) {
  const q = query.trim()
  if (!q) return escapeHtml(text)
  const escaped = escapeHtml(text)
  const re = new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi')
  return escaped.replace(re, (m) => `<mark>${m}</mark>`)
}

export default function ReaderText({ target, query }: Props) {
  const [content, setContent] = useState<string>('')
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    readTextFile(target.path).then(setContent).catch(() => setContent(''))
  }, [target.path])

  const html = useMemo(() => buildHighlightedHtml(content, query), [content, query])

  useEffect(() => {
    // Scroll to first highlight
    const el = ref.current?.querySelector('mark') as HTMLElement | null
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }, [html])

  return (
    <div style={{ padding: 16, overflow: 'auto' }}>
      <Card style={{ whiteSpace: 'pre-wrap', fontFamily: 'var(--font-mono)' }}>
        <div ref={ref} dangerouslySetInnerHTML={{ __html: html }} />
      </Card>
    </div>
  )
}
