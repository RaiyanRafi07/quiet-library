import React, { useEffect, useRef } from 'react'

type Props = { target: { path: string; section?: string }; query: string }

// Placeholder: UI will later use bundled epub.js under /public/epubjs
export default function ReaderEPUB({ target, query }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (ref.current) {
      ref.current.textContent = `EPUB viewer placeholder for ${target.path} @ section ${target.section ?? '-'} | query: ${query}`
    }
  }, [target, query])
  return <div ref={ref} style={{ padding: 'var(--sp-5)', fontFamily: 'var(--font-body)', fontSize: 'var(--fs-sm)' }} />
}
