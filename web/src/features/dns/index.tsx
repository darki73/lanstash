/**
 * DNS: the network's DNS server, which sends the game CDN names to lanstash, set up and checked from
 * here. Over the page a band says how many of the names lanstash wants the server sends to it (and how
 * many go to the internet). Once a server is set up it shows in a line (where, HTTP or HTTPS and how
 * its certificate is trusted, as whom, what the rewrites answer) with Change, which opens the form;
 * before, the form is open. The names stand as tags: missing (they go to the internet), there twice,
 * sent to lanstash by the server's own rules or a wider rewrite (no rewrite added; the rule shows on
 * hover), and those that point at lanstash for another name (left alone). Server: its kind (the kinds lanstash knows), address, HTTP or HTTPS, port (the kind's usual
 * one for it to start with), over HTTPS how its certificate is trusted (checked against the public
 * authorities, one certificate trusted by its SHA-256, or not checked, with what that risks; a test
 * that meets an untrusted certificate offers to trust exactly it and test again), user (for a kind
 * that signs in with one; Pi-hole takes a password alone), password
 * (hidden; once one is kept, left empty it stays as it is) and what the rewrites answer: lanstash's
 * address, or a name that leads to it (empty: as its names' rewrites already do); Test tries the setup
 * without keeping it, Save tries and keeps it, Forget drops it. Once a server is set up: Rewrites says
 * how many of the names lanstash wants are rewritten to its address, lists those missing and those
 *  there twice, and in a last tab of their own those that point at lanstash for names it does not want (left alone), with a bar
 * to add the missing ones; Coverage lists the names of a cached service that clients looked up and no
 * rewrite answered (they went to the internet), and the hosts lanstash refused since it started. What
 * the last action did, or why it was refused, is said at the top. A lanstash older than the page
 * (its API has no DNS paths) is said to be so.
 */
import { useEffect, useState } from "react"
import {
  type DnsActions,
  type DnsConfig,
  type DnsDone,
  type DnsPage,
  type DnsReport,
  noDns,
  untrustedIn,
} from "@/contracts/dns"
import { useAccess } from "@/features/access"
import { platforms } from "@/features/platforms/model"
import { useT, type Words } from "@/i18n"
import { count } from "@/lib/count"
import { ActionBar } from "@/styled/actionbar"
import { Button } from "@/styled/button"
import { Field, Select, TextInput } from "@/styled/field"
import { Ledger } from "@/styled/ledger"
import { Panel, PanelGrid } from "@/styled/panel"
import { Segmented } from "@/styled/segmented"
import { Signal } from "@/styled/signal"
import { Cell, Row, Table } from "@/styled/table"
import { Muted, Quiet } from "@/styled/text"
import { View } from "@/styled/view"

/** How an HTTPS server's certificate is trusted: the public roots, one pinned certificate, or not at all. */
type Trust = "check" | "pin" | "none"

const trusts: readonly Trust[] = ["check", "pin", "none"]

/** The form's fields, as typed. */
interface Draft {
  readonly kind: string
  readonly host: string
  readonly port: string
  readonly https: boolean
  readonly trust: Trust
  readonly pin: string
  readonly user: string
  readonly password: string
  readonly answer: string
  /** The names point at the address, or at a name that points at it. */
  readonly pointAt: "address" | "name"
  readonly via: string
}

function draftOf(page: DnsPage): Draft {
  const c = page.setup.config
  const kind = c?.kind ?? page.setup.kinds[0]?.id ?? ""
  const port = c?.port ?? page.setup.kinds.find((k) => k.id === kind)?.port ?? 0
  const trust: Trust = c?.unchecked ? "none" : c?.pin ? "pin" : "check"
  return {
    kind,
    host: c?.host ?? "",
    port: String(port),
    https: c?.https ?? false,
    trust,
    pin: c?.pin ?? "",
    user: c?.user ?? "",
    password: "",
    answer: c?.answer ?? "",
    pointAt: c?.via ? "name" : "address",
    via: c?.via ?? "",
  }
}

function configOf(d: Draft): DnsConfig {
  return {
    kind: d.kind,
    host: d.host.trim(),
    port: Number.parseInt(d.port, 10) || 0,
    https: d.https,
    pin: d.https && d.trust === "pin" && d.pin.trim() !== "" ? d.pin.trim() : null,
    unchecked: d.https && d.trust === "none",
    user: d.user.trim(),
    answer: d.answer.trim() === "" ? null : d.answer.trim(),
    via: d.pointAt === "name" && d.via.trim() !== "" ? d.via.trim() : null,
  }
}

