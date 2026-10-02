//! Local image policy (ADR-0017): static files inside the collection, never fetched.
//!
//! Terminal-free: this module only decides *whether* a markdown image destination may be read
//! and returns the canonical file. Decoding, sizing and drawing live in the render and TUI crates.

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Largest image file read from disk.
pub const MAX_IMAGE_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// Largest decoded image (width × height) accepted before decoding.
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;

/// Why an image destination renders as the text placeholder instead of a picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageReject {
    /// `http(s):`, `file:`, `data:` or any other URL scheme.
    Url,
    /// Resolves outside the collection root (`..` escape, absolute path or symlink).
    OutsideCollection,
    /// No such file.
    Missing,
    /// Exists but is not a regular file.
    NotAFile,
    /// File is larger than [`MAX_IMAGE_FILE_BYTES`].
    TooLarge,
    /// Extension is not a supported static format.
    UnsupportedFormat,
    /// Decoded size exceeds [`MAX_IMAGE_PIXELS`].
    TooManyPixels,
    /// Header could not be read as an image.
    Unreadable,
    /// No collection root was provided to the renderer.
    NoRoot,
    /// The terminal has no usable graphics protocol (the image tier is an upgrade).
    NoGraphics,
}

impl fmt::Display for ImageReject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Url => "URL images are never fetched",
            Self::OutsideCollection => "outside the collection",
            Self::Missing => "file not found",
            Self::NotAFile => "not a file",
            Self::TooLarge => "file too large",
            Self::UnsupportedFormat => "unsupported image format",
            Self::TooManyPixels => "image too large",
            Self::Unreadable => "unreadable image",
            Self::NoRoot => "no collection root",
            Self::NoGraphics => "no graphics protocol",
        })
    }
}

/// A validated local image file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalImage {
    /// Canonical absolute path, inside the collection root.
    pub path: PathBuf,
    /// File size in bytes (part of the freshness stamp).
    pub bytes: u64,
    /// Modified time in nanoseconds since the epoch (part of the freshness stamp).
    pub modified_nanos: u64,
}

/// Formats ADR-0017 allows (SVG via `resvg` with no external refs).
fn supported_extension(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
        )
    })
}

