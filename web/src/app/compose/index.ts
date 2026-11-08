/**
 * Composition root: builds the live status and connects it to lanstash's status stream, asks
 * the API which version runs and each platform's builds: again when the stream says a platform's
 * builds changed (every five minutes besides; every minute and as soon as a prefill ends from a
 * lanstash that does not say) and with them what was updated when, every minute which games there are, and every few seconds about
 * the game whose page is open, says where
 * games' cover art is, starts and stops prefills, deletes games from the stash, follows Steam's and
 * Epic's accounts (every minute, every two seconds while a QR sign-in is under way, and at once
 * after signing in or out), sets up and checks the network's DNS server, and reads the app's path from the page's address (and follows its links). The pieces that
 * touch the outside (the event-source maker, fetch, the timer, the window's location) are passed
 * in by the entry, fakes by the tests. Nothing runs until start is called; start returns the
 * function that stops.
 */
import { accountActions as accountApi } from "@/adapters/api/account"
import { artLink } from "@/adapters/api/art"
import { readBuilds } from "@/adapters/api/builds"
import { readCatalog } from "@/adapters/api/catalog"
import { clientNamer, clientsReader } from "@/adapters/api/clients"
import { dnsActions, type SendWith } from "@/adapters/api/dns"
import { readGame } from "@/adapters/api/game"
import { healthReader } from "@/adapters/api/health"
import { hidingActions, readHidden } from "@/adapters/api/hidden"
import { readHistory } from "@/adapters/api/history"
import { follow, type OpenSource, type Visibility } from "@/adapters/api/live"
import { type Post, prefillActions } from "@/adapters/api/prefill"
import { privateActions } from "@/adapters/api/private"
import { runsReader } from "@/adapters/api/runs"
import { readSession, sessionActions } from "@/adapters/api/session"
import { settingsActions } from "@/adapters/api/settings"
import { setupLink } from "@/adapters/api/setup"
import { type Delete, stashActions } from "@/adapters/api/stash"
import { statsReader, topReader } from "@/adapters/api/stats"
import { readFollowed } from "@/adapters/api/updates"
import { type Fetch, readVersion } from "@/adapters/api/version"
import { type PathWindow, pathLocation } from "@/adapters/browser/location"
import { choice, type PreferenceStorage, preference } from "@/adapters/browser/preference"
import {
  type AccountActions,
  type AccountState,
  type AccountsByPlatform,
  noAccount,
  signingIn,
} from "@/contracts/account"
import type { ArtLink } from "@/contracts/art"
import type { Builds, BuildsByPlatform } from "@/contracts/builds"
import type { Catalog, CatalogByPlatform } from "@/contracts/catalog"
import type { ClientNamer, ClientsReader } from "@/contracts/clients"
import { type DnsActions, noDns } from "@/contracts/dns"
import type { GameDetail } from "@/contracts/game"
import type { HealthReader } from "@/contracts/health"
import { type Hidden, type HiddenActions, noHiding } from "@/contracts/hidden"
import { type History, noHistory } from "@/contracts/history"
import { noPrefill, type PrefillActions } from "@/contracts/prefill"
import { noPrivate, type PrivateActions } from "@/contracts/private"
import type { RunsReader } from "@/contracts/runs"
import { can, noSession, type Refusal, type Session, type SessionActions, unknownSession } from "@/contracts/session"
import { noSettings, type SettingsActions } from "@/contracts/settings"
import type { SetupLink } from "@/contracts/setup"
import { noStash, type StashActions } from "@/contracts/stash"
import type { StatsReader, TopReader } from "@/contracts/stats"
import { isSnapshot, type Snapshot } from "@/contracts/status"
import type { Version } from "@/contracts/version"
import { place } from "@/features/frame/route"
import { directionOf, pickLanguage } from "@/i18n/catalogue"
import { createLive, type LiveStatus } from "@/state/live"
import { createStore } from "@/state/store"
import type { Readable } from "@/state/store/use"

/** The platforms whose builds lanstash checks. */
export const buildPlatforms = ["blizzard", "steam", "epicgames"] as const

/** The platforms whose games lanstash lists beyond the cache. */
export const catalogPlatforms = ["blizzard", "steam", "epicgames"] as const

/** How soon a catalog lanstash is still reading is asked for again. */
export const catalogSoon = 5_000

/** How often the builds and Battle.net's catalog are asked for again. */
export const buildsEvery = 60_000

