//! `hub-server` — Kama Hub ikilisi (KM-HUB-TDD-001 §5).
//!
//! ```text
//! hub-server --config <hub.json> [--listen <ip:port>]    hub'ı koşar (--listen: yerel koşumda hub.json'u ezer)
//! hub-server probe --device <device.json> [--hub <h:p>]  bind + 3 heartbeat (KM-DEPLOY-TDD-001 §4);
//!                                                        --hub yoksa cihaz dosyasındaki hub.addr
//! ```
//!
//! Hub: tek iş parçacığı, `recv_from` → [`Hub::on_datagram`] → `send_to`. Okuma zaman aşımı yalnız
//! zamanlayıcının trafik yokken de koşması içindir. Loglar standart çıktıya satır satır.
//!
//! Çıkış kodu: hub döngüsü 0 dönmez · 1 başlatma hatası (`config unreadable: <yol>` dahil) ya da kalıcı
//! soket hatası · probe 0 yeşil / 1 kırmızı · 2 kullanım hatası.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fmt;
use std::io::{ErrorKind, Write};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use hub_server::config::{self, ConfigError};
use hub_server::{probe, Hub, Sink, SystemClock};
use kama_noise_wire::entropy::OsEntropy;

const USAGE: &str = "usage: hub-server --config <hub.json> [--listen <ip:port>]\n       hub-server probe --device <device.json> [--hub <host:port>]";
/// Zamanlayıcının en geç koşma aralığı.
const TICK: Duration = Duration::from_secs(1);
const ERROR_BACKOFF: Duration = Duration::from_millis(50);
/// Art arda bu kadar soket hatasından (~10 s) sonra süreç çıkar; yeniden başlatma systemd'nin işi.
const MAX_CONSECUTIVE_FAILURES: u32 = 200;
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;

struct UdpSink<'a> {
    socket: &'a UdpSocket,
}

impl Sink for UdpSink<'_> {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) {
        let _ = self.socket.send_to(datagram, to);
    }

    fn log(&mut self, line: fmt::Arguments<'_>) {
        // `println!` kapalı boruda panikler; yazılamayan satır düşer.
        let _ = writeln!(std::io::stdout().lock(), "{line}");
    }
}

enum Command {
    Serve {
        config: PathBuf,
        listen: Option<SocketAddr>,
    },
    Probe {
        device: PathBuf,
        hub: Option<String>,
    },
}

/// `--bayrak değer` çiftleri; bilinmeyen ya da yinelenen bayrak, eksik değer → `None`.
fn flags<'a>(args: &'a [OsString], known: &[&str]) -> Option<Vec<(&'a str, &'a OsString)>> {
    let mut out: Vec<(&str, &OsString)> = Vec::new();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let flag = flag.to_str()?;
        let value = rest.next()?;
        if !known.contains(&flag) || out.iter().any(|(seen, _)| *seen == flag) {
            return None;
        }
        out.push((flag, value));
    }
    Some(out)
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Option<Command> {
    let args: Vec<OsString> = args.skip(1).collect();
    let get = |pairs: &[(&str, &OsString)], name: &str| {
        pairs
            .iter()
            .find(|(f, _)| *f == name)
            .map(|(_, v)| (*v).clone())
    };
    if args.first().is_some_and(|first| first == "probe") {
        let pairs = flags(args.get(1..)?, &["--device", "--hub"])?;
        return Some(Command::Probe {
            device: PathBuf::from(get(&pairs, "--device")?),
            hub: match get(&pairs, "--hub") {
                Some(hub) => Some(hub.into_string().ok()?),
                None => None,
            },
        });
    }
    let pairs = flags(&args, &["--config", "--listen"])?;
    Some(Command::Serve {
        config: PathBuf::from(get(&pairs, "--config")?),
        listen: match get(&pairs, "--listen") {
            Some(listen) => Some(listen.to_str()?.parse().ok()?),
            None => None,
        },
    })
}

fn main() -> ExitCode {
    match parse_args(std::env::args_os()) {
        Some(Command::Serve { config, listen }) => serve(&config, listen),
        Some(Command::Probe { device, hub }) => run_probe(&device, hub.as_deref()),
        None => {
            eprintln!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn run_probe(device: &std::path::Path, hub: Option<&str>) -> ExitCode {
    let device = match probe::load_device(device) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("probe: {error}");
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    let Some(hub) = hub
        .or(device.hub_addr.as_deref())
        .and_then(|hub| hub.to_socket_addrs().ok())
        .and_then(|mut addrs| addrs.next())
    else {
        eprintln!(
            "probe: hub address missing or not host:port (--hub or hub.addr in the device file)"
        );
        return ExitCode::from(EXIT_USAGE);
    };
    match probe::run(&device, hub, &mut OsEntropy::new()) {
        Ok(outcome) => {
            println!("{outcome}");
            if outcome.healthy() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_FAILURE)
            }
        }
        Err(error) => {
            eprintln!("probe: {error}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

fn serve(path: &std::path::Path, listen: Option<SocketAddr>) -> ExitCode {
    let mut config = match config::load(path) {
        Ok(config) => config,
        Err(error @ ConfigError::Unreadable(_)) => {
            // T-C3 / Deploy T-DEP-6b: tek satır, başka çıktı yok.
            eprintln!("{error}");
            return ExitCode::from(EXIT_FAILURE);
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    // `--listen`: hub.json'daki adresi yerel koşum için ezer (INT-02H; YAAZ-EK-1).
    if let Some(listen) = listen {
        config.listen = listen;
    }
    let socket = match UdpSocket::bind(config.listen) {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("cannot bind {}: {error}", config.listen);
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    if let Err(error) = socket.set_read_timeout(Some(TICK)) {
        eprintln!("cannot set read timeout: {error}");
        return ExitCode::from(EXIT_FAILURE);
    }
    let listen = socket.local_addr().unwrap_or(config.listen);
    let devices = config.devices.len();
    let mut hub = match Hub::new(config, SystemClock::new(), OsEntropy::new()) {
        Ok(hub) => hub,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    // Sınırın bir fazlası: daha büyük datagram tampona kesilerek gelir ve boyundan tanınır.
    let mut buffer = vec![0u8; hub_server::wire::MAX_DATAGRAM.saturating_add(1)];
    let mut sink = UdpSink { socket: &socket };
    sink.log(format_args!(
        "hub listening addr={listen} devices={devices}"
    ));

    let mut failures: u32 = 0;
    loop {
        match socket.recv_from(&mut buffer) {
            Ok((length, src)) => {
                failures = 0;
                if let Some(datagram) = buffer.get(..length) {
                    hub.on_datagram(src, datagram, &mut sink);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(error) => {
                failures = failures.saturating_add(1);
                if failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!("recv keeps failing ({error}); exiting");
                    return ExitCode::from(EXIT_FAILURE);
                }
                if failures == 1 {
                    eprintln!("recv: {error}");
                }
                std::thread::sleep(ERROR_BACKOFF);
            }
        }
        hub.on_tick(&mut sink);
    }
}
