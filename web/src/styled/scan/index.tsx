/**
 * Scan: a code to scan with a phone (a QR code image), on a white tile whatever the theme so
 * cameras read it, with what to do beside it (below it on a narrow screen).
 */
import type { ReactNode } from "react"

export interface ScanProps {
  /** The code's image. */
  readonly src: string
  /** What the code is for, for those who cannot see it. */
  readonly alt: string
  readonly children: ReactNode
}

export function Scan({ src, alt, children }: ScanProps) {
  return (
    <div className="flex flex-wrap items-center gap-6">
      <img src={src} alt={alt} width={192} height={192} className="size-48 rounded-md bg-white p-1" />
      <div className="flex max-w-sm flex-col gap-2">{children}</div>
    </div>
  )
}
