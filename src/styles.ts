import React from 'react';

// Neo‑Brutalist style tokens (use CSS variables from index.css)
export const buttonStyle: React.CSSProperties = {
  background: 'var(--nb-accent)',
  border: '3px solid var(--nb-border)',
  borderRadius: 'var(--radius-sm)',
  padding: 'var(--pad-button-y) var(--pad-button-x)',
  boxShadow: 'var(--nb-shadow)',
  cursor: 'pointer',
  color: '#111',
  fontWeight: 800,
  fontSize: 'var(--fs-sm)'
};

export const toolbarStyle: React.CSSProperties = {
  background: 'var(--nb-surface)',
  borderBottom: '3px solid var(--nb-border)',
  // Full-width bottom shadow with no horizontal offset to avoid left gap
  boxShadow: '0 6px 0 var(--nb-border)',
  position: 'sticky',
  top: 0,
  zIndex: 10,
};

export const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '12px 14px',
  fontSize: 'var(--fs-lg)',
  borderRadius: 'var(--radius-sm)',
  border: '3px solid var(--nb-border)',
  background: 'var(--nb-surface)',
  boxShadow: 'var(--nb-shadow)',
  outline: 'none',
  color: 'var(--nb-card-text)'
};

export const cardStyle: React.CSSProperties = {
  background: 'var(--nb-surface)',
  border: '3px solid var(--nb-border)',
  borderRadius: 'var(--radius-md)',
  boxShadow: 'var(--nb-shadow)',
  padding: 'var(--sp-4)',
  color: 'var(--nb-card-text)'
};

export const subtleLink: React.CSSProperties = {
  background: 'transparent',
  border: 'none',
  color: 'var(--nb-accent)',
  padding: 0,
  cursor: 'pointer',
  fontWeight: 800,
};

export const pageStyle: React.CSSProperties = {
  padding: 'var(--sp-6)',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--sp-5)',
  maxWidth: 1100,
  margin: '0 auto',
};
