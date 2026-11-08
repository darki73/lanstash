/**
 * Platforms nav: the game platforms in the sidebar, each with its mark and what the cache holds of
 * it, linking to its page. Apart from the platform pages, so the sidebar does not load them.
 */
import type { Snapshot } from "@/contracts/status"
import { platformLink, platforms } from "@/features/platforms/model"
import { useT } from "@/i18n"
import { size } from "@/lib/bytes"
import { Brand } from "@/styled/brand"
import { Monogram } from "@/styled/monogram"
import { SideLink, SideSection } from "@/styled/sidebar"

export interface PlatformsNavProps {
  readonly snapshot: Snapshot
  /** The platform whose page is shown, if one is. */
  readonly current: string | undefined
}

export function PlatformsNav({ snapshot, current }: PlatformsNavProps) {
  const t = useT("platforms")
  return (
    <SideSection title={t("nav")}>
      {platforms(snapshot.usage).map((p) => (
        <SideLink
          key={p.id}
          href={platformLink(p.id)}
          current={current === p.id}
          icon={p.brand === undefined ? <Monogram letters={p.letters} /> : <Brand name={p.brand} />}
          label={p.name}
          figure={p.bytes > 0 ? size(p.bytes) : "–"}
        />
      ))}
    </SideSection>
  )
}
