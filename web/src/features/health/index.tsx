/**
 * Health: the checks lanstash makes of itself, as `lanstash doctor` prints them for the service:
 * what the store holds and what loading it could not take, the files it has open against its
 * limit, the store disk's free space, what the background check found. Each is a row with a
 * signal (ok, warning, failure: the word is always there), what it is about and the detail; over
 * them, the worst of them. Asked when the page opens and every minute while it is open.
 */
import { useEffect, useState } from "react"
import { type HealthCheck, type HealthLevel, type HealthReader, worst } from "@/contracts/health"
import { useT } from "@/i18n"
import { Checklist } from "@/styled/checklist"
import { Panel } from "@/styled/panel"
import { Signal, type SignalTone } from "@/styled/signal"
import { Quiet } from "@/styled/text"
import { View } from "@/styled/view"

const tones: Readonly<Record<HealthLevel, SignalTone>> = { ok: "good", warn: "waiting", fail: "bad" }

export interface HealthViewProps {
  readonly health: HealthReader
  /** How often it is asked again (ms); a test passes none. */
  readonly every?: number
}

export function HealthView({ health, every = 60_000 }: HealthViewProps) {
  const [checks, setChecks] = useState<readonly HealthCheck[] | undefined | null>(null)
  useEffect(() => {
    let live = true
    const ask = () =>
      void health().then((c) => {
        if (live) setChecks(c)
      })
    ask()
    const timer = every > 0 ? setInterval(ask, every) : undefined
    return () => {
      live = false
      if (timer !== undefined) clearInterval(timer)
    }
  }, [health, every])
  const t = useT("health")
  const level = checks ? worst(checks) : undefined
  return (
    <View title={t("title")} subtitle={t("subtitle")} aside={level === undefined ? undefined : t(`summary.${level}`)}>
      <Panel title={t("checks")}>
        {checks === null ? (
          <Quiet title={t("asking")} />
        ) : checks === undefined ? (
          <Quiet title={t("none.title")} hint={t("none.hint")} />
        ) : (
          <Checklist
            label={t("checks")}
            items={checks.map((c) => ({
              key: c.what,
              badge: (
                <Signal tone={tones[c.level]} still>
                  {t(`level.${c.level}`)}
                </Signal>
              ),
              title: c.what,
              detail: c.detail,
            }))}
          />
        )}
      </Panel>
    </View>
  )
}
