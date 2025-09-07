import type { PDFDocumentProxy } from 'pdfjs-dist/build/pdf'
import { getPdfDocument } from '@/lib/pdfDocCache'

type TextContent = any // pdf.js TextContent type

const textCache = new Map<string, TextContent>() // key: `${path}#${page}`
const inflight = new Map<string, Promise<TextContent>>()

function key(path: string, page: number) { return `${path}#${page}` }

export async function getPageText(path: string, page: number): Promise<TextContent> {
  const k = key(path, page)
  const c = textCache.get(k)
  if (c) return c
  const f = inflight.get(k)
  if (f) return f
  const p = (async () => {
    const doc: PDFDocumentProxy = await getPdfDocument(path)
    const pdfPage = await doc.getPage(page)
    const text = await pdfPage.getTextContent({ disableCombineTextItems: true })
    textCache.set(k, text)
    return text
  })()
  inflight.set(k, p)
  try { return await p } finally { inflight.delete(k) }
}

export async function prewarmPageText(path: string, page: number) {
  try { await getPageText(path, page) } catch { /* ignore */ }
}

export function clearTextCache() { textCache.clear(); inflight.clear(); }

