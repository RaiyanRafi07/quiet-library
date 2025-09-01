import { invoke } from '@tauri-apps/api/tauri'
import { writeText } from '@tauri-apps/api/clipboard'

export type SearchResult = {
  title: string
  path: string
  page?: number
  section?: string
  snippet: string
  score: number
}

export async function addWatchedFolder(path: string) {
  return invoke<void>('add_watched_folder', { path })
}
export async function listWatchedFolders() {
  return invoke<string[]>('list_watched_folders')
}
export async function removeWatchedFolder(path: string) {
  return invoke<void>('remove_watched_folder', { path })
}
export async function reindexAll() {
  return invoke<void>('reindex_all')
}
export async function indexIncremental() {
  return invoke<void>('index_incremental')
}
export async function clearExtractCache() {
  return invoke<void>('clear_extract_cache')
}
export async function search(query: string, limit: number) {
  return invoke<SearchResult[]>('search', { query, limit })
}
export async function resolveOpenTarget(path: string, page?: number, section?: string) {
  return invoke<{ url: string; path: string; page?: number; section?: string }>('resolve_open_target', { path, page, section })
}
export async function addBookmark(path: string, page?: number, section?: string, note?: string) {
  return invoke<void>('add_bookmark', { path, page, section, note })
}
export async function listBookmarks(path?: string) {
  return invoke<Array<{ id: string; path: string; page?: number; section?: string; note?: string; createdAt: string }>>('list_bookmarks', { path })
}
export async function removeBookmark(id: string) {
  return invoke<void>('remove_bookmark', { id })
}
export async function revealInOS(path: string) {
  return invoke<void>('reveal_in_os', { path })
}
export async function openExternal(path: string, page?: number, section?: string) {
  return invoke<void>('open_external', { path, page, section })
}
export async function pdfPageCount(path: string) {
  return invoke<number>('pdf_page_count', { path })
}
export async function renderPdfPage(path: string, page: number, width: number, query?: string) {
  return invoke<{ data_url?: string; file_path?: string; width_px: number; height_px: number; width_pt: number; height_pt: number; rects_px: Array<[number, number, number, number]> }>('render_pdf_page', { path, page, width, query })
}
export async function pdfFindMatches(path: string, query: string, limit?: number) {
  return invoke<Array<{ page: number; rects: Array<[number, number, number, number]> }>>('pdf_find_matches', { path, query, limit })
}
export async function warmRenderCache(path: string, page: number, width: number) {
  return invoke<void>('warm_render_cache', { path, page, width })
}

export async function copyCitation(sel: { path: string; page?: number; section?: string }) {
  const title = sel.path.split(/[\\/]/).pop() || sel.path
  const where = sel.page ? `p.${sel.page}` : sel.section ? sel.section : ''
  const text = `${title} — ${where} — ${sel.path}`.trim()
  await writeText(text)
}

export async function pdfNextPrevPageWithMatch(path: string, query: string, fromPage: number, direction: 1 | -1, scanLimit?: number) {
  return invoke<number | null>('pdf_next_prev_page_with_match', { path, query, fromPage, direction, scanLimit })
}
