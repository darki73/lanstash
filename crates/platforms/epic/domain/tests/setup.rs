use lanstash_epic::setup::{
    EMS_ENGINE_INI, EXIT_DECLINED, EXIT_NO_FILE, EXIT_REFUSED, HTTP_SETTINGS, INSTALLED, IniSetting, LAUNCHER_PROCESS,
    LAUNCHER_SCHEME, Label, SHORTCUT, WinPath, Word, edit_script, encoded, shortcut_script, windows_cmd,
};

fn lines() -> Vec<String> {
    windows_cmd().split("\r\n").map(str::to_owned).collect()
}

#[test]
fn the_cmd_is_ascii_with_crlf_line_ends_and_short_lines() {
    let cmd = windows_cmd();
    assert!(cmd.is_ascii());
    assert!(cmd.ends_with("\r\n"));
    assert_eq!(cmd.matches('\n').count(), cmd.matches("\r\n").count(), "no bare LF");
    assert_eq!(cmd.matches('\r').count(), cmd.matches("\r\n").count(), "no bare CR");
    // cmd reads at most 8191 characters of a line.
    let longest = lines().iter().map(String::len).max().unwrap_or(0);
    assert!(longest < 8000, "longest line {longest}");
}

#[test]
fn the_cmd_has_no_blocks_and_every_goto_has_its_label() {
    let lines = lines();
    // A parenthesised block would break on a path holding `)` (the downloaded file's own name does).
    for l in &lines {
        let t = l.trim();
        assert!(!t.ends_with('(') && !t.starts_with(')'), "a block: {t}");
    }
    let labels: Vec<&str> = lines.iter().filter_map(|l| l.strip_prefix(':')).collect();
    for l in &lines {
        if let Some((_, target)) = l.split_once("goto ") {
            assert!(labels.contains(&target.trim()), "goto {target} has no label");
        }
    }
}

#[test]
fn what_it_says_is_only_text_to_cmd() {
    for l in lines() {
        let Some(said) = l.strip_prefix("echo ") else { continue };
        // A path (which may hold `&`, say) only in quotes, where cmd reads it as text.
        let said = said.replace("\"%LANSTASH_HOME%\"", "");
        for c in ['&', '|', '<', '>', '^', '"', '(', ')'] {
            assert!(!said.contains(c), "{c} in: {said}");
        }
        let rest = said.replace("%LANSTASH_RC%", "");
        assert!(!rest.contains('%'), "% (a path not in quotes?) in: {said}");
    }
}

#[test]
fn it_closes_then_edits_then_starts_and_only_the_edit_is_elevated() {
    let lines = lines();
    let at = |needle: &str| lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no {needle}"));
    let close = at("taskkill.exe");
    let edit = at(":edit");
    let start = at(":start");
    assert!(close < edit && edit < start);
    let elevated: Vec<&String> = lines.iter().filter(|l| l.contains("-Verb RunAs")).collect();
    assert_eq!(elevated.len(), 1, "one elevated step");
    let edit_encoded = encoded(&edit_script(EMS_ENGINE_INI, &HTTP_SETTINGS));
    assert!(elevated[0].contains(&edit_encoded), "the elevated step is the edit");
    assert!(!elevated[0].contains(LAUNCHER_SCHEME.as_str()), "the launcher never starts elevated");
    // The launcher starts from cmd itself, the user's own process.
    let starts: Vec<&String> = lines.iter().filter(|l| l.starts_with("start \"\"")).collect();
    assert!(!starts.is_empty());
    assert!(starts.iter().all(|l| **l == format!("start \"\" \"{}://\"", LAUNCHER_SCHEME.as_str())));
    // The edit is tried as the user first.
    let as_user = at(&format!("-EncodedCommand {edit_encoded}"));
    assert!(as_user < lines.iter().position(|l| l.contains("-Verb RunAs")).unwrap_or(0));
    assert!(lines.iter().any(|l| l == &format!("if %LANSTASH_RC%=={EXIT_DECLINED} goto declined")));
    assert!(lines.iter().any(|l| l == &format!("if not %LANSTASH_RC%=={EXIT_REFUSED} goto failed")));
    assert!(lines.iter().any(|l| l == &format!("if %LANSTASH_RC%=={EXIT_NO_FILE} goto no_file")));
}

