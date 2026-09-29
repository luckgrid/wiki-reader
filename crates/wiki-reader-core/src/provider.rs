//! Collection access: `CollectionProvider` trait and filesystem provider.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;

/// Stable identity for a page inside a collection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageKey {
    /// Collection this page belongs to.
    pub collection_id: String,
    /// Path relative to the collection root.
    pub relative_path: PathBuf,
}

/// Discovered page metadata (identity only until parse/index land).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    /// Page identity.
    pub key: PageKey,
}

/// Seam for all content access. Index must not touch `std::fs` directly.
pub trait CollectionProvider {
    /// Absolute collection root.
    fn root(&self) -> &Path;

    /// Discover markdown pages under the root.
    ///
    /// # Errors
    ///
    /// Returns when the walk fails or the root is unreadable.
    fn list_pages(&self) -> Result<Vec<PageMeta>, Error>;

    /// Read the raw text of a page.
    ///
    /// # Errors
    ///
    /// Returns when the path escapes the root or the file cannot be read.
    fn read(&self, key: &PageKey) -> Result<String, Error>;
}

/// Filesystem-backed [`CollectionProvider`].
#[derive(Debug, Clone)]
pub struct FsProvider {
    root: PathBuf,
    collection_id: String,
}

impl FsProvider {
    /// Open `root` as a collection. `collection_id` defaults to the directory name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotADirectory`] when `root` is missing or not a directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root_ref = root.as_ref();
        let root = root_ref.canonicalize().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                Error::NotADirectory(root_ref.to_path_buf())
            } else {
                Error::Io(err)
            }
        })?;
        if !root.is_dir() {
            return Err(Error::NotADirectory(root));
        }
        let collection_id = root.file_name().map_or_else(
            || "collection".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        Ok(Self {
            root,
            collection_id,
        })
    }
}

impl CollectionProvider for FsProvider {
    fn root(&self) -> &Path {
        &self.root
    }

    fn list_pages(&self) -> Result<Vec<PageMeta>, Error> {
        // TODO(C1): config exclude globs.
        let mut pages = Vec::new();
        let walker = ignore::WalkBuilder::new(&self.root)
            .hidden(false) // content-model: include dot-dirs unless gitignored
            .require_git(false)
            .filter_entry(|e| {
                let name = e.file_name();
                name != ".git" && name != ".jj"
            })
            .build();

        for entry in walker {
            let entry = entry?;
            // Symlinked .md files are skipped: follow_links is false.
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let path = entry.path();
            if !is_markdown(path) {
                continue;
            }
            let relative_path = path
                .strip_prefix(&self.root)
                .map_err(|_| Error::PathOutsideRoot(path.to_path_buf()))?
                .to_path_buf();
            pages.push(PageMeta {
                key: PageKey {
                    collection_id: self.collection_id.clone(),
                    relative_path,
                },
            });
        }

        pages.sort_by(|a, b| a.key.relative_path.cmp(&b.key.relative_path));
        Ok(pages)
    }

    fn read(&self, key: &PageKey) -> Result<String, Error> {
        let abs = self.resolve(&key.relative_path)?;
        Ok(fs::read_to_string(abs)?)
    }
}

impl FsProvider {
    fn resolve(&self, relative: &Path) -> Result<PathBuf, Error> {
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(Error::PathOutsideRoot(relative.to_path_buf()));
        }
        let abs = self.root.join(relative);
        let abs = abs.canonicalize().map_err(Error::Io)?;
        if !abs.starts_with(&self.root) {
            return Err(Error::PathOutsideRoot(relative.to_path_buf()));
        }
        Ok(abs)
    }
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown")
    )
}

#[cfg(test)]
mod tests {
    use super::{CollectionProvider, FsProvider, PageKey};
    use crate::Error;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example")
    }

    fn rel_paths(provider: &FsProvider) -> Vec<String> {
        provider
            .list_pages()
            .expect("list")
            .iter()
            .map(|p| p.key.relative_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn discovers_worked_example_pages() {
        let provider = FsProvider::open(fixture_root()).expect("open fixture");
        let paths = rel_paths(&provider);

        assert_eq!(
            paths,
            [
                "README.md",
                "architecture/README.md",
                "architecture/design-system/README.md",
                "architecture/design-system/tokens.md",
                "architecture/wfos/README.md",
                "decisions/0001-stack.md",
                "decisions/0002-adapters.md",
            ]
        );
        assert_eq!(provider.root().file_name().unwrap(), "worked-example");
    }

    #[test]
    fn reads_page_body() {
        let provider = FsProvider::open(fixture_root()).expect("open fixture");
        let pages = provider.list_pages().expect("list");
        let root = pages
            .iter()
            .find(|p| p.key.relative_path == Path::new("README.md"))
            .expect("root README");
        let body = provider.read(&root.key).expect("read");
        assert!(body.contains("Worked Example Wiki"));
    }

    #[test]
    fn discovery_respects_ignore_hidden_markdown_and_vcs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        fs::write(root.join(".gitignore"), "secret.md\n").expect("gitignore");
        fs::write(root.join("secret.md"), "ignored\n").expect("secret");
        fs::write(root.join("notes.txt"), "not markdown\n").expect("txt");
        fs::write(root.join("y.markdown"), "markdown ext\n").expect("markdown");
        fs::create_dir(root.join(".planning")).expect("planning dir");
        fs::write(root.join(".planning/x.md"), "hidden dir\n").expect("planning page");
        fs::create_dir(root.join(".git")).expect("git dir");
        fs::write(root.join(".git/z.md"), "vcs noise\n").expect("git md");

        let provider = FsProvider::open(root).expect("open");
        let paths = rel_paths(&provider);

        assert_eq!(paths, [".planning/x.md", "y.markdown"]);
        assert!(paths.iter().all(|p| !p.split('/').any(|c| c == ".git")));
    }

    #[test]
    fn open_rejects_missing_path_and_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope");
        assert!(matches!(
            FsProvider::open(&missing),
            Err(Error::NotADirectory(_))
        ));

        let file = dir.path().join("file.md");
        fs::write(&file, "x\n").expect("write");
        assert!(matches!(
            FsProvider::open(&file),
            Err(Error::NotADirectory(_))
        ));
    }

    #[test]
    fn read_rejects_parent_and_absolute_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("ok.md"), "hi\n").expect("write");
        let provider = FsProvider::open(dir.path()).expect("open");

        let parent = PageKey {
            collection_id: provider
                .root()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into(),
            relative_path: PathBuf::from("../ok.md"),
        };
        assert!(matches!(
            provider.read(&parent),
            Err(Error::PathOutsideRoot(_))
        ));

        let absolute = PageKey {
            collection_id: parent.collection_id.clone(),
            relative_path: dir.path().join("ok.md"),
        };
        assert!(matches!(
            provider.read(&absolute),
            Err(Error::PathOutsideRoot(_))
        ));
    }
}
