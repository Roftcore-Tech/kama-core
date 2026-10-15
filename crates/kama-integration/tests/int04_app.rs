//! INT-04 — uygulama yolunun UÇTAN UCA gece testi: iki `KamaNode`, netem altında.
//!
//! NEDEN. Motorlar (Messenger / FileTransfer / Ptt / Signaling) birim testleriyle, uçtan uca ise
//! yalnız bozulmasız loopback'te (K4G-11 CORE testi) ve INT-02'de TEXT ile sınandı. Dosya, PTT ve
//! sinyalleşme bozulmuş ağda hiç koşmadı; oturum değişimi ve uzun kesinti de ölçülmedi.
//!
//! NE. Tek süreçte iki düğüm (ÜRÜN yolu `KamaNode::start`: gerçek anahtar, saat, entropi, depo,
//! UDP), veri datagramları netem'li bir arayüzden geçer:
//!   1. TEXT ↔ RECEIPT (iki yön)
//!   2. dosya (≈150 KB) → `FILE_RECEIPT(ok)` (AMP §3 FILE-R1)
//!   3. PTT akışı: OPEN → sıralı demetler → CLOSE (AMP §3 PTT-R0: geç parça DÜŞMEZ)
//!   4. `SIGNALING_BLOB` gidiş-dönüşü, baytlar aynen (AMP §3 SIG-R0)
//!   5. oturum değişimi: eş yeniden başlar (peer-restart supersession, S1 K7 / identity §16) —
//!      eş kapalıyken kuyruğa giren metin yeni oturumda AYNI `msg_id` ile teslim olur
//!   6. kesinti: `loss 100%` ile tam kesinti, sonra normal — mesaj KAYBOLUYOR mu
//!      (PoC-KAPSAM-001 K8; hub mailbox yok, yük tamamen gönderen outbox'ı + transport ARQ'da)
//!
//! ÖLÇÜM 2026-10-15 — KESİNTİDEN SONRA KENDİLİĞİNDEN TOPARLANMA YOK. Kesinti SIRASINDA Transport'a
//! verilen birim, o oturumun ömrü boyunca teslim edilmiyor: hat düzeldikten sonra 240 s beklendi,
//! gelmedi. Denenen ve İŞE YARAMAYAN kurtarma yolları: beklemek · `drain_background()` ·
//! `connect()` (iki yönde de). Teslimi YALNIZ eşin yeniden başlaması getirdi (gerçek supersession).
//! Süreye bağlı DEĞİL: 10 s, 20 s ve 60 s kesintide aynı. Zincir — (1) Transport tam kesintide
//! birimi terk eder; (2) Application outbox'tan yeniden gönderimi yalnız YENİ oturumda yapar
//! (S1 K7); (3) PoC'de ölü oturumu fark eden bir şey yoktur (heartbeat D18 ile kapsam dışı);
//! (4) `connect()` canlı sayılan oturumda idempotenttir, yeni oturum açmaz. Bu bir DUR değil,
//! ölçümdür (plan K4G-05 ek 2026-10-11); kalem BL KATMAN 6'dadır.
//!
//! Bu yüzden 6. adım kendiliğinden toparlanmayı İDDİA ETMEZ, ÖLÇER (`INT04_BLACKOUT_SELF_HEAL`);
//! iddia ettiği şey mesajın KAYBOLMADIĞIDIR: gelmediyse belgelenmiş kurtarma yolu (eş yeniden
//! başlar) sürülür ve kayıt AYNI `msg_id` ile teslim olur. Canlılık tespiti ileride eklenirse
//! ölçüm `yes`e döner ve test yine yeşil kalır.
//!
//! BEYANLI BOŞLUK (INT-02 ile aynı): el sıkışma zarfları süreç-içi röleden (`LoopbackEnvelopeRelay`,
//! DUR-K4G-11 → B) geçer, netem'den GEÇMEZ; netem yalnız veri datagramlarına uygulanır. İki cihaz
//! arası gerçek zarf taşıyıcısı = hub Plane 1 rölesi (S4, hub sprinti).
//!
//! KOŞUM. Bozulma bu testin DIŞINDA kurulur (`scripts/netem/int04_netns.sh`: rootless netns,
//! `unshare -r -n` + `tc netem` `lo` üzerinde); test yalnız kesinti penceresini kendi sürer (netns
//! içinde CAP_NET_ADMIN vardır). Bozulma `lo`'dadır çünkü iki düğüm AYNI SÜREÇTEDİR (zarf rölesi
//! süreç-içi) ve tek süreç tek ağ ad alanındadır: iki yerel adres arasındaki trafiği çekirdek
//! yerel teslim eder, veth'e konan netem ATLANIRDI. Ortam değişkenleri:
//!   * `KAMA_INT04_ADDRS`      — `<alice>,<bob>` veri adresleri. Yoksa 127.0.0.1'de boş portlar.
//!   * `KAMA_INT04_NETEM_DEV`  — kesintinin uygulanacağı arayüz. Yoksa kesinti adımı ATLANIR.
//!   * `KAMA_INT04_NETEM_BASE` — kesinti sonrası geri yüklenecek profil (vars. `delay 50ms loss 5%`).
//!   * `KAMA_INT04_BLACKOUT_S` — kesinti süresi, saniye (vars. 60).
//!
//! Test `#[ignore]`dir (kesinti penceresi tek başına ≥60 s): gecelik iş `-- --ignored` ile koşar.
//! Koştuğu kip çıktıya `INT04_MODE=<netem|loopback>` olarak yazılır; gecelik adım `netem` olmayan
//! koşuyu KABUL ETMEZ (sessiz bozulmasız geçiş yok).

