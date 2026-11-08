//! An app as PICS describes it (text KeyValues), the parts lanstash needs:
//! its name and type, its public build, and its depots with their current
//! manifests and who they are for (operating system, language,
//! architecture). The shape (`depots/<id>/manifests/public/{gid, size,
//! download}`, `config/oslist`, `depotfromapp`, `branches/public/buildid`).
//! Its store art's file names (`common/header_image`,
//! `common/library_assets_full/library_hero|library_logo/image`, the English
//! one first): newer apps keep their art under a hash (`<sha1>/header.jpg`) a
//! client cannot guess. Its icon is named by its SHA-1 (`common/icon`).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::kv::Kv;

/// A depot of an app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depot {
    pub id: u32,
    /// The public branch's manifest (`None`: no content of its own, such as a
    /// redistributable another app provides).
    pub manifest: Option<u64>,
    /// Bytes installed and downloaded (the public manifest's).
    pub size: u64,
    pub download: u64,
    /// Its public manifest says it holds nothing (size and download both given, both 0): a placeholder
    /// an install skips; Steam refuses it to an account without it ("access denied").
    pub empty: bool,
    /// Operating systems it is for (empty: every one).
    pub oslist: Vec<String>,
    /// Its language (`None`: every language).
    pub language: Option<String>,
    /// `32` or `64` (`None`: either).
    pub arch: Option<String>,
    /// The app that provides it, when another one does.
    pub from_app: Option<u32>,
    /// The DLC it belongs to, if it is a DLC's.
    pub dlc: Option<u32>,
}

/// An app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub id: u32,
    pub name: Option<String>,
    /// `Game`, `Tool`, `DLC`, `Application`, ... as Steam writes it.
    pub kind: Option<String>,
    /// The public branch's build.
    pub build: Option<u64>,
    pub depots: Vec<Depot>,
    /// For a DLC: the app it belongs to (`common/parent`).
    pub parent: Option<u32>,
    /// For a game: its DLCs (`extended/listofdlc`).
    pub dlcs: Vec<u32>,
    /// Its store art's file names, under `store_item_assets/steam/apps/<id>/`.
    pub art: Assets,
}

/// File names of an app's store art (`<sha1>/header.jpg`, or plain `header.jpg`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assets {
    pub header: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    /// Its 32×32 icon's SHA-1 (`common/icon`), the name of `<sha1>.jpg` among the community images.
    pub icon: Option<String>,
}

/// A SHA-1 in lower-case hex, as `common/icon` writes it.
fn sha1_hex(v: Option<&Kv>) -> Option<String> {
    let h = v?.text()?.trim().to_ascii_lowercase();
    (h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(h)
}

/// An asset's file name in English (else any language), when it is a safe relative path.
fn asset(v: Option<&Kv>) -> Option<String> {
    let v = v?;
    let name = v.get("english").or_else(|| v.entries().first().map(|(_, x)| x))?.text()?;
    let name = name.split('?').next().unwrap_or("").trim().to_string();
    let safe = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('/')
        && name.split('/').all(|part| !part.is_empty() && part != "..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b));
    safe.then_some(name)
}

/// The operating systems Steam installs for, as depots name them (`config/oslist`).
pub const SYSTEMS: [&str; 3] = ["windows", "macos", "linux"];

/// Who an install is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub os: String,
    /// The languages the clients play in, comma-separated (`english,german`): an
    /// install takes the depots of each.
    pub language: String,
    pub arch: String,
}

impl Default for Target {
    /// Windows, English, 64-bit: what lancache's clients mostly are.
    fn default() -> Self {
        Self { os: "windows".into(), language: "english".into(), arch: "64".into() }
    }
}

impl Target {
    /// Whether one of its languages is `language` (case ignored).
    #[must_use]
    pub fn speaks(&self, language: &str) -> bool {
        self.language.split(',').any(|l| l.trim().eq_ignore_ascii_case(language.trim()))
    }
}

fn number<T: core::str::FromStr>(v: Option<&Kv>) -> Option<T> {
    v?.text()?.trim().parse().ok()
}

fn nonempty(v: Option<&Kv>) -> Option<String> {
    v?.text().filter(|s| !s.trim().is_empty())
}

/// What a depot downloads, from its manifest's `size` (installed) and `download`: the
/// download, never more than the size (the chunks are compressed). Steam's figures are often
/// off: many older depots give a download of 0 beside a real size (Fallout: New Vegas's main
/// depot), and some give one far past it (Lara Croft and the Temple of Osiris).
fn estimate(size: Option<u64>, download: Option<u64>) -> u64 {
    match (size, download) {
        (Some(s), Some(d)) if s > 0 => {
            if d == 0 {
                s
            } else {
                d.min(s)
            }
        }
        (s, d) => d.or(s).unwrap_or(0),
    }
}

fn depot(id: u32, d: &Kv) -> Depot {
    let public = d.at(&["manifests", "public"]);
    // Older answers give the gid directly under `public`.
    let gid = public.and_then(|p| p.get("gid").or(Some(p)));
    let size = number(public.and_then(|p| p.get("size")));
    let download = number(public.and_then(|p| p.get("download")));
    Depot {
        id,
        manifest: number(gid),
        size: size.unwrap_or(0),
        download: estimate(size, download),
        empty: size == Some(0) && download == Some(0),
        oslist: nonempty(d.at(&["config", "oslist"]))
            .map(|s| s.split(',').map(|o| o.trim().to_ascii_lowercase()).collect())
            .unwrap_or_default(),
        language: nonempty(d.at(&["config", "language"])),
        arch: nonempty(d.at(&["config", "osarch"])),
        from_app: number(d.get("depotfromapp")),
        dlc: number(d.get("dlcappid")),
    }
}

/// Reads app `id` from its PICS text: the top section (named `appinfo` or by the id).
#[must_use]
pub fn app(id: u32, info: &Kv) -> App {
    let root = info.entries().first().map_or(info, |(_, v)| v);
    let depots = root
        .get("depots")
        .map(|d| d.entries().iter().filter_map(|(k, v)| Some(depot(k.parse().ok()?, v))).collect())
        .unwrap_or_default();
    App {
        id,
        name: nonempty(root.at(&["common", "name"])),
        kind: nonempty(root.at(&["common", "type"])),
        build: number(root.at(&["depots", "branches", "public", "buildid"])),
        depots,
        parent: number(root.at(&["common", "parent"])),
        dlcs: nonempty(root.at(&["extended", "listofdlc"]))
            .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_default(),
        art: Assets {
            header: asset(root.at(&["common", "header_image"])),
            hero: asset(root.at(&["common", "library_assets_full", "library_hero", "image"])),
            logo: asset(root.at(&["common", "library_assets_full", "library_logo", "image"])),
            icon: sha1_hex(root.at(&["common", "icon"])),
        },
    }
}

impl Depot {
    /// Whether it has a public manifest with something in it (an install downloads it).
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.manifest.is_some() && !self.empty
    }

    /// Whether an install for `t` takes this depot (its own content, not a DLC's).
    #[must_use]
    pub fn wanted(&self, t: &Target) -> bool {
        self.has_content() && self.fits(t)
    }

    /// Whether it is for `t` (operating system, language, architecture), and not a DLC's.
    #[must_use]
    pub fn fits(&self, t: &Target) -> bool {
        self.dlc.is_none() && self.suits(t)
    }

    /// Whether it is for `t`'s operating system, language and architecture.
    #[must_use]
    pub fn suits(&self, t: &Target) -> bool {
        (self.oslist.is_empty() || self.oslist.contains(&t.os))
            && self.language.as_ref().is_none_or(|l| t.speaks(l))
            && self.arch.as_ref().is_none_or(|a| *a == t.arch)
    }
}

