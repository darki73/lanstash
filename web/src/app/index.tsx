/**
 * App: draws a composition: the theme (following the system) and the words in the language in use
 * around the frame, which reads the live status, the version, the app's path and where games' art
 * is. It starts the composition's stream while it is on screen and stops it when it goes.
 */
import { useEffect } from "react"
import type { Composition } from "@/app/compose"
import { Frame } from "@/features/frame"
import { I18nProvider } from "@/i18n"
import { useStore } from "@/state/store/use"
import { ThemeProvider } from "@/styled/theme"

export function App({ composition }: { readonly composition: Composition }) {
  useEffect(() => composition.start(), [composition])
  const language = useStore(composition.language)
  return (
    <I18nProvider language={language}>
      <ThemeProvider mode="system">
        <Frame
          status={composition.status}
          version={composition.version}
          path={composition.path}
          art={composition.art}
          setup={composition.setup}
          builds={composition.builds}
          prefill={composition.prefill}
          stash={composition.stash}
          catalog={composition.catalog}
          game={composition.game}
          accounts={composition.accounts}
          accountActions={composition.accountActions}
          settings={composition.settings}
          dns={composition.dns}
          history={composition.history}
          followed={composition.followed}
          stats={composition.stats}
          top={composition.top}
          runs={composition.runs}
          health={composition.health}
          clients={composition.clients}
          nameClient={composition.nameClient}
          hidden={composition.hidden}
          hiding={composition.hiding}
          privateGames={composition.privateGames}
          reveal={composition.reveal}
          session={composition.session}
          users={composition.users}
          joinLink={composition.joinLink}
          language={composition.language}
        />
      </ThemeProvider>
    </I18nProvider>
  )
}
