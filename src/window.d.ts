// What her pages find on window that no script of theirs made: Tauri's
// (withGlobalTauri in tauri.conf.json; in the preview, its stand-in,
// tools/preview/fake-tauri.js), and what Rust calls on the page. Only what the
// pages use of each.

interface TauriEvent<T> {
  event: string
  id: number
  payload: T
}

// listen()'s target: this window alone (Tauri 2 sends to every window else).
interface TauriTarget {
  kind: 'Any' | 'AnyLabel' | 'App' | 'Window' | 'Webview' | 'WebviewWindow'
  label?: string
}

interface Window {
  __TAURI__: {
    core: {
      invoke<T = any>(cmd: string, args?: Record<string, unknown>): Promise<T>
    }
    event: {
      listen<T = any>(event: string, handler: (event: TauriEvent<T>) => void, options?: { target?: string | TauriTarget }): Promise<() => void>
    }
    webviewWindow?: {
      getCurrentWebviewWindow?(): { label: string }
    }
  }
  __TAURI_INTERNALS__?: {
    metadata?: { currentWebview?: { label?: string } }
  }
  // /debug/eval (debug.rs): Rust hands the page a function to run and takes
  // back what it gives (bridge.ts, desk.ts).
  __wkEval?: (id: string | number, fn: () => unknown) => void
}
