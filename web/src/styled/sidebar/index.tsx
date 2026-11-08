/**
 * Sidebar: the frame of the app. On the left the sidebar: the mark and the name at the top, then
 * the sections of navigation links. Beside it the page: a header that stays at the top (where you
 * are, as a trail of names, each above the page a link to its own, and what goes on the right of it, such as the connection), the page's
 * content, and a footer after it (such as the version, and a notice on the right). On a narrow screen the
 * sidebar sits above the page and each section is a row that scrolls sideways, and the trail
 * shows only the page's own name. A section is a label
 * in small spaced capitals over its links; a link is a row with an icon, its words and a figure,
 * marked when it is the page shown. At the sidebar's foot, who uses the page (SideAccount), kept in
 * view while the links scroll.
 */
import { Fragment, type ReactNode } from "react"
import { useT } from "@/i18n"
import { classes } from "@/lib/classes"
import { Guard } from "@/styled/guard"
import { Avatar } from "@/styled/roster"

/** The mark: three stacked platters, the top one lit in the cache's amber. */
function Mark() {
  return (
    <svg aria-hidden viewBox="0 0 24 24" className="size-7 shrink-0">
      <rect x="3" y="3.5" width="18" height="4.5" rx="1.5" className="fill-cache" />
      <rect x="3" y="9.75" width="18" height="4.5" rx="1.5" className="fill-text" opacity="0.55" />
      <rect x="3" y="16" width="18" height="4.5" rx="1.5" className="fill-text" opacity="0.28" />
    </svg>
  )
}

/** One name in the trail, and where its page is (if it has one). */
export interface Crumb {
  readonly name: string
  readonly href?: string
}

export interface AppFrameProps {
  readonly title: string
  readonly tagline?: string
  /** The sections of links. */
  readonly sidebar?: ReactNode
  /** Who uses the page, at the foot of the sidebar ([`SideAccount`]). */
  readonly account?: ReactNode
  /** Where the page is: the names from the section down to the page, each above it a link when it has a page. */
  readonly trail: readonly Crumb[]
  /** The right of the header. */
  readonly status?: ReactNode
  /** The left of the footer, and its right. */
  readonly footer?: ReactNode
  readonly notice?: ReactNode
  /** The page's path: content that failed to draw says so in its place, and is tried again on another. */
  readonly path?: string
  readonly children?: ReactNode
}

export function AppFrame({
  title,
  tagline,
  sidebar,
  account,
  trail,
  status,
  footer,
  notice,
  path,
  children,
}: AppFrameProps) {
  const t = useT("common")
  return (
    <div className="min-h-screen lg:grid lg:grid-cols-[17rem_minmax(0,1fr)]">
      <aside className="flex flex-col border-b bg-canvas/70 backdrop-blur-sm lg:sticky lg:top-0 lg:h-screen lg:overflow-y-auto lg:border-r lg:border-b-0">
        <div className="flex h-16 shrink-0 items-center gap-3 px-5">
          <Mark />
          <div className="flex flex-col leading-tight">
            <span className="font-semibold text-lg tracking-tight">{title}</span>
            {tagline !== undefined && <span className="text-faint text-xs">{tagline}</span>}
          </div>
        </div>
        <nav aria-label={t("ui.mainNav")} className="flex flex-1 flex-col gap-5 px-3 pt-2 pb-4">
          {sidebar}
        </nav>
        {account !== undefined && account !== null && (
          <div className="flex shrink-0 flex-col gap-3 border-t px-3 py-3 lg:sticky lg:bottom-0 lg:bg-canvas/90 lg:backdrop-blur-sm">
            {account}
          </div>
        )}
      </aside>
      <div className="flex min-h-screen min-w-0 flex-col">
        <header className="sticky top-0 z-10 flex h-16 shrink-0 items-center justify-between gap-4 border-b bg-canvas/80 px-4 backdrop-blur-md sm:px-8">
          <ol aria-label={t("ui.youAreHere")} className="flex min-w-0 items-center gap-2 text-sm">
            {trail.map(({ name, href }, i) => {
              const last = i === trail.length - 1
              return (
                <Fragment key={name}>
                  {i > 0 && (
                    <li aria-hidden className="hidden text-faint sm:block">
                      /
                    </li>
                  )}
                  <li className={classes("truncate", last ? "font-medium" : "hidden text-muted sm:block")}>
                    {href === undefined || last ? (
                      <span aria-current={last ? "page" : undefined}>{name}</span>
                    ) : (
                      <a
                        href={href}
                        className="rounded-sm transition-colors hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache"
                      >
                        {name}
                      </a>
                    )}
                  </li>
                </Fragment>
              )
            })}
          </ol>
          <div className="flex shrink-0 items-center gap-4 text-xs">{status}</div>
        </header>
        <main className="flex-1 px-4 py-6 sm:px-8 lg:py-8">
          <div className="mx-auto max-w-[84rem]">
            <Guard reset={path ?? ""}>{children}</Guard>
          </div>
        </main>
        {(footer !== undefined || notice !== undefined) && (
          <footer className="flex flex-wrap items-center justify-between gap-3 border-t bg-canvas/80 px-4 py-3 text-faint text-xs backdrop-blur-md sm:px-8">
            <div className="flex items-center gap-3 font-mono">{footer}</div>
            <div className="flex items-center gap-3">{notice}</div>
          </footer>
        )}
      </div>
    </div>
  )
}

