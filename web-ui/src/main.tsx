import React from 'react'
import ReactDOM from 'react-dom/client'
import 'katex/dist/katex.min.css'
import App from './App'
import { initializeApiBridge } from './api'
import './index.css'

initializeApiBridge()
  .catch((error) => {
    console.error('Failed to initialize desktop API bridge', error)
  })
  .finally(() => {
    ReactDOM.createRoot(document.getElementById('root')!).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    )
  })