impl App {
    /// The depots of its own an install for `t` downloads (not those another app provides).
    #[must_use]
    pub fn install(&self, t: &Target) -> Vec<&Depot> {
        self.depots.iter().filter(|d| d.wanted(t) && d.from_app.is_none()).collect()
    }

    /// Depots of other apps an install for `t` takes too (redistributables):
    /// (the app that provides it, the depot). Most list no manifest of their own;
    /// one that does is still the provider's, fetched as the provider; one that lists an
    /// empty manifest is a placeholder, skipped.
    #[must_use]
    pub fn shared(&self, t: &Target) -> Vec<(u32, u32)> {
        self.depots
            .iter()
            .filter(|d| (d.manifest.is_none() || d.has_content()) && d.fits(t))
            .filter_map(|d| Some((d.from_app?, d.id)))
            .collect()
    }

    /// Depots of the app's DLCs for `t`: (the DLC, the depot); an install takes
    /// those of the DLCs the account owns.
    #[must_use]
    pub fn dlc_depots(&self, t: &Target) -> Vec<(u32, &Depot)> {
        self.depots.iter().filter(|d| d.has_content() && d.suits(t)).filter_map(|d| Some((d.dlc?, d))).collect()
    }

    /// Whether it is a DLC (of type `DLC`; a soundtrack app is `Music`) of `game`:
    /// it names `game` its parent, or `game` lists it.
    #[must_use]
    pub fn is_dlc_of(&self, game: &Self) -> bool {
        self.id != game.id
            && self.kind.as_deref().is_some_and(|k| k.eq_ignore_ascii_case("dlc"))
            && (self.parent == Some(game.id) || game.dlcs.contains(&self.id))
    }

    /// The depots of its own a DLC's install for `t` downloads: its content, not
    /// a shared depot nor another DLC's (some DLCs ship their content this way
    /// rather than as depots of their game).
    #[must_use]
    pub fn dlc_content(&self, t: &Target) -> Vec<&Depot> {
        self.depots
            .iter()
            .filter(|d| d.has_content() && d.suits(t) && d.from_app.is_none())
            .filter(|d| d.dlc.is_none_or(|x| x == self.id))
            .collect()
    }

    /// The operating systems of [`SYSTEMS`] it has an install for, `t`'s language and
    /// architecture kept: those its own depots name (`config/oslist`). A depot naming none is
    /// for every one, but on its own says nothing (many Windows-only apps name none).
    #[must_use]
    pub fn systems(&self, t: &Target) -> Vec<&'static str> {
        let named = |os: &str| self.depots.iter().any(|d| d.has_content() && d.oslist.iter().any(|o| o == os));
        SYSTEMS
            .into_iter()
            .filter(|os| named(os))
            .filter(|os| !self.install(&Target { os: (*os).to_string(), ..t.clone() }).is_empty())
            .collect()
    }

    /// Bytes an install for `t` downloads.
    #[must_use]
    pub fn download(&self, t: &Target) -> u64 {
        self.install(t).iter().map(|d| d.download).sum()
    }

    /// The name, or the id when there is none.
    #[must_use]
    pub fn title(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.to_string())
    }
}
