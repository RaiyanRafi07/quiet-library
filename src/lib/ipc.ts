import { invoke } from '@tauri-apps/api/tauri'
// copyCitation temporarily disabled; keep function as no-op

export type SearchResult = {
  title: string
  path: string
  page?: number
  section?: string
  snippet: string
  score: number
}

export type Bookmark = {
  id: string
  path: string
  page?: number
  section?: string
  note?: string
  createdAt: string
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
  return invoke<Bookmark[]>('list_bookmarks', { path })
}
export async function removeBookmark(id: string) {
  return invoke<void>('remove_bookmark', { id })
}
export async function revealInOS(_path: string) { /* no-op */ }
export async function openExternal(path: string, page?: number, section?: string) {
  return invoke<void>('open_external', { path, page, section })
}

export async function copyCitation(_sel: { path: string; page?: number; section?: string }) { /* no-op */ }
