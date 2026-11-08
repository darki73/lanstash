/**
 * Privacy: what every page may ask about the games kept private, from the frame down: how a game
 * shows (masked, revealed to this browser, or plainly), the list itself, whether this browser shows
 * hidden games, and the actions that hide and show a game and switch this browser's choice. Without
 * a provider nothing is hidden and nothing can be. HidePanel hides and shows a game from its page;
 * HiddenGames lists them for the settings page, with this browser's choice where there is one (a
 * lanstash without accounts; with them, whoever may hide games sees them, and no browser chooses).
 */
import { createContext, type ReactNode, useContext, useState } from "react"
import { type Hidden, type HiddenActions, noHiding, type Privacy, privacyOf } from "@/contracts/hidden"
import { useT } from "@/i18n"
import { Button } from "@/styled/button"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Stack } from "@/styled/stack"
import { Cell, Row, Table } from "@/styled/table"
import { Muted } from "@/styled/text"

export interface PrivacyValue {
  /** How a game (`platform`, its id there) shows. */
  readonly of: (platform: string, game: string) => Privacy
  /** The games kept private; null when lanstash keeps none, undefined until known. */
  readonly hidden: Hidden | null | undefined
  /** This browser shows hidden games. */
  readonly reveal: boolean
  readonly actions: HiddenActions
  /** Switches whether this browser shows hidden games; none: it cannot. */
  readonly setReveal?: (on: boolean) => void
}

const nothingHidden: PrivacyValue = { of: () => undefined, hidden: undefined, reveal: false, actions: noHiding }

const Context = createContext<PrivacyValue>(nothingHidden)

/** The value for a list, a choice and the actions. */
export function privacyValue(
  hidden: Hidden | null | undefined,
  reveal: boolean,
  actions: HiddenActions,
  setReveal?: (on: boolean) => void,
): PrivacyValue {
  return {
    of: (platform, game) => privacyOf(hidden, reveal, platform, game),
    hidden,
    reveal,
    actions,
    ...(setReveal === undefined ? {} : { setReveal }),
  }
}

export function PrivacyProvider({ value, children }: { readonly value: PrivacyValue; readonly children?: ReactNode }) {
  return <Context.Provider value={value}>{children}</Context.Provider>
}

export function usePrivacy(): PrivacyValue {
  return useContext(Context)
}

export interface HiddenGamesProps {
  /** A hidden game's name and its platform's, as the stash or the catalog know them. */
  readonly nameOf: (platform: string, game: string) => { readonly name: string; readonly platform: string }
}

/**
 * HiddenGames: on the settings page, the games kept private and this browser's choice: masked
 * (each listed as "A hidden game") or shown (by name); each can be shown to everyone again.
 * Nothing from a lanstash that keeps no hidden games.
 */
export function HiddenGames({ nameOf }: HiddenGamesProps) {
  const privacy = usePrivacy()
  const t = useT("privacy")
  const tCommon = useT("common")
  const [refused, setRefused] = useState<string>()
  if (privacy.hidden == null) return null
  const show = async (platform: string, game: string) => {
    const r = await privacy.actions.set(platform, game, false)
    setRefused(typeof r === "string" ? r : undefined)
  }
  return (
    <Panel
      title={t("hidden.title")}
      action={
        privacy.setReveal === undefined ? undefined : (
          <Button onClick={() => privacy.setReveal?.(!privacy.reveal)}>
            {privacy.reveal ? t("hidden.maskHere") : t("hidden.showHere")}
          </Button>
        )
      }
    >
      <Stack wide>
        <Muted>
          {!privacy.reveal
            ? t("hidden.masked")
            : privacy.setReveal === undefined
              ? t("hidden.youSee")
              : t("hidden.browserSees")}
        </Muted>
        {privacy.hidden.length === 0 ? (
          <Muted>{t("hidden.none")}</Muted>
        ) : (
          <Table columns={[t("hidden.game"), t("hidden.platform"), ""]}>
            {privacy.hidden.map((key) => {
              const [platform = "", ...rest] = key.split("/")
              const game = rest.join("/")
              const named = nameOf(platform, game)
              return (
                <Row key={key}>
                  <Cell>{privacy.reveal ? named.name : tCommon("hiddenGame")}</Cell>
                  <Cell muted>{named.platform}</Cell>
                  <Cell>
                    <Button onClick={() => void show(platform, game)}>{t("hidden.show")}</Button>
                  </Cell>
                </Row>
              )
            })}
          </Table>
        )}
        {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
      </Stack>
    </Panel>
  )
}

export interface HidePanelProps {
  readonly platform: string
  /** The game's id there (a Steam app id, a Battle.net product code). */
  readonly game: string
}

/**
 * HidePanel: on a game's page, hiding it (its name, art and builds masked on every page; it stays
 * cached and updated) or, for a hidden game this browser shows, showing it to everyone again.
 * Nothing from a lanstash that keeps no hidden games.
 */
export function HidePanel({ platform, game }: HidePanelProps) {
  const privacy = usePrivacy()
  const t = useT("privacy")
  const [busy, setBusy] = useState(false)
  const [refused, setRefused] = useState<string>()
  if (privacy.hidden == null) return null
  const hidden = privacy.of(platform, game) !== undefined
  const go = async () => {
    setBusy(true)
    const r = await privacy.actions.set(platform, game, !hidden)
    setRefused(typeof r === "string" ? r : undefined)
    setBusy(false)
  }
  return (
    <Panel title={t("panel.title")} note={hidden ? t("panel.hidden") : undefined}>
      <Stack>
        <Muted>{hidden ? t("panel.isHidden") : t("panel.hideHelp")}</Muted>
        <Button onClick={() => void go()} disabled={busy}>
          {hidden ? t("panel.show") : t("panel.hide")}
        </Button>
        {refused !== undefined && <Signal tone="bad">{refused}</Signal>}
      </Stack>
    </Panel>
  )
}