/** What the last action did, in words. */
function said(done: DnsDone, t: Words<"dns">): string {
  if ("added" in done) {
    const rewrites = t("done.rewrites", { count: done.added, n: count(done.added) })
    return done.deduplicated > 0
      ? t("done.addedMerged", {
          rewrites,
          duplicates: t("done.duplicates", { count: done.deduplicated, n: count(done.deduplicated) }),
        })
      : t("done.added", { rewrites })
  }
  const rewrites = t("done.rewrites", { count: done.rewrites, n: count(done.rewrites) })
  return done.kept
    ? t("done.saved", { server: done.server, rewrites })
    : t("done.tested", { server: done.server, rewrites })
}

/** A setup in a line: where, how, as whom, and what the rewrites answer. */
function described(c: DnsConfig, report: DnsReport | null, t: Words<"dns">): string {
  const tls = c.https
    ? c.unchecked
      ? t("described.httpsUnchecked")
      : c.pin
        ? t("described.httpsPinned")
        : t("described.https")
    : t("described.http")
  const writes = report?.plan?.write_as ?? c.answer
  const as = c.user === "" ? t("described.password") : t("described.user", { user: c.user })
  return [
    t("described.address", { host: c.host, port: c.port }),
    tls,
    as,
    writes ? t("described.answer", { answer: writes }) : undefined,
  ]
    .filter((s) => s !== undefined)
    .join(" · ")
}

/** The band over the page: how many of the names lanstash wants the server sends to it. */
function Summary({ report }: { report: DnsReport }) {
  const t = useT("dns")
  const p = report.plan
  if (p === null) return null
  const fixed = p.wanted - p.missing.length
  const share = (n: number) => (p.wanted === 0 ? 0 : n / p.wanted)
  return (
    <Ledger
      label={t("summary.label")}
      headline={t("summary.headline", { fixed: count(fixed), wanted: count(p.wanted) })}
      caption={t("summary.caption", { server: report.server, answer: p.answer })}
      split={[
        { label: t("summary.toLanstash"), value: count(fixed), share: share(fixed), tone: "cache" },
        {
          label: t("summary.toInternet"),
          value: count(p.missing.length),
          share: share(p.missing.length),
          tone: "origin",
        },
      ]}
      rows={[]}
    />
  )
}

/** What the server names when a wider rewrite, not a rule, covers a name. */
const byRewrite = "another rewrite"

/** The tab of the names that point at lanstash but are on none of its lists. */
const notOurs = "not-ours"

/** The services in the order the platforms are shown. */
const rank = ["steam", "blizzard", "epicgames", "riot"]

/** A service by its platform's name. */
const serviceName = (s: string) => platforms([]).find((p) => p.id === s)?.name ?? s

