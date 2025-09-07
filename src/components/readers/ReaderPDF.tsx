import React, { useEffect, useRef, useState } from 'react'
import { GlobalWorkerOptions, type PDFDocumentProxy } from 'pdfjs-dist/build/pdf'
import * as pdfjs from 'pdfjs-dist/build/pdf'
import Toolbar from '@/ui/Toolbar'
import Button from '@/ui/Button'
// Vite will turn this import into a served asset URL string
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
import workerSrc from 'pdfjs-dist/build/pdf.worker.min.js?url'
import { setupWorker, getPdfDocument } from '@/lib/pdfDocCache'
import { getPageText } from '@/lib/pdfTextCache'

type Props = { target: { path: string; page?: number; startedAt?: number }; query: string }
// Geometry for a single character at the current viewport scale
type Glyph = { ch: string; left: number; top: number; width: number; height: number }
const MAX_GLYPH_CACHE_KEYS = 12 // LRU cap per document/scale

setupWorker(workerSrc as string)

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
  const renderTokenRef = useRef(0)
  const textItemsCacheRef = useRef<Map<number, any>>(new Map())
  const combinedTextCacheRef = useRef<Map<number, string>>(new Map())
  type GlyphEntry = { glyphs: Glyph[]; full: string; fullLC: string }
  const glyphsCacheRef = useRef<Map<string, GlyphEntry>>(new Map())
  const resizeTimerRef = useRef<number | null>(null)
  const warmupTokenRef = useRef(0)
  const [hitPages, setHitPages] = useState<number[] | null>(null)
  const hitTokenRef = useRef(0)
  const prewarmTokenRef = useRef(0)
  const docLoadMsRef = useRef<number | null>(null)
  const openStartedAtRef = useRef<number | null>(null)
  const [metrics, setMetrics] = useState<{ renderMs: number; glyphBuildMs: number; highlightMs: number; textCache: boolean; glyphCache: boolean } | null>(null)
  const navLastTsRef = useRef<number>(0)
  const prewarmDepthRef = useRef<number>(1)
  const progressiveDoneRef = useRef<Set<string>>(new Set())

  useEffect(() => { setPage(target.page || 1) }, [target.path, target.page])

  // Clear scale-dependent glyph caches when zoom or document changes
  useEffect(() => { glyphsCacheRef.current.clear() }, [zoom, doc])

  // Load the PDF (from cache if available) and create a PDF.js document
  useEffect(() => {
    let cancelled = false
    setLoading(true) // show spinner immediately on first open / document switch
    setDoc(null)
    setCount(0)
    setError(null)
    textItemsCacheRef.current.clear()
    combinedTextCacheRef.current.clear()
    glyphsCacheRef.current.clear()
    ;(async () => {
      try {
        const start = (target.startedAt ?? (typeof performance !== 'undefined' ? performance.now() : Date.now()))
        openStartedAtRef.current = start
        const pdf = await getPdfDocument(target.path)
        if (cancelled) return
        setDoc(pdf)
        setCount(pdf.numPages)
        // Doc load metric (click->doc ready)
        const now = (typeof performance !== 'undefined' ? performance.now() : Date.now())
        docLoadMsRef.current = Math.max(0, Math.round(now - start))
        // Background warm-up of combined text to speed up Next/Prev result
        const token = ++warmupTokenRef.current
        ;(async () => {
          for (let p = 1; p <= pdf.numPages; p++) {
            if (token !== warmupTokenRef.current) return
            if (!combinedTextCacheRef.current.has(p)) {
              try {
                const pg = await pdf.getPage(p)
                if (token !== warmupTokenRef.current) return
                const text = await pg.getTextContent({ disableCombineTextItems: true })
                if (token !== warmupTokenRef.current) return
                const combined = (text.items as any[]).map(i => i.str || '').join(' ')
                combinedTextCacheRef.current.set(p, combined)
              } catch {
                // ignore background failures
              }
            }
            if (p % 5 === 0) { await new Promise(r => setTimeout(r, 0)) }
          }
        })()
      } catch (e: any) {
        if (!cancelled) { setError(String(e)); setLoading(false) }
      }
    })()
    return () => { cancelled = true }
  }, [target.path])

  // Do not destroy doc on unmount; LRU cache owns lifecycle

  // Ask backend (index or cache) for pages containing the query in this document.
  useEffect(() => {
    const q = (query || '').trim()
    if (!doc || !q) { setHitPages(null); return }
    const token = ++hitTokenRef.current
    import('@/lib/ipc').then(async ({ searchDocumentPages }) => {
      try {
        const pages = await searchDocumentPages(target.path, q, Math.max(1000, (doc.numPages || 0)))
        if (token !== hitTokenRef.current) return
        setHitPages(pages)
      } catch {
        if (token !== hitTokenRef.current) return
        setHitPages(null)
      }
    })
    return () => { /* cancel via token */ }
  }, [doc, query, target.path])

  // Pre-warm caches for surrounding hit pages at current zoom to speed navigation
  useEffect(() => {
    if (!doc || !containerRef.current) return
    const hits = hitPages && hitPages.length ? hitPages : null
    if (!hits || !hits.length) return
    const depth = Math.max(1, Math.min(3, prewarmDepthRef.current || 1))
    const before = [...hits].filter(h => h < page).slice(-depth)
    const afterIdx = hits.findIndex(h => h > page)
    const afterList = afterIdx >= 0 ? hits.slice(afterIdx, afterIdx + depth) : []
    const toWarm = [...before, ...afterList]
    if (!toWarm.length) return
    const token = ++prewarmTokenRef.current
    ;(async () => {
      for (const p of toWarm) {
        try {
          const pdfPage = await doc.getPage(p)
          if (token !== prewarmTokenRef.current) return
          const el = containerRef.current!
          const dpr = window.devicePixelRatio || 1
          const baseViewport = pdfPage.getViewport({ scale: 1 })
          const desiredCSSWidth = Math.max(400, Math.min(1200, el.clientWidth * zoom))
          const scale = (desiredCSSWidth * dpr) / baseViewport.width
          const viewport = pdfPage.getViewport({ scale })
          let text = textItemsCacheRef.current.get(p)
          if (!text) {
            text = await getPageText(target.path, p)
            if (token !== prewarmTokenRef.current) return
            textItemsCacheRef.current.set(p, text)
          }
          const glyphKey = `${p}@${Math.round(viewport.scale * 1000)}`
          if (!glyphsCacheRef.current.has(glyphKey)) {
            const local: Glyph[] = []
            const m = (pdfjs as any).Util.transform
            for (const item of text.items as any[]) {
              const str: string = item.str || ''
              if (!str) continue
              const tr = m(viewport.transform, item.transform)
              const x = tr[4]
              const y = tr[5]
              const fontHeight = Math.hypot(tr[2], tr[3])
              const scaleX = Math.hypot(tr[0], tr[1]) || 1
              const totalWidth = typeof item.width === 'number' ? item.width * viewport.scale : (item.width || 0) * scaleX
              if (!fontHeight) continue
              const n = Math.max(1, str.length)
              for (let i = 0; i < str.length; i++) {
                const left = x + (i / n) * totalWidth
                const width = totalWidth / n
                local.push({ ch: str[i], left, top: y - fontHeight, width, height: fontHeight })
              }
            }
            const full = local.map(g => g.ch).join('')
            glyphsCacheRef.current.set(glyphKey, { glyphs: local, full, fullLC: full.toLowerCase() })
          }
        } catch {
          // ignore warm errors
        }
        // Yield to avoid blocking UI
        await new Promise(r => setTimeout(r, 0))
      }
    })()
  }, [doc, page, zoom, hitPages])

  const renderPage = async () => {
    if (!doc || !containerRef.current || !canvasRef.current) return
    const p = Math.max(1, Math.min(count || 1, page))
    try {
      setLoading(true)
      const benchStart = (typeof performance !== 'undefined' ? performance.now() : Date.now())
      const localToken = ++renderTokenRef.current
      const pdfPage = await doc.getPage(p)
      if (localToken !== renderTokenRef.current) return
      const el = containerRef.current
      const dpr = window.devicePixelRatio || 1
      const baseViewport = pdfPage.getViewport({ scale: 1 })
      const desiredCSSWidth = Math.max(400, Math.min(1200, el.clientWidth * zoom))
      const scale = (desiredCSSWidth * dpr) / baseViewport.width
      const viewport = pdfPage.getViewport({ scale })
      const canvas = canvasRef.current
      const ctx = canvas.getContext('2d')!
      // Progressive render: quick pass at lower scale, then full
      const progressiveKey = `${p}@${zoom.toFixed(2)}`
      const needQuick = !progressiveDoneRef.current.has(progressiveKey) && scale > 1.2
      let firstPaintTime = 0
      const doRender = async (vp: any) => {
        canvas.width = vp.width
        canvas.height = vp.height
        canvas.style.width = `${vp.width / dpr}px`
        canvas.style.height = `${vp.height / dpr}px`
        ctx.setTransform(1, 0, 0, 1, 0, 0)
        ctx.clearRect(0, 0, canvas.width, canvas.height)
        if (renderTaskRef.current) { try { renderTaskRef.current.cancel(); } catch {} ; renderTaskRef.current = null }
        const t = pdfPage.render({ canvasContext: ctx, viewport: vp })
        renderTaskRef.current = t
        await t.promise.catch(() => {})
        if (renderTaskRef.current === t) renderTaskRef.current = null
      }
      if (needQuick) {
        const quickViewport = pdfPage.getViewport({ scale: Math.max(0.5, scale * 0.5) })
        await doRender(quickViewport)
        if (localToken !== renderTokenRef.current) return
        setLoading(false) // first paint
        firstPaintTime = (typeof performance !== 'undefined' ? performance.now() : Date.now())
        // continue to full render
        await doRender(viewport)
        progressiveDoneRef.current.add(progressiveKey)
      } else {
        await doRender(viewport)
        if (localToken !== renderTokenRef.current) return
        setLoading(false)
        firstPaintTime = (typeof performance !== 'undefined' ? performance.now() : Date.now())
      }
      const afterRender = (typeof performance !== 'undefined' ? performance.now() : Date.now())
      let glyphBuildStart = afterRender

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
          // Build per-character geometry for accurate highlights across items
          let text = textItemsCacheRef.current.get(p)
          const hadTextCache = !!text
          if (!text) {
            text = await getPageText(target.path, p)
            if (localToken !== renderTokenRef.current) return
            textItemsCacheRef.current.set(p, text)
          }
          const glyphKey = `${p}@${Math.round(viewport.scale * 1000)}`
          let entry = glyphsCacheRef.current.get(glyphKey)
          let glyphs = entry?.glyphs
          const usedGlyphCache = !!entry
          if (entry) {
            // touch LRU
            glyphsCacheRef.current.delete(glyphKey)
            glyphsCacheRef.current.set(glyphKey, entry)
          }
          if (!glyphs) {
            const local: Glyph[] = []
            const m = (pdfjs as any).Util.transform
            let processed = 0
            let sliceStart = (typeof performance !== 'undefined' ? performance.now() : Date.now())
            for (const item of text.items as any[]) {
              const str: string = item.str || ''
              if (!str) continue
              const tr = m(viewport.transform, item.transform)
              const x = tr[4]
              const y = tr[5]
              const fontHeight = Math.hypot(tr[2], tr[3])
              // Horizontal scale from transform
              const scaleX = Math.hypot(tr[0], tr[1]) || 1
              // item.width is reported in unscaled CSS units for the default viewport.
              // We want device-pixel width, consistent with tr[4]/tr[5].
              // viewport.scale already includes DPR; we later divide by DPR for CSS.
              const totalWidth = typeof item.width === 'number'
                ? item.width * viewport.scale
                : (item.width || 0) * scaleX // best-effort fallback
              if (!fontHeight) continue
              const n = Math.max(1, str.length)
              for (let i = 0; i < str.length; i++) {
                const left = x + (i / n) * totalWidth
                const width = totalWidth / n
                local.push({ ch: str[i], left, top: y - fontHeight, width, height: fontHeight })
              }
              processed++
              if (processed % 12 === 0) {
                const now = (typeof performance !== 'undefined' ? performance.now() : Date.now())
                if (now - sliceStart > 12) { await new Promise<void>(r => setTimeout(r, 0)); sliceStart = now; }
                if (localToken !== renderTokenRef.current) return
              }
            }
            const full = local.map(g => g.ch).join('')
            const fullLC = full.toLowerCase()
            glyphs = local
            const newEntry: GlyphEntry = { glyphs: local, full, fullLC }
            // LRU insert: touch then evict if needed
            if (glyphsCacheRef.current.has(glyphKey)) glyphsCacheRef.current.delete(glyphKey)
            glyphsCacheRef.current.set(glyphKey, newEntry)
            while (glyphsCacheRef.current.size > MAX_GLYPH_CACHE_KEYS) {
              const oldestKey = glyphsCacheRef.current.keys().next().value as string | undefined
              if (!oldestKey) break
              glyphsCacheRef.current.delete(oldestKey)
            }
          }
          glyphBuildStart = (typeof performance !== 'undefined' ? performance.now() : Date.now())
          const full = entry?.full ?? glyphs!.map(g => g.ch).join('')
          const fullLC = entry?.fullLC ?? full.toLowerCase()
          const targetLC = q.toLowerCase()
          let firstTop: number | null = null
          if (targetLC) {
            let idx = 0
            let appended = 0
            let sliceStart = (typeof performance !== 'undefined' ? performance.now() : Date.now())
            while (idx <= fullLC.length) {
              const found = fullLC.indexOf(targetLC, idx)
              if (found === -1) break
              const start = found
              const end = found + targetLC.length
              const slice = glyphs.slice(start, end)
              if (slice.length) {
                // Split into line runs if the glyphs jump vertically
                let runStart = 0
                const thresholdFor = (g: Glyph) => Math.max(1, g.height * 0.4)
                for (let i = 1; i <= slice.length; i++) {
                  const prev = slice[i - 1]
                  const curr = slice[i]
                  const lineBreak = i === slice.length || (curr && Math.abs(curr.top - prev.top) > thresholdFor(prev))
                  if (lineBreak) {
                    const run = slice.slice(runStart, i)
                    const first = run[0]
                    const last = run[run.length - 1]
                    if (first && last) {
                      const left = first.left
                      const right = last.left + last.width
                      const top = Math.min(...run.map(g => g.top))
                      const height = Math.max(...run.map(g => g.height))
                      const div = document.createElement('div')
                      div.style.position = 'absolute'
                      div.style.left = `${left / dpr}px`
                      div.style.top = `${top / dpr}px`
                      div.style.width = `${(right - left) / dpr}px`
                      div.style.height = `${height / dpr}px`
                      div.style.background = 'var(--nb-highlight)'
                      div.style.outline = '2px solid var(--nb-highlight-stroke)'
                      div.style.pointerEvents = 'none'
                      overlay.appendChild(div)
                      if (firstTop == null) firstTop = top / dpr
                      appended++
                      if (appended % 20 === 0) {
                        const now = (typeof performance !== 'undefined' ? performance.now() : Date.now())
                        if (now - sliceStart > 12) { await new Promise<void>(r => setTimeout(r, 0)); sliceStart = now }
                        if (localToken !== renderTokenRef.current) return
                      }
                    }
                    runStart = i
                  }
                }
              }
              idx = end
            }
          }
          if (firstTop != null && containerRef.current) {
            const elScroll = containerRef.current
            const targetTop = Math.max(0, firstTop - elScroll.clientHeight / 2)
            elScroll.scrollTo({ top: targetTop, behavior: 'smooth' })
          }
          const afterHighlights = (typeof performance !== 'undefined' ? performance.now() : Date.now())
          const metricsNow = {
            renderMs: Math.round(afterRender - benchStart),
            glyphBuildMs: Math.round(afterHighlights - glyphBuildStart),
            highlightMs: Math.round(afterHighlights - afterRender),
            textCache: hadTextCache,
            glyphCache: usedGlyphCache,
          }
          setMetrics(metricsNow)
          try {
            const openStart = openStartedAtRef.current
            const docLoadMs = docLoadMsRef.current
            const openToFirstRender = openStart ? Math.round((firstPaintTime || afterRender) - openStart) : undefined
            const openToHighlights = openStart ? Math.round(afterHighlights - openStart) : undefined
            console.log('[ReaderPDF] render', { page: p, zoom, ...metricsNow, docLoadMs, openToFirstRender, openToHighlights })
          } catch {}
        }
      }
    } catch (e: any) {
      setError(String(e))
    } finally {
      // ensure not stuck spinning on errors or cancellations
      setLoading(false)
    }
  }

  useEffect(() => { renderPage() }, [doc, page, zoom])

  // Re-render on container resize (debounced)
  useEffect(() => {
    if (!containerRef.current) return
    const ro = new ResizeObserver(() => {
      if (resizeTimerRef.current) window.clearTimeout(resizeTimerRef.current)
      // Small debounce to avoid thrash during window resizes
      resizeTimerRef.current = window.setTimeout(() => { renderPage() }, 80)
    })
    ro.observe(containerRef.current)
    return () => ro.disconnect()
  }, [doc, page, zoom])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <Toolbar style={{ alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-2)' }}>
          <Button
          onClick={async () => {
            const q = (query || '').trim()
            if (!doc) return
            if (!q) { setPage(p => Math.max(1, p - 1)); return }
            const nowTs = (typeof performance !== 'undefined' ? performance.now() : Date.now())
            const dt = nowTs - navLastTsRef.current
            prewarmDepthRef.current = dt < 600 ? 3 : 1
            navLastTsRef.current = nowTs
            const token = ++searchTokenRef.current
            const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
            const re = new RegExp(esc, 'i')
            // Prefer backend-provided hit pages
            const hits = hitPages && hitPages.length ? hitPages : null
            if (hits) {
              const before = hits.filter(h => h < page)
              if (before.length) { setPage(before[before.length - 1]); return }
            }
            // Fallback: linear scan in memory
            for (let p = Math.max(1, page - 1); p >= 1; p--) {
              if (token !== searchTokenRef.current) return
              let combined = combinedTextCacheRef.current.get(p)
              if (!combined) {
                const pdfPage = await doc.getPage(p)
                const text = await pdfPage.getTextContent({ disableCombineTextItems: true })
                combined = (text.items as any[]).map(i => i.str || '').join(' ')
                combinedTextCacheRef.current.set(p, combined)
              }
              if (re.test(combined)) { setPage(p); return }
            }
          }}
          disabled={page <= 1}
        >Prev {query.trim() ? 'result' : 'page'}</Button>
          <div className="nb-meta" style={{ fontSize: 'var(--fs-xs)', fontWeight: 800 }}>Page {page} / {count || '?'}</div>
          <Button
          onClick={async () => {
            const q = (query || '').trim()
            if (!doc) return
            if (!q) { setPage(p => Math.min(count || p + 1, p + 1)); return }
            const nowTs = (typeof performance !== 'undefined' ? performance.now() : Date.now())
            const dt = nowTs - navLastTsRef.current
            prewarmDepthRef.current = dt < 600 ? 3 : 1
            navLastTsRef.current = nowTs
            const token = ++searchTokenRef.current
            const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
            const re = new RegExp(esc, 'i')
            // Prefer backend-provided hit pages
            const hits = hitPages && hitPages.length ? hitPages : null
            if (hits) {
              const after = hits.find(h => h > page)
              if (after != null) { setPage(after); return }
            }
            // Fallback: linear scan in memory
            for (let p = Math.min((count || page), page + 1); p <= (count || page); p++) {
              if (token !== searchTokenRef.current) return
              let combined = combinedTextCacheRef.current.get(p)
              if (!combined) {
                const pdfPage = await doc.getPage(p)
                const text = await pdfPage.getTextContent({ disableCombineTextItems: true })
                combined = (text.items as any[]).map(i => i.str || '').join(' ')
                combinedTextCacheRef.current.set(p, combined)
              }
              if (re.test(combined)) { setPage(p); return }
            }
          }}
          disabled={!count || page >= count}
        >Next {query.trim() ? 'result' : 'page'}</Button>
          {/* Spinner sits next to Next button; uses visibility to avoid layout shift */}
          <span style={{ width: 18, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
            <span className="nb-spinner" style={{ visibility: loading ? 'visible' as const : 'hidden' as const }} />
          </span>
          {/* Optional metrics (toggle with localStorage.setItem('nbMetrics','1')) */}
          <span className="nb-meta" style={{ width: 120, display: 'inline-block', fontSize: 'var(--fs-xs)', opacity: 0.7 }}>
            {typeof window !== 'undefined' && window.localStorage?.getItem('nbMetrics') === '1' && metrics ? (
              <>r:{metrics.renderMs}ms g:{metrics.glyphBuildMs}ms h:{metrics.highlightMs}ms</>
            ) : null}
          </span>
        </div>

        <div style={{ flex: 1 }} />

        <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-1)' }}>
          <Button onClick={() => setZoom(z => Math.max(0.25, z * 0.9))}>-</Button>
          <span
            className="nb-meta"
            style={{
              fontSize: 'var(--fs-xs)',
              fontWeight: 800,
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              minWidth: 56,
              textAlign: 'center'
            }}
          >{Math.round(zoom * 100)}%</span>
          <Button onClick={() => setZoom(z => Math.min(6, z * 1.1))}>+</Button>
          <Button onClick={() => setZoom(1)}>Reset</Button>
        </div>
      </Toolbar>
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
