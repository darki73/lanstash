//! The file that sets up a Windows PC so that lanstash keeps its Epic Games
//! downloads: a `.cmd` the user starts Epic with, in place of Epic's own
//! shortcut.
//!
//! Epic's launcher downloads over HTTPS, which lanstash only passes through. A
//! few lines in the launcher's settings file (the one Epic's servers deliver,
//! under `ProgramData`) make it download over plain HTTP; the launcher reads
//! that file at its start and then replaces it with a fresh copy, so the lines
//! go back in before every start. Each start, the file:
//! 1. as the user, copies itself to `%LOCALAPPDATA%\lanstash` and makes a
//!    desktop and a Start menu shortcut to that copy (which can be pinned),
//!    when started from anywhere else;
//! 2. as the user, closes the launcher if it runs;
//! 3. adds the lines: as the user (Epic's installer lets every user change the
//!    folder), else as administrator after Windows asks,
//!    in a process of its own that does only that;
//! 4. as the user, starts the launcher (by its `com.epicgames.launcher:` link).
//!
//! Nothing in the file is typed in by anyone: every word, path and setting is a
//! value of a type whose constructor refuses characters `cmd` or PowerShell
//! would read as more than text, checked when the constant is made (a bad one
//! does not compile). The PowerShell parts travel base64-encoded
//! (`-EncodedCommand`), so no quoting reaches them. Lines end in CRLF, as `cmd`
//! wants; the text is ASCII.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::checksum::sha1::base64;

/// A word written into the file as it is: a setting's section, key or value, a
/// process's or a file's name. ASCII letters, digits, `.`, `_`, `-`, and (in a
/// file name) spaces and parentheses are refused elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word(&'static str);

