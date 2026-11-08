/**
 * Prune panel: what of a game old builds still take in the stash. Check asks lanstash what a prune
 * would delete (files no current build of any product, platform or language needs, unused for a
 * week) and what it keeps for now (used lately); Prune deletes it, asked once more, and the
 * deletion panel follows it.
 */
import { useState } from "react"
import type { Prunable } from "@/contracts/stash"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { Button } from "@/styled/button"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Muted } from "@/styled/text"

export interface PrunePanelProps {
  readonly name: string
  /** What a prune would delete, or why that is unknown. */
  readonly check: () => Promise<Prunable | string>
  /** Starts the prune; undefined when it started, else why not. */
  readonly prune: () => Promise<string | undefined>
  /** Another deletion or a prefill of the game is running. */
  readonly busy: boolean
}

export function PrunePanel({ name, check, prune, busy }: PrunePanelProps) {
  const [found, setFound] = useState<Prunable | string>()
  const [step, setStep] = useState<"idle" | "checking" | "asking" | "pruning">("idle")
  // `ask`: what to prune is asked about once found (from a fresh look, never an old one's figures).
  const look = (ask = false) => {
    setStep("checking")
    void check().then((f) => {
      setFound(f)
      setStep(ask && typeof f === "object" && f.files > 0 ? "asking" : "idle")
    })
  }
  const go = () => {
    setStep("pruning")
    void prune().then((why) => {
      setFound(why ?? undefined)
      setStep("idle")
    })
  }
  const t = useT("platforms")
  const p = typeof found === "object" ? found : undefined
  const action =
    step === "asking" && p ? (
      <>
        <Button onClick={() => setStep("idle")}>{t("prune.keep")}</Button>
        <Button onClick={go} tone="danger" label={t("prune.goLabel", { name })}>
          {t("prune.go", { size: size(p.bytes) })}
        </Button>
      </>
    ) : p && p.files > 0 ? (
      <Button onClick={() => look(true)} disabled={busy || step !== "idle"} label={t("prune.askLabel", { name })}>
        {t(step === "pruning" ? "prune.pruning" : step === "checking" ? "prune.checking" : "prune.ask")}
      </Button>
    ) : (
      <Button onClick={() => look()} disabled={step !== "idle"} label={t("prune.checkLabel", { name })}>
        {t(step === "checking" ? "prune.checking" : "prune.check")}
      </Button>
    )
  return (
    <Panel title={t("prune.title")} action={action}>
      {p === undefined ? (
        <Muted>{t("prune.idle")}</Muted>
      ) : p.files === 0 ? (
        <Muted>{t("prune.nothing")}</Muted>
      ) : (
        <Muted>{t("prune.found", { size: size(p.bytes), n: count(p.files) })}</Muted>
      )}
      {p?.compact !== undefined && p.compact > 0 && <Muted>{t("prune.compact", { n: count(p.compact) })}</Muted>}
      {p !== undefined && p.recent_bytes > 0 && <Muted>{t("prune.recent", { size: size(p.recent_bytes) })}</Muted>}
      {typeof found === "string" && <Signal tone="bad">{found}</Signal>}
    </Panel>
  )
}
