//! **Hızlı teşhis aracı** — ürün runtime'ında üç FEC kolunu AYNI kanalda karşılaştırır.
//!
//! Sorduğu soru tek: *burst kanalda adaptif kontrolcü (Q18 yapı-gate'i) statik-r'nin gerisinde mi
//! kalıyor?* Yapı-gate `burst_delta > 50_000 ppm` gördüğünde parity'yi SIFIR bırakıp işi ARQ'ya
//! devrediyor (`sender.rs::structure_gate_fec`); hostile ağ tam olarak o rejim.
//!
//! **Ne DEĞİL:** bu bir benchmark, bir ön-kayıtlı deney veya bir ürün iddiası DEĞİLDİR. netem yok,
//! iki-netns yok, quinn yok, tekrar-kümesi yok. Kanal in-process bir seam-erasure modeli, saat
//! loopback. Buradan çıkan sayılar **teşhis** tierindedir (sim / tek-hücre / saniyeler): bir kolun
//! diğerinden belirgin ayrıştığını GÖSTERİR, ürün-düzeyi bir üstünlük İDDİA ETMEZ. Ön-kayıtlı
//! doğrulama netem matrisinde yapılır.
//!
//! ## [ARACIN KENDİ SINIRI — ÖNCE BUNU OKU] Loopback'te kayıp girdisi YAPISAL OLARAK ÖLÜDÜR
//!
//! Bu araçla koşulan ilk hücrede ölçüldü (200×256 B, GE p_gb=3% / p_bg=20% / loss_bad=60%):
//!
//! ```text
//! ilan_pkt=82   un_loss=0   spurious_units=0      → 82 kayıp İLAN EDİLDİ, hiçbiri geri alınmadı
//! peak_ppm=0                                       → ama loss_windowed_ppm koşu boyunca TEPEDE 0
//! gate: kayıp<band = değerlendirmelerin %100'ü     → yapı-gate hep KAYIP ÖNKOŞULUNDA durdu
//! ```
//!
//! En tutarlı açıklama (doğrudan ölçülmedi, aritmetik gereklilik): ledger penceresi
//! `W_loss = K_WL(8) × ack_epoch`, `ack_epoch = max(srtt, EAI)`. Loopback'te srtt ~onlarca µs ve
//! pacer hızı çok yüksek olduğundan `W_loss` ~sub-millisecond'a iner; `fuzzy_evaluate` ise PACE
//! tick'iyle ~1 ms'de bir koşar. `ledger_tick` önce süre-dolduranı ATAR, sonra confirm eder →
//! her olay iki tick arasında `declared_ns < cutoff` olup düşer ve HİÇBİRİ `Confirmed` olamaz.
//! `confirm_age = 1.5×EAI` kısa olmasına rağmen sonuç değişmez: sıra atma-sonra-confirm'dir.
//!
//! **Sonuç:** bu hücrede kayıp-BAĞIMLI hiçbir fuzzy kuralı (FEC dahil) ateşleyemez. Yani araç
//! "hangi FEC kolu daha iyi" sorusunu YANITLAYAMAZ; yalnız YAPISAL soruları yanıtlar
//! ("kodlama devreye girdi mi", "gate hangi önkoşulda durdu", "prior mı veri mi"). Kol
//! karşılaştırması için GERÇEK RTT gerekir (netem). Bunu ölçmeden bu aracın teslim/ms/byte
//! sütunlarından performans hükmü ÇIKARMA.
//!
//! Bu bir ÜRÜN KUSURU İDDİASI DEĞİLDİR: gerçek ağda `ack_epoch ≈ srtt` (ms mertebesi) olduğundan
//! `W_loss` ~onlarca ms olur ve olaylar confirm'e yetişir. Kusur mu artefakt mı olduğu ancak
//! RTT'li bir hücrede ayrılır.
//!
//! Determinizm (ADR-17): kanal RNG'si seed'den türeyen saf-tamsayı LCG; aynı seed → aynı silme
//! deseni → üç kol AYNI kanalı görür. Kol karşılaştırması bu eşleştirmeye dayanır.
//!
//! Kullanım:
//!   cargo run -p client-core --example fec_arm_compare -- [--msgs N] [--payload B]
//!       [--seed S] [--p-gb PPM] [--p-bg PPM] [--loss-good PPM] [--loss-bad PPM]
//!
//! Varsayılan kanal: Gilbert-Elliott, GOOD'da ~%1, BAD'de ~%60 silme, ortalama burst ≈ 5 paket.

