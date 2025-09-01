import React, { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { renderPdfPage, pdfPageCount, pdfNextPrevPageWithMatch } from '@/lib/ipc'
import { convertFileSrc } from '@tauri-apps/api/tauri'

type Props = { target: { path: string; page?: number }; query: string }

export default function ReaderPDF({ target, query }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [page, setPage] = useState<number>(target.page || 1)
  const [imgSrc, setImgSrc] = useState<string>('')
  const [imgSize, setImgSize] = useState<{ w: number, h: number } | null>(null)
  const [pageSizePts, setPageSizePts] = useState<{ w: number, h: number } | null>(null)
  const [count, setCount] = useState<number>(0)
  const [loading, setLoading] = useState<boolean>(false)
  const [hasQuery, setHasQuery] = useState<boolean>(!!query.trim())
  const [zoom, setZoom] = useState<number>(1)

  useEffect(() => { setPage(target.page || 1) }, [target.path, target.page])
  useEffect(() => { pdfPageCount(target.path).then(setCount).catch(() => setCount(0)) }, [target.path])
  useEffect(() => { setHasQuery(!!query.trim()) }, [query])

  const [currentDistinctIndex, setCurrentDistinctIndex] = useState<number>(0)

  const reqIdRef = useRef(0)
  const render = async () => {
    const el = containerRef.current
    if (!el) return
    const dpr = (window.devicePixelRatio || 1)
    const width = Math.min(800, Math.max(400, Math.round(el.clientWidth * zoom * dpr)))
    setLoading(true)
    try {
      const myId = ++reqIdRef.current
      const res = await renderPdfPage(target.path, page, width, query)
      const src = (res as any).data_url || (res.file_path ? convertFileSrc(res.file_path) : '')
      if (myId !== reqIdRef.current) { console.debug('stale render dropped'); return }
      setImgSrc(src)
      setImgSize({ w: res.width_px, h: res.height_px })
      setPageSizePts({ w: res.width_pt, h: res.height_pt })
      // Use server-provided pixel rectangles for highlights
      setHighlightsPx(res.rects_px.map(r => ({ left: r[0], top: r[1], width: r[2], height: r[3] })))
      // Warm neighbor pages
      if (page > 1) warmRenderCache(target.path, page - 1, width).catch(()=>{})
      if (count && page < count) warmRenderCache(target.path, page + 1, width).catch(()=>{})
    } catch (e) {
      console.error('renderPdfPage failed', e);
      setImgSrc('');
    } finally {
      setLoading(false)
    }
  }

  useLayoutEffect(() => { render() }, [target.path, page, query, zoom])

  const [highlightsPx, setHighlightsPx] = useState<Array<{left:number, top:number, width:number, height:number}>>([])

  const gotoResultPage = async (delta: number) => {
    if (!hasQuery) return
    const dir: 1 | -1 = delta >= 0 ? 1 : -1
    console.debug('gotoResultPage', { dir, query, page })
    const next = await pdfNextPrevPageWithMatch(target.path, query, page, dir)
    console.debug('gotoResultPage result', next)
    if (typeof next === 'number') {
      setPage(next)
    }
  }

  useEffect(() => {
    // After rendering and highlights computed, scroll to first highlight
    const el = containerRef.current
    if (!el || !highlightsPx.length) return
    const targetTop = Math.max(0, highlightsPx[0].top - el.clientHeight / 2)
    el.scrollTo({ top: targetTop, behavior: 'smooth' })
  }, [imgSrc, highlightsPx])

  
  

  // Keyboard zoom shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const plus = e.key === '+' || e.key === '='
      const minus = e.key === '-' || e.key === '_'
      const reset = e.key === '0'
      if ((e.ctrlKey || e.metaKey) && (plus || minus || reset)) {
        e.preventDefault()
        if (plus) setZoom(z => Math.min(6, z * 1.1))
        else if (minus) setZoom(z => Math.max(0.25, z * 0.9))
        else if (reset) setZoom(1)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const scale = useMemo(() => {
    const el = containerRef.current
    if (!el || !imgSize) return 1
    const desired = el.clientWidth * zoom
    return Math.max(0.25, Math.min(6, desired / imgSize.w))
  }, [imgSize, zoom, containerRef.current])


  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ padding: 8, display: 'flex', alignItems: 'center', gap: 8 }}>
        <button onClick={() => gotoResultPage(-1)} disabled={!hasQuery}>Prev result</button>
        <div style={{ fontSize: 12 }}>Page {page} / {count || '?'} </div>
        <button onClick={() => gotoResultPage(1)} disabled={!hasQuery}>Next result</button>
        <div style={{ marginLeft: 8, display: 'flex', gap: 6 }}>
          <button onClick={() => setZoom(z => Math.max(0.25, z * 0.9))}>-</button>
          <span style={{ fontSize: 12 }}>{Math.round(zoom * 100)}%</span>
          <button onClick={() => setZoom(z => Math.min(6, z * 1.1))}>+</button>
          <button onClick={() => setZoom(1)}>Reset</button>
        </div>
        <div style={{ marginLeft: 'auto', fontSize: 12, opacity: 0.7 }}>{loading ? 'Rendering…' : ''}</div>
      </div>
      <div ref={containerRef} style={{ flex: 1, overflow: 'auto', display: 'flex', justifyContent: 'center', position: 'relative' }}>
        {imgSrc ? (
          <div style={{ position: 'relative', width: imgSize?.w, height: imgSize?.h, transform: `scale(${scale})`, transformOrigin: 'top left' }}>
            <img src={imgSrc} style={{ width: imgSize?.w, height: imgSize?.h, display: 'block' }} onError={(e) => console.error('img load error', e)} />
            {imgSize && highlightsPx.map((r, idx) => (
              <div key={idx} style={{ position: 'absolute', left: r.left, top: r.top, width: r.width, height: r.height, background: 'rgba(255, 230, 0, 0.35)', outline: '2px solid rgba(255,200,0,0.9)' }} />
            ))}
          </div>
        ) : <div style={{ padding: 16 }}>No image</div>}
      </div>
    </div>
  )
}
