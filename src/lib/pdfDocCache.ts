import { readBinaryFile } from '@tauri-apps/api/fs'
import { GlobalWorkerOptions, getDocument, type PDFDocumentProxy } from 'pdfjs-dist/build/pdf'

let workerSet = false
export function setupWorker(workerSrc: string) {
  if (!workerSet) {
    GlobalWorkerOptions.workerSrc = workerSrc as string
    workerSet = true
  }
}

const DOC_CACHE_MAX = 3
const docCache = new Map<string, PDFDocumentProxy>()
const docLoaders = new Map<string, Promise<PDFDocumentProxy>>()

function lruTouch(key: string) {
  const v = docCache.get(key)
  if (!v) return
  docCache.delete(key)
  docCache.set(key, v)
}

export async function getPdfDocument(path: string): Promise<PDFDocumentProxy> {
  const cached = docCache.get(path)
  if (cached) { lruTouch(path); return cached }
  const inflight = docLoaders.get(path)
  if (inflight) return inflight
  const loader = (async () => {
    const bytes = await readBinaryFile(path)
    const task = getDocument({ data: bytes })
    const pdf = await task.promise
    if (docCache.has(path)) { try { pdf.destroy() } catch {} ; return docCache.get(path)! }
    docCache.set(path, pdf)
    while (docCache.size > DOC_CACHE_MAX) {
      const oldestKey = docCache.keys().next().value as string | undefined
      if (!oldestKey) break
      const old = docCache.get(oldestKey)
      docCache.delete(oldestKey)
      try { old?.destroy?.() } catch {}
    }
    return pdf
  })()
  docLoaders.set(path, loader)
  try { return await loader } finally { docLoaders.delete(path) }
}

export async function prewarmPdfDocument(path: string) {
  try { await getPdfDocument(path) } catch {/* ignore */}
}

export function clearPdfCache() {
  for (const [, pdf] of docCache) { try { (pdf as any)?.destroy?.() } catch {} }
  docCache.clear()
}

