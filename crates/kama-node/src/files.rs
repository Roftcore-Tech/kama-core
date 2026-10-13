//! Dosya sistemi kenarı: alınan dosyanın diske yazılması ve gönderilecek dosyanın okunması.
//!
//! # Alınan dosya (plan D8 v1.1: "alınan dosya diske yazılır, yolu olayla verilir")
//!
//! Yol `<data_dir>/files/<eş>-<transfer_id>`'dir. **Plan yolundan beyanlı sapma
//! (`<data_dir>/files/<transfer_id>`): çakışma.** `transfer_id` EŞ BAŞINADIR; iki eşin aktarımı
//! aynı kimliği alabilir. `<eş>` provisioning dosyasındaki eş adıdır (`[a-z0-9][a-z0-9_-]{0,31}`;
//! yol ayırıcı, nokta, büyük harf yok). Ad **eşten gelen `FILE_BEGIN.name` alanından TÜRETİLMEZ**
//! (yol enjeksiyonu yüzeyi yok).
//!
//! Yazım atomiktir: içerik önce aynı dizinde `<ad>.part`'a yazılır ve eşitlenir (`sync_all`),
//! sonra `rename` ile yerine konur — okuyan yarım dosya görmez. Var olan dosyanın üstüne ASLA
//! yazılmaz: ad doluysa (aynı eş, aynı `transfer_id` — yalnız D4 dışı durumda, kalıcı durumunu
//! yitirmiş bir eşin aynı kimliği yeniden kullanmasıyla olur) `-<n>` eki denenir. Dizinin tek
//! yazanı düğüm iş parçacığıdır: denetim ile `rename` arasında yarış yoktur. Dosyalar Unix'te
//! `0600`, dizinler `0700` oluşturulur (at-rest şifreleme yok — PoC-KAPSAM-001 §2, kabul edilmiş
//! risk; izinler bu riski büyütmemek içindir).
//!
//! # Gönderilecek dosya
//!
//! Boy, okumadan ÖNCE `MAX_REASSEMBLY_BYTES` (S7) ile denetlenir ve okuma aynı tavanla sınırlıdır
//! (SR-AUDIT-001 bounds-before-alloc; okuma sırasında büyüyen dosya da kesilir). `FILE_BEGIN.name`
//! kaynak dosyanın adıdır (UTF-8 olmalı; uzunluk tavanını codec denetler).

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use kama_app::file_transfer::{FileSendError, TransferId, MAX_REASSEMBLY_BYTES};

use crate::event::FileFailReason;

/// Alınan dosyaların `data_dir` altındaki dizini.
pub const RECEIVED_FILES_DIR: &str = "files";

/// Yazım sırasında kullanılan geçici dosyanın eki (aynı dizinde: `rename` atomik kalır).
const PART_SUFFIX: &str = ".part";

/// Ad çakışmasında denenecek ad sayısı (ilk ad dahil) — yerel sınır, normatif DEĞİL
/// `[KALİBRASYON]`. Çakışma yalnız D4 dışı durumda doğar (modül belgesi); sınır, dizin taramasının
/// sınırsız sürmesini engeller. Tükenirse dosya yazılmaz (`AlreadyExists`) ve olay
/// `FileFailReason::Storage` olur.
const NAME_ATTEMPTS: u32 = 16;

/// Unix izinleri (kama-provision D9 hijyeniyle aynı).
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

/// Dizini (yoksa ebeveynleriyle) oluşturur; yeni dizinler Unix'te `0700`. Var olan dizine dokunmaz.
#[cfg(unix)]
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(dir)
}

/// Unix dışında izin uygulanmaz.
#[cfg(not(unix))]
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)
}

/// Alınan dosyaların dizini (bkz. modül belgesi).
#[derive(Debug)]
pub(crate) struct ReceivedFiles {
    dir: PathBuf,
}

