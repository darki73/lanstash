/**
 * Setup (Epic Games): what a PC needs so that its launcher's downloads are kept by lanstash. First
 * the two ways a download can go: as the launcher comes (HTTPS, which lanstash only passes through)
 * and once set up (plain HTTP, kept in the stash and served to every PC after the first). Then the
 * operating systems to choose from, each saying where it stands (Windows first: a file lanstash
 * makes; macOS: not checked; Linux: no launcher), and the chosen one's steps. Epic's servers replace
 * the launcher's settings file at every start (the launcher reads it first), so a file the user
 * starts Epic with puts the lines back each time. It closes and starts the launcher as the user;
 * it changes the settings file as the user too, and only where Windows refuses that, as
 * administrator after Windows asks, that step alone: the launcher never starts from the
 * administrator's session. The first time, it puts itself in place with shortcuts to pin. The file
 * comes from lanstash (its link); without one, the download is offered disabled.
 */
import { useState } from "react"
import type { ClientOs } from "@/contracts/setup"
import { useT, type Words } from "@/i18n"
import { Choices } from "@/styled/choices"
import { type Lane, Lanes } from "@/styled/lanes"
import { Panel } from "@/styled/panel"
import { Signal } from "@/styled/signal"
import { Command, FileCard, Runs, Step, Steps } from "@/styled/steps"
import { Muted, Path, Quiet } from "@/styled/text"

const systems = [
  { id: "windows", label: "Windows", tone: "good" },
  { id: "macos", label: "macOS", tone: "waiting" },
  { id: "linux", label: "Linux", tone: "bad" },
] as const

/** The launcher's settings file that Epic's servers deliver, on Windows. */
export const windowsFile = "C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\EMS\\EpicGamesLauncher\\Engine.ini"

/** The name the Windows file is saved under, and the shortcuts it makes. */
export const windowsName = "Epic Games (lanstash).cmd"
const shortcutName = "Epic Games (lanstash)"

/** The lines that make the launcher download over plain HTTP. */
export const httpLines = [
  "[Launcher]",
  "ForceNonSslCdn=true",
  "",
  "[HTTP]",
  "ForceNonSslCdn=true",
  "bUseSslCdn=false",
  "",
  "[Portal.BuildPatch]",
  "bForceNonSSL=true",
].join("\n")

/** The two ways a download can go: as the launcher comes, and once set up. */
function lanesOf(t: Words<"platforms">): readonly Lane[] {
  return [
    {
      label: t("setup.lane.asItComes"),
      tone: "origin",
      stops: [
        { name: t("setup.lane.pc"), icon: "pc", via: "HTTPS" },
        { name: "lanstash", icon: "stash", via: t("setup.lane.passedOn"), passing: true },
        { name: t("setup.lane.cdn"), icon: "cloud" },
      ],
      verdict: t("setup.lane.nothingKept"),
    },
    {
      label: t("setup.lane.setUp"),
      tone: "cache",
      stops: [
        { name: t("setup.lane.pc"), icon: "pc", via: "HTTP" },
        { name: "lanstash", icon: "stash", via: t("setup.lane.firstTime"), marked: true },
        { name: t("setup.lane.cdn"), icon: "cloud" },
      ],
      verdict: t("setup.lane.keptForAll"),
    },
  ]
}

function Windows({ href }: { readonly href: string | undefined }) {
  const t = useT("platforms")
  return (
    <Steps label={t("setup.windows.label")}>
      <Step n={1} title={t("setup.windows.download.title")}>
        <FileCard
          name={windowsName}
          note={href === undefined ? t("setup.windows.download.none") : t("setup.windows.download.made")}
          href={href}
        />
        <Muted>{t("setup.windows.download.text", { shortcut: shortcutName })}</Muted>
      </Step>
      <Step n={2} title={t("setup.windows.shortcut.title")}>
        <Muted>{t("setup.windows.shortcut.text")}</Muted>
      </Step>
      <Step n={3} title={t("setup.windows.runs.title")}>
        <Runs
          label={t("setup.windows.runs.label")}
          runs={[
            { who: "user", does: t("setup.windows.runs.close") },
            {
              who: "user",
              does: t("setup.windows.runs.add"),
              detail: (
                <>
                  <Path>{windowsFile}</Path>
                  <Command label={t("setup.lines")} text={httpLines} />
                </>
              ),
            },
            { who: "administrator", does: t("setup.windows.runs.admin") },
            { who: "user", does: t("setup.windows.runs.start") },
          ]}
        />
      </Step>
      <Step n={4} title={t("setup.windows.check.title")} last>
        <Muted>{t("setup.windows.check.text")}</Muted>
      </Step>
    </Steps>
  )
}

function MacOs() {
  const t = useT("platforms")
  return (
    <Steps label={t("setup.macos.label")}>
      <Step n={1} title={t("setup.macos.title")} last>
        <Signal tone="waiting" still title={t("setup.macos.why")}>
          {t("setup.system.macos")}
        </Signal>
        <Muted>{t("setup.macos.text")}</Muted>
        <Command label={t("setup.lines")} text={httpLines} />
      </Step>
    </Steps>
  )
}

function Linux() {
  const t = useT("platforms")
  return <Quiet title={t("setup.linux.title")} hint={t("setup.linux.hint")} />
}

export interface SetupPanelProps {
  /** The system shown first. */
  readonly os?: ClientOs
  /** Where the file for a system is, if lanstash makes one. */
  readonly link?: (os: ClientOs) => string | undefined
}

export function SetupPanel({ os = "windows", link = () => undefined }: SetupPanelProps) {
  const t = useT("platforms")
  const [shown, setShown] = useState<ClientOs>(os)
  return (
    <>
      <Panel title={t("setup.why")}>
        <Lanes label={t("setup.lanesLabel")} lanes={lanesOf(t)} />
      </Panel>
      <Panel title={t("setup.title")}>
        <Choices
          label={t("setup.os")}
          options={systems.map((s) => ({ ...s, note: t(`setup.system.${s.id}`) }))}
          value={shown}
          onChange={setShown}
        />
      </Panel>
      <Panel title={t("setup.on", { os: systems.find((s) => s.id === shown)?.label ?? "" })}>
        {shown === "windows" ? <Windows href={link("windows")} /> : shown === "macos" ? <MacOs /> : <Linux />}
      </Panel>
    </>
  )
}
