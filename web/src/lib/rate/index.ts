/**
 * Rate: a transfer rate in MB/s with one decimal, or a dash when none is known yet (the first
 * snapshot of a viewer has nothing to compare with).
 */
export function rate(mbPerSecond: number | undefined): string {
  return mbPerSecond === undefined ? "–" : `${mbPerSecond.toFixed(1)} MB/s`
}