impl ReceivedFiles {
    /// `<data_dir>/files`'ı (yoksa) oluşturur.
    pub(crate) fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join(RECEIVED_FILES_DIR);
        create_private_dir(&dir)?;
        Ok(Self { dir })
    }

    /// `bytes`'ı `<eş>-<transfer_id>`'ye (çakışmada `-<n>` ekiyle) atomik yazar; yolu döner.
    /// `peer_name` provisioning dosyasının eş adıdır — teldeki hiçbir alan ada girmez.
    pub(crate) fn store(
        &self,
        peer_name: &str,
        transfer_id: TransferId,
        bytes: &[u8],
    ) -> io::Result<PathBuf> {
        for attempt in 0..NAME_ATTEMPTS {
            let name = if attempt == 0 {
                format!("{peer_name}-{transfer_id}")
            } else {
                format!("{peer_name}-{transfer_id}-{attempt}")
            };
            let path = self.dir.join(&name);
            match fs::symlink_metadata(&path) {
                // Dolu ad: üstüne yazılmaz, sıradaki denenir.
                Ok(_) => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let part = self.dir.join(format!("{name}{PART_SUFFIX}"));
            if let Err(error) = write_private(&part, bytes).and_then(|()| fs::rename(&part, &path))
            {
                // En iyi çaba: yarım geçici dosya bırakılmaz (silme hatası asıl hatayı örtmez).
                let _ = fs::remove_file(&part);
                return Err(error);
            }
            return Ok(path);
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "every candidate name for the received file is taken",
        ))
    }
}

/// `path`'e (varsa keserek) yazar ve eşitler; Unix'te `0600`.
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        // `mode` yalnız yeni dosyada uygulanır ve umask'a tabidir; önceki bir koşudan kalmış
        // geçici dosya da tam `0600` olsun.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    }
    file.write_all(bytes)?;
    file.sync_all()
}

