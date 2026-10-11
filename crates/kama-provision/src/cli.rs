//! `kama-provision` komut satırı (PROVISIONING-FORMAT-v0 §2). Argümanlar `std::env::args_os` ile
//! ayrıştırılır (clap yok, D11); `--out` UTF-8 olmayan bir yol olabilir. Standart çıktıya yalnız
//! yazılan dosyaların yolları basılır — anahtar, `dest_hash` ve sertifika basılmaz.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use kama_noise_wire::entropy::OsEntropy;

use crate::format::{DeviceName, NameError};
use crate::provision::{init, InitError, InitRequest};

/// Çalışma hatası çıkış kodu (G/Ç, entropi, saat, öz-denetim).
pub const EXIT_FAILURE: u8 = 1;
/// Kullanım hatası çıkış kodu (hiçbir dosya yazılmaz).
pub const EXIT_USAGE: u8 = 2;

const FLAG_OUT: &str = "--out";
const FLAG_DEVICES: &str = "--devices";
const FLAG_HUB: &str = "--hub";
const FLAG_DIRECT: &str = "--direct";
const FLAGS: [&str; 4] = [FLAG_OUT, FLAG_DEVICES, FLAG_HUB, FLAG_DIRECT];

/// Kullanım metni.
pub const USAGE: &str = "\
kullanım:
  kama-provision init --out <dizin> --devices <ad>,<ad>[,...]
                      [--hub <host:port>] [--direct <ad>=<ip:port>[,...]]
  kama-provision --help

biçim: docs/provisioning/PROVISIONING-FORMAT-v0.md
";

/// Başarılı `init` sonrası hatırlatma (D9).
const ROOT_REMINDER: &str = "root.json provisioning otoritesinde kalır; cihaza, Git'e, release \
paketine ve .so zip'ine girmez (D9).";

/// Ayrıştırılmış komut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Init(InitRequest),
}

/// Kullanım hatası. Kullanıcının verdiği değeri yalnız teşhis için taşır (CLI'a gizli değer verilmez).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    #[error("komut yok")]
    MissingCommand,
    #[error("bilinmeyen komut {0:?}")]
    UnknownCommand(String),
    #[error("bilinmeyen argüman {0:?}")]
    UnknownArgument(String),
    #[error("{0} birden fazla verildi")]
    Repeated(&'static str),
    #[error("{0} için değer yok")]
    MissingValue(&'static str),
    #[error("{0} zorunlu")]
    MissingRequired(&'static str),
    #[error("{0} değeri UTF-8 değil")]
    NotUtf8(&'static str),
    #[error("geçersiz cihaz adı {name:?}: {reason}")]
    BadName { name: String, reason: NameError },
    #[error("--direct girdisi ad=ip:port değil ya da ad tekrar ediyor: {0:?}")]
    BadDirect(String),
}

/// Komut satırını ayrıştırır; `args` program adını İÇERİR (`std::env::args_os()` gibi).
///
/// # Errors
/// [`UsageError`].
pub fn parse_args<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        return Err(UsageError::MissingCommand);
    };
    match command.to_str() {
        Some("init") => parse_init(rest).map(Command::Init),
        Some("--help" | "-h" | "help") => Ok(Command::Help),
        _ => Err(UsageError::UnknownCommand(lossy(command))),
    }
}

/// `init` argümanları: her bayrak tam bir değer alır ve en çok bir kez verilir.
fn parse_init(args: &[OsString]) -> Result<InitRequest, UsageError> {
    let mut out = None;
    let mut devices = None;
    let mut hub = None;
    let mut direct = None;
    for pair in args.chunks(2) {
        let flag = &pair[0];
        let Some(name) = known_flag(flag) else {
            return Err(UsageError::UnknownArgument(lossy(flag)));
        };
        let value = pair.get(1).ok_or(UsageError::MissingValue(name))?;
        let slot = match name {
            FLAG_OUT => &mut out,
            FLAG_DEVICES => &mut devices,
            FLAG_HUB => &mut hub,
            _ => &mut direct,
        };
        if slot.replace(value).is_some() {
            return Err(UsageError::Repeated(name));
        }
    }
    let out = out.ok_or(UsageError::MissingRequired(FLAG_OUT))?;
    let devices = devices.ok_or(UsageError::MissingRequired(FLAG_DEVICES))?;
    let devices = parse_devices(utf8(devices, FLAG_DEVICES)?)?;
    let hub = match hub {
        Some(value) => Some(utf8(value, FLAG_HUB)?.to_owned()),
        None => None,
    };
    let direct = match direct {
        Some(value) => parse_direct(utf8(value, FLAG_DIRECT)?)?,
        None => BTreeMap::new(),
    };
    Ok(InitRequest {
        out_dir: PathBuf::from(out),
        devices,
        hub,
        direct,
    })
}

fn known_flag(flag: &OsStr) -> Option<&'static str> {
    let text = flag.to_str()?;
    FLAGS.into_iter().find(|known| *known == text)
}

fn utf8(value: &OsStr, flag: &'static str) -> Result<&str, UsageError> {
    value.to_str().ok_or(UsageError::NotUtf8(flag))
}