#[test]
fn it_installs_itself_and_its_shortcuts_once() {
    let lines = lines();
    assert!(lines.contains(&format!("set \"LANSTASH_SELF=%LANSTASH_HOME%\\{}\"", INSTALLED.as_str())));
    assert!(lines.contains(&"if /i \"%~f0\"==\"%LANSTASH_SELF%\" goto close".to_owned()));
    assert!(lines.iter().any(|l| l.ends_with(&format!("-EncodedCommand {}", encoded(&shortcut_script())))));
    let s = shortcut_script();
    assert!(s.contains(&format!("'{}.lnk'", SHORTCUT.as_str())));
    assert!(s.contains("GetFolderPath('Desktop')") && s.contains("GetFolderPath('Programs')"));
}

#[test]
fn the_edit_names_the_file_and_every_setting() {
    let s = edit_script(EMS_ENGINE_INI, &HTTP_SETTINGS);
    assert!(s.contains(&format!("$file = '{}'", EMS_ENGINE_INI.as_str())));
    for w in HTTP_SETTINGS {
        assert!(s.contains(&format!("@('{}','{}','{}')", w.section.as_str(), w.key.as_str(), w.value.as_str())));
    }
    assert!(windows_cmd().contains(&format!("imagename eq {}", LAUNCHER_PROCESS.as_str())));
}

#[test]
fn encoding_is_utf16le_base64() {
    // "ab" is 61 00 62 00.
    assert_eq!(encoded("ab"), "YQBiAA==");
}

#[test]
#[should_panic(expected = "a word holds only")]
fn a_word_refuses_a_space() {
    let _ = Word::new("a b");
}

#[test]
#[should_panic(expected = "a word holds only")]
fn a_word_refuses_a_quote() {
    let _ = Word::new("a'b");
}

#[test]
#[should_panic(expected = "an empty word")]
fn a_word_is_never_empty() {
    let _ = Word::new("");
}

#[test]
#[should_panic(expected = "a path holds only")]
fn a_path_refuses_a_percent() {
    let _ = WinPath::new("C:\\%TEMP%\\x");
}

#[test]
#[should_panic(expected = "a path holds only")]
fn a_path_refuses_a_quote() {
    let _ = WinPath::new("C:\\a'b");
}

#[test]
#[should_panic(expected = "a path never holds ..")]
fn a_path_refuses_going_up() {
    let _ = WinPath::new("C:\\a\\..\\b");
}

#[test]
#[should_panic(expected = "a path starts")]
fn a_path_is_absolute() {
    let _ = WinPath::new("a\\b");
}

#[test]
#[should_panic(expected = "a label holds only")]
fn a_label_refuses_a_quote() {
    let _ = Label::new("Epic's");
}

#[test]
fn settings_are_words() {
    let s = IniSetting::new("Portal.BuildPatch", "bForceNonSSL", "true");
    assert_eq!((s.section.as_str(), s.key.as_str(), s.value.as_str()), ("Portal.BuildPatch", "bForceNonSSL", "true"));
}

/// The edit itself, run by Windows PowerShell on files of its own.
#[cfg(windows)]
mod on_windows {
    use std::path::PathBuf;
    use std::process::Command;

    use lanstash_epic::setup::{
        EMS_ENGINE_INI, EXIT_DONE, EXIT_NO_FILE, EXIT_REFUSED, HTTP_SETTINGS, WinPath, edit_script, encoded,
    };

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lanstash-epic-setup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Runs the edit on `file`: its exit code.
    fn edit(file: &std::path::Path) -> i32 {
        let path = file.to_str().unwrap();
        let script = edit_script(WinPath::new(path), &HTTP_SETTINGS);
        let out = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"])
            .arg(encoded(&script))
            .output()
            .unwrap();
        out.status.code().unwrap_or(-1)
    }