/// Gönderilecek dosya: (`FILE_BEGIN.name`, içerik). Boy okumadan ÖNCE denetlenir.
///
/// # Errors
/// [`FileFailReason::SourceName`] · [`FileFailReason::SourceUnreadable`] ·
/// [`FileFailReason::NotQueued`] (`FileSendError::TooLarge`).
pub(crate) fn read_outgoing(path: &Path) -> Result<(String, Vec<u8>), FileFailReason> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(FileFailReason::SourceName)?
        .to_owned();
    let unreadable = |error: io::Error| FileFailReason::SourceUnreadable(error.kind());
    let metadata = fs::metadata(path).map_err(unreadable)?;
    if !metadata.is_file() {
        return Err(FileFailReason::SourceUnreadable(
            io::ErrorKind::InvalidInput,
        ));
    }
    let too_large =
        |file_size: u64| FileFailReason::NotQueued(FileSendError::TooLarge { file_size });
    let declared = metadata.len();
    if declared > MAX_REASSEMBLY_BYTES {
        return Err(too_large(declared));
    }
    let capacity = usize::try_from(declared).map_err(|_| too_large(declared))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)
        .and_then(|file| file.take(MAX_REASSEMBLY_BYTES + 1).read_to_end(&mut bytes))
        .map_err(unreadable)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_REASSEMBLY_BYTES {
        return Err(too_large(read));
    }
    Ok((name, bytes))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static SEQ: AtomicU32 = AtomicU32::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let seq = SEQ.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path =
                std::env::temp_dir().join(format!("kama-node-files-{label}-{pid}-{seq}-{nanos}"));
            fs::create_dir(&path).expect("geçici dizin");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn received_file_name_comes_from_the_peer_book_not_the_wire_and_never_overwrites() {
        let dir = TempDir::new("received");
        let files = ReceivedFiles::open(&dir.0).expect("dizin");
        let first = files.store("alice", 7, b"ilk").expect("yazım");
        assert_eq!(first, dir.0.join(RECEIVED_FILES_DIR).join("alice-7"));
        assert_eq!(fs::read(&first).expect("okuma"), b"ilk");
        // Aynı (eş, transfer_id) yeniden: var olan dosyaya dokunulmaz, ek ad seçilir.
        let second = files.store("alice", 7, b"ikinci").expect("yazım");
        assert_eq!(second, dir.0.join(RECEIVED_FILES_DIR).join("alice-7-1"));
        assert_eq!(fs::read(&first).expect("okuma"), b"ilk", "üstüne yazılmadı");
        assert_eq!(fs::read(&second).expect("okuma"), b"ikinci");
        // Başka eşin aynı transfer_id'si çakışmaz.
        let other = files.store("bob", 7, b"bob").expect("yazım");
        assert_eq!(other, dir.0.join(RECEIVED_FILES_DIR).join("bob-7"));
        // Geçici dosya kalmaz.
        let leftovers: Vec<PathBuf> = fs::read_dir(dir.0.join(RECEIVED_FILES_DIR))
            .expect("dizin")
            .map(|entry| entry.expect("girdi").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "part"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn exhausted_names_fail_without_touching_existing_files() {
        let dir = TempDir::new("exhausted");
        let files = ReceivedFiles::open(&dir.0).expect("dizin");
        for _ in 0..NAME_ATTEMPTS {
            files.store("carol", 1, b"x").expect("yazım");
        }
        let error = files.store("carol", 1, b"y").expect_err("ad kalmadı");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(dir.0.join(RECEIVED_FILES_DIR).join("carol-1")).expect("okuma"),
            b"x"
        );
    }

    #[cfg(unix)]
    #[test]
    fn received_files_and_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;
        const MODE_BITS: u32 = 0o777;
        let dir = TempDir::new("private");
        let files = ReceivedFiles::open(&dir.0).expect("dizin");
        let path = files.store("alice", 1, b"gizli").expect("yazım");
        let file_mode = fs::metadata(&path).expect("üstveri").permissions().mode() & MODE_BITS;
        assert_eq!(file_mode, PRIVATE_FILE_MODE);
        let dir_mode = fs::metadata(dir.0.join(RECEIVED_FILES_DIR))
            .expect("üstveri")
            .permissions()
            .mode()
            & MODE_BITS;
        assert_eq!(dir_mode, PRIVATE_DIR_MODE);
    }

    #[test]
    fn outgoing_file_is_read_with_its_name_and_bounded() {
        let dir = TempDir::new("outgoing");
        let path = dir.0.join("foto.jpg");
        fs::write(&path, b"jpeg").expect("yazım");
        let (name, bytes) = read_outgoing(&path).expect("okuma");
        assert_eq!(name, "foto.jpg");
        assert_eq!(bytes, b"jpeg");

        let missing = read_outgoing(&dir.0.join("yok.jpg"));
        assert_eq!(
            missing,
            Err(FileFailReason::SourceUnreadable(io::ErrorKind::NotFound))
        );
        let not_a_file = read_outgoing(&dir.0);
        assert_eq!(
            not_a_file,
            Err(FileFailReason::SourceUnreadable(
                io::ErrorKind::InvalidInput
            ))
        );

        // Seyrek dosya: tavanın bir bayt üstü — içerik okunmadan (ayırmadan önce) reddedilir.
        let big = dir.0.join("büyük.bin");
        File::create(&big)
            .and_then(|file| file.set_len(MAX_REASSEMBLY_BYTES + 1))
            .expect("seyrek dosya");
        assert_eq!(
            read_outgoing(&big),
            Err(FileFailReason::NotQueued(FileSendError::TooLarge {
                file_size: MAX_REASSEMBLY_BYTES + 1
            }))
        );
    }

    #[cfg(unix)]
    #[test]
    fn outgoing_file_name_must_be_utf8() {
        use std::os::unix::ffi::OsStrExt;
        let dir = TempDir::new("name");
        const NOT_UTF8: &[u8] = b"\xFF\xFE.bin";
        let path = dir.0.join(OsStr::from_bytes(NOT_UTF8));
        fs::write(&path, b"x").expect("yazım");
        assert_eq!(read_outgoing(&path), Err(FileFailReason::SourceName));
    }
}
