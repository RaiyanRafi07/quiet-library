import React from 'react'
import { inputStyle } from '@/styles'

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
      style={inputStyle}
    />
  )
}
