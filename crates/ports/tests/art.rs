use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::upstream::Source;

struct Icons;

impl ArtSource for Icons {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        match kind {
            ArtKind::Icon => Ok(Source { host: "cdn".into(), port: Some(8080), target: format!("/{id}.png") }),
            ArtKind::Header | ArtKind::Background | ArtKind::Logo => Err(ArtError::Missing),
        }
    }
}

#[test]
fn kinds_by_name_and_header_by_default() {
    assert_eq!(ArtKind::parse(None), Some(ArtKind::Header));
    assert_eq!(ArtKind::parse(Some("header")), Some(ArtKind::Header));
    assert_eq!(ArtKind::parse(Some("icon")), Some(ArtKind::Icon));
    assert_eq!(ArtKind::parse(Some("background")), Some(ArtKind::Background));
    assert_eq!(ArtKind::parse(Some("logo")), Some(ArtKind::Logo));
    assert_eq!(ArtKind::parse(Some("poster")), None);
}

#[test]
fn a_source_names_where_an_image_is() {
    assert_eq!(Icons.locate("x", ArtKind::Icon).unwrap().target, "/x.png");
    assert_eq!(Icons.locate("x", ArtKind::Header), Err(ArtError::Missing));
}
