import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'
import { appWindow } from '@tauri-apps/api/window'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)

// Show the window shortly after the first paint; do not block on webfonts
;(async () => {
  try {
    // Wait a tick for React to paint, then show
    await new Promise((r) => requestAnimationFrame(() => r(null)))
  } finally {
    await appWindow.show()
  }
})()