function Rewrites({ report }: { report: DnsReport }) {
  const t = useT("dns")
  const [tab, setTab] = useState<string>()
  const p = report.plan
  if (p === null) return <Quiet title={t("rewrites.noPlan")} hint={report.plan_error ?? undefined} />
  const fixed = p.wanted - p.missing.length
  const from = p.answer_from === "config" ? t("rewrites.fromConfig") : t("rewrites.fromRewrites")
  // The names by service, a tab each (from an older lanstash: the missing ones, in one tab).
  const services = p.services ?? Object.fromEntries(p.missing.map((n) => [n, "names"]))
  // Names that point at lanstash but are on none of its lists: a tab of their own, last.
  const order = [
    ...[...new Set(Object.values(services))].sort(
      (a, b) => (rank.indexOf(a) + 1 || 99) - (rank.indexOf(b) + 1 || 99) || a.localeCompare(b),
    ),
    ...(p.extra.length > 0 ? [notOurs] : []),
  ]
  const shown = order.includes(tab ?? "") ? (tab as string) : (order[0] ?? "")
  const names: [string, string][] =
    shown === notOurs ? p.extra.map((n) => [n, notOurs]) : Object.entries(services).filter(([, s]) => s === shown)
  const missing = (s: string) => Object.entries(services).filter(([n, x]) => x === s && p.missing.includes(n)).length
  const label = (s: string) =>
    s === notOurs
      ? t("rewrites.tabNotOurs", { n: count(p.extra.length) })
      : missing(s) > 0
        ? t("rewrites.tabMissing", { service: serviceName(s), n: missing(s) })
        : serviceName(s)
  const via = p.via ?? undefined
  const covered = p.covered ?? {}
  const ruled = Object.keys(covered).length
  return (
    <>
      <Muted>{t("rewrites.stand", { fixed: count(fixed), wanted: count(p.wanted), answer: p.answer, from })}</Muted>
      {via !== undefined && (
        <Signal tone={p.via_missing ? "waiting" : "good"} still>
          {p.via_missing
            ? t("rewrites.viaMissing", { via, answer: p.answer })
            : t("rewrites.via", { via, answer: p.answer })}
        </Signal>
      )}
      {order.length > 0 && (
        <Segmented
          label={t("rewrites.service")}
          options={order.map((s) => ({ id: s, label: label(s) }))}
          value={shown}
          onChange={setTab}
        />
      )}
      <Table columns={[t("rewrites.name"), t("rewrites.rewrite")]}>
        {names.map(([n]) => (
          <Row key={n}>
            <Cell path>{n}</Cell>
            <Cell>
              {shown === notOurs ? (
                <Signal tone="waiting" still title={t("rewrites.notOursTitle")}>
                  {t("rewrites.notOurs")}
                </Signal>
              ) : p.missing.includes(n) ? (
                <Signal tone="bad">{t("rewrites.missing")}</Signal>
              ) : p.duplicates.includes(n) ? (
                <Signal tone="waiting">{t("rewrites.twice")}</Signal>
              ) : covered[n] !== undefined ? (
                <Signal tone="good" still title={covered[n]}>
                  {covered[n] === byRewrite ? t("rewrites.byWider") : t("rewrites.byRule")}
                </Signal>
              ) : (
                <Signal tone="good" still>
                  {t("rewrites.to", { answer: p.write_as })}
                </Signal>
              )}
            </Cell>
          </Row>
        ))}
      </Table>
      <Muted>
        {[
          p.missing.length > 0 ? t("rewrites.totalMissing", { n: count(p.missing.length) }) : undefined,
          ruled > 0 ? t("rewrites.totalRuled", { n: count(ruled), server: report.server }) : undefined,
          p.duplicates.length > 0 ? t("rewrites.totalTwice", { n: count(p.duplicates.length) }) : undefined,
          p.extra.length > 0 ? t("rewrites.totalNotOurs", { n: count(p.extra.length) }) : undefined,
        ]
          .filter((s) => s !== undefined)
          .join(" · ")}
      </Muted>
    </>
  )
}

/** The Apply button of the rewrites, in their panel's header, while something is missing or there twice. */
function ApplyButton({ report, apply }: { report: DnsReport; apply: () => void }) {
  const t = useT("dns")
  const p = report.plan
  // The name the names point at, when it has no rewrite yet, is added first.
  const adds = (p?.missing.length ?? 0) + (p?.via_missing ? 1 : 0)
  if (p === null || adds + p.duplicates.length === 0) return null
  return (
    <Button onClick={apply} tone="accent" label={t("rewrites.applyLabel", { n: count(adds), server: report.server })}>
      {t("rewrites.apply", { count: adds, n: count(adds) })}
    </Button>
  )
}

function Coverage({ report }: { report: DnsReport }) {
  const t = useT("dns")
  return (
    <PanelGrid wide>
      <Panel
        title={t("coverage.missed")}
        note={report.lookups_error ?? t("coverage.missedNote", { n: count(report.lookups), server: report.server })}
      >
        {report.missed.length === 0 ? (
          <Quiet title={t("coverage.none")} hint={t("coverage.noneHint")} />
        ) : (
          <Table columns={[t("coverage.name"), t("coverage.platform"), t("coverage.lookups"), t("coverage.clients")]}>
            {report.missed.map((m) => (
              <Row key={m.name}>
                <Cell path>{m.name}</Cell>
                <Cell>{m.service}</Cell>
                <Cell>{count(m.count)}</Cell>
                <Cell muted>{m.clients.join(", ")}</Cell>
              </Row>
            ))}
          </Table>
        )}
      </Panel>
      <Panel title={t("coverage.refused")} note={t("coverage.refusedNote")}>
        {report.refused.length === 0 ? (
          <Quiet title={t("coverage.none")} />
        ) : (
          <Table columns={[t("coverage.host"), t("coverage.refusedCount")]}>
            {report.refused.map((r) => (
              <Row key={r.host}>
                <Cell path>{r.host}</Cell>
                <Cell>{count(r.count)}</Cell>
              </Row>
            ))}
          </Table>
        )}
      </Panel>
    </PanelGrid>
  )
}