mod common_app;

use std::fs;
use std::net::SocketAddr;
use std::process::Command;
use std::time::{Duration, Instant};

use kama_app::codec::PttCodec;
use kama_app::messaging::MsgId;
use kama_node::{FileStage, KamaNode, LinkState, NodeEvent};

use common_app::{free_ports, listener, provision_at, Device, Events, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// PoC fotoğrafı ≈150 KB (PoC-KAPSAM-001 §2).
const PHOTO_BYTES: usize = 150 * 1024;
/// İçerik deseni: asal modül, parça sınırlarıyla hizalanmaz (yer değiştiren parça görünür olur).
const PATTERN: usize = 251;

/// PTT basışı: demet sayısı, süresi ve boyu (değerler media'nındır; düğüm yorumlamaz).
const BUNDLES: usize = 8;
const BUNDLE_MS: u8 = 40;
const BUNDLE_LEN: usize = 64;

/// Sinyalleşme blob'unun boyu (ürün limitinin altında; içerik yalnız ayırt edilebilir olmalı).
const BLOB_LEN: usize = 128;

/// Kesinti sonrası teslim için ÜST SINIR: ARQ geri çekilmesi kesinti boyunca büyümüş olabilir.
const AFTER_BLACKOUT_WAIT: Duration = Duration::from_secs(240);
/// Hat düzeldikten sonra "kendiliğinden toparlandı mı" GÖZLEMİ için pencere (iddia değil, ölçüm).
const SELF_HEAL_OBSERVATION: Duration = Duration::from_secs(30);
/// Kesinti SIRASINDA "teslim olmadı" gözlemi için bekleme (olumsuz gözlem; bloklamaz).
const DURING_BLACKOUT_PROBE: Duration = Duration::from_secs(5);

const DEFAULT_BLACKOUT_S: u64 = 60;
const DEFAULT_NETEM_BASE: &str = "delay 50ms loss 5%";

/// Testin koştuğu kip. `Loopback` bozulmasızdır: geliştiricinin yerelde akışı koşturabilmesi
/// içindir, gecelik kapı onu KABUL ETMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Netem,
    Loopback,
}

/// Kesintiyi süren arayüz (yalnız netem kipinde).
struct Netem {
    dev: String,
    base: String,
}

