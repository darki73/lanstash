/**
 * Counters: the raw counter groups of a resource, as lanstash sends them, in one table: a group's
 * name over its counters, each with how much it rose over the overview's range (from what was
 * kept; a level such as the bytes stored has none), its value since lanstash started and, for a
 * byte counter, its rate now. Byte counts are sizes, the rest counts; a zero is faint. A group
 * lanstash does not send (a part that is switched off) is left out.
 */
import type { Snapshot } from "@/contracts/status"
import type { Kept } from "@/features/status/kept"
import { counterGroups, isGauge } from "@/features/status/model"
import { spanOf } from "@/features/status/range"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { count } from "@/lib/count"
import { rate } from "@/lib/rate"
import { Cell, Row, Table } from "@/styled/table"

export interface CountersProps {
  readonly snapshot: Snapshot
  readonly groups: readonly string[]
  /** What was kept over the range; none: that column is left empty. */
  readonly kept?: Kept | null
}

/** The names (`group.counter`) of the counters the groups hold now. */
export function counterNames(snapshot: Snapshot, groups: readonly string[]): string[] {
  return groups.flatMap((g) => Object.keys(snapshot.counters[g] ?? {}).map((c) => `${g}.${c}`))
}

export function Counters({ snapshot, groups, kept }: CountersProps) {
  const t = useT("status")
  const range = kept ? t("range.last", { range: spanOf(t, kept.range) }) : t("counters.overRange")
  return (
    <Table columns={[t("counters.counter"), range, t("counters.sinceStart"), t("counters.now")]}>
      {counterGroups(snapshot, groups).flatMap((g) => [
        <Row key={g.name} tone="strong">
          <Cell>{g.name}</Cell>
          <Cell />
          <Cell />
          <Cell />
        </Row>,
        ...g.rows.map((r) => {
          const key = `${g.name}.${r.key}`
          const shown = (v: number) => (r.bytes ? size(v) : count(v))
          const over = kept && !isGauge(key) ? kept.total(key) : undefined
          return (
            <Row key={key}>
              <Cell indent>{r.name}</Cell>
              <Cell muted={over === undefined || over === 0}>{over === undefined ? "–" : shown(over)}</Cell>
              <Cell muted={r.value === 0}>{shown(r.value)}</Cell>
              <Cell muted={r.rate === undefined || r.rate === 0}>{r.rate === undefined ? "" : rate(r.rate)}</Cell>
            </Row>
          )
        }),
      ])}
    </Table>
  )
}
