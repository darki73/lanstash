/**
 * Download list: the objects being downloaded, one row each, with the range, the progress and the
 * size. An object fetched in parallel pieces has one summed row; its pieces are listed under it
 * when it is opened. Which objects are open is kept while snapshots come and go; one that
 * finished is forgotten. At most 200 objects are listed; with none, it says why the list is empty.
 */
import { Fragment, useState } from "react"
import type { Download } from "@/contracts/status"
import { type DownloadGroup, groupDownloads } from "@/features/status/model"
import { useT, type Words } from "@/i18n"
import { megabytes, shortSize } from "@/lib/bytes"
import { Meter } from "@/styled/meter"
import { Panel } from "@/styled/panel"
import { Cell, Row, Table } from "@/styled/table"
import { Quiet } from "@/styled/text"
import { Toggle } from "@/styled/toggle"

const shown = 200

/** The range column: "whole", "at 252.7 MB, 48 KB" for one piece, "0.0 MB – 64.0 MB" for many. */
function range(g: DownloadGroup, t: Words<"status">): string {
  if (g.progress.kind === "open") return t("downloads.list.whole")
  const { from, to } = g.progress
  return g.count === 1
    ? t("downloads.list.at", { from: megabytes(from), size: shortSize(to - from) })
    : t("downloads.list.span", { from: megabytes(from), to: megabytes(to) })
}

function progress(g: DownloadGroup) {
  if (g.progress.kind === "open") return <Meter fraction={0} label={megabytes(g.progress.received)} />
  const { done, length } = g.progress
  return <Meter fraction={length > 0 ? done / length : 1} />
}

function note(groups: number, downloads: number, t: Words<"status">): string {
  return downloads === groups
    ? t("downloads.list.objects", { count: groups })
    : t("downloads.list.objectsInPieces", { count: groups, pieces: downloads })
}

export function DownloadList({ downloads }: { readonly downloads: readonly Download[] }) {
  const t = useT("status")
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set())
  const groups = groupDownloads(downloads)
  const toggle = (key: string) =>
    setOpen((was) => {
      const next = new Set([...was].filter((k) => groups.some((g) => g.key === k)))
      if (was.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  return (
    <Panel title={t("downloads.inFlight")} note={note(groups.length, downloads.length, t)}>
      {groups.length === 0 ? (
        <Quiet title={t("downloads.list.empty.title")} hint={t("downloads.list.empty.hint")} />
      ) : (
        <Table
          columns={[
            t("downloads.list.column.object"),
            t("downloads.list.column.range"),
            t("downloads.list.column.pieces"),
            t("downloads.list.column.progress"),
            t("downloads.list.column.size"),
          ]}
        >
          {groups.slice(0, shown).map((g) => {
            const expanded = open.has(g.key) && g.pieces.length > 1
            return (
              <Fragment key={g.key}>
                <Row>
                  <Cell path>{g.key}</Cell>
                  <Cell>{range(g, t)}</Cell>
                  <Cell>
                    {g.count > 1 && (
                      <Toggle open={expanded} onToggle={() => toggle(g.key)}>
                        {t("downloads.list.pieces", { n: g.count })}
                      </Toggle>
                    )}
                  </Cell>
                  <Cell>{progress(g)}</Cell>
                  <Cell>{g.total === null ? "?" : megabytes(g.total)}</Cell>
                </Row>
                {expanded &&
                  g.pieces.map((p) => (
                    <Row key={p.start} tone="quiet">
                      <Cell />
                      <Cell>{t("downloads.list.at", { from: megabytes(p.start), size: shortSize(p.length) })}</Cell>
                      <Cell />
                      <Cell>
                        <Meter fraction={p.length > 0 ? p.done / p.length : 1} />
                      </Cell>
                      <Cell />
                    </Row>
                  ))}
              </Fragment>
            )
          })}
        </Table>
      )}
    </Panel>
  )
}
