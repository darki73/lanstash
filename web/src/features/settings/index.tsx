/**
 * Settings: the settings the web UI may change, in groups (Updates, Steam, Battle.net, Storage),
 * read from lanstash when the page opens. Each one has its label, a line of help, where its value
 * comes from (default, config file, changed here, command-line flag) and its controls: a choice
 * between its values where there is a short list (with a choice of its own value where lanstash
 * offers one, such as automatic updates' daily window, which then shows a box to type it in), a
 * dropdown for a long list, several picked of a list where the value is several (Steam's languages:
 * each a pill to drop, a dropdown to add one), a dropdown per part of a value in parts
 * (Battle.net's platform, bitness and language; tags it does not know are kept), a box to type it in
 * otherwise, Save while the typed value differs, and Reset once it was changed here. A setting a
 * command-line flag gives cannot be changed here, and one who may not change settings sees only
 * the values. Why lanstash refused
 * a change is shown at the top. A change applies at the next start: once one is made, a bar at the
 * bottom offers Restart; while lanstash is at work (a prefill, a client downloading) it says so and
 * offers Restart anyway. Once the restarted lanstash is back (it says another start time), the bar
 * goes and the settings are read again.
 */
import { type ReactNode, useEffect, useState } from "react"
import { noSettings, type Setting, type SettingSource, type SettingsActions, source } from "@/contracts/settings"
import { useAccess } from "@/features/access"
import { settingWords } from "@/features/settings/words"
import { useT } from "@/i18n"
import { ActionBar } from "@/styled/actionbar"
import { Button } from "@/styled/button"
import { Field, Many, Select, TextInput } from "@/styled/field"
import { Panel } from "@/styled/panel"
import { Segmented } from "@/styled/segmented"
import { Signal } from "@/styled/signal"
import { Muted, Quiet } from "@/styled/text"
import { View } from "@/styled/view"

interface RowProps {
  readonly s: Setting
  readonly save: (key: string, value: string) => void
  readonly reset: (key: string) => void
  /** Shown only (no controls): the reader may not change settings. */
  readonly readOnly: boolean
}

function Row({ s, save, reset, readOnly }: RowProps) {
  const t = useT("settings")
  const w = settingWords(t)
  const label = w.label(s)
  const [draft, setDraft] = useState<string>()
  const value = s.value ?? ""
  const typed = draft ?? value
  const dirty = draft !== undefined && draft !== value
  const from: SettingSource = source(s)
  const commit = () => {
    if (dirty) save(s.key, typed)
    setDraft(undefined)
  }
  const badge =
    from === "ui" ? (
      <Signal tone="waiting" still>
        {t(`source.${from}`)}
      </Signal>
    ) : (
      <Muted>{t(`source.${from}`)}</Muted>
    )
  const parts = s.parts ?? []
  const custom = s.custom ?? undefined
  const many = s.multi === true && s.choices.length > 0
  // A long list is a dropdown; a short one is a row of choices (with the custom choice).
  const long = !many && s.choices.length > 6
  const listed = s.choices.includes(value)
  const [own, setOwn] = useState(false)
  const picking = custom !== undefined && (own || (!listed && value !== ""))
  const choices = [...s.choices, ...(custom === undefined ? [] : [custom.label])]
  const typeable =
    !many && parts.length === 0 && !long && (s.choices.length === 0 || picking || (!listed && custom === undefined))
  const pick = (c: string) => {
    setDraft(undefined)
    if (c === custom?.label) setOwn(true)
    else {
      setOwn(false)
      save(s.key, c)
    }
  }
  return (
    <Field label={label} help={w.help(s)} badge={badge}>
      {s.flag || readOnly ? (
        <Muted>{value === "" ? "–" : value}</Muted>
      ) : (
        <>
          {parts.length > 0 && <Parts s={s} value={value} save={save} />}
          {many && (
            <Many
              label={label}
              values={value
                .split(",")
                .map((v) => v.trim())
                .filter(Boolean)}
              options={s.choices}
              onChange={(vs) => save(s.key, vs.join(","))}
            />
          )}
          {long && (
            <Select
              label={label}
              value={value}
              options={listed || value === "" ? s.choices : [value, ...s.choices]}
              onChange={(v) => save(s.key, v)}
            />
          )}
          {!many && parts.length === 0 && !long && choices.length > 0 && (
            <Segmented
              label={label}
              options={choices.map((c) => ({ id: c, label: w.choice(c) }))}
              value={picking ? (custom?.label ?? "") : listed ? value : ""}
              onChange={pick}
            />
          )}
          {typeable && (
            <TextInput
              label={t("row.value", { label })}
              value={typed}
              placeholder={(custom === undefined ? undefined : w.hint(s)) ?? s.default ?? ""}
              options={long ? s.choices : undefined}
              onChange={setDraft}
              onEnter={commit}
              onEscape={() => setDraft(undefined)}
            />
          )}
          {dirty && (
            <Button onClick={commit} tone="accent" label={t("row.saveLabel", { label })}>
              {t("row.save")}
            </Button>
          )}
          {s.ui !== null && (
            <Button onClick={() => reset(s.key)} label={t("row.resetLabel", { label })}>
              {t("row.reset")}
            </Button>
          )}
        </>
      )}
    </Field>
  )
}

