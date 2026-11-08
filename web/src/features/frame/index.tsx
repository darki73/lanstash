/**
 * Frame: the app around its pages. The sidebar holds the navigation: the overview, then the
 * platforms (more sections join as lanstash grows). The header says where you are (the overview and a
 * game's platform are links back) and shows the
 * connection and the uptime; the footer shows the version lanstash runs, and offers a newer release
 * when the API knows of one. The page the path leads to (the overview, a platform, one of its
 * games) is drawn between them. The header also holds the palette that goes to any page or game by
 * typing (Ctrl+K, or /). Until the first
 * snapshot the page says it is waiting; while reconnecting it keeps showing the last one. The tab's
 * title names the page and, while lanstash fetches or serves, how fast (./meta); the head also
 * carries the description. At the sidebar's foot, who uses the page and, where lanstash ships more
 * than one, the language. Each page loads when first shown (./pages).
 */
import { Suspense } from "react"
import type { AccountActions, AccountsByPlatform } from "@/contracts/account"
import { type ArtLink, noArt } from "@/contracts/art"
import type { BuildsByPlatform } from "@/contracts/builds"
import type { CatalogByPlatform } from "@/contracts/catalog"
import { type ClientNamer, type ClientsReader, noClients } from "@/contracts/clients"
import type { DnsActions } from "@/contracts/dns"
import type { GameDetail } from "@/contracts/game"
import { type HealthReader, noHealth } from "@/contracts/health"
import { type Hidden, type HiddenActions, noHiding } from "@/contracts/hidden"
import { type History, noHistory } from "@/contracts/history"
import { noPrefill, type PrefillActions } from "@/contracts/prefill"
import type { PrivateActions } from "@/contracts/private"
import type { RunsReader } from "@/contracts/runs"
import { can, noSession, type Session, type SessionActions, withoutAccounts } from "@/contracts/session"
import type { SettingsActions } from "@/contracts/settings"
import type { SetupLink } from "@/contracts/setup"
import type { StashActions } from "@/contracts/stash"
import type { StatsReader, TopReader } from "@/contracts/stats"
import type { UsageRow } from "@/contracts/status"
import { updateAvailable, type Version } from "@/contracts/version"
import { AccessProvider, accountsHome, SignIn, Who } from "@/features/access"
import { activity } from "@/features/frame/meta"
import {
  ClientsView,
  DnsView,
  GameView,
  HealthView,
  HistoryView,
  Join,
  Overview,
  OwnAccount,
  PlatformView,
  SettingsView,
  UsersView,
} from "@/features/frame/pages"
import {
  clientsHome,
  dnsHome,
  healthHome,
  historyHome,
  overviewHome,
  place,
  settingsHome,
} from "@/features/frame/route"
import { GoTo } from "@/features/palette"
import { markHidden } from "@/features/platforms/hidden"
import { dirGame, owns, platform, platformLink } from "@/features/platforms/model"
import { PlatformsNav } from "@/features/platforms/nav"
import { HiddenGames, PrivacyProvider, privacyValue } from "@/features/privacy"
import { offered, useT } from "@/i18n"
import { uptime } from "@/lib/uptime"
import type { Connection, LiveStatus } from "@/state/live"
import { type Readable, useStore } from "@/state/store/use"
import { Glyph } from "@/styled/glyph"
import { Head } from "@/styled/head"
import { Language } from "@/styled/language"
import { AppFrame, type Crumb, SideLink, SideSection } from "@/styled/sidebar"
import { Signal, type SignalTone } from "@/styled/signal"
import { Muted, Quiet } from "@/styled/text"

/** A store that never has anything, for a frame given none. */
const nothing = { get: () => undefined, subscribe: () => () => {} }
const noneBuilt: BuildsByPlatform = {}
const noBuilds: Readable<BuildsByPlatform> = { get: () => noneBuilt, subscribe: () => () => {} }
const noReveal: Readable<boolean> = { get: () => false, subscribe: () => () => {} }
const noAccountsStore: Readable<Session> = { get: () => withoutAccounts, subscribe: () => () => {} }

/** A game's own name and its platform's, from the stash as sent (unmasked), else the catalog. */
function realName(usage: readonly UsageRow[], catalogs: CatalogByPlatform, platformId: string, game: string) {
  const p = platform(usage, platformId)
  const g = p?.games.find((x) => owns(x, game))
  const name = g?.named ? g.name : (catalogs[platformId]?.products.find((c) => c.game === game)?.name ?? game)
  return { name, platform: p?.name ?? platformId }
}
/** The platform accounts lanstash is signed in to, as links (`steam/<account>`). */
const linkChoices = (accounts: AccountsByPlatform) =>
  Object.entries(accounts).flatMap(([p, s]) => s?.view.accounts.map((a) => `${p}/${a.name}`) ?? [])