impl Netem {
    /// `tc qdisc replace dev <dev> root netem <args>`; netns içinde CAP_NET_ADMIN vardır.
    fn apply(&self, args: &str) {
        let status = Command::new("tc")
            .args(["qdisc", "replace", "dev", &self.dev, "root", "netem"])
            .args(args.split_whitespace())
            .status()
            .unwrap_or_else(|err| panic!("tc çalıştırılamadı ({err}); netns içinde mi?"));
        assert!(status.success(), "tc netem `{args}` başarısız: {status}");
    }

    fn blackout(&self) {
        self.apply("loss 100%");
    }

    fn restore(&self) {
        self.apply(&self.base);
    }
}

/// Ortamdan kip ve adresler. `KAMA_INT04_ADDRS` yoksa 127.0.0.1'de boş portlar kullanılır.
fn environment() -> (Mode, Vec<SocketAddr>, Option<Netem>) {
    let addrs: Vec<SocketAddr> = match std::env::var("KAMA_INT04_ADDRS") {
        Ok(raw) => raw
            .split(',')
            .map(|part| {
                part.trim()
                    .parse()
                    .unwrap_or_else(|err| panic!("KAMA_INT04_ADDRS `{part}` çözümlenmedi: {err}"))
            })
            .collect(),
        Err(_) => free_ports(2)
            .into_iter()
            .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
            .collect(),
    };
    assert_eq!(addrs.len(), 2, "iki düğüm, iki adres: {addrs:?}");
    let netem = std::env::var("KAMA_INT04_NETEM_DEV").ok().map(|dev| Netem {
        dev,
        base: std::env::var("KAMA_INT04_NETEM_BASE")
            .unwrap_or_else(|_| DEFAULT_NETEM_BASE.to_owned()),
    });
    let mode = if netem.is_some() {
        Mode::Netem
    } else {
        Mode::Loopback
    };
    (mode, addrs, netem)
}

fn blackout_duration() -> Duration {
    let secs = std::env::var("KAMA_INT04_BLACKOUT_S")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_BLACKOUT_S);
    Duration::from_secs(secs)
}

fn start(device: &Device) -> (KamaNode, Events) {
    let (listener, events) = listener();
    let node = KamaNode::start(&device.config, &device.data, listener).expect("düğüm kurulmalı");
    (node, events)
}

fn wait_connected(alice_events: &mut Events, bob_events: &mut Events) {
    alice_events.wait("alice: Connected(bob)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == BOB).then_some(())
    });
    bob_events.wait("bob: Connected(alice)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == ALICE).then_some(())
    });
}

fn expect_text(events: &mut Events, from: &str, msg_id: MsgId, text: &str) {
    events.wait(&format!("TextReceived({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text: body }
                if peer_id == from && *got == msg_id && body == text
        )
        .then_some(())
    });
}

fn expect_receipt(events: &mut Events, from: &str, msg_id: MsgId) {
    events.wait(&format!("Receipt({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::Receipt { peer_id, msg_id: got } if peer_id == from && *got == msg_id
        )
        .then_some(())
    });
}

/// Eşle oturum hazır; bağlam-önce dağıtım ve teslim kanalı hiçbir şey düşürmedi. Anlık görüntü bu
/// çağrıdan SONRA hesaplanmış olmalıdır (`two_nodes_loopback::expect_clean_metrics` ile aynı
/// gerekçe: boşaltmadan sonraki İLK `Metrics` atlanır).
fn expect_clean_metrics(events: &mut Events, peer: &str) -> u8 {
    events.drain_now();
    events.wait("Metrics (boşaltmadan sonraki ilk; atlanır)", |event| {
        matches!(event, NodeEvent::Metrics(_)).then_some(())
    });
    let metrics = events.wait(&format!("Metrics: {peer} bağlı"), |event| match event {
        NodeEvent::Metrics(metrics)
            if metrics
                .connect_state
                .iter()
                .any(|state| state.peer_id == peer && state.state == LinkState::Connected) =>
        {
            Some(metrics.clone())
        }
        _ => None,
    });
    assert_eq!(metrics.ctx_mismatch_drops, 0, "{metrics:?}");
    assert_eq!(metrics.delivery_channel_drops, 0, "{metrics:?}");
    assert_eq!(metrics.hub_frames_dropped, 0, "{metrics:?}");
    metrics
        .connect_state
        .iter()
        .find(|state| state.peer_id == peer)
        .and_then(|state| state.epoch)
        .expect("bağlı eşin epoch'u")
}