export interface SideSectionProps {
  /** The label over the links; a section without one (the first, usually) shows none. */
  readonly title?: string
  readonly children?: ReactNode
}

export function SideSection({ title, children }: SideSectionProps) {
  return (
    <section aria-label={title} className="flex flex-col gap-1.5">
      {title !== undefined && (
        <h2 className="px-2 font-mono text-[11px] text-faint uppercase tracking-[0.14em]">{title}</h2>
      )}
      <div className="-mx-1 flex gap-1 overflow-x-auto px-1 pb-1 lg:mx-0 lg:flex-col lg:overflow-visible lg:px-0 lg:pb-0">
        {children}
      </div>
    </section>
  )
}

export interface SideAccountProps {
  /** The account's name; none: no one is signed in. */
  readonly name?: string
  /** What stands for the name when no one is signed in. */
  readonly nobody?: string
  /** A line under it (its role). */
  readonly note?: string
  /** Its own page. */
  readonly href?: string
  /** On the right: Sign out, or Sign in. */
  readonly action?: ReactNode
}

/**
 * Who uses the page, at the foot of the sidebar: an avatar, the name over a muted line (a link to the
 * account's own page), and an action on the right.
 */
export function SideAccount({ name, nobody, note, href, action }: SideAccountProps) {
  const who = (
    <>
      {name !== undefined && <Avatar name={name} small />}
      <span className="flex min-w-0 flex-col leading-tight">
        <span className="truncate font-medium text-sm text-text">{name ?? nobody}</span>
        {note !== undefined && <span className="truncate text-muted text-xs">{note}</span>}
      </span>
    </>
  )
  return (
    <div className="flex items-center gap-2">
      {href === undefined ? (
        <div className="flex min-w-0 flex-1 items-center gap-2.5 px-2 py-1.5">{who}</div>
      ) : (
        <a
          href={href}
          className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        >
          {who}
        </a>
      )}
      {action}
    </div>
  )
}

export interface SideLinkProps {
  readonly href: string
  readonly current: boolean
  readonly icon?: ReactNode
  readonly label: string
  readonly figure?: string
}

export function SideLink({ href, current, icon, label, figure }: SideLinkProps) {
  return (
    <a
      href={href}
      aria-current={current ? "page" : undefined}
      className={classes(
        "flex shrink-0 items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors",
        current
          ? "bg-raised font-medium text-text shadow-panel ring-1 ring-line"
          : "text-muted hover:bg-hover hover:text-text",
      )}
    >
      {icon}
      <span className="flex-1 truncate">{label}</span>
      {figure !== undefined && <span className="font-mono text-faint text-xs tabular-nums">{figure}</span>}
    </a>
  )
}
