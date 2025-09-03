import React, { useEffect, useRef, useState } from 'react'
import { readBinaryFile } from '@tauri-apps/api/fs'
import { GlobalWorkerOptions, getDocument, type PDFDocumentProxy } from 'pdfjs-dist/build/pdf'
import * as pdfjs from 'pdfjs-dist/build/pdf'
// Vite will turn this import into a served asset URL string
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
import workerSrc from 'pdfjs-dist/build/pdf.worker.min.js?url'

type Props = { target: { path: string; page?: number }; query: string }

GlobalWorkerOptions.workerSrc = workerSrc as string

export default function ReaderPDF({ target, query }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null)
  const [page, setPage] = useState<number>(target.page || 1)
  const [count, setCount] = useState<number>(0)
  const [loading, setLoading] = useState<boolean>(false)
  const [error, setError] = useState<string | null>(null)
  const [zoom, setZoom] = useState<number>(1)
  const renderTaskRef = useRef<any>(null)
  const searchTokenRef = useRef(0)

  useEffect(() => { setPage(target.page || 1) }, [target.path, target.page])

  // Load the PDF bytes and create a PDF.js document
  useEffect(() => {
    let cancelled = false
    setDoc(null)
    setCount(0)
    setError(null)
    ;(async () => {
      try {
        const bytes = await readBinaryFile(target.path)
        const task = getDocument({ data: new Uint8Array(bytes) })
        const pdf = await task.promise
        if (cancelled) return
        setDoc(pdf)
        setCount(pdf.numPages)
      } catch (e: any) {
        if (!cancelled) setError(String(e))
      }
    })()
    return () => { cancelled = true }
  }, [target.path])

  const renderPage = async () => {
    if (!doc || !containerRef.current || !canvasRef.current) return
    const p = Math.max(1, Math.min(count || 1, page))
    try {
      setLoading(true)
      const pdfPage = await doc.getPage(p)
      const el = containerRef.current
      const dpr = window.devicePixelRatio || 1
      const baseViewport = pdfPage.getViewport({ scale: 1 })
      const desiredCSSWidth = Math.max(400, Math.min(1200, el.clientWidth * zoom))
      const scale = (desiredCSSWidth * dpr) / baseViewport.width
      const viewport = pdfPage.getViewport({ scale })
      const canvas = canvasRef.current
      const ctx = canvas.getContext('2d')!
      canvas.width = viewport.width
      canvas.height = viewport.height
      canvas.style.width = `${viewport.width / dpr}px`
      canvas.style.height = `${viewport.height / dpr}px`
      ctx.setTransform(1, 0, 0, 1, 0, 0)
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      // Cancel any in-flight render to avoid PDF.js canvas reuse error
      if (renderTaskRef.current) {
        try { renderTaskRef.current.cancel(); } catch {}
        renderTaskRef.current = null
      }
      const task = pdfPage.render({ canvasContext: ctx, viewport })
      renderTaskRef.current = task
      await task.promise.catch(() => {})
      if (renderTaskRef.current === task) renderTaskRef.current = null

      // Highlights overlay
      if (highlightRef.current) {
        const overlay = highlightRef.current
        overlay.style.width = `${viewport.width / dpr}px`
        overlay.style.height = `${viewport.height / dpr}px`
        overlay.innerHTML = ''
        const q = (query || '').trim()
        if (q) {
          const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
          const re = new RegExp(esc, 'gi')
          // Use fine-grained items for better positioning
          const text = await pdfPage.getTextContent({ disableCombineTextItems: true })
          let firstTop: number | null = null
          for (const item of text.items as any[]) {
            const str: string = item.str || ''
            if (!str) continue
            const m = (pdfjs as any).Util.transform
            const tr = m(viewport.transform, item.transform)
            const x = tr[4]
            const y = tr[5]
            const fontHeight = Math.hypot(tr[2], tr[3])
            const totalWidth = (item.width || 0) * viewport.scale
            if (!totalWidth || !fontHeight) continue
            let match: RegExpExecArray | null
            while ((match = re.exec(str))) {
              const start = match.index
              const len = match[0].length
              // With disableCombineTextItems true, items are small; per-char width approx is acceptable
              const left = x + (start / Math.max(1, str.length)) * totalWidth
              const width = (len / Math.max(1, str.length)) * totalWidth
              const top = y - fontHeight
              const div = document.createElement('div')
              div.style.position = 'absolute'
              div.style.left = `${left / dpr}px`
              div.style.top = `${top / dpr}px`
              div.style.width = `${width / dpr}px`
              div.style.height = `${fontHeight / dpr}px`
              div.style.background = 'rgba(255, 230, 0, 0.35)'
              div.style.outline = '2px solid rgba(255,200,0,0.9)'
              div.style.pointerEvents = 'none'
              overlay.appendChild(div)
              if (firstTop == null) firstTop = top / dpr
            }
          }
          if (firstTop != null && containerRef.current) {
            const elScroll = containerRef.current
            const targetTop = Math.max(0, firstTop - elScroll.clientHeight / 2)
            elScroll.scrollTo({ top: targetTop, behavior: 'smooth' })
          }
        }
      }
    } catch (e: any) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { renderPage() }, [doc, page, zoom])

  // Re-render on container resize
  useEffect(() => {
    if (!containerRef.current) return
    const ro = new ResizeObserver(() => { renderPage() })
    ro.observe(containerRef.current)
    return () => ro.disconnect()
  }, [doc, page, zoom])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ padding: 8, display: 'flex', alignItems: 'center', gap: 8 }}>
        <button
          onClick={async () => {
            const q = (query || '').trim()
            if (!doc) return
            if (!q) { setPage(p => Math.max(1, p - 1)); return }
            const token = ++searchTokenRef.current
            const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
            const re = new RegExp(esc, 'i')
            for (let p = Math.max(1, page - 1); p >= 1; p--) {
              // stop early if a newer search started
              if (token !== searchTokenRef.current) return
              const pdfPage = await doc.getPage(p)
            const text = await pdfPage.getTextContent({ disableCombineTextItems: true })
            const combined = (text.items as any[]).map(i => i.str || '').join(' ')
              if (re.test(combined)) { setPage(p); return }
            }
          }}
          disabled={page <= 1}
        >Prev {query.trim() ? 'result' : 'page'}</button>
        <div style={{ fontSize: 12 }}>Page {page} / {count || '?'}</div>
        <button
          onClick={async () => {
            const q = (query || '').trim()
            if (!doc) return
            if (!q) { setPage(p => Math.min(count || p + 1, p + 1)); return }
            const token = ++searchTokenRef.current
            const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
            const re = new RegExp(esc, 'i')
            for (let p = Math.min((count || page), page + 1); p <= (count || page); p++) {
              if (token !== searchTokenRef.current) return
              const pdfPage = await doc.getPage(p)
              const text = await pdfPage.getTextContent()
              const combined = (text.items as any[]).map(i => i.str || '').join(' ')
              if (re.test(combined)) { setPage(p); return }
            }
          }}
          disabled={!count || page >= count}
        >Next {query.trim() ? 'result' : 'page'}</button>
        <div style={{ marginLeft: 8, display: 'flex', gap: 6 }}>
          <button onClick={() => setZoom(z => Math.max(0.25, z * 0.9))}>-</button>
          <span style={{ fontSize: 12 }}>{Math.round(zoom * 100)}%</span>
          <button onClick={() => setZoom(z => Math.min(6, z * 1.1))}>+</button>
          <button onClick={() => setZoom(1)}>Reset</button>
        </div>
        <div style={{ marginLeft: 'auto', fontSize: 12, opacity: 0.7 }}>{loading ? 'Rendering…' : ''}</div>
      </div>
      {error && (
        <div style={{ padding: 8, color: '#b00020', fontSize: 12, borderTop: '1px solid #eee', borderBottom: '1px solid #eee' }}>
          Error: {error}
        </div>
      )}
      <div ref={containerRef} style={{ flex: 1, overflow: 'auto', display: 'flex', justifyContent: 'center' }}>
        <div style={{ position: 'relative' }}>
          <canvas ref={canvasRef} />
          <div ref={highlightRef} style={{ position: 'absolute', left: 0, top: 0, pointerEvents: 'none' }} />
        </div>
      </div>
    </div>
  )
}