const noneListed: CatalogByPlatform = {}
const noCatalog: Readable<CatalogByPlatform> = { get: () => noneListed, subscribe: () => () => {} }
const noGame: Readable<GameDetail | undefined> = nothing
const noneSignedIn: AccountsByPlatform = {}
const noAccounts: Readable<AccountsByPlatform> = { get: () => noneSignedIn, subscribe: () => () => {} }
const noHistoryStore: Readable<History> = { get: () => noHistory, subscribe: () => () => {} }
const noFollowedStore: Readable<readonly string[] | null | undefined> = {
  get: () => undefined,
  subscribe: () => () => {},
}

const tones: Readonly<Record<Connection, SignalTone>> = {
  connecting: "waiting",
  live: "good",
  reconnecting: "bad",
}

/** A choice of language, at the sidebar's foot: only where lanstash ships more than one. */
function LanguageChoice({ language }: { readonly language: Readable<string> & { set(code: string): void } }) {
  const t = useT("frame")
  const code = useStore(language)
  if (offered.length < 2) return null
  return <Language label={t("language")} options={offered} value={code} onChange={language.set} />
}

export interface FrameProps {
  readonly status: Readable<LiveStatus>
  readonly version: Readable<Version | undefined>
  /** The app's path ("/overview/storage"). */
  readonly path: Readable<string>
  /** Where a game's cover image is; no art without it. */
  readonly art?: ArtLink
  /** Where the file that sets up a PC for a platform is; none offered without it. */
  readonly setup?: SetupLink
  /** Each platform's builds, once the API said them. */
  readonly builds?: Readable<BuildsByPlatform>
  /** Deletes games from the stash; none without it. */
  readonly stash?: StashActions
  /** Starts and stops prefills; none without it. */
  readonly prefill?: PrefillActions
  /** Every Battle.net game there is, once the API said them. */
  readonly catalog?: Readable<CatalogByPlatform>
  /** The Battle.net game whose page is open, once the API said about it. */
  readonly game?: Readable<GameDetail | undefined>
  /** Each platform's accounts (Steam's, Epic's), once the API said them. */
  readonly accounts?: Readable<AccountsByPlatform>
  /** Signs in to and out of a platform's accounts. */
  readonly accountActions?: (platform: string) => AccountActions
  /** Reads and changes the settings; the settings page says it has none without them. */
  readonly settings?: SettingsActions
  /** Sets up and checks the network's DNS server; the DNS page says it has none without them. */
  readonly dns?: DnsActions
  /** What was updated when; the history page is empty without it. */
  readonly history?: Readable<History>
  /** The games kept up to date on their own (`<platform>/<game>`); a game page says so. */
  readonly followed?: Readable<readonly string[] | null | undefined>
  /** Counters over time as lanstash kept them; no traffic over time without it. */
  readonly stats?: StatsReader
  /** The counters that went up most (the games sent most); none listed without it. */
  readonly top?: TopReader
  /** The prefill jobs that ended, as lanstash kept them; no past prefills without it. */
  readonly runs?: RunsReader
  /** The checks lanstash makes of itself; the health page says it has none without them. */
  readonly health?: HealthReader
  /** What each client got over a range; the clients page says none is kept without it. */
  readonly clients?: ClientsReader
  /** Names clients; no renaming without it. */
  readonly nameClient?: ClientNamer
  /** The games kept private; nothing is masked without it. */
  readonly hidden?: Readable<Hidden | null | undefined>
  /** Hides and shows games; none without it. */
  readonly hiding?: HiddenActions
  /** The signed-in player's private games; none: no private games offered. */
  readonly privateGames?: PrivateActions
  /** Whether this browser shows hidden games, and the switch. */
  readonly reveal?: Readable<boolean> & { set(on: boolean): void }
  /** Who uses the page; without it, lanstash has no accounts and everyone may do everything. */
  readonly session?: Readable<Session>
  /** Signs in and out, and manages lanstash's own accounts. */
  readonly users?: SessionActions
  /** The address an invite's token is sent as (`<origin>/join/<token>`). */
  readonly joinLink?: (token: string) => string
  /** The language the page is in, and the choice; none: no choice offered. */
  readonly language?: Readable<string> & { set(code: string): void }
}