/** How often the builds are asked for again where lanstash says when they change. */
export const buildsFallback = 300_000

/** How often the accounts are asked about, and how often while a sign-in is under way. */
export const accountEvery = 60_000
export const signInEvery = 2_000
/** Reads of a waiting sign-in between two fetches of its QR code (Steam turns its code about every 30 s). */
export const qrReads = 5

/** The platforms whose accounts the page follows. */
export const accountPlatforms = ["steam", "epicgames"] as const

/** How often the open game's page asks about it again. */
export const gameEvery = 5_000

/** Where the API is; "" is the page's own origin (lanstash itself, or the dev server's proxy). */
export const statusEvents = (apiBase: string) => `${apiBase}/api/v1/events`

export interface ComposeOptions {
  readonly apiBase: string
  readonly openSource: OpenSource
  readonly fetch: Fetch
  /** DELETEs (a game from the stash); none: the page cannot delete. */
  readonly del?: Delete
  /** POSTs (prefill); none: the page cannot prefill. */
  readonly post?: Post
  /** PUTs (a setting, the DNS server's setup, with headers); none (or no POST or DELETE): the page cannot change settings. */
  readonly put?: SendWith
  readonly window: PathWindow
  readonly later?: (run: () => void, ms: number) => unknown
  /** The page's visibility: a hidden page holds no status stream. None: always seen. */
  readonly visibility?: Visibility
  /** Where this browser remembers its choices (showing hidden games); none: for the page's life only. */
  readonly storage?: PreferenceStorage
  /** The page's origin ("https://cache.lan:8443"), for links sent elsewhere (an invite's); none: a path. */
  readonly origin?: string
  /** The browser's languages, most wanted first; none: English unless chosen. */
  readonly languages?: readonly string[]
  /** The page's root element: its language and script direction follow the one in use. */
  readonly root?: { lang: string; dir: string }
}

/** Where this browser remembers the language chosen. */
const LANGUAGE_KEY = "lanstash.language"

/** Where this browser remembers whether it shows hidden games. */
const REVEAL_KEY = "lanstash.show-hidden-games"

export interface Composition {
  readonly status: Readable<LiveStatus>
  /** The version running, once the API said it. */
  readonly version: Readable<Version | undefined>
  /** The app's path ("/overview/storage"). */
  readonly path: Readable<string>
  /** Where a game's cover image is. */
  readonly art: ArtLink
  /** Where the file that sets up a PC for a platform is. */
  readonly setup: SetupLink
  /** Each platform's builds (Battle.net's, Steam's), once the API said them. */
  readonly builds: Readable<BuildsByPlatform>
  /** Every Battle.net game there is, once the API said them. */
  readonly catalog: Readable<CatalogByPlatform>
  /** The Battle.net game whose page is open, once the API said about it. */
  readonly game: Readable<GameDetail | undefined>
  /** What was updated when, newest first (empty until the API said it). */
  readonly history: Readable<History>
  /** The games kept up to date on their own (`<platform>/<game>`); null when that is off, undefined until known. */
  readonly followed: Readable<readonly string[] | null | undefined>
  /** Asks for counters over time as lanstash kept them. */
  readonly stats: StatsReader
  /** Asks which counters went up most (the games sent most). */
  readonly top: TopReader
  /** Asks for a platform's prefill jobs that ended, as lanstash kept them. */
  readonly runs: RunsReader
  /** Asks for the checks lanstash makes of itself. */
  readonly health: HealthReader
  /** Asks what each client got over a range. */
  readonly clients: ClientsReader
  /** Names clients (none without a way to send PUT and DELETE). */
  readonly nameClient?: ClientNamer
  /** Deletes games from the stash. */
  readonly stash: StashActions
  /** Starts and stops prefills. */
  readonly prefill: PrefillActions
  /** Each platform's accounts (Steam's, Epic's), once the API said them. */
  readonly accounts: Readable<AccountsByPlatform>
  /** Signs in to and out of a platform's accounts. */
  readonly accountActions: (platform: string) => AccountActions
  /** Reads and changes the settings the UI may change, and restarts lanstash. */
  readonly settings: SettingsActions
  /** Sets up and checks the network's DNS server. */
  readonly dns: DnsActions
  /** The games kept private; null from a lanstash that keeps none, undefined until known. */
  readonly hidden: Readable<Hidden | null | undefined>
  /** Hides and shows games (the list follows). */
  readonly hiding: HiddenActions
  /** The signed-in player's private games: read and marked. */
  readonly privateGames: PrivateActions
  /** Whether this browser shows hidden games (remembered), and the switch. */
  readonly reveal: Readable<boolean> & { set(on: boolean): void }
  /** Who uses the page (a guest until lanstash said). */
  readonly session: Readable<Session>
  /** Signs in and out, and manages lanstash's own accounts. */
  readonly users: SessionActions
  /** The address an invite's token is sent as. */
  readonly joinLink: (token: string) => string
  /** The language the page is in (the one chosen here, else the browser's), and the choice. */
  readonly language: Readable<string> & { set(code: string): void }
  start(): () => void
}