use std::time::Duration;

use client_core::runtime::{
    ClientEndpoint, FecRuntimeConfig, FecStructurePolicy, RuntimeConfig, RuntimeError,
    ServerEndpoint,
};
use client_core::seam::IdentitySeam;

/// Profil 0x03 (K=8, R=4, E=64) → chunk kapasitesi 53 B.
const PROFILE: u8 = 0x03;

fn arg(a: &[String], k: &str, d: u64) -> u64 {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Kanal modeli + seam: Gilbert-Elliott silme, YALNIZ ileri yönde (client→server DATA/FEC).
/// Dönüş yolu (ACK) temiz — ölçülen şey ileri-yön kurtarma, ACK yolu değil.
mod ge_seam {
    use client_core::seam::{
        OpenReject, OpenedDatagram, SealArgs, SealError, SealedDatagram, SeamPeer, SecuritySeam,
        IDENTITY_TAG,
    };

    /// Saf-tamsayı LCG (ADR-17: RNG kütüphanesi yok, aynı seed → aynı akış).
    struct Lcg(u64);

    impl Lcg {
        /// [0, 1_000_000) aralığında bir sonraki örnek.
        fn next_ppm(&mut self) -> u64 {
            // Numerical Recipes LCG sabitleri; kalite değil TEKRARLANABİLİRLİK amaçlı.
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (self.0 >> 33) % 1_000_000
        }
    }

    /// GE parametreleri (ppm).
    #[derive(Clone, Copy)]
    pub struct Ge {
        pub p_gb: u64,
        pub p_bg: u64,
        pub loss_good: u64,
        pub loss_bad: u64,
    }

    /// IdentitySeam + ileri-yön GE silme. `open` çağrıldığı sırada durum ilerletilir; her datagram
    /// bir kanal adımıdır (retransmit ve repair DAHİL — kanal içeriği bilmez).
    pub struct GeSeam {
        rng: Lcg,
        ge: Ge,
        bad: bool,
        pub seen: u64,
        pub dropped: u64,
    }

    impl GeSeam {
        #[must_use]
        pub fn new(seed: u64, ge: Ge) -> Self {
            Self {
                // Seed'i 0'dan uzaklaştır: LCG'nin ilk çıktısı seed'e fazla bağlı olmasın.
                rng: Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5DEE_CE66),
                ge,
                bad: false,
                seen: 0,
                dropped: 0,
            }
        }

        /// Bir kanal adımı: durum geçişi + silme kararı.
        fn step_drops(&mut self) -> bool {
            let t = self.rng.next_ppm();
            if self.bad {
                if t < self.ge.p_bg {
                    self.bad = false;
                }
            } else if t < self.ge.p_gb {
                self.bad = true;
            }
            let l = self.rng.next_ppm();
            let thr = if self.bad {
                self.ge.loss_bad
            } else {
                self.ge.loss_good
            };
            l < thr
        }
    }

    impl SecuritySeam for GeSeam {
        fn seal(
            &mut self,
            _peer: SeamPeer,
            mut frame: Vec<u8>,
            args: SealArgs,
        ) -> Result<SealedDatagram, SealError> {
            frame.resize(frame.len() + IDENTITY_TAG, 0);
            Ok(SealedDatagram {
                bytes: frame,
                full_wire_pn: args.wire_pn,
                dh_epoch: args.dh_epoch,
            })
        }

        fn open(&mut self, wire: &[u8]) -> Result<OpenedDatagram, OpenReject> {
            if wire.len() <= IDENTITY_TAG {
                return Err(OpenReject::Malformed);
            }
            self.seen += 1;
            if self.step_drops() {
                self.dropped += 1;
                return Err(OpenReject::AuthFail); // kanal silmesi (verify-öncesi düşme ile aynı yol)
            }
            Ok(OpenedDatagram {
                peer: 0,
                plain: wire[..wire.len() - IDENTITY_TAG].to_vec(),
            })
        }

        fn sealed_len(&self, frame_len: usize) -> usize {
            frame_len + IDENTITY_TAG
        }
    }
}