fn photo() -> Vec<u8> {
    (0..PHOTO_BYTES)
        .map(|i| u8::try_from(i % PATTERN).expect("desen bir bayta sığar"))
        .collect()
}

fn blob(seed: usize) -> Vec<u8> {
    (0..BLOB_LEN)
        .map(|i| u8::try_from((seed + i) % PATTERN).expect("desen bir bayta sığar"))
        .collect()
}

#[test]
#[ignore = "INT-04: netem + 60 s kesinti; gecelik iş `-- --ignored` ile koşar"]
fn app_path_survives_netem_session_change_and_a_blackout() {
    let (mode, addrs, netem) = environment();
    println!(
        "INT04_MODE={}",
        match mode {
            Mode::Netem => "netem",
            Mode::Loopback => "loopback",
        }
    );
    if let Some(netem) = netem.as_ref() {
        println!("INT04_NETEM_DEV={} base={}", netem.dev, netem.base);
    }

    let root = TempDir::new("int04");
    let provisioned = provision_at(root.path(), &[ALICE, BOB], &addrs);
    let alice_device = &provisioned.devices[ALICE];
    let bob_device = &provisioned.devices[BOB];

    // Röle kayıtsız eşe zarf saklamaz: iki düğüm de bağlanmadan ÖNCE başlatılır.
    let (bob, mut bob_events) = start(bob_device);
    let (alice, mut alice_events) = start(alice_device);
    alice.connect(BOB).expect("connect");
    wait_connected(&mut alice_events, &mut bob_events);

    // ── 1. TEXT ↔ RECEIPT, iki yön ──────────────────────────────────────────────────────────
    let m1 = alice.send_text(BOB, "merhaba bob").expect("metin");
    expect_text(&mut bob_events, ALICE, m1, "merhaba bob");
    expect_receipt(&mut alice_events, BOB, m1);
    let n1 = bob.send_text(ALICE, "merhaba alice").expect("metin");
    expect_text(&mut alice_events, BOB, n1, "merhaba alice");
    expect_receipt(&mut bob_events, ALICE, n1);

    // ── 2. Dosya: içerik bayt-özdeş, gönderende `FILE_RECEIPT(ok)` (FILE-R1) ─────────────────
    let content = photo();
    let source = root.path().join("foto.jpg");
    fs::write(&source, &content).expect("kaynak dosya");
    let file = alice.send_file(BOB, &source).expect("dosya kuyruğa");
    let (path, size) = bob_events.wait("bob: FileReceived(alice)", |event| match event {
        NodeEvent::FileReceived {
            peer_id,
            path,
            size,
            ..
        } if peer_id == ALICE => Some((path.clone(), *size)),
        _ => None,
    });
    assert_eq!(size, u64::try_from(PHOTO_BYTES).expect("boy"));
    assert_eq!(
        fs::read(&path).expect("alınan dosya"),
        content,
        "bozulmuş ağda da içerik bayt-özdeş"
    );
    alice_events.wait("alice: FILE_RECEIPT(ok)", |event| {
        matches!(
            event,
            NodeEvent::FileProgress { file: got, stage: FileStage::Delivered, .. } if *got == file
        )
        .then_some(())
    });

    // ── 3. PTT: OPEN → sıralı demetler → CLOSE (PTT-R0: geç parça düşmez) ───────────────────
    let stream = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("ptt_open");
    let bundles: Vec<Vec<u8>> = (0..BUNDLES)
        .map(|i| vec![u8::try_from(i).expect("demet sırası"); BUNDLE_LEN])
        .collect();
    for bundle in &bundles {
        alice.ptt_push(stream, bundle.clone()).expect("demet");
    }
    alice.ptt_close(stream).expect("ptt_close");
    let stream_id = bob_events.wait("bob: PttOpened(alice)", |event| match event {
        NodeEvent::PttOpened {
            peer_id,
            stream_id,
            codec: PttCodec::Lyra,
            bundle_ms,
        } if peer_id == ALICE && *bundle_ms == BUNDLE_MS => Some(*stream_id),
        _ => None,
    });
    for (index, expected) in bundles.iter().enumerate() {
        let got = bob_events.wait(&format!("bob: PttBundle #{index}"), |event| match event {
            NodeEvent::PttBundle {
                peer_id,
                stream_id: got,
                payload,
            } if peer_id == ALICE && *got == stream_id => Some(payload.clone()),
            _ => None,
        });
        assert_eq!(
            &got, expected,
            "demetler gönderim sırasıyla ve eksiksiz (PTT-R0)"
        );
    }
    bob_events.wait("bob: PttClosed(alice)", |event| {
        matches!(
            event,
            NodeEvent::PttClosed { peer_id, stream_id: got }
                if peer_id == ALICE && *got == stream_id
        )
        .then_some(())
    });

    // ── 4. SIGNALING_BLOB: iki yön, baytlar aynen (SIG-R0) ──────────────────────────────────
    let to_bob = blob(0);
    alice
        .send_signaling(BOB, to_bob.clone())
        .expect("signaling alice→bob");
    let got = bob_events.wait("bob: SignalingReceived(alice)", |event| match event {
        NodeEvent::SignalingReceived { peer_id, bytes } if peer_id == ALICE => Some(bytes.clone()),
        _ => None,
    });
    assert_eq!(got, to_bob, "blob opak ve bayt-özdeş");
    let to_alice = blob(101);
    bob.send_signaling(ALICE, to_alice.clone())
        .expect("signaling bob→alice");
    let got = alice_events.wait("alice: SignalingReceived(bob)", |event| match event {
        NodeEvent::SignalingReceived { peer_id, bytes } if peer_id == BOB => Some(bytes.clone()),
        _ => None,
    });
    assert_eq!(got, to_alice, "blob opak ve bayt-özdeş");

    expect_clean_metrics(&mut alice_events, BOB);
    expect_clean_metrics(&mut bob_events, ALICE);

    // ── 5. Oturum değişimi: eş yeniden başlar (peer-restart supersession, S1 K7) ─────────────
    // Bob kapalıyken kuyruğa giren metin, yeni oturumda AYNI `msg_id` ile gider (AMP §3 S2).
    bob.shutdown().expect("bob kapanış");
    let m2 = alice
        .send_text(BOB, "bob kapalıyken")
        .expect("metin kuyruğa");
    assert!(m2 > m1, "eş başına msg_id kesin artan");
    let (mut bob, mut bob_events) = start(bob_device);
    // Yeniden başlatmayı EŞ başlatır (identity §16 peer-restart supersession adım 1): Alice ölü
    // oturumu kendiliğinden fark etmez — heartbeat PoC'de yoktur (D18). Yeni Bob'un oturumu yok,
    // INIT gönderir; Alice'te eski oturum yenisiyle değişir (`Ready(h')` → `Closed(h, Superseded)`).
    bob.connect(ALICE).expect("yeniden başlayan eş bağlanır");
    // `Connected` yalnız EŞİN GÜNCEL OTURUMU YOKKEN doğar: yeni Bob onu görür, Alice GÖRMEZ (onun
    // için bu supersession'dır, eş bağlı kalır). Alice tarafının kanıtı olayın kendisidir: AYNI
    // `msg_id` yeni oturumda teslim olur (S1 K7 / AMP §3 S2 — yeniden gönderim ESKİ `msg_id` ile).
    bob_events.wait("bob: Connected(alice)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == ALICE).then_some(())
    });
    expect_text(&mut bob_events, ALICE, m2, "bob kapalıyken");
    expect_receipt(&mut alice_events, BOB, m2);
    expect_clean_metrics(&mut alice_events, BOB);

    // ── 6. Kesinti: `loss 100%`, sonra normal (K8 ölçümü) ───────────────────────────────────
    if let Some(netem) = netem.as_ref() {
        let outage = blackout_duration();
        netem.blackout();
        let m3 = alice.send_text(BOB, "kesinti sırasında").expect("metin");

        // Kesinti sırasında teslim OLMAMALI: olumsuz gözlem, bloklamayan boşaltmayla.
        std::thread::sleep(DURING_BLACKOUT_PROBE);
        let seen = bob_events.drain_all();
        assert!(
            !seen.iter().any(|event| matches!(
                event,
                NodeEvent::TextReceived { peer_id, msg_id, .. } if peer_id == ALICE && *msg_id == m3
            )),
            "tam kesintide teslim olmamalı; görülen: {seen:?}"
        );

        let left = outage.saturating_sub(DURING_BLACKOUT_PROBE);
        std::thread::sleep(left);
        let restored_at = Instant::now();
        netem.restore();

        // ÖLÇÜM (iddia DEĞİL): hat düzelince oturum kendiliğinden toparlanıyor mu? 2026-10-15
        // ölçümü HAYIR diyor (modül belgesi) — ama bu bir KUSUR İDDİASI değildir ve ileride
        // canlılık tespiti gelirse `yes`e döner; test o zaman da yeşil kalmalıdır. Bu yüzden
        // sonuç yalnız BASILIR.
        let self_healed = {
            let deadline = Instant::now() + SELF_HEAL_OBSERVATION;
            let mut seen = false;
            while !seen && Instant::now() < deadline {
                std::thread::sleep(Duration::from_secs(1));
                seen = bob_events.drain_all().iter().any(|event| {
                    matches!(event, NodeEvent::TextReceived { peer_id, msg_id, .. }
                        if peer_id == ALICE && *msg_id == m3)
                });
            }
            seen
        };
        println!(
            "INT04_BLACKOUT_SELF_HEAL={} after_ms={}",
            if self_healed { "yes" } else { "no" },
            restored_at.elapsed().as_millis()
        );

        // İDDİA: mesaj KAYBOLMAZ. Kendiliğinden gelmediyse belgelenmiş kurtarma yolu sürülür —
        // eş yeniden başlar, yeni oturum kurulur ve RECEIPT'i gelmemiş TEXT kalıcı outbox'tan
        // AYNI `msg_id` ile yeniden gönderilir (S1 K7 / AMP §3 S2). Kesinti bunu değiştirmez.
        if !self_healed {
            bob.shutdown().expect("bob kapanış");
            let (restarted, fresh) = start(bob_device);
            bob = restarted;
            bob_events = fresh;
            bob.connect(ALICE).expect("yeniden başlayan eş bağlanır");
            bob_events.wait("bob: Connected(alice)", |event| {
                matches!(event, NodeEvent::Connected { peer_id } if peer_id == ALICE).then_some(())
            });
        }
        bob_events.wait_for(
            &format!("bob: kesinti sonrası TextReceived(alice, {m3})"),
            AFTER_BLACKOUT_WAIT,
            |event| {
                matches!(
                    event,
                    NodeEvent::TextReceived { peer_id, msg_id, .. }
                        if peer_id == ALICE && *msg_id == m3
                )
                .then_some(())
            },
        );
        alice_events.wait_for(
            &format!("alice: kesinti sonrası Receipt(bob, {m3})"),
            AFTER_BLACKOUT_WAIT,
            |event| {
                matches!(
                    event,
                    NodeEvent::Receipt { peer_id, msg_id } if peer_id == BOB && *msg_id == m3
                )
                .then_some(())
            },
        );
        println!(
            "INT04_BLACKOUT_DELIVERED_MS={}",
            restored_at.elapsed().as_millis()
        );
        expect_clean_metrics(&mut alice_events, BOB);
    } else {
        println!("INT04_BLACKOUT=atlandi (KAMA_INT04_NETEM_DEV yok)");
    }

    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}