impl Word {
    /// # Panics
    /// When `s` is empty or holds anything but letters, digits, `.`, `_` and
    /// `-`; in a constant, that is a compile error.
    #[must_use]
    pub const fn new(s: &'static str) -> Self {
        let b = s.as_bytes();
        assert!(!b.is_empty(), "an empty word");
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            assert!(
                c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-',
                "a word holds only [A-Za-z0-9._-]"
            );
            i += 1;
        }
        Self(s)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// An absolute Windows path: a drive letter, `:\`, then letters, digits,
/// spaces, `\`, `.`, `_` and `-`; never a quote, `%`, `!`, `&`, `^`, a bracket or
/// `..`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinPath<'a>(&'a str);

impl<'a> WinPath<'a> {
    /// # Panics
    /// When `s` is not such a path; in a constant, that is a compile error.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        let b = s.as_bytes();
        assert!(b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\', "a path starts X:\\");
        let mut i = 3;
        while i < b.len() {
            let c = b[i];
            assert!(
                c.is_ascii_alphanumeric() || c == b' ' || c == b'\\' || c == b'.' || c == b'_' || c == b'-',
                "a path holds only [A-Za-z0-9 \\._-]"
            );
            assert!(!(c == b'.' && i + 1 < b.len() && b[i + 1] == b'.'), "a path never holds ..");
            i += 1;
        }
        Self(s)
    }

    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

/// One line of an INI file: `key=value` in `[section]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IniSetting {
    pub section: Word,
    pub key: Word,
    pub value: Word,
}

impl IniSetting {
    #[must_use]
    pub const fn new(section: &'static str, key: &'static str, value: &'static str) -> Self {
        Self { section: Word::new(section), key: Word::new(key), value: Word::new(value) }
    }
}

/// The settings that make the launcher download over plain HTTP.
pub const HTTP_SETTINGS: [IniSetting; 4] = [
    IniSetting::new("Launcher", "ForceNonSslCdn", "true"),
    IniSetting::new("HTTP", "ForceNonSslCdn", "true"),
    IniSetting::new("HTTP", "bUseSslCdn", "false"),
    IniSetting::new("Portal.BuildPatch", "bForceNonSSL", "true"),
];

/// The launcher's settings file that Epic's servers deliver.
pub const EMS_ENGINE_INI: WinPath<'static> =
    WinPath::new("C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\EMS\\EpicGamesLauncher\\Engine.ini");

/// The launcher's process.
pub const LAUNCHER_PROCESS: Word = Word::new("EpicGamesLauncher.exe");

/// The link scheme the launcher registers; it starts the launcher.
pub const LAUNCHER_SCHEME: Word = Word::new("com.epicgames.launcher");

/// The name the file is downloaded under.
pub const FILE_NAME: &str = "Epic Games (lanstash).cmd";

/// A name shown to the user (a shortcut's): letters, digits, spaces, `(`, `)`,
/// `.`, `_` and `-`. Only PowerShell reads it, between single quotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(&'static str);

impl Label {
    /// # Panics
    /// When `s` is empty or holds anything else; in a constant, that is a compile error.
    #[must_use]
    pub const fn new(s: &'static str) -> Self {
        let b = s.as_bytes();
        assert!(!b.is_empty(), "an empty label");
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            assert!(
                c.is_ascii_alphanumeric() || matches!(c, b' ' | b'(' | b')' | b'.' | b'_' | b'-'),
                "a label holds only [A-Za-z0-9 ()._-]"
            );
            i += 1;
        }
        Self(s)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// The name of the shortcuts it makes.
pub const SHORTCUT: Label = Label::new("Epic Games (lanstash)");

/// The name of its copy under `%LOCALAPPDATA%\lanstash`, which the shortcuts start.
pub const INSTALLED: Word = Word::new("epic-games.cmd");

/// What the edit's PowerShell exits with: done; the file is not there; the
/// lines are not there after writing; Windows refused (try as administrator).
pub const EXIT_DONE: u8 = 0;
pub const EXIT_NO_FILE: u8 = 2;
pub const EXIT_UNCHECKED: u8 = 3;
pub const EXIT_REFUSED: u8 = 5;
/// What the elevation exits with when the user says No to Windows' question.
pub const EXIT_DECLINED: u16 = 1223;

/// The PowerShell that sets `settings` in the INI file `file`: each key set
/// where its section has it, else added at the end of the section's first
/// appearance, else the section added at the end; the rest of the file, its
/// line ends, its final line end and its byte order mark kept. It exits
/// [`EXIT_NO_FILE`] without the file, [`EXIT_REFUSED`] when Windows refuses
/// the write, [`EXIT_UNCHECKED`] when a line is not there after it, 1 on
/// anything else, [`EXIT_DONE`] once done.
#[must_use]
pub fn edit_script(file: WinPath<'_>, settings: &[IniSetting]) -> String {
    let want: Vec<String> = settings
        .iter()
        .map(|s| format!("@('{}','{}','{}')", s.section.as_str(), s.key.as_str(), s.value.as_str()))
        .collect();
    format!(
        "$ErrorActionPreference = 'Stop'
$file = '{file}'
$want = @({want})
if (-not (Test-Path -LiteralPath $file)) {{ exit {EXIT_NO_FILE} }}
try {{
  $raw = [IO.File]::ReadAllBytes($file)
  $bom = $raw.Length -ge 3 -and $raw[0] -eq 0xEF -and $raw[1] -eq 0xBB -and $raw[2] -eq 0xBF
  $text = (New-Object Text.UTF8Encoding $false).GetString($raw)
  if ($bom) {{ $text = $text.Substring(1) }}
  $nl = if ($text.Contains(\"`r`n\")) {{ \"`r`n\" }} else {{ \"`n\" }}
  $lines = [Collections.Generic.List[string]]::new([string[]]($text -split '\\r?\\n'))
  $ends = $lines.Count -gt 0 -and $lines[$lines.Count - 1] -eq ''
  if ($ends) {{ $lines.RemoveAt($lines.Count - 1) }}
  foreach ($w in $want) {{
    $head = '[' + $w[0] + ']'
    $line = $w[1] + '=' + $w[2]
    $key = '^\\s*' + [regex]::Escape($w[1]) + '\\s*='
    $in = $false; $first = -1; $set = $false
    for ($i = 0; $i -lt $lines.Count; $i++) {{
      $t = $lines[$i].Trim()
      if ($t.StartsWith('[')) {{ $in = ($t -ieq $head); if ($in -and $first -lt 0) {{ $first = $i }}; continue }}
      if ($in -and $lines[$i] -match $key) {{ $lines[$i] = $line; $set = $true }}
    }}
    if ($set) {{ continue }}
    if ($first -ge 0) {{
      $at = $first + 1
      while ($at -lt $lines.Count -and -not $lines[$at].Trim().StartsWith('[')) {{ $at++ }}
      while ($at -gt $first + 1 -and $lines[$at - 1].Trim() -eq '') {{ $at-- }}
      $lines.Insert($at, $line)
    }} else {{
      if ($lines.Count -gt 0 -and $lines[$lines.Count - 1].Trim() -ne '') {{ $lines.Add('') }}
      $lines.Add($head); $lines.Add($line)
    }}
  }}
  $out = $lines -join $nl
  if ($ends) {{ $out += $nl }}
  [IO.File]::WriteAllText($file, $out, (New-Object Text.UTF8Encoding $bom))
  $check = [IO.File]::ReadAllText($file)
  foreach ($w in $want) {{ if (-not $check.Contains($w[1] + '=' + $w[2])) {{ exit {EXIT_UNCHECKED} }} }}
}} catch [UnauthorizedAccessException] {{ exit {EXIT_REFUSED} }} catch [Security.SecurityException] {{ exit {EXIT_REFUSED} }} catch {{
  [Console]::Error.WriteLine($_.Exception.Message); exit 1
}}
exit {EXIT_DONE}
",
        file = file.as_str(),
        want = want.join(","),
    )
}

/// The PowerShell that makes the desktop and Start menu shortcuts to the copy
/// at `%LANSTASH_SELF%`, with the launcher's icon where the launcher's link
/// says where it is.
#[must_use]
pub fn shortcut_script() -> String {
    format!(
        "$ErrorActionPreference = 'Stop'
$self = $env:LANSTASH_SELF
$icon = $null
try {{
  $open = (Get-ItemProperty -LiteralPath 'Registry::HKEY_CLASSES_ROOT\\{scheme}\\shell\\open\\command').'(default)'
  if ($open -match '^\"([^\"]+)\"') {{ $icon = $Matches[1] + ',0' }}
}} catch {{}}
$shell = New-Object -ComObject WScript.Shell
foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {{
  $link = $shell.CreateShortcut((Join-Path $dir '{name}.lnk'))
  $link.TargetPath = $env:ComSpec
  $link.Arguments = '/c \"' + $self + '\"'
  $link.WorkingDirectory = Split-Path -Parent $self
  $link.Description = 'Epic Games, with its downloads kept by lanstash'
  if ($icon) {{ $link.IconLocation = $icon }}
  $link.Save()
}}
",
        scheme = LAUNCHER_SCHEME.as_str(),
        name = SHORTCUT.as_str(),
    )
}

/// `script` as `-EncodedCommand` takes it: UTF-16LE, base64.
#[must_use]
pub fn encoded(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&utf16)
}

/// The `.cmd`: see the module's description. CRLF line ends, ASCII.
#[must_use]
pub fn windows_cmd() -> String {
    let ps = "\"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\" -NoProfile -NonInteractive -ExecutionPolicy Bypass";
    let edit = encoded(&edit_script(EMS_ENGINE_INI, &HTTP_SETTINGS));
    let shortcuts = encoded(&shortcut_script());
    let process = LAUNCHER_PROCESS.as_str();
    let scheme = LAUNCHER_SCHEME.as_str();
    let installed = INSTALLED.as_str();
    let running = format!(
        "\"%SystemRoot%\\System32\\tasklist.exe\" /fi \"imagename eq {process}\" /nh 2>nul | \"%SystemRoot%\\System32\\find.exe\" /i \"{process}\" >nul"
    );
    let lines: Vec<String> = [
        "@echo off",
        "setlocal EnableExtensions DisableDelayedExpansion",
        "title Epic Games, kept by lanstash",
        "rem Made by lanstash. Start Epic Games with this, in place of Epic's own shortcut:",
        "rem it makes the launcher download over plain HTTP, which lanstash keeps for every PC.",
        "rem 1. As you: copy this file to %LOCALAPPDATA%\\lanstash and make shortcuts to it, once.",
        "rem 2. As you: close the launcher, if it runs.",
        "rem 3. Add the lines to Epic's settings file: as you, or as administrator once Windows asks.",
        "rem 4. As you: start the launcher.",
        "",
        "set \"LANSTASH_HOME=%LOCALAPPDATA%\\lanstash\"",
        &format!("set \"LANSTASH_SELF=%LANSTASH_HOME%\\{installed}\""),
        "if /i \"%~f0\"==\"%LANSTASH_SELF%\" goto close",
        "if not exist \"%LANSTASH_HOME%\" mkdir \"%LANSTASH_HOME%\"",
        "copy /y \"%~f0\" \"%LANSTASH_SELF%\" >nul",
        "if errorlevel 1 goto no_copy",
        "echo Making the Epic Games shortcuts on the desktop and in the Start menu...",
        &format!("{ps} -EncodedCommand {shortcuts}"),
        "",
        ":close",
        &format!("{running} || goto edit"),
        "echo Closing the Epic Games launcher...",
        &format!("\"%SystemRoot%\\System32\\taskkill.exe\" /f /t /im {process} >nul 2>&1"),
        "set /a LANSTASH_WAIT=0",
        ":closing",
        &format!("{running} || goto edit"),
        "set /a LANSTASH_WAIT+=1",
        "if %LANSTASH_WAIT% geq 20 goto stuck",
        "\"%SystemRoot%\\System32\\timeout.exe\" /t 1 /nobreak >nul",
        "goto closing",
        "",
        ":edit",
        "echo Setting Epic Games to download over plain HTTP...",
        &format!("{ps} -EncodedCommand {edit}"),
        "set LANSTASH_RC=%errorlevel%",
        &format!("if %LANSTASH_RC%=={EXIT_DONE} goto start"),
        &format!("if %LANSTASH_RC%=={EXIT_NO_FILE} goto no_file"),
        &format!("if not %LANSTASH_RC%=={EXIT_REFUSED} goto failed"),
        "echo Windows asks to allow this one change to Epic's settings file.",
        &format!(
            "{ps} -Command \"try {{ $p = Start-Process -FilePath powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand','{edit}'; exit $p.ExitCode }} catch {{ exit {EXIT_DECLINED} }}\""
        ),
        "set LANSTASH_RC=%errorlevel%",
        &format!("if %LANSTASH_RC%=={EXIT_DONE} goto start"),
        &format!("if %LANSTASH_RC%=={EXIT_DECLINED} goto declined"),
        "goto failed",
        "",
        ":start",
        "echo Starting Epic Games...",
        &format!("start \"\" \"{scheme}://\""),
        "\"%SystemRoot%\\System32\\timeout.exe\" /t 2 /nobreak >nul",
        "exit /b 0",
        "",
        ":no_copy",
        "echo Could not copy this file to \"%LANSTASH_HOME%\"; it carries on from where it is.",
        "goto close",
        "",
        ":no_file",
        "echo Epic's settings file is not there yet: Epic makes it at its first start.",
        "echo Epic starts now as usual; start it with this file next time.",
        &format!("start \"\" \"{scheme}://\""),
        "pause",
        &format!("exit /b {EXIT_NO_FILE}"),
        "",
        ":declined",
        "echo The change was not allowed, so Epic downloads over HTTPS this time: lanstash cannot keep that.",
        &format!("start \"\" \"{scheme}://\""),
        "pause",
        &format!("exit /b {EXIT_DECLINED}"),
        "",
        ":stuck",
        "echo The Epic Games launcher did not close. Quit it from the tray, then start this again.",
        "pause",
        "exit /b 4",
        "",
        ":failed",
        "echo Epic's settings file could not be changed, code %LANSTASH_RC%. Epic starts as usual.",
        &format!("start \"\" \"{scheme}://\""),
        "pause",
        "exit /b 1",
    ]
    .iter()
    .map(|l| (*l).to_owned())
    .collect();
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}