/// Tek kolun sonucu.
struct ArmResult {
    label: String,
    delivered: u64,
    offered: u64,
    tx_bytes: u64,
    fec_bytes: u64,
    retx_bytes: u64,
    dup_bytes: u64,
    blocks_latched: u64,
    repairs_tx: u64,
    elapsed_ms: u128,
    fate: client_core::runtime::FateWindowDiag,
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
async fn run_arm(
    label: &str,
    fec: Option<FecRuntimeConfig>,
    policy: FecStructurePolicy,
    msgs: u64,
    payload_bytes: usize,
    deadline: Duration,
    seed: u64,
    ge: ge_seam::Ge,
) -> ArmResult {
    // Fragmenter: FEC açıkken chunk kapasitesi profilden gelir; kapalıyken mesaj = birim.
    let unit = match fec {
        Some(f) => client_core::runtime::fec_chunk_capacity(f.profile_id)
            .unwrap_or(payload_bytes)
            .min(payload_bytes),
        None => payload_bytes,
    };
    let cfg = || RuntimeConfig {
        max_unit_bytes: unit.max(1),
        max_message_bytes: (payload_bytes > unit).then_some(payload_bytes),
        fec,
        fec_structure_policy: policy,
        drain_cap: Duration::from_millis(200),
        ..RuntimeConfig::default()
    };

    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().expect("addr"),
        cfg(),
        Box::new(ge_seam::GeSeam::new(seed, ge)),
    )
    .await
    .expect("server bind");
    let addr = server.local_addr();
    let client = ClientEndpoint::connect(addr, cfg(), Box::new(IdentitySeam))
        .await
        .expect("client connect");

    let t0 = std::time::Instant::now();
    let payload: Vec<u8> = (0..payload_bytes).map(|i| (i % 251) as u8).collect();
    let mut delivered = 0u64;
    let _ = tokio::time::timeout(deadline, async {
        for _ in 0..msgs {
            loop {
                match client.send(payload.clone()) {
                    Ok(()) => break,
                    Err(RuntimeError::QueueFull) => {
                        tokio::time::sleep(Duration::from_millis(2)).await;
                    }
                    Err(e) => panic!("offer: {e}"),
                }
            }
        }
        while delivered < msgs {
            if server.next_delivery().await.is_some() {
                delivered += 1;
            }
        }
    })
    .await;
    let elapsed_ms = t0.elapsed().as_millis();