/// True when `dest` starts with a URL scheme (`https:`, `file:`, `data:`, …).
fn has_scheme(dest: &str) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    // Single-letter "schemes" are drive letters, not URLs.
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.len() > 1
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Normalise `.` and `..` lexically. `None` when the path climbs above its own start.
fn lexical_normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// Resolve a markdown image destination written in the page at `page_rel`.
///
/// `root` is the collection root; `page_rel` is the page path relative to it. Never touches the
/// network, and never stats a path that lexically escapes `root`.
///
/// # Errors
///
/// Returns the [`ImageReject`] reason when the destination must render as a placeholder.
pub fn resolve_local_image(
    root: &Path,
    page_rel: &Path,
    dest: &str,
) -> Result<LocalImage, ImageReject> {
    let dest = dest.trim();
    if dest.is_empty() {
        return Err(ImageReject::Missing);
    }
    if has_scheme(dest) {
        return Err(ImageReject::Url);
    }
    let without_suffix = dest.split(['#', '?']).next().unwrap_or(dest);
    let decoded = percent_encoding::percent_decode_str(without_suffix)
        .decode_utf8()
        .map_err(|_| ImageReject::Missing)?;
    let dest_path = Path::new(decoded.as_ref());

    let root_canon = root.canonicalize().map_err(|_| ImageReject::Missing)?;
    let candidate = if dest_path.is_absolute() {
        dest_path.to_path_buf()
    } else {
        let dir = page_rel.parent().unwrap_or_else(|| Path::new(""));
        // Normalise relative to the root first so `..` can't climb out before any fs access.
        let rel = lexical_normalize(&dir.join(dest_path)).ok_or(ImageReject::OutsideCollection)?;
        root_canon.join(rel)
    };
    if !candidate.starts_with(&root_canon) && !candidate.starts_with(root) {
        return Err(ImageReject::OutsideCollection);
    }
    if !supported_extension(&candidate) {
        return Err(ImageReject::UnsupportedFormat);
    }
    let canonical = candidate.canonicalize().map_err(|_| ImageReject::Missing)?;
    // Symlinks are resolved by `canonicalize`: the real file must still live under the root.
    if !canonical.starts_with(&root_canon) {
        return Err(ImageReject::OutsideCollection);
    }
    let meta = std::fs::metadata(&canonical).map_err(|_| ImageReject::Missing)?;
    if !meta.is_file() {
        return Err(ImageReject::NotAFile);
    }
    if meta.len() > MAX_IMAGE_FILE_BYTES {
        return Err(ImageReject::TooLarge);
    }
    let modified_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
        .unwrap_or(0);
    Ok(LocalImage {
        path: canonical,
        bytes: meta.len(),
        modified_nanos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// `root/` is the collection; `outside.png` sits next to it.
    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("wiki");
        fs::create_dir_all(root.join("img")).expect("img dir");
        fs::create_dir_all(root.join("docs")).expect("docs dir");
        fs::write(root.join("img/a.png"), b"png").expect("a.png");
        fs::write(root.join("img/my pic.png"), b"png").expect("spaced");
        fs::write(root.join("img/notes.txt"), b"txt").expect("txt");
        fs::write(root.join("logo.PNG"), b"png").expect("logo");
        fs::write(dir.path().join("outside.png"), b"png").expect("outside");
        Fixture {
            root: root.canonicalize().expect("canonical root"),
            _dir: dir,
        }
    }

    fn resolve(fx: &Fixture, page: &str, dest: &str) -> Result<LocalImage, ImageReject> {
        resolve_local_image(&fx.root, Path::new(page), dest)
    }

    #[test]
    fn resolves_relative_to_the_page() {
        let fx = fixture();
        let img = resolve(&fx, "docs/page.md", "../img/a.png").expect("resolves");
        assert_eq!(img.path, fx.root.join("img/a.png"));
        assert_eq!(img.bytes, 3);
        assert_eq!(
            resolve(&fx, "page.md", "img/a.png")
                .expect("root page")
                .path,
            fx.root.join("img/a.png")
        );
    }

    #[test]
    fn decodes_percent_escapes_and_drops_fragment_and_query() {
        let fx = fixture();
        assert!(resolve(&fx, "page.md", "img/my%20pic.png").is_ok());
        assert!(resolve(&fx, "page.md", "img/a.png#frag").is_ok());
        assert!(resolve(&fx, "page.md", "img/a.png?v=2").is_ok());
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        let fx = fixture();
        assert!(resolve(&fx, "page.md", "logo.PNG").is_ok());
    }

    #[test]
    fn urls_are_never_resolved() {
        let fx = fixture();
        for dest in [
            "https://example.com/a.png",
            "http://example.com/a.png",
            "file:///etc/passwd.png",
            "data:image/png;base64,AAAA",
            "ftp://host/a.png",
        ] {
            assert_eq!(
                resolve(&fx, "page.md", dest),
                Err(ImageReject::Url),
                "{dest}"
            );
        }
    }

    #[test]
    fn dotdot_escape_is_rejected_without_touching_the_fs() {
        let fx = fixture();
        // `outside.png` exists next to the root; a lexical escape must not reach it.
        assert_eq!(
            resolve(&fx, "page.md", "../outside.png"),
            Err(ImageReject::OutsideCollection)
        );
        assert_eq!(
            resolve(&fx, "docs/page.md", "../../outside.png"),
            Err(ImageReject::OutsideCollection)
        );
        assert_eq!(
            resolve(&fx, "docs/page.md", "../../../etc/hosts.png"),
            Err(ImageReject::OutsideCollection)
        );
    }

    #[test]
    fn absolute_path_must_stay_inside_the_root() {
        let fx = fixture();
        let inside = fx.root.join("img/a.png");
        assert!(resolve(&fx, "page.md", inside.to_str().expect("utf8")).is_ok());
        let outside = fx.root.parent().expect("parent").join("outside.png");
        assert_eq!(
            resolve(&fx, "page.md", outside.to_str().expect("utf8")),
            Err(ImageReject::OutsideCollection)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_out_of_the_root_is_rejected() {
        let fx = fixture();
        let target = fx.root.parent().expect("parent").join("outside.png");
        std::os::unix::fs::symlink(&target, fx.root.join("img/link.png")).expect("symlink");
        assert_eq!(
            resolve(&fx, "page.md", "img/link.png"),
            Err(ImageReject::OutsideCollection)
        );
        // A symlink that stays inside is fine.
        std::os::unix::fs::symlink(fx.root.join("img/a.png"), fx.root.join("img/inner.png"))
            .expect("inner symlink");
        assert!(resolve(&fx, "page.md", "img/inner.png").is_ok());
    }

    #[test]
    fn missing_directory_and_unsupported_files_are_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve(&fx, "page.md", "img/nope.png"),
            Err(ImageReject::Missing)
        );
        assert_eq!(resolve(&fx, "page.md", ""), Err(ImageReject::Missing));
        assert_eq!(
            resolve(&fx, "page.md", "img/notes.txt"),
            Err(ImageReject::UnsupportedFormat)
        );
        fs::write(fx.root.join("img/diagram.svg"), b"<svg/>").expect("svg");
        assert!(resolve(&fx, "page.md", "img/diagram.svg").is_ok());
        fs::create_dir_all(fx.root.join("dir.png")).expect("dir named like an image");
        assert_eq!(
            resolve(&fx, "page.md", "dir.png"),
            Err(ImageReject::NotAFile)
        );
    }

    #[test]
    fn oversized_file_is_rejected_before_decode() {
        let fx = fixture();
        let big = fs::File::create(fx.root.join("big.png")).expect("big");
        big.set_len(MAX_IMAGE_FILE_BYTES + 1).expect("sparse len");
        assert_eq!(
            resolve(&fx, "page.md", "big.png"),
            Err(ImageReject::TooLarge)
        );
    }

    #[test]
    fn scheme_detection_ignores_drive_letters_and_plain_colons() {
        assert!(has_scheme("https://x"));
        assert!(has_scheme("mailto:a@b"));
        assert!(!has_scheme("C:/img.png"));
        assert!(!has_scheme("img/a:b.png"));
        assert!(!has_scheme("a.png"));
    }
}
