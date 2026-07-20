import { useEffect, useMemo, useState, type MouseEvent } from 'react'
import { Copy, Minus, Square, X } from 'lucide-react'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

interface Props {
  language: string
}

export function DesktopTitleBar({ language }: Props) {
  const enabled = isTauri() && navigator.userAgent.includes('Windows')
  const appWindow = useMemo(() => (enabled ? getCurrentWindow() : null), [enabled])
  const [maximized, setMaximized] = useState(false)
  const labels = language === 'en-US'
    ? {
        controls: 'Window controls',
        minimize: 'Minimize',
        maximize: 'Maximize',
        restore: 'Restore',
        close: 'Close',
      }
    : {
        controls: '窗口控制',
        minimize: '最小化',
        maximize: '最大化',
        restore: '还原',
        close: '关闭',
      }

  useEffect(() => {
    if (!appWindow) return

    let disposed = false
    let unlisten: (() => void) | undefined
    const syncMaximized = () => {
      void appWindow.isMaximized().then((value) => {
        if (!disposed) setMaximized(value)
      })
    }

    syncMaximized()
    void appWindow.onResized(syncMaximized).then((stopListening) => {
      if (disposed) {
        stopListening()
      } else {
        unlisten = stopListening
      }
    })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [appWindow])

  if (!appWindow) return null

  const runWindowAction = (action: () => Promise<void>) => {
    void action().catch((error) => {
      console.error('Window action failed', error)
    })
  }
  const toggleMaximized = async () => {
    await appWindow.toggleMaximize()
    setMaximized(await appWindow.isMaximized())
  }
  const handleTitleBarMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    if (event.detail === 2) {
      runWindowAction(toggleMaximized)
      return
    }
    runWindowAction(() => appWindow.startDragging())
  }

  return (
    <div className="desktop-titlebar" aria-label={labels.controls}>
      <div
        className="desktop-titlebar-drag-region"
        onMouseDown={handleTitleBarMouseDown}
      />
      <div className="desktop-window-controls">
        <button
          type="button"
          className="desktop-window-control"
          title={labels.minimize}
          aria-label={labels.minimize}
          onClick={() => runWindowAction(() => appWindow.minimize())}
        >
          <Minus size={16} strokeWidth={1.6} />
        </button>
        <button
          type="button"
          className="desktop-window-control"
          title={maximized ? labels.restore : labels.maximize}
          aria-label={maximized ? labels.restore : labels.maximize}
          onClick={() => runWindowAction(toggleMaximized)}
        >
          {maximized
            ? <Copy size={13} strokeWidth={1.5} />
            : <Square size={13} strokeWidth={1.5} />}
        </button>
        <button
          type="button"
          className="desktop-window-control desktop-window-control-close"
          title={labels.close}
          aria-label={labels.close}
          onClick={() => runWindowAction(() => appWindow.close())}
        >
          <X size={17} strokeWidth={1.6} />
        </button>
      </div>
    </div>
  )
}
