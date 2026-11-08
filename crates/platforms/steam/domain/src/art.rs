//! Where a Steam game's art lives. lanstash has no TLS client, so only a CDN
//! that answers plain HTTP is used: `cdn.akamai.steamstatic.com` serves
//! `/steam/apps/<app>/header.jpg` over HTTP, and the library's wide hero
//! (`library_hero.jpg`) and logo (`logo.png`) the same way. Newer apps keep
//! their art under a hash only their app info names (`<sha1>/header.jpg`), at
//! `shared.akamai.steamstatic.com/store_item_assets/steam/apps/<app>/`, also
//! over HTTP (their plain `header.jpg` is 404): where the app info names a
//! file, that is asked for. An icon is the 32×32 `<sha1>.jpg` app info names
//! (`common/icon`), among the community images of `cdn.akamai.steamstatic.com`
//! (`/steamcommunity/public/images/apps/<app>/`); an app whose app info names
//! none has no icon.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;

use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::upstream::Source;

use crate::apps::Assets;

/// Host of Steam's store images by their plain names.
pub const STEAM_ART_HOST: &str = "cdn.akamai.steamstatic.com";
/// Host of Steam's store images by the names app info gives them.
pub const STEAM_ASSET_HOST: &str = "shared.akamai.steamstatic.com";

/// A Steam app id: 1 to 10 ASCII digits, no leading zero.
#[must_use]
pub fn is_app_id(id: &str) -> bool {
    (1..=10).contains(&id.len()) && !id.starts_with('0') && id.bytes().all(|b| b.is_ascii_digit())
}

/// An app's art file names, where they are known (its app info).
pub type AssetsOf = Arc<dyn Fn(u32) -> Option<Assets> + Send + Sync>;

/// Steam's images, by app id.
#[derive(Clone, Default)]
pub struct SteamArt {
    assets: Option<AssetsOf>,
}

impl core::fmt::Debug for SteamArt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SteamArt").field("assets", &self.assets.is_some()).finish()
    }
}

impl SteamArt {
    /// Asks for the files app info names, where `assets` knows them.
    #[must_use]
    pub fn with_assets(mut self, assets: AssetsOf) -> Self {
        self.assets = Some(assets);
        self
    }

    fn named(&self, id: &str, kind: ArtKind) -> Option<String> {
        let app: u32 = id.parse().ok()?;
        let a = self.assets.as_ref()?(app)?;
        match kind {
            ArtKind::Header => a.header,
            ArtKind::Background => a.hero,
            ArtKind::Logo => a.logo,
            ArtKind::Icon => a.icon,
        }
    }
}

impl ArtSource for SteamArt {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        if !is_app_id(id) {
            return Err(ArtError::Unknown);
        }
        let file = match kind {
            ArtKind::Header => "header.jpg",
            ArtKind::Background => "library_hero.jpg",
            ArtKind::Logo => "logo.png",
            ArtKind::Icon => {
                let sha = self.named(id, kind).ok_or(ArtError::Unknown)?;
                let target = format!("/steamcommunity/public/images/apps/{id}/{sha}.jpg");
                return Ok(Source { host: STEAM_ART_HOST.to_string(), port: None, target });
            }
        };
        if let Some(name) = self.named(id, kind) {
            return Ok(Source {
                host: STEAM_ASSET_HOST.to_string(),
                port: None,
                target: format!("/store_item_assets/steam/apps/{id}/{name}"),
            });
        }
        Ok(Source { host: STEAM_ART_HOST.to_string(), port: None, target: format!("/steam/apps/{id}/{file}") })
    }

    /// An older game with no library hero has its store page's background instead.
    fn fallback(&self, id: &str, kind: ArtKind) -> Option<Source> {
        (kind == ArtKind::Background && is_app_id(id)).then(|| Source {
            host: STEAM_ART_HOST.to_string(),
            port: None,
            target: format!("/steam/apps/{id}/{PAGE_BACKGROUND}"),
        })
    }
}

/// The store page's background, which most older games without a plain `library_hero.jpg`
/// have instead.
pub const PAGE_BACKGROUND: &str = "page_bg_generated_v6b.jpg";
