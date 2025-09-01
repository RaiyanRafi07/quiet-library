import React from 'react'

export default function SearchBar({ value, onChange }: { value: string; onChange: (q: string) => void }) {
  return (
    <input
      autoFocus
      placeholder="Search (Ctrl/Cmd-K)"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Escape') (e.target as HTMLInputElement).blur()
      }}
      style={{
        width: '100%',
        padding: '10px 12px',
        fontSize: 16,
        borderRadius: 8,
        border: '1px solid #ccc'
      }}
    />
  )
}

