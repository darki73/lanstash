/**
 * Live: the adapter for a stream lanstash sends as server-sent events (the status on
 * /api/v1/events). Each message is one JSON value; a message that is not JSON, or not one the
 * follower accepts, is skipped. While
 * the connection is down the follower is told it is paused; the browser opens it again by itself
 * (lanstash asks for a retry after two seconds), and one the browser gives up on is opened again
 * here after a pause. A page no one sees holds no stream: it is closed while the page is hidden
 * and opened again when it is shown (a browser keeps only six connections to a host, and every
 * open tab's stream is one of them: with more tabs than that, the others' requests waited for
 * ever). The event-source maker and the page's visibility are passed in: the browser's
 * EventSource and document in the app, fakes in tests.
 */

/** The part of the browser's EventSource this adapter uses. */
export interface Source {
  readonly readyState: number
  addEventListener(type: string, listener: (event: MessageEvent<string>) => void): void
  close(): void
}

export type OpenSource = (url: string) => Source

/** What a stream tells its follower. */
export interface LiveHandlers<E> {
  event(e: E): void
  /** Whether a message is one the follower reads; one that is not is skipped (none: every JSON value). */
  accept?(v: unknown): v is E
  /** The connection is down and being opened again. */
  paused?(): void
}

/** Whether the page is hidden, and its changes (the browser's document.hidden and visibilitychange). */
export interface Visibility {
  hidden(): boolean
  /** Calls `run` on each change; returns what stops that. */
  onChange(run: () => void): () => void
}

export interface LiveOptions {
  /** The pause before opening a connection the browser gave up on, in milliseconds. */
  retry?: number
  later?: (run: () => void, ms: number) => unknown
  /** The page's visibility; none: always seen. */
  visibility?: Visibility
}

/** EventSource's readyState once the browser gave up on a connection. */
const closed = 2

/** follow opens the stream at url and returns the function that stops following it. */
export function follow<E>(open: OpenSource, url: string, h: LiveHandlers<E>, o: LiveOptions = {}): () => void {
  const retry = o.retry ?? 2000
  const later = o.later ?? ((run, ms) => setTimeout(run, ms))
  let stopped = false
  let source: Source | undefined
  const hidden = () => o.visibility?.hidden() === true
  const connect = () => {
    if (stopped || source !== undefined || hidden()) return
    const s = open(url)
    source = s
    s.addEventListener("message", (e) => {
      let value: unknown
      try {
        value = JSON.parse(e.data)
      } catch {
        return
      }
      if (h.accept !== undefined && !h.accept(value)) return
      h.event(value as E)
    })
    s.addEventListener("error", () => {
      if (stopped || source !== s) return
      h.paused?.()
      if (s.readyState === closed) {
        s.close()
        source = undefined
        later(connect, retry)
      }
    })
  }
  const close = () => {
    source?.close()
    source = undefined
  }
  const unwatch = o.visibility?.onChange(() => {
    if (stopped) return
    if (hidden()) close()
    else connect()
  })
  connect()
  return () => {
    stopped = true
    unwatch?.()
    close()
  }
}
