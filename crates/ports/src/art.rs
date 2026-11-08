//! Where a platform keeps a game's images.

use alloc::string::String;

use crate::upstream::Source;

/// Which image of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtKind {
    /// A wide banner (Steam's header, Battle.net's key art).
    Header,
    /// A square icon.
    Icon,
    /// A large wide image behind the game's page (Steam's library hero,
    /// Battle.net's launcher background).
    Background,
    /// The game's logo, on a clear ground.
    Logo,
}

impl ArtKind {
    /// The kind a path names: none is the header.
    #[must_use]
    pub fn parse(s: Option<&str>) -> Option<Self> {
        match s {
            None | Some("header") => Some(Self::Header),
            Some("icon") => Some(Self::Icon),
            Some("background") => Some(Self::Background),
            Some("logo") => Some(Self::Logo),
            Some(_) => None,
        }
    }
}

/// Why there is no image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtError {
    /// No art source for this platform or kind, or not a valid id for it.
    Unknown,
    /// The platform has no image for it.
    Missing,
    /// The CDN could not be reached or answered something unusable.
    Upstream(String),
}

/// Finds a platform's images.
pub trait ArtSource: Send + Sync {
    /// Where the `kind` image of game `id` is.
    ///
    /// # Errors
    /// [`ArtError::Unknown`] for an id or kind this source has no images for,
    /// [`ArtError::Missing`] when the platform has none, [`ArtError::Upstream`]
    /// when the source could not be asked.
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError>;

    /// The image's bytes, where the source fetches them itself (from a CDN that answers only
    /// over HTTPS, say); `None` (the default): fetch what [`ArtSource::locate`] says.
    fn fetch(&self, _id: &str, _kind: ArtKind) -> Option<Result<alloc::vec::Vec<u8>, ArtError>> {
        None
    }

    /// Where else to look when the image [`ArtSource::locate`] names is missing (an older
    /// game's other picture); `None` (the default): nowhere.
    fn fallback(&self, _id: &str, _kind: ArtKind) -> Option<Source> {
        None
    }
}
