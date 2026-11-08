/**
 * Private: on the page of one of a player's own games, whether they keep it to themselves, and the
 * switch. Kept private, others see the game as a hidden one, unless another owner did not mark it
 * (then it shows, but never who owns it); admins see all. It reads the player's list when drawn.
 */
import { useEffect, useState } from "react"
import type { PrivateActions } from "@/contracts/private"
import { useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Muted } from "@/styled/text"

export interface PrivatePanelProps {
  readonly platform: string
  readonly game: string
  readonly actions: PrivateActions
}

export function PrivatePanel({ platform, game, actions }: PrivatePanelProps) {
  const t = useT("privacy")
  const key = `${platform}/${game}`
  const [mine, setMine] = useState<readonly string[]>()
  const [busy, setBusy] = useState(false)
  const [refused, setRefused] = useState<string>()
  useEffect(() => {
    let live = true
    void actions.read().then((l) => {
      if (live) setMine(l)
    })
    return () => {
      live = false
    }
  }, [actions])
  if (mine === undefined) return null
  const kept = mine.includes(key)
  const go = async () => {
    setBusy(true)
    const r = await actions.set(platform, game, !kept)
    if (typeof r === "string") setRefused(r)
    else {
      setRefused(undefined)
      setMine(r)
    }
    setBusy(false)
  }
  return (
    <Panel title={t("own.title")} note={kept ? t("own.kept") : undefined}>
      <Stack>
        <Muted>{kept ? t("own.isKept") : t("own.help")}</Muted>
        <Button onClick={() => void go()} disabled={busy}>
          {kept ? t("own.share") : t("own.keep")}
        </Button>
        {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
      </Stack>
    </Panel>
  )
}