    let cs = client.shutdown().await;
    let _ss = server.shutdown().await;
    ArmResult {
        label: label.to_string(),
        delivered,
        offered: cs.offered,
        tx_bytes: cs.tx_bytes,
        fec_bytes: cs.tx_fec_bytes,
        retx_bytes: cs.tx_retx_bytes,
        dup_bytes: cs.tx_dup_bytes,
        blocks_latched: cs.fec_blocks_latched,
        repairs_tx: cs.fec_repairs_tx,
        elapsed_ms,
        fate: cs.fate,
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let msgs = arg(&a, "--msgs", 200);
    let payload = usize::try_from(arg(&a, "--payload", 256)).unwrap_or(256);
    let seed = arg(&a, "--seed", 1);
    let ge = ge_seam::Ge {
        p_gb: arg(&a, "--p-gb", 30_000),           // GOOD→BAD: %3/paket
        p_bg: arg(&a, "--p-bg", 200_000),          // BAD→GOOD: %20/paket → ortalama burst ≈ 5
        loss_good: arg(&a, "--loss-good", 10_000), // %1
        loss_bad: arg(&a, "--loss-bad", 600_000),  // %60
    };
    let deadline = Duration::from_secs(arg(&a, "--deadline-s", 20));

    println!("# fec_arm_compare — TEŞHİS (sim/tek-hücre); ürün iddiası DEĞİL");
    println!(
        "# kanal GE: p_gb={} p_bg={} loss_good={} loss_bad={} (ppm) · seed={seed} · \
         msgs={msgs} payload={payload}B · profil {PROFILE:#04x} (K8/R4/E64)",
        ge.p_gb, ge.p_bg, ge.loss_good, ge.loss_bad
    );
    println!(
        "# üç kol AYNI seed'i, dolayısıyla AYNI silme desenini görür (eşleştirilmiş karşılaştırma)"
    );

    let adaptive = Some(FecRuntimeConfig {
        profile_id: PROFILE,
        manual_r: None,
    });
    let arms: Vec<(&str, Option<FecRuntimeConfig>, FecStructurePolicy)> = vec![
        (
            "fec-off (yalnız ARQ+tail)",
            None,
            FecStructurePolicy::Q18Iid,
        ),
        (
            "fec-static-r4",
            Some(FecRuntimeConfig {
                profile_id: PROFILE,
                manual_r: Some(4),
            }),
            FecStructurePolicy::Q18Iid,
        ),
        (
            "adaptive · sevk (Q18Iid)",
            adaptive,
            FecStructurePolicy::Q18Iid,
        ),
        (
            "adaptive · BurstFusion (opt-in)",
            adaptive,
            FecStructurePolicy::BurstFusion,
        ),
    ];

    let mut out = Vec::new();
    for (label, fec, policy) in arms {
        out.push(run_arm(label, fec, policy, msgs, payload, deadline, seed, ge).await);
    }

    println!(
        "\n{:<30} {:>9} {:>9} {:>10} {:>9} {:>9} {:>9} {:>7} {:>8} {:>8}",
        "kol", "teslim", "sunulan", "tx_B", "fec_B", "retx_B", "dup_B", "blok", "repair", "ms"
    );
    for r in &out {
        println!(
            "{:<30} {:>9} {:>9} {:>10} {:>9} {:>9} {:>9} {:>7} {:>8} {:>8}",
            r.label,
            r.delivered,
            r.offered,
            r.tx_bytes,
            r.fec_bytes,
            r.retx_bytes,
            r.dup_bytes,
            r.blocks_latched,
            r.repairs_tx,
            r.elapsed_ms,
        );
    }
    // Yapı-gate'in KARAR TABANI: geçiş sayaçları 0 ise `burst_delta` prior'dır, ölçüm değil.
    println!("\n# yapı-gate karar tabanı (fate penceresi, koşu sonu):");
    println!(
        "{:<32} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>11} {:>7} {:>11}",
        "kol", "n", "lost", "n00", "n01", "n10", "n11", "delta_ppm", "opened", "ledger_ppm"
    );
    // ledger kaybı ayrı sütunda: gate'in KAYIP önkoşulu bunu okur (fate lost/n'i değil).
    for r in &out {
        println!(
            "{:<32} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>11} {:>7} {:>11}",
            r.label,
            r.fate.n,
            r.fate.lost,
            r.fate.n00,
            r.fate.n01,
            r.fate.n10,
            r.fate.n11,
            r.fate.burst_delta_ppm,
            r.fate.structure_gate_opened,
            r.fate.ledger_loss_ppm,
        );
    }
    // Karar-yolu histogramı: "gate açmadı" gözleminin NEDENİ.
    println!("\n# yapı-gate karar yolu (neden açmadı/açtı) + ledger kaybının TEPESİ:");
    println!(
        "{:<32} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8} {:>12}",
        "kol",
        "kapsamdışı",
        "kayıp<band",
        "pencere<32",
        "kanıt<8",
        "burst→ARQ",
        "AÇILDI",
        "peak_ppm"
    );
    for r in &out {
        let g = r.fate.gate_outcomes;
        println!(
            "{:<32} {:>10} {:>10} {:>10} {:>10} {:>10} {:>8} {:>12}",
            r.label, g[0], g[1], g[2], g[3], g[4], g[5], g[6]
        );
    }
    println!("\n# ledger un-loss muhasebesi (kayıp ilan edildi mi, geri mi alındı?):");
    for r in &out {
        println!(
            "{:<32} ilan_pkt={:<6} ilan_time={:<6} un_loss={:<6} spurious_units={}",
            r.label,
            r.fate.loss_declared_pkt,
            r.fate.loss_declared_time,
            r.fate.ledger_unloss,
            r.fate.ledger_spurious_units
        );
    }
    println!(
        "\n# Okuma notu:\n\
         #  · `fec_B=0` + `repair=0` olan bir FEC kolu, kodlamanın o rejimde HİÇ devreye\n\
         #    girmediğini gösterir.\n\
         #  · `opened` MONOTON bir sayaçtır: 0 ise yapı-gate koşunun HİÇBİR anında açmamıştır.\n\
         #    Koşu-boyu geçerli tek çıkarım budur.\n\
         #  · `ledger_ppm` ANLIK ölçerdir ve kapanışta örneklenir; trafik durunca decay ettiği\n\
         #    için 0 görmek 'hiç eşiği geçmedi' DEMEK DEĞİLDİR. Bu sütunla önkoşul teşhisi\n\
         #    YAPILAMAZ (peak-hold gerekir).\n\
         #  · `n00/n01/n10/n11` toplamı 0 ise `delta_ppm` ölçüm değil Laplace PRIOR'ıdır.\n\
         #  · Teslim/ms sütunları loopback'tedir (RTT≈0 → ARQ neredeyse bedava); performans\n\
         #    hükmü DEĞİLDİR."
    );
}
