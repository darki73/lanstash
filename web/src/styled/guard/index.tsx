/**
 * Guard: draws its children, or, when drawing them failed, says so in their place: the rest of the
 * page (the sidebar, the other views) stays. Given a new `reset` (the view's path), it tries again.
 */
import { Component, type ReactNode } from "react"
import { useT } from "@/i18n"
import { Quiet } from "@/styled/text"

export interface GuardProps {
  /** Tries the children again when this changes. */
  readonly reset: string
  readonly children?: ReactNode
}

interface GuardState {
  readonly failed?: { readonly reset: string; readonly why: string }
}

export class Guard extends Component<GuardProps, GuardState> {
  override state: GuardState = {}

  static getDerivedStateFromError(e: unknown): Partial<GuardState> {
    return { failed: { reset: "", why: e instanceof Error ? e.message : String(e) } }
  }

  override componentDidCatch() {
    // The reset the failure belongs to, once known: a new one draws the children again.
    this.setState((s) => (s.failed === undefined ? s : { failed: { ...s.failed, reset: this.props.reset } }))
  }

  override render() {
    const f = this.state.failed
    if (f !== undefined && (f.reset === "" || f.reset === this.props.reset)) {
      return <Failed why={f.why} />
    }
    return this.props.children
  }
}

/** What stands in for a view that could not be drawn. */
function Failed({ why }: { readonly why: string }) {
  const t = useT("common")
  return <Quiet title={t("ui.failedView")} hint={t("ui.failedHint", { why })} />
}
