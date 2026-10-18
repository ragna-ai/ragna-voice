//! Model store: the bundled Model, the catalogue of downloadable Models, and verified
//! downloads.
//!
//! Downloading a Model is the app's only network call (`CONTEXT.md`, Properties), and
//! only ever starts from a user action.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

/// A whisper.cpp Model from the `ggerganov/whisper.cpp` repository on HuggingFace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// The file name without `.bin`, e.g. `ggml-base.en`. Stable: preferences store it.
    pub id: &'static str,
    pub name: &'static str,
    /// English-only Models (`*.en`) can't transcribe other languages.
    pub multilingual: bool,
    pub size_bytes: u64,
    /// Lowercase hex. Checked after every download.
    pub sha256: &'static str,
}

impl Model {
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}.bin", self.id)
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.file_name()
        )
    }
}

/// Ships inside the app, so a first run works offline.
pub const BUNDLED: Model = Model {
    id: "ggml-base.en-q5_1",
    name: "Base (English, compact)",
    multilingual: false,
    size_bytes: 59_721_011,
    sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
};

/// Every Model the app offers, smallest first within each group. Sizes and hashes are the
/// HuggingFace LFS metadata, checked on 2026-10-11.
pub const CATALOG: &[Model] = &[
    BUNDLED,
    model(
        "ggml-tiny.en",
        "Tiny (English)",
        false,
        77_704_715,
        "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    ),
    model(
        "ggml-base.en",
        "Base (English)",
        false,
        147_964_211,
        "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    ),
    model(
        "ggml-small.en",
        "Small (English)",
        false,
        487_614_201,
        "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    ),
    model(
        "ggml-medium.en",
        "Medium (English)",
        false,
        1_533_774_781,
        "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
    ),
    model(
        "ggml-tiny",
        "Tiny (multilingual)",
        true,
        77_691_713,
        "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    ),
    model(
        "ggml-base",
        "Base (multilingual)",
        true,
        147_951_465,
        "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    ),
    model(
        "ggml-small",
        "Small (multilingual)",
        true,
        487_601_967,
        "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    ),
    model(
        "ggml-medium",
        "Medium (multilingual)",
        true,
        1_533_763_059,
        "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    ),
    model(
        "ggml-large-v3",
        "Large v3 (multilingual)",
        true,
        3_095_033_483,
        "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    ),
];

const fn model(
    id: &'static str,
    name: &'static str,
    multilingual: bool,
    size_bytes: u64,
    sha256: &'static str,
) -> Model {
    Model {
        id,
        name,
        multilingual,
        size_bytes,
        sha256,
    }
}

/// The catalogue entry with this id.
#[must_use]
pub fn find(id: &str) -> Option<&'static Model> {
    CATALOG.iter().find(|model| model.id == id)
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Http(String),
    /// The download is not the file the catalogue describes: corrupt, truncated, or
    /// replaced upstream. Nothing was installed.
    Mismatch {
        expected: String,
        actual: String,
    },
    /// The server sent more than the Model's size. Stopped to protect the disk.
    TooLarge,
    Cancelled,
    /// The bundled Model is part of the app and can't be deleted.
    Bundled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Http(e) => write!(f, "download failed: {e}"),
            Self::Mismatch { .. } => write!(f, "the download is damaged (checksum mismatch)"),
            Self::TooLarge => write!(f, "the download is larger than expected"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Bundled => write!(f, "the bundled Model can't be deleted"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Where Models live: downloads in `dir`, the bundled Model in `bundled_dir` (the app's
/// `Resources`; `None` when running unbundled).
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    bundled_dir: Option<PathBuf>,
}

impl Store {
    #[must_use]
    pub fn new(dir: PathBuf, bundled_dir: Option<PathBuf>) -> Self {
        Self { dir, bundled_dir }
    }

    /// The installed file for `model`, if there is one.
    #[must_use]
    pub fn path(&self, model: &Model) -> Option<PathBuf> {
        let bundled = (*model == BUNDLED)
            .then(|| {
                self.bundled_dir
                    .as_ref()
                    .map(|dir| dir.join(model.file_name()))
            })
            .flatten();
        bundled
            .into_iter()
            .chain([self.dir.join(model.file_name())])
            .find(|path| path.is_file())
    }

    #[must_use]
    pub fn is_installed(&self, model: &Model) -> bool {
        self.path(model).is_some()
    }

    /// Deletes a downloaded Model.
    ///
    /// # Errors
    ///
    /// The Model is the bundled one, or the file could not be removed.
    pub fn delete(&self, model: &Model) -> Result<(), Error> {
        if *model == BUNDLED {
            return Err(Error::Bundled);
        }
        match fs::remove_file(self.dir.join(model.file_name())) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// Downloads `model` from HuggingFace and installs it once its checksum matches.
    /// `progress(received, total)` runs as bytes arrive; setting `cancel` stops the
    /// download. Blocks: call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Network or disk errors, a checksum mismatch, an oversized response, or
    /// cancellation. Nothing is left behind on error.
    pub fn download(
        &self,
        model: &Model,
        progress: impl FnMut(u64, u64),
        cancel: &AtomicBool,
    ) -> Result<PathBuf, Error> {
        let response = ureq::get(&model.url())
            .call()
            .map_err(|e| Error::Http(e.to_string()))?;
        let body = response.into_body().into_reader();
        install(body, model, &self.dir, progress, cancel)
    }
}

/// Streams `source` into `dir`, hashing as it goes, and moves it into place only if it
/// is exactly the Model's size and checksum. The partial file is removed on any error.
///
/// # Errors
///
/// See [`Store::download`].
pub fn install(
    source: impl Read,
    model: &Model,
    dir: &Path,
    progress: impl FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, Error> {
    fs::create_dir_all(dir)?;
    let partial = dir.join(format!("{}.part", model.file_name()));
    let result = write_verified(source, model, &partial, progress, cancel).and_then(|()| {
        let path = dir.join(model.file_name());
        fs::rename(&partial, &path)?;
        Ok(path)
    });
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

fn write_verified(
    mut source: impl Read,
    model: &Model,
    partial: &Path,
    mut progress: impl FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<(), Error> {
    let mut file = io::BufWriter::new(fs::File::create(partial)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    let mut received: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let n = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        let chunk = buffer.get(..n).unwrap_or_default();
        received = received.saturating_add(n as u64);
        if received > model.size_bytes {
            return Err(Error::TooLarge);
        }
        hasher.update(chunk);
        file.write_all(chunk)?;
        progress(received, model.size_bytes);
    }
    file.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    let actual = hex(&hasher.finalize());
    if actual != model.sha256 {
        return Err(Error::Mismatch {
            expected: model.sha256.to_owned(),
            actual,
        });
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTENT: &[u8] = b"not really a whisper model";

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn test_model(sha256: &'static str, size_bytes: u64) -> Model {
        model("ggml-test", "Test", false, size_bytes, sha256)
    }

    fn leak(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ragna-voice-models-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|it| {
                it.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn catalog_is_consistent() {
        for (i, m) in CATALOG.iter().enumerate() {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(
                m.sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "{}",
                m.id
            );
            assert_eq!(m.multilingual, !m.id.contains(".en"), "{}", m.id);
            assert!(
                CATALOG.iter().skip(i + 1).all(|other| other.id != m.id),
                "duplicate {}",
                m.id
            );
        }
        assert_eq!(find(BUNDLED.id), Some(&BUNDLED));
        assert_eq!(find("ggml-nope"), None);
    }

    #[test]
    fn installs_a_matching_download() {
        let dir = temp_dir("ok");
        let model = test_model(leak(sha(CONTENT)), CONTENT.len() as u64);
        let mut last = (0, 0);
        let path = install(
            CONTENT,
            &model,
            &dir,
            |r, t| last = (r, t),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), CONTENT);
        assert_eq!(last, (CONTENT.len() as u64, CONTENT.len() as u64));
        assert_eq!(entries(&dir), ["ggml-test.bin"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn checksum_mismatch_installs_nothing() {
        let dir = temp_dir("mismatch");
        let model = test_model(leak(sha(b"something else")), CONTENT.len() as u64);
        let err = install(CONTENT, &model, &dir, |_, _| {}, &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(err, Error::Mismatch { .. }), "{err}");
        assert!(entries(&dir).is_empty(), "{:?}", entries(&dir));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn oversized_download_stops_and_installs_nothing() {
        let dir = temp_dir("large");
        let model = test_model(leak(sha(CONTENT)), 4);
        let err = install(CONTENT, &model, &dir, |_, _| {}, &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(err, Error::TooLarge), "{err}");
        assert_eq!(entries(&dir), Vec::<String>::new());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cancel_installs_nothing() {
        let dir = temp_dir("cancel");
        let model = test_model(leak(sha(CONTENT)), CONTENT.len() as u64);
        let err = install(CONTENT, &model, &dir, |_, _| {}, &AtomicBool::new(true)).unwrap_err();
        assert!(matches!(err, Error::Cancelled), "{err}");
        assert_eq!(entries(&dir), Vec::<String>::new());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bundled_model_is_found_in_resources_and_cannot_be_deleted() {
        let (dir, resources) = (temp_dir("store"), temp_dir("resources"));
        fs::create_dir_all(&resources).unwrap();
        let store = Store::new(dir.clone(), Some(resources.clone()));
        assert!(!store.is_installed(&BUNDLED));
        fs::write(resources.join(BUNDLED.file_name()), b"x").unwrap();
        assert_eq!(
            store.path(&BUNDLED),
            Some(resources.join(BUNDLED.file_name()))
        );
        assert!(matches!(store.delete(&BUNDLED), Err(Error::Bundled)));
        let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(resources));
    }

    #[test]
    fn downloaded_model_can_be_deleted() {
        let dir = temp_dir("delete");
        fs::create_dir_all(&dir).unwrap();
        let tiny = find("ggml-tiny.en").unwrap();
        fs::write(dir.join(tiny.file_name()), b"x").unwrap();
        let store = Store::new(dir.clone(), None);
        assert!(store.is_installed(tiny));
        store.delete(tiny).unwrap();
        assert!(!store.is_installed(tiny));
        store.delete(tiny).unwrap();
        let _ = fs::remove_dir_all(dir);
    }
}