/**
 * A comma-separated value in parts (Battle.net's tags: platform, bitness, language), each a dropdown
 * of its values; tags the parts do not know (optional content) are kept after them.
 */
function Parts({ s, value, save }: { s: Setting; value: string; save: (key: string, value: string) => void }) {
  const t = useT("settings")
  const w = settingWords(t)
  const parts = s.parts ?? []
  const tokens = value
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean)
  const known = new Set(parts.flatMap((p) => p.choices))
  const picked = parts.map((p) => tokens.find((x) => p.choices.includes(x)) ?? p.choices[0] ?? "")
  const others = tokens.filter((x) => !known.has(x))
  return (
    <>
      {parts.map((p, i) => (
        <Select
          key={p.label}
          label={t("row.part", { label: w.label(s), part: w.part(p.label) })}
          value={picked[i] ?? ""}
          options={p.choices}
          onChange={(v) => save(s.key, [...picked.map((x, j) => (j === i ? v : x)), ...others].join(","))}
        />
      ))}
      {others.length > 0 && <Muted>{`+ ${others.join(", ")}`}</Muted>}
    </>
  )
}

export interface SettingsViewProps {
  readonly actions?: SettingsActions
  /** When the running lanstash started (the live status's started_ms): a new one ends a restart. */
  readonly started?: number
  /** More parts after the settings (such as the hidden games). */
  readonly more?: ReactNode
}

/** Where a restart stands; `from`: when the lanstash that was asked to restart had started. */
type Restart = { step: "idle" | "busy"; why?: string } | { step: "restarting"; from: number | undefined }

export function SettingsView({ actions = noSettings, started, more }: SettingsViewProps) {
  const { can } = useAccess()
  const may = can("settings")
  const t = useT("settings")
  const [list, setList] = useState<readonly Setting[]>()
  const [problem, setProblem] = useState<string>()
  const [changed, setChanged] = useState(false)
  const [restart, setRestart] = useState<Restart>({ step: "idle" })
  const [reads, setReads] = useState(0)
  useEffect(() => {
    void reads
    void actions.read().then((a) => {
      if (typeof a === "string") setProblem(a)
      else setList(a)
    })
  }, [actions, reads])
  // Back from the restart (another start time): the changes apply now; the settings are read again.
  useEffect(() => {
    if (restart.step !== "restarting" || started === undefined || started === restart.from) return
    setRestart({ step: "idle" })
    setChanged(false)
    setReads((n) => n + 1)
  }, [restart, started])
  const apply = (done: Promise<readonly Setting[] | string>) =>
    void done.then((a) => {
      if (typeof a === "string") {
        setProblem(a)
        return
      }
      setProblem(undefined)
      setList(a)
      setChanged(true)
    })
  const go = (force: boolean) =>
    void actions
      .restart(force)
      .then((why) => setRestart(why === undefined ? { step: "restarting", from: started } : { step: "busy", why }))
  if (list === undefined) {
    return (
      <View title={t("title")} subtitle={t("subtitle.short")}>
        {problem === undefined ? <Muted>{t("asking")}</Muted> : <Quiet title={t("none")} hint={problem} />}
      </View>
    )
  }
  const groups = [...new Set(list.map((s) => s.group))]
  return (
    <View title={t("title")} subtitle={t("subtitle.full")}>
      {!may && <Signal tone="waiting">{t("signIn")}</Signal>}
      {problem !== undefined && <Signal tone="bad">{problem}</Signal>}
      {groups.map((g) => (
        <Panel key={g} title={settingWords(t).group(g)}>
          {list
            .filter((s) => s.group === g)
            .map((s) => (
              <Row
                key={s.key}
                s={s}
                save={(k, v) => apply(actions.set(k, v))}
                reset={(k) => apply(actions.reset(k))}
                readOnly={!may}
              />
            ))}
        </Panel>
      ))}
      {more}
      {changed && (
        <ActionBar
          label={t("restart.label")}
          actions={
            restart.step === "busy" ? (
              <Button onClick={() => go(true)} tone="danger">
                {t("restart.anyway")}
              </Button>
            ) : restart.step === "restarting" ? (
              <Button onClick={() => {}} disabled label={t("restart.restartingLabel")}>
                {t("restart.restarting")}
              </Button>
            ) : (
              <Button onClick={() => go(false)} tone="accent" label={t("restart.goLabel")}>
                {t("restart.go")}
              </Button>
            )
          }
        >
          {restart.step === "restarting" ? <Muted>{t("restart.back")}</Muted> : <Muted>{t("restart.applies")}</Muted>}
          {restart.step === "busy" && (
            <Signal tone="bad">
              {t("restart.notNow", { why: ("why" in restart && restart.why) || t("restart.atWork") })}
            </Signal>
          )}
        </ActionBar>
      )}
    </View>
  )
}