fn lossy(text: &OsStr) -> String {
    text.to_string_lossy().into_owned()
}

fn parse_devices(text: &str) -> Result<Vec<DeviceName>, UsageError> {
    text.split(',').map(parse_name).collect()
}

fn parse_name(text: &str) -> Result<DeviceName, UsageError> {
    DeviceName::new(text).map_err(|reason| UsageError::BadName {
        name: text.to_owned(),
        reason,
    })
}

/// `ad=ip:port[,…]` — adres IP sabiti olmalı (ana makine adı yok); ad bir kez geçer.
fn parse_direct(text: &str) -> Result<BTreeMap<DeviceName, SocketAddr>, UsageError> {
    let mut direct = BTreeMap::new();
    for entry in text.split(',') {
        let bad = || UsageError::BadDirect(entry.to_owned());
        let (name, addr) = entry.split_once('=').ok_or_else(bad)?;
        let addr: SocketAddr = addr.parse().map_err(|_| bad())?;
        if direct.insert(parse_name(name)?, addr).is_some() {
            return Err(bad());
        }
    }
    Ok(direct)
}

/// CLI giriş noktası (`src/main.rs`).
#[must_use]
pub fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    match parse_args(args) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Init(request)) => run_init(&request),
        Err(error) => {
            eprintln!("kama-provision: {error}");
            eprint!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn run_init(request: &InitRequest) -> ExitCode {
    let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        eprintln!("kama-provision: sistem saati Unix başlangıcından önce");
        return ExitCode::from(EXIT_FAILURE);
    };
    let mut entropy = OsEntropy::new();
    match init(request, &mut entropy, elapsed.as_secs()) {
        Ok(report) => {
            if !report.permissions_enforced {
                eprintln!("kama-provision: uyarı: 0600/0700 izinleri yalnız Unix'te uygulanır");
            }
            println!("kama-provision: {} cihaz yazıldı:", request.devices.len());
            for path in &report.written {
                println!("  {}", path.display());
            }
            println!("{ROOT_REMINDER}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("kama-provision: {error}");
            let code = if matches!(error, InitError::Request(_)) {
                EXIT_USAGE
            } else {
                EXIT_FAILURE
            };
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(list: &[&str]) -> Vec<OsString> {
        std::iter::once("kama-provision")
            .chain(list.iter().copied())
            .map(OsString::from)
            .collect()
    }

    fn name(text: &str) -> DeviceName {
        DeviceName::new(text).expect("geçerli ad")
    }

    #[test]
    fn parses_a_full_init_command() {
        let parsed = parse_args(argv(&[
            "init",
            "--out",
            "/tmp/kama-out",
            "--devices",
            "alice,bob",
            "--hub",
            "hub.example:7000",
            "--direct",
            "alice=127.0.0.1:47001,bob=[::1]:47002",
        ]));
        let mut direct = BTreeMap::new();
        direct.insert(name("alice"), "127.0.0.1:47001".parse().expect("adres"));
        direct.insert(name("bob"), "[::1]:47002".parse().expect("adres"));
        let expected = InitRequest {
            out_dir: PathBuf::from("/tmp/kama-out"),
            devices: vec![name("alice"), name("bob")],
            hub: Some("hub.example:7000".to_owned()),
            direct,
        };
        assert_eq!(parsed, Ok(Command::Init(expected)));
        assert_eq!(parse_args(argv(&["--help"])), Ok(Command::Help));
    }

    #[test]
    fn malformed_command_lines_are_usage_errors() {
        let bad_name = UsageError::BadName {
            name: "Bob".to_owned(),
            reason: NameError::Charset,
        };
        let cases: &[(&[&str], UsageError)] = &[
            (&[], UsageError::MissingCommand),
            (&["make"], UsageError::UnknownCommand("make".to_owned())),
            (
                &["init", "--devices", "a,b"],
                UsageError::MissingRequired(FLAG_OUT),
            ),
            (
                &["init", "--out", "x"],
                UsageError::MissingRequired(FLAG_DEVICES),
            ),
            (&["init", "--out"], UsageError::MissingValue(FLAG_OUT)),
            (
                &["init", "--out", "x", "--out", "y"],
                UsageError::Repeated(FLAG_OUT),
            ),
            (
                &["init", "--bogus", "x"],
                UsageError::UnknownArgument("--bogus".to_owned()),
            ),
            (&["init", "--out", "x", "--devices", "alice,Bob"], bad_name),
        ];
        for (list, expected) in cases {
            let parsed = parse_args(argv(list));
            assert_eq!(parsed, Err(expected.clone()), "{list:?}");
        }
        // `--direct`: `ad=ip:port` değil / ana makine adı / tekrar eden ad.
        let direct_cases = [
            ("a:1", "a:1"),
            ("a=host:1", "a=host:1"),
            ("a=127.0.0.1:1,a=127.0.0.1:2", "a=127.0.0.1:2"),
        ];
        for (value, entry) in direct_cases {
            let list = ["init", "--out", "x", "--devices", "a,b", "--direct", value];
            let parsed = parse_args(argv(&list));
            let expected = UsageError::BadDirect(entry.to_owned());
            assert_eq!(parsed, Err(expected), "{value}");
        }
    }
}