export function Frame(props: FrameProps) {
  const { status, version, path, art = noArt, builds = noBuilds, prefill = noPrefill } = props
  const t = useT("frame")
  const tStatus = useT("status")
  const tPlatforms = useT("platforms")
  const tCommon = useT("common")
  const catalogs = useStore(props.catalog ?? noCatalog)
  const detail = useStore(props.game ?? noGame)
  const accounts = useStore(props.accounts ?? noAccounts)
  const history = useStore(props.history ?? noHistoryStore)
  const followed = useStore(props.followed ?? noFollowedStore)
  const session = useStore(props.session ?? noAccountsStore)
  // Each part is handed its actions only where the session holds the permission they need.
  const may = (permission: string) => can(session, permission)
  const hidden = useStore(props.hidden ?? nothing)
  // With accounts, whoever may hide games sees them by name (marked hidden); lanstash masks them for
  // everyone else. Without, this browser's own switch says.
  const switched = useStore(props.reveal ?? noReveal)
  const reveal = session.accounts ? can(session, "hide") : switched
  const { connection, snapshot: sent, recent, keep } = useStore(status)
  // Every page draws the stash with the hidden games marked (masked, or revealed to this browser).
  const snapshot = sent === undefined ? undefined : { ...sent, usage: markHidden(sent.usage, hidden, reveal) }
  const privacy = privacyValue(
    hidden,
    reveal,
    props.hiding ?? noHiding,
    session.accounts ? undefined : props.reveal?.set,
  )
  const running = useStore(version)
  const allBuilds = useStore(builds)
  const at = useStore(path)
  const where = place(at)
  // The platform a page is about, if it is one's (a platform's page or one of its games).
  const onPlatform = where.kind === "platform" || where.kind === "game" ? where.platform : undefined
  const checked = onPlatform === undefined ? undefined : allBuilds[onPlatform]
  const listed = onPlatform === undefined ? undefined : catalogs[onPlatform]
  const live = snapshot === undefined ? undefined : { snapshot, recent, keep }
  const newer = running === undefined ? undefined : updateAvailable(running)
  const platformName = (id: string) => (snapshot && platform(snapshot.usage, id)?.name) ?? id
  const gameName = (platformId: string, id: string) =>
    privacy.of(platformId, id) === "masked"
      ? tCommon("hiddenGame")
      : ((snapshot && platform(snapshot.usage, platformId)?.games.find((g) => owns(g, id) && g.named)?.name) ??
        listed?.products.find((c) => c.game === id)?.name ??
        id)
  const trail: readonly Crumb[] =
    where.kind === "overview"
      ? where.resource === undefined
        ? [{ name: t("nav.overview") }]
        : [{ name: t("nav.overview"), href: overviewHome }, { name: tStatus(`resource.${where.resource}`) }]
      : where.kind === "platform" || where.kind === "game"
        ? [
            { name: t("nav.platforms") },
            { name: platformName(where.platform), href: platformLink(where.platform) },
            ...(where.kind === "game" ? [{ name: gameName(where.platform, where.game) }] : []),
            ...(where.kind === "platform" && where.section !== "cached"
              ? [{ name: tPlatforms(`part.${where.section}`) }]
              : []),
          ]
        : [{ name: t(`page.${where.kind}`) }]
  // The tab names the page (the trail's last name: a hidden game masked as the trail is), and while
  // lanstash fetches or serves, how fast.
  const busy = activity(sent)
  const moving = [
    ...(busy.fetching === undefined ? [] : [t("title.fetching", { rate: busy.fetching })]),
    ...(busy.serving === undefined ? [] : [t("title.serving", { rate: busy.serving })]),
  ]
  const page = trail.at(-1)?.name ?? "lanstash"
  const title = moving.length === 0 ? t("title.page", { page }) : t("title.busy", { page, activity: moving.join(" ") })
  const foot =
    session.accounts || offered.length > 1 ? (
      <>
        {props.language !== undefined && <LanguageChoice language={props.language} />}
        {session.accounts && <Who />}
      </>
    ) : undefined
  return (
    <AccessProvider session={session} actions={props.users ?? noSession} joinLink={props.joinLink}>
      <PrivacyProvider value={privacy}>
        <Head title={title} description={t("description")} />
        <AppFrame
          path={at}
          title="lanstash"
          tagline={t("tagline")}
          trail={trail}
          account={foot}
          sidebar={
            <>
              <SideSection>
                <SideLink
                  href={overviewHome}
                  current={where.kind === "overview"}
                  icon={<Glyph name="overview" />}
                  label={t("nav.overview")}
                />
                <SideLink
                  href={settingsHome}
                  current={where.kind === "settings"}
                  icon={<Glyph name="settings" />}
                  label={t("nav.settings")}
                />
                <SideLink
                  href={clientsHome}
                  current={where.kind === "clients"}
                  icon={<Glyph name="clients" />}
                  label={t("nav.clients")}
                />
                <SideLink
                  href={historyHome}
                  current={where.kind === "history"}
                  icon={<Glyph name="history" />}
                  label={t("nav.history")}
                />
                {may("dns") && (
                  <SideLink
                    href={dnsHome}
                    current={where.kind === "dns"}
                    icon={<Glyph name="dns" />}
                    label={t("nav.dns")}
                  />
                )}
                <SideLink
                  href={healthHome}
                  current={where.kind === "health"}
                  icon={<Glyph name="health" />}
                  label={t("nav.health")}
                />
                {session.accounts && can(session, "accounts") && (
                  <SideLink
                    href={accountsHome}
                    current={where.kind === "accounts"}
                    icon={<Glyph name="accounts" />}
                    label={t("nav.accounts")}
                  />
                )}
              </SideSection>
              {live && <PlatformsNav snapshot={live.snapshot} current={onPlatform} />}
            </>
          }
          status={
            <>
              {snapshot && <GoTo snapshot={snapshot} catalogs={catalogs} art={art} />}
              {snapshot && <Muted>{t("uptime", uptime(snapshot.now_ms - snapshot.started_ms))}</Muted>}
              <Signal tone={tones[connection]}>{t(`connection.${connection}`)}</Signal>
            </>
          }
          footer={running === undefined ? "lanstash" : t("version", { version: running.version })}
          notice={newer && <Signal tone="waiting">{t("update", { version: newer })}</Signal>}
        >
          {/* A page not loaded yet: a quiet line while it comes. */}
          <Suspense fallback={<Quiet title={t("loading")} />}>
            {where.kind === "sign-in" ? (
              <SignIn done={overviewHome} />
            ) : where.kind === "join" ? (
              <Join token={where.token} done={overviewHome} />
            ) : where.kind === "account" ? (
              <OwnAccount />
            ) : where.kind === "accounts" ? (
              <UsersView linkChoices={linkChoices(accounts)} />
            ) : live === undefined ? (
              <Quiet title={t("waiting.title")} hint={t("waiting.hint")} />
            ) : where.kind === "overview" ? (
              <Overview
                live={live}
                resource={where.resource}
                stats={props.stats}
                top={props.top}
                health={props.health}
                healthHref={healthHome}
                art={art}
              />
            ) : where.kind === "settings" ? (
              <SettingsView
                actions={props.settings}
                started={live.snapshot.started_ms}
                more={<HiddenGames nameOf={(p, g) => realName(sent?.usage ?? [], catalogs, p, g)} />}
              />
            ) : where.kind === "dns" ? (
              may("dns") ? (
                <DnsView actions={props.dns} />
              ) : (
                <Quiet title={t("dnsClosed.title")} hint={t("dnsClosed.hint")} />
              )
            ) : where.kind === "history" ? (
              <HistoryView history={history} snapshot={live.snapshot} catalogs={catalogs} privacy={privacy.of} />
            ) : where.kind === "health" ? (
              <HealthView health={props.health ?? noHealth} />
            ) : where.kind === "clients" && session.accounts && session.role === "guest" ? (
              <Quiet title={t("clientsClosed.title")} hint={t("clientsClosed.hint")} />
            ) : where.kind === "clients" ? (
              <ClientsView
                now={live.snapshot.clients}
                recent={live.recent.slice(-60)}
                art={art}
                gameOf={(p, dir) => dirGame(live.snapshot.usage, p, dir, tCommon("launcherData"))}
                clients={props.clients ?? noClients}
                name={may("clients") ? props.nameClient : undefined}
                at={live.snapshot.now_ms}
              />
            ) : where.kind === "platform" ? (
              <PlatformView
                snapshot={live.snapshot}
                id={where.platform}
                section={where.section}
                history={live.recent}
                art={art}
                builds={checked}
                prefill={prefill}
                catalog={listed}
                account={may("platforms") ? accounts[where.platform] : undefined}
                accountActions={may("platforms") ? props.accountActions?.(where.platform) : undefined}
                setup={props.setup}
                runs={props.runs}
              />
            ) : (
              <GameView
                snapshot={live.snapshot}
                platformId={where.platform}
                gameId={where.game}
                detail={detail?.game === where.game ? detail : undefined}
                builds={checked}
                catalog={listed}
                art={art}
                prefill={prefill}
                stash={may("delete") ? props.stash : undefined}
                history={live.recent}
                updates={history}
                followed={followed == null ? undefined : followed.includes(`${where.platform}/${where.game}`)}
                clients={props.clients}
                privateGames={props.privateGames}
              />
            )}
          </Suspense>
        </AppFrame>
      </PrivacyProvider>
    </AccessProvider>
  )
}
