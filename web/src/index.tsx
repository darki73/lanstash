/**
 * Entry: composes the app with the browser's own pieces (EventSource, fetch, POST, DELETE and PUT, the timer, the
 * window's location, the document's visibility, the browser's languages and the page's root element) and mounts it
 * into the page's #root element. The API is on the page's own origin.
 */
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { App } from "@/app"
import { compose } from "@/app/compose"
import "@/index.css"

const element = document.getElementById("root")
if (!element) throw new Error("the page has no #root element")

const composition = compose({
  apiBase: "",
  openSource: (url) => new EventSource(url),
  fetch: (url) => window.fetch(url),
  post: (url, headers) => window.fetch(url, { method: "POST", headers }),
  del: (url) => window.fetch(url, { method: "DELETE" }),
  put: (url, headers) => window.fetch(url, { method: "PUT", headers }),
  window,
  origin: window.location.origin,
  languages: navigator.languages,
  root: document.documentElement,
  later: (run, ms) => window.setTimeout(run, ms),
  storage: (() => {
    try {
      return window.localStorage
    } catch {
      return undefined
    }
  })(),
  visibility: {
    hidden: () => document.hidden,
    onChange: (run) => {
      document.addEventListener("visibilitychange", run)
      return () => document.removeEventListener("visibilitychange", run)
    },
  },
})

createRoot(element).render(
  <StrictMode>
    <App composition={composition} />
  </StrictMode>,
)