export interface DnsViewProps {
  readonly actions?: DnsActions
}

export function DnsView({ actions = noDns }: DnsViewProps) {
  const { can } = useAccess()
  const canChange = can("dns")
  const t = useT("dns")
  const [page, setPage] = useState<DnsPage>()
  const [draft, setDraft] = useState<Draft>()
  const [problem, setProblem] = useState<string>()
  const [busy, setBusy] = useState(false)
  const [editing, setEditing] = useState(false)
  const take = (done: Promise<DnsPage | string>, keepDraft: boolean) => {
    setBusy(true)
    void done.then((a) => {
      setBusy(false)
      if (typeof a === "string") {
        setProblem(a)
        return
      }
      setProblem(undefined)
      setPage(a)
      if (!keepDraft) {
        setDraft(draftOf(a))
        setEditing(false)
      }
    })
  }
  useEffect(() => {
    void actions.read().then((a) => {
      if (typeof a === "string") setProblem(a)
      else {
        setPage(a)
        setDraft(draftOf(a))
      }
    })
  }, [actions])
  const subtitle = t("subtitle")
  if (page === undefined || draft === undefined) {
    return (
      <View title={t("title")} subtitle={subtitle}>
        {problem === undefined ? (
          <Muted>{t("asking")}</Muted>
        ) : problem === "not found" ? (
          <Quiet title={t("older.title")} hint={t("older.hint")} />
        ) : (
          <Quiet title={t("noPage")} hint={problem} />
        )}
      </View>
    )
  }
  const set = (k: keyof Draft) => (v: string) => setDraft({ ...draft, [k]: v })
  const kindName = (id: string) => page.setup.kinds.find((k) => k.id === id)?.name ?? id
  const saveWith = (d: Draft, test: boolean) =>
    take(actions.save(configOf(d), d.password === "" ? undefined : d.password, test), test)
  const save = (test: boolean) => saveWith(draft, test)
  // A test that met a certificate it does not trust: that certificate, to trust with one click.
  const untrusted = problem === undefined ? undefined : untrustedIn(problem)
  const trustIt = (fp: string) => {
    const d = { ...draft, https: true, trust: "pin" as const, pin: fp }
    setDraft(d)
    saveWith(d, true)
  }
  const kindOf = page.setup.kinds.find((k) => k.id === draft.kind)
  // The form while changing the setup, or before there is one; else the setup in a line.
  const showForm = editing || page.setup.config === null
  const setHttps = (https: boolean) => {
    // The kind's usual port for the other protocol, when the port was the usual one for this.
    const from = https ? kindOf?.port : kindOf?.https_port
    const to = https ? kindOf?.https_port : kindOf?.port
    const port = to !== undefined && (draft.port === String(from) || draft.port === "") ? String(to) : draft.port
    setDraft({ ...draft, https, port })
  }
  return (
    <View title={t("title")} subtitle={subtitle}>
      {!canChange && <Signal tone="waiting">{t("signIn")}</Signal>}
      {problem !== undefined && <Signal tone="bad">{problem}</Signal>}
      {untrusted !== undefined && (
        <ActionBar
          label={t("untrusted.label")}
          actions={
            <Button onClick={() => trustIt(untrusted)} tone="accent" label={t("untrusted.trustLabel")}>
              {t("untrusted.trust")}
            </Button>
          }
        >
          <Muted>{t("untrusted.text", { fingerprint: untrusted })}</Muted>
        </ActionBar>
      )}
      {problem === undefined && page.done !== null && <Signal tone="good">{said(page.done, t)}</Signal>}
      {page.report !== null && <Summary report={page.report} />}
      <Panel
        title={t("server.title")}
        note={
          page.setup.config === null
            ? t("server.notSetUp")
            : t("server.at", { kind: kindName(page.setup.config.kind), host: page.setup.config.host })
        }
        action={
          showForm ? undefined : (
            <Button onClick={() => setEditing(true)} label={t("server.changeLabel")}>
              {t("server.change")}
            </Button>
          )
        }
      >
        {!showForm && page.setup.config !== null && <Muted>{described(page.setup.config, page.report, t)}</Muted>}
        {showForm && (
          <>
            <Field label={t("form.kind")} help={t("form.kindHelp")}>
              <Select
                label={t("form.kind")}
                value={draft.kind}
                options={page.setup.kinds.map((k) => k.id)}
                names={Object.fromEntries(page.setup.kinds.map((k) => [k.id, k.name]))}
                onChange={(v) =>
                  setDraft({
                    ...draft,
                    kind: v,
                    port: String(page.setup.kinds.find((k) => k.id === v)?.port ?? draft.port),
                  })
                }
              />
            </Field>
            <Field label={t("form.address")} help={t("form.addressHelp")}>
              <TextInput
                label={t("form.address")}
                value={draft.host}
                onChange={set("host")}
                placeholder="192.168.1.53"
              />
            </Field>
            <Field label={t("form.connection")} help={t("form.connectionHelp")}>
              <Segmented
                label={t("form.connection")}
                options={[
                  { id: "http", label: "HTTP" },
                  { id: "https", label: "HTTPS" },
                ]}
                value={draft.https ? "https" : "http"}
                onChange={(v) => setHttps(v === "https")}
              />
            </Field>
            <Field label={t("form.port")} help={t("form.portHelp")}>
              <TextInput label={t("form.port")} value={draft.port} onChange={set("port")} />
            </Field>
            {draft.https && (
              <Field label={t("form.certificate")} help={t(`form.trustHelp.${draft.trust}`)}>
                <Segmented
                  label={t("form.certificate")}
                  options={trusts.map((id) => ({ id, label: t(`form.trust.${id}`) }))}
                  value={draft.trust}
                  onChange={(v) => setDraft({ ...draft, trust: v })}
                />
                {draft.trust === "pin" && (
                  <TextInput label={t("form.pin")} value={draft.pin} onChange={set("pin")} placeholder="AB:CD:…" />
                )}
              </Field>
            )}
            {kindOf?.user !== false && (
              <Field label={t("form.user")} help={t("form.userHelp")}>
                <TextInput label={t("form.user")} value={draft.user} onChange={set("user")} />
              </Field>
            )}
            <Field
              label={t("form.password")}
              help={kindOf?.user === false ? t("form.passwordHelpOf", { kind: kindOf.name }) : t("form.passwordHelp")}
              badge={page.setup.password_set ? <Muted>{t("form.passwordKept")}</Muted> : undefined}
            >
              <TextInput
                label={t("form.password")}
                value={draft.password}
                onChange={set("password")}
                placeholder={page.setup.password_set ? t("form.passwordKeep") : ""}
                secret
              />
            </Field>
            <Field label={t("form.answer")} help={t("form.answerHelp")}>
              <TextInput
                label={t("form.answer")}
                value={draft.answer}
                onChange={set("answer")}
                placeholder={t("form.answerPlaceholder")}
              />
            </Field>
            <Field label={t("form.pointAt")} help={t(`form.pointAtHelp.${draft.pointAt}`)}>
              <Segmented
                label={t("form.pointAt")}
                options={[
                  { id: "address", label: t("form.pointAtAddress") },
                  { id: "name", label: t("form.pointAtName") },
                ]}
                value={draft.pointAt}
                onChange={(v) => setDraft({ ...draft, pointAt: v })}
              />
              {draft.pointAt === "name" && (
                <TextInput label={t("form.name")} value={draft.via} onChange={set("via")} placeholder="cache.lan" />
              )}
            </Field>
            <Field label="" help={t("form.buttonsHelp")}>
              <Button onClick={() => save(true)} disabled={busy} label={t("form.testLabel")}>
                {t("form.test")}
              </Button>
              <Button onClick={() => save(false)} disabled={busy} tone="accent" label={t("form.saveLabel")}>
                {t("form.save")}
              </Button>
              {page.setup.config !== null && (
                <Button
                  onClick={() => take(actions.forget(), false)}
                  disabled={busy}
                  tone="danger"
                  label={t("form.forgetLabel")}
                >
                  {t("form.forget")}
                </Button>
              )}
              {editing && page.setup.config !== null && (
                <Button onClick={() => setEditing(false)} disabled={busy} label={t("form.cancelLabel")}>
                  {t("form.cancel")}
                </Button>
              )}
            </Field>
          </>
        )}
      </Panel>
      {page.report === null ? (
        page.setup.config !== null && <Quiet title={t("noReport")} hint={page.report_error ?? undefined} />
      ) : (
        <>
          <Panel
            title={t("rewrites.title")}
            note={t("rewrites.note", {
              server: page.report.server,
              answer: page.report.plan?.write_as ?? "lanstash",
            })}
            action={<ApplyButton report={page.report} apply={() => take(actions.apply(), true)} />}
          >
            <Rewrites report={page.report} />
          </Panel>
          <Coverage report={page.report} />
        </>
      )}
    </View>
  )
}
