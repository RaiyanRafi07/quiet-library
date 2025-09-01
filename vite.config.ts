import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5500
  },
  build: { target: 'es2020' },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src')
    }
  }
})