    const WANTED: &[&str] = &["ForceNonSslCdn=true", "bUseSslCdn=false", "bForceNonSSL=true"];

    #[test]
    fn adds_the_sections_a_file_lacks_and_keeps_the_rest() {
        let d = dir("add");
        let f = d.join("Engine.ini");
        std::fs::write(&f, "[Core.System]\r\nPaths=../x\r\n\r\n[Portal.BuildPatch]\r\nOther=1\r\n").unwrap();
        assert_eq!(edit(&f), i32::from(EXIT_DONE));
        let got = std::fs::read_to_string(&f).unwrap();
        assert_eq!(
            got,
            "[Core.System]\r\nPaths=../x\r\n\r\n[Portal.BuildPatch]\r\nOther=1\r\nbForceNonSSL=true\r\n\r\n[Launcher]\r\nForceNonSslCdn=true\r\n\r\n[HTTP]\r\nForceNonSslCdn=true\r\nbUseSslCdn=false\r\n"
        );
        // Twice is the same as once.
        assert_eq!(edit(&f), i32::from(EXIT_DONE));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), got);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sets_a_key_that_is_there_and_keeps_lf_and_the_byte_order_mark() {
        let d = dir("set");
        let f = d.join("Engine.ini");
        std::fs::write(&f, b"\xEF\xBB\xBF[HTTP]\nbUseSslCdn=true\nKeep=1\n").unwrap();
        assert_eq!(edit(&f), i32::from(EXIT_DONE));
        let got = std::fs::read(&f).unwrap();
        assert!(got.starts_with(b"\xEF\xBB\xBF"), "the byte order mark stays");
        let text = String::from_utf8(got[3..].to_vec()).unwrap();
        assert!(!text.contains('\r'), "LF stays LF");
        assert!(text.starts_with("[HTTP]\nbUseSslCdn=false\nKeep=1\nForceNonSslCdn=true\n"), "{text}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn says_when_the_file_is_not_there() {
        let d = dir("none");
        assert_eq!(edit(&d.join("Engine.ini")), i32::from(EXIT_NO_FILE));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn says_when_windows_refuses_the_write() {
        let d = dir("refused");
        let f = d.join("Engine.ini");
        std::fs::write(&f, "[HTTP]\r\n").unwrap();
        let mut perms = std::fs::metadata(&f).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&f, perms.clone()).unwrap();
        assert_eq!(edit(&f), i32::from(EXIT_REFUSED));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "[HTTP]\r\n", "untouched");
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&f, perms).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// On a copy of this PC's own launcher settings file, where there is one: only lines added.
    #[test]
    fn on_a_copy_of_the_real_file_only_adds_lines() {
        let real = std::path::Path::new(EMS_ENGINE_INI.as_str());
        let Ok(before) = std::fs::read_to_string(real) else { return };
        let d = dir("real");
        let f = d.join("Engine.ini");
        std::fs::write(&f, &before).unwrap();
        assert_eq!(edit(&f), i32::from(EXIT_DONE));
        let after = std::fs::read_to_string(&f).unwrap();
        let old: Vec<&str> = before.lines().collect();
        let new: Vec<&str> = after.lines().collect();
        // Every old line is still there, in order.
        let mut it = new.iter();
        for line in &old {
            assert!(it.any(|n| n == line), "lost: {line}");
        }
        let added: Vec<&&str> = new.iter().filter(|n| !old.contains(n)).collect();
        for w in WANTED {
            assert!(added.iter().any(|a| a == &w), "{w} not added");
        }
        assert!(added.iter().all(|a| a.is_empty() || a.starts_with('[') || WANTED.contains(a)), "{added:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
