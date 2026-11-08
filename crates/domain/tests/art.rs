//! Tests for `lanstash_domain::art`.

use lanstash_domain::art::image_type;

#[test]
fn image_types_by_signature() {
    assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
    assert_eq!(image_type(b"\xFF\xD8\xFF\xE0"), Some("image/jpeg"));
    assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
    assert_eq!(image_type(b"GIF89a"), Some("image/gif"));
    assert_eq!(image_type(b"<svg xmlns="), None, "SVG may carry script");
    assert_eq!(image_type(b""), None);
}