export function compose(o: ComposeOptions): Composition {
  const live = createLive()
  const version = createStore<Version | undefined>(undefined)
  const builds = createStore<BuildsByPlatform>({})
  const catalog = createStore<CatalogByPlatform>({})
  const game = createStore<GameDetail | undefined>(undefined)
  const history = createStore<History>(noHistory)
  const followed = createStore<readonly string[] | null | undefined>(undefined)
  const hidden = createStore<Hidden | null | undefined>(undefined)
  const remembered = preference(o.storage, REVEAL_KEY)
  const revealStore = createStore(remembered.read())
  const reveal = {
    get: revealStore.get,
    subscribe: revealStore.subscribe,
    set: (on: boolean) => {
      remembered.write(on)
      revealStore.set(on)
    },
  }
  const chosen = choice(o.storage, LANGUAGE_KEY)
  const languageStore = createStore(pickLanguage(chosen.read(), o.languages ?? []))
  const language = {
    get: languageStore.get,
    subscribe: languageStore.subscribe,
    set: (code: string) => {
      chosen.write(code)
      languageStore.set(pickLanguage(code, []))
    },
  }
  // The page's root says the language in use and its script's direction, for the browser and
  // screen readers.
  const showLanguage = () => {
    if (o.root === undefined) return
    o.root.lang = languageStore.get()
    o.root.dir = directionOf(languageStore.get())
  }
  showLanguage()
  languageStore.subscribe(showLanguage)
  const session = createStore<Session>(unknownSession)
  const readSessionNow = () =>
    readSession(o.fetch, o.apiBase).then((s) => {
      if (s !== undefined) session.set(s)
    })
  // What follows a sign-in or a sign-out; set while the composition runs.
  let afterSession = () => {}
  const signing = o.post && o.put && o.del ? sessionActions(o.fetch, o.post, o.put, o.del, o.apiBase) : undefined
  // A sign-in (or a new account from an invite) that made a session: the page follows it.
  const signedIn = (r: Session | Refusal) => {
    if ("role" in r) {
      session.set(r)
      afterSession()
    }
    return r
  }
  const users: SessionActions =
    signing === undefined
      ? noSession
      : {
          ...signing,
          signIn: async (name, password) => signedIn(await signing.signIn(name, password)),
          join: async (token, name, password) => signedIn(await signing.join(token, name, password)),
          signOut: async () => {
            await signing.signOut()
            await readSessionNow()
            hidden.set(undefined)
            afterSession()
          },
          setPassword: async (name, password, old) => {
            const r = await signing.setPassword(name, password, old)
            // A changed password ends its sessions: this page's too, if it was its own.
            if (r === undefined) await readSessionNow()
            return r
          },
        }
  const hidingApi = o.put && o.del ? hidingActions(o.put, o.del, o.apiBase) : undefined
  const hiding: HiddenActions =
    hidingApi === undefined
      ? noHiding
      : {
          set: async (platform, game, on) => {
            const r = await hidingApi.set(platform, game, on)
            if (typeof r !== "string") hidden.set(r)
            return r
          },
        }
  const later = o.later ?? ((run: () => void, ms: number) => setTimeout(run, ms))
  const path = pathLocation(o.window)
  const accounts = createStore<AccountsByPlatform>({})
  const api = o.post && o.del ? accountApi(o.fetch, o.post, o.del, o.apiBase) : undefined
  // Asks about a platform's accounts again at once (after an action); set while the composition runs.
  let kick = (_platform: string) => {}
  const after =
    (platform: string) =>
    async <T>(done: Promise<T>): Promise<T> => {
      const r = await done
      kick(platform)
      return r
    }
  const accountActions = (platform: string): AccountActions =>
    api === undefined
      ? noAccount
      : {
          signIn: () => after(platform)(api.signIn(platform)),
          signInWithCode: (pasted: string) => after(platform)(api.signInWithCode(platform, pasted)),
          cancel: () => after(platform)(api.cancel(platform)),
          signOut: (account: string) => after(platform)(api.signOut(platform, account)),
          setClient: (client: string | null) => after(platform)(api.setClient(platform, client)),
        }
  return {
    status: live.store,
    version,
    path,
    art: artLink(o.apiBase),
    setup: setupLink(o.apiBase),
    builds,
    catalog,
    game,
    history,
    followed,
    stats: statsReader(o.fetch, o.apiBase),
    top: topReader(o.fetch, o.apiBase),
    // The adapter checks the fields a run is drawn by; the rest is lanstash's prefill status as sent.
    runs: runsReader(o.fetch, o.apiBase) as RunsReader,
    health: healthReader(o.fetch, o.apiBase),
    clients: clientsReader(o.fetch, o.apiBase),
    nameClient: o.put && o.del ? clientNamer(o.put, o.del, o.apiBase) : undefined,
    prefill: o.post === undefined ? noPrefill : prefillActions(o.post, o.apiBase, o.del),
    stash: o.del === undefined ? noStash : stashActions(o.del, o.apiBase, o.post, o.fetch),
    accounts,
    accountActions,
    settings: o.put && o.del && o.post ? settingsActions(o.fetch, o.put, o.del, o.post, o.apiBase) : noSettings,
    dns: o.put && o.del && o.post ? dnsActions(o.fetch, o.put, o.del, o.post, o.apiBase) : noDns,
    hidden,
    hiding,
    privateGames: o.put && o.del ? privateActions(o.fetch, o.put, o.del, o.apiBase) : noPrivate,
    reveal,
    session,
    users,
    joinLink: (token) => `${o.origin ?? ""}/join/${token}`,
    language,
    start: () => {
      let stopped = false
      if (api !== undefined) {
        // Per platform, one chain of reads at a time: a kick starts a new one and the old one ends.
        const chains: Record<string, number> = {}
        let qrTurn = 0
        // Per platform: the view last read, and the reads since the QR code's address last moved.
        const seen: Record<string, { view: string; reads: number }> = {}
        const pollAccount = (platform: string, mine: number) => {
          if (stopped || mine !== chains[platform]) return
          // Only whoever signs lanstash in to platforms reads its accounts; anyone else holds none
          // (an admin's view goes with its sign-out).
          if (!can(session.get(), "platforms")) {
            const { [platform]: _, ...rest } = accounts.get()
            accounts.set(rest)
            later(() => pollAccount(platform, mine), accountEvery)
            return
          }
          api.read(platform).then((v) => {
            if (stopped || mine !== chains[platform]) return
            if (v === null) accounts.set({ ...accounts.get(), [platform]: null })
            if (v !== undefined && v !== null) {
              // A new image only when the sign-in changed, or as often as Steam turns its code.
              const was = seen[platform]
              const view = JSON.stringify(v)
              const moved = was === undefined || was.view !== view || was.reads + 1 >= qrReads
              if (moved) qrTurn += 1
              seen[platform] = { view, reads: moved ? 0 : (was?.reads ?? 0) + 1 }
              const state: AccountState = { view: v, qr: api.qr(platform, qrTurn) }
              accounts.set({ ...accounts.get(), [platform]: state })
            }
            const waiting = v !== null && signingIn(v)
            later(() => pollAccount(platform, mine), waiting ? signInEvery : accountEvery)
          })
        }
        kick = (platform: string) => {
          chains[platform] = (chains[platform] ?? 0) + 1
          pollAccount(platform, chains[platform] ?? 0)
        }
        for (const p of accountPlatforms) kick(p)
      }
      readVersion(o.fetch, o.apiBase).then((v) => {
        if (!stopped && v !== undefined) version.set(v)
      })
      // Each platform's builds are shown as they arrive: a slow one holds back none of the others.
      const readOne = (p: string) =>
        readBuilds(o.fetch, o.apiBase, p).then((b) => {
          if (stopped || b === undefined) return
          const next: Record<string, Builds> = { ...builds.get(), [p]: b }
          builds.set(next)
        })
      // What was updated when: noted as the builds are worked out, so asked for with them.
      // So are the games kept up to date (a game once held whole joins them).
      const readHistoryNow = () =>
        Promise.all([
          readHistory(o.fetch, o.apiBase).then((h) => {
            if (!stopped && h !== undefined) history.set(h)
          }),
          readFollowed(o.fetch, o.apiBase).then((f) => {
            if (!stopped && f !== undefined) followed.set(f)
          }),
          // The list of hidden games is for whoever may hide them; lanstash masks them for the rest.
          can(session.get(), "hide")
            ? readHidden(o.fetch, o.apiBase).then((h) => {
                if (!stopped && h !== undefined) hidden.set(h)
              })
            : Promise.resolve(hidden.set(undefined)),
        ])
      const read = () => Promise.all([...buildPlatforms.map(readOne), readHistoryNow()])
      // lanstash says in the status stream when a platform's builds change; while it does,
      // asking every so often is only a fallback.
      let told = false
      const poll = () => {
        if (stopped) return
        read().then(() => {
          if (!stopped) later(poll, told ? buildsFallback : buildsEvery)
        })
      }
      poll()
      const marks: Record<string, number> = {}
      const changed = (s: Snapshot) => {
        const all = s.builds_changed
        if (all === undefined) return
        told = true
        let any = false
        for (const p of buildPlatforms) {
          const at = all[p]
          if (at === undefined || at === marks[p]) continue
          // The first mark seen is of the builds the first read has (or older).
          if (marks[p] !== undefined) {
            void readOne(p)
            any = true
          }
          marks[p] = at
        }
        if (any) void readHistoryNow()
      }
      const pollCatalog = () => {
        if (stopped) return
        Promise.all(catalogPlatforms.map((p) => readCatalog(o.fetch, o.apiBase, p))).then((all) => {
          if (stopped) return
          const next: Record<string, Catalog> = { ...catalog.get() }
          all.forEach((c, i) => {
            const p = catalogPlatforms[i]
            if (c !== undefined && p !== undefined) next[p] = c
          })
          catalog.set(next)
          // A catalog lanstash is still reading (none yet, or no games) is asked for again soon.
          const reading = all.some((c) => c === undefined || c.products.length === 0)
          later(pollCatalog, reading ? catalogSoon : buildsEvery)
        })
      }
      pollCatalog()
      // The open game's page: asked about every few seconds while it is open.
      let watching: string | undefined
      let turn = 0
      const pollGame = (id: string, mine: number) => {
        if (stopped || mine !== turn) return
        readGame(o.fetch, o.apiBase, id).then((g) => {
          if (stopped || mine !== turn) return
          if (g !== undefined) game.set(g)
          later(() => pollGame(id, mine), gameEvery)
        })
      }
      const watch = () => {
        const where = place(path.get())
        const id = where.kind === "game" && where.platform === "blizzard" ? where.game : undefined
        if (id === watching) return
        watching = id
        turn += 1
        game.set(undefined)
        if (id !== undefined) pollGame(id, turn)
      }
      watch()
      const leave = path.subscribe(watch)
      // A prefill that ends changes how much of a build is stashed: ask again at once.
      let finished: number | null | undefined
      const event = (s: Snapshot) => {
        live.snapshot(s)
        changed(s)
        const now = s.prefill?.finished_ms ?? null
        if (finished !== undefined && now !== null && now !== finished) void read()
        finished = now
      }
      const followLive = () =>
        follow<Snapshot>(
          o.openSource,
          statusEvents(o.apiBase),
          { event, paused: live.paused, accept: isSnapshot },
          { later: o.later, visibility: o.visibility },
        )
      let stop = followLive()
      // Signed in or out: lanstash masks hidden games per viewer, so the stream starts again, and
      // what only an admin may read is read again.
      afterSession = () => {
        if (stopped) return
        stop()
        stop = followLive()
        void readHistoryNow()
      }
      // What the session may read is asked for again as soon as it is known or changes.
      let held = JSON.stringify(session.get().permissions ?? null) + session.get().role
      const unwatchSession = session.subscribe(() => {
        const now = JSON.stringify(session.get().permissions ?? null) + session.get().role
        if (now === held) return
        held = now
        for (const p of accountPlatforms) kick(p)
        void readHistoryNow()
      })
      void readSessionNow()
      return () => {
        stopped = true
        kick = () => {}
        afterSession = () => {}
        unwatchSession()
        leave()
        stop()
      }
    },
  }
}
