/**
 * Range: the span the overview shows what lanstash kept over (the last day, week or month), in
 * buckets fine enough to draw and coarse enough to read in one answer, and how it is said (the
 * status words `range.*`).
 */
import type { Words } from "@/i18n"

export type Range = "day" | "week" | "month"

export const HOUR = 3_600_000

/** A range: how far back, and in what buckets. */
export interface RangeSpec {
  readonly span: number
  readonly step: number
}

export const ranges: Readonly<Record<Range, RangeSpec>> = {
  day: { span: 24 * HOUR, step: HOUR / 6 },
  week: { span: 7 * 24 * HOUR, step: HOUR },
  month: { span: 30 * 24 * HOUR, step: 6 * HOUR },
}

/** The switch's choices, in order. */
export const rangeOptions: readonly { readonly id: Range }[] = [{ id: "day" }, { id: "week" }, { id: "month" }]

/** The switch's choices, in order, in words. */
export const rangeChoices = (t: Words<"status">) =>
  rangeOptions.map(({ id }) => ({ id, label: t(`range.option.${id}`) }))

/** A range's span in words ("24 hours"). */
export const spanOf = (t: Words<"status">, range: Range) => t(`range.span.${range}`)

/** A bucket's length in words ("10 min", "1 h", "6 h"). */
export const per = (step: number, t: Words<"status">) =>
  step < HOUR
    ? t("range.per.minutes", { count: Math.round(step / 60_000) })
    : t("range.per.hours", { count: Math.round(step / HOUR) })
