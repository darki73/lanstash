/**
 * Bytes: byte counts in decimal units, as lanstash's logs and documents say them: a size in the
 * unit that fits ("52.97 GB", "7.32 TB", "16.8 MB", "48 KB", "512 B"), gigabytes with two
 * decimals ("29.61 GB"), megabytes with one ("16.8 MB"), and a short size that is kilobytes
 * below a megabyte ("48 KB"). What is not a number reads "–".
 */

export function size(bytes: number): string {
  if (!Number.isFinite(bytes)) return "–"
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(2)} TB`
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB`
  return `${Math.max(0, Math.round(bytes))} B`
}

export function gigabytes(bytes: number): string {
  return Number.isFinite(bytes) ? `${(bytes / 1e9).toFixed(2)} GB` : "–"
}

export function megabytes(bytes: number): string {
  return Number.isFinite(bytes) ? `${(bytes / 1e6).toFixed(1)} MB` : "–"
}

export function shortSize(bytes: number): string {
  return bytes < 1e6 ? `${(bytes / 1e3).toFixed(0)} KB` : megabytes(bytes)
}
