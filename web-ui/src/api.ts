let backendBaseUrl: string | null = null
let initPromise: Promise<void> | null = null

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

export function initializeApiBridge(): Promise<void> {
  initPromise ??= initialize()
  return initPromise
}

export function apiUrl(path: string): string {
  if (!backendBaseUrl) return path

  try {
    const url = new URL(path, window.location.origin)
    if (url.origin !== window.location.origin || !url.pathname.startsWith('/api')) return path
    return `${backendBaseUrl}${url.pathname}${url.search}${url.hash}`
  } catch {
    return path.startsWith('/api') ? `${backendBaseUrl}${path}` : path
  }
}

export function wsUrl(path: string): string {
  if (!backendBaseUrl) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    return `${protocol}//${window.location.host}${path}`
  }
  return `${backendBaseUrl.replace(/^http/, 'ws')}${path}`
}

export async function getDesktopDataDir(): Promise<string | null> {
  const { invoke, isTauri } = await import('@tauri-apps/api/core')
  if (!isTauri()) return null
  return invoke<string>('get_data_dir')
}

export async function openDesktopDataDir(): Promise<void> {
  const { invoke, isTauri } = await import('@tauri-apps/api/core')
  if (!isTauri()) return
  await invoke('open_data_dir')
}

export async function openExternalUrl(url: string): Promise<boolean> {
  const { invoke, isTauri } = await import('@tauri-apps/api/core')
  if (!isTauri()) return false
  await invoke('open_external_url', { url })
  return true
}

async function initialize() {
  const { invoke, isTauri } = await import('@tauri-apps/api/core')
  if (!isTauri()) return

  backendBaseUrl = await invoke<string>('get_backend_url')
  patchFetch()
  patchXhr()
}

function patchFetch() {
  const originalFetch = window.fetch.bind(window)
  window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
    if (typeof input === 'string') return originalFetch(apiUrl(input), init)
    if (input instanceof URL) return originalFetch(apiUrl(input.toString()), init)
    if (input instanceof Request) {
      const nextUrl = apiUrl(input.url)
      if (nextUrl !== input.url) return originalFetch(new Request(nextUrl, input), init)
    }
    return originalFetch(input, init)
  }
}

function patchXhr() {
  const originalOpen = XMLHttpRequest.prototype.open
  XMLHttpRequest.prototype.open = function open(
    method: string,
    url: string | URL,
    async?: boolean,
    username?: string | null,
    password?: string | null,
  ) {
    return originalOpen.call(this, method, apiUrl(url.toString()), async ?? true, username ?? null, password ?? null)
  }
}
