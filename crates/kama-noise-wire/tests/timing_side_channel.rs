//! ZAMANLAMA YAN-KANALI — `open()` reddetme sınıflarının ayırt edilebilirliği.
//!
//! **Kapsanan boşluk:** `wire_observability.rs` (TC-11..TC-14) yalnız BYTE ve DURUM düzeyi
//! gözlemlenebilirliği ölçüyor; dosya başında "zamanlama kanalı kapsam dışı" diye yazılı.
//! Bu dosya o boşluğu kapatır.
//!
//! **Tehdit modeli bağlamı:** Proje metadata sızıntısını birinci sınıf düşman sinyali sayar.
//! `open()` reddetme yolları FARKLI DERİNLİKLERDE düşer:
//!
//! ```text
//! uzunluk kapısı   → DatagramTooShort      (en sığ, ~hiç iş yok)
//! wire_format      → UnknownWireFormat
//! reserved flags   → ReservedWireFlags
//! h_ext uyuşmazlığı→ InnerParse
//! epoch kabul      → EpochNotAccepted
//! PN precheck      → PnReject              (pencere aritmetiği)
//! AEAD             → AeadFail              (en derin, tam ChaCha20-Poly1305)
//! ```
//!
//! Byte kanalı kapalı (`open()` yanıt üretmez, yalnız `Result` döner) ve durum kanalı da
//! kapalı (TC-14'te ölçüldü: hiçbir reddetme `recv_expected_pn`'i ilerletmiyor). Geriye
//! ZAMAN kalıyor.
//!
//! **YÖNTEM — kalibrasyon-önce:**
//!
//!   1. **Gürültü tabanı:** AYNI girdi iki ayrı gruba bölünüp ölçülür. İki grubun medyan
//!      farkı, ortamın ölçüm gürültüsüdür (OS zamanlayıcı, cache, termal).
//!   2. **Sinyal:** İki FARKLI reddetme sınıfının medyan farkı.
//!   3. **Karar:** `sinyal > K × gürültü` ise ayırt edilebilir; değilse **ÖLÇÜLEMEDİ**.
//!
//! Üçüncü adım kritiktir: sinyal gürültünün altındaysa doğru sonuç "sızıntı yok" DEĞİL,
//! "bu ortamda ölçülemez"dir. Bir timing testinin ölçemediği hâlde yeşil yanması, hiç test
//! olmamasından kötüdür.
//!
//! **A/B interleaving:** Sınıflar sırayla değil DÖNÜŞÜMLÜ ölçülür; aksi hâlde termal drift
//! ve turbo-boost geçişleri sistematik sapma üretir ve sahte sinyal görünür.
//!
//! **Bu test `#[ignore]`'dur.** Sonucu ortama bağlıdır (WSL2 ≠ bare metal ≠ CI runner) ve
//! bir CI kapısı olarak kullanılamaz. Kayıt ve teşhis içindir.
//!
//! Koş: `cargo test -p kama-noise-wire --release --test timing_side_channel -- --ignored --nocapture`
//! `--release` ZORUNLU: debug build'de optimizasyon yokluğu ölçümü anlamsızlaştırır.

use kama_noise_wire::outer::OUTER_HDR_LEN;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use std::time::Instant;
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;
/// Sınıf başına ölçüm sayısı. Yüksek tutulur: medyan tahmininin kendi varyansı
/// gürültü tabanına karışmasın.
const N: usize = 40_000;
/// Isınma: ilk ölçümler cache-soğuk ve branch-predictor eğitimsiz.
const WARMUP: usize = 4_000;
/// Sinyalin gürültünün kaç katı olması "ayırt edilebilir" sayılsın.
const K: f64 = 5.0;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn shard() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: vec![0xA5u8; 64],
    }
}

/// Geçerli bir sealed datagram üretir.
fn valid_datagram() -> Vec<u8> {
    let mut a = KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22));
    a.seal(
        &shard(),
        SealArgs {
            wire_pn: 5,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .expect("seal")
    .bytes
}

fn receiver() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11))
}

/// Bir datagram sınıfını `n` kez açıp nanosaniye sürelerini döndürür.
fn measure(datagram: &[u8], n: usize) -> Vec<u64> {
    let mut rx = receiver();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        let r = rx.open(datagram);
        let dt = t0.elapsed().as_nanos() as u64;
        // Sonucu tüket: optimizasyon çağrıyı elemesin.
        std::hint::black_box(&r);
        out.push(dt);
    }
    out
}

/// İki sınıfı DÖNÜŞÜMLÜ ölçer (termal drift iptali).
fn measure_interleaved(a: &[u8], b: &[u8], n: usize) -> (Vec<u64>, Vec<u64>) {
    let mut rx_a = receiver();
    let mut rx_b = receiver();
    let (mut va, mut vb) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for _ in 0..n {
        let t0 = Instant::now();
        let ra = rx_a.open(a);
        let da = t0.elapsed().as_nanos() as u64;
        std::hint::black_box(&ra);

        let t1 = Instant::now();
        let rb = rx_b.open(b);
        let db = t1.elapsed().as_nanos() as u64;
        std::hint::black_box(&rb);

        va.push(da);
        vb.push(db);
    }
    (va, vb)
}

fn median(v: &mut [u64]) -> f64 {
    v.sort_unstable();
    let n = v.len();
    if n.is_multiple_of(2) {
        (v[n / 2 - 1] as f64 + v[n / 2] as f64) / 2.0
    } else {
        v[n / 2] as f64
    }
}

/// Alt-yüzdelik: OS zamanlayıcı kesintileri üst kuyruğu şişirir; p10 daha kararlıdır.
fn p10(v: &mut [u64]) -> f64 {
    v.sort_unstable();
    v[v.len() / 10] as f64
}

/// Aynı girdinin iki yarısı arasındaki fark = ortamın gürültü tabanı.
fn noise_floor(datagram: &[u8]) -> (f64, f64) {
    let mut all = measure(datagram, WARMUP + 2 * N);
    let mut a: Vec<u64> = all.drain(WARMUP..WARMUP + N).collect();
    let mut b: Vec<u64> = all.split_off(WARMUP);
    b.truncate(N);
    (
        (median(&mut a) - median(&mut b)).abs(),
        (p10(&mut a) - p10(&mut b)).abs(),
    )
}

// ════════════════════════════════════════════════════════════════════════════

/// Reddetme sınıflarının zamanlama ile ayırt edilebilirliği.
///
/// **Bu test ASSERTION YAPMAZ, ÖLÇER ve RAPORLAR.** Sebebi: sonuç ortama bağlıdır ve bir
/// eşik pinlemek yanlış güven verir. Çıktı `[BEKLİYOR — Abkarada]` değerlendirmesi içindir.
#[test]
#[ignore = "zamanlama ölçümü — ortam-bağımlı, ~60 s; --release --ignored ile koş"]
fn rejection_classes_timing_separation() {
    let valid = valid_datagram();

    // Sınıf üreticileri.
    let short = valid[..25].to_vec(); // uzunluk kapısı
    let mut bad_fmt = valid.clone();
    bad_fmt[4] = 0x02; // wire_format
    let mut bad_flags = valid.clone();
    bad_flags[5] |= 0b0000_0010; // reserved flags
    let mut bad_hext = valid.clone();
    bad_hext[0] ^= 0xFF; // h_ext
    let mut bad_tag = valid.clone();
    let n = bad_tag.len();
    bad_tag[n - 1] ^= 0xFF; // AEAD
    let mut bad_ct = valid.clone();
    bad_ct[OUTER_HDR_LEN] ^= 0xFF; // AEAD (ciphertext)

    println!("\n════ ZAMANLAMA YAN-KANALI ÖLÇÜMÜ ════");
    println!("N={N} örnek/sınıf, warmup={WARMUP}, eşik K={K}×gürültü\n");

    // 1) KALİBRASYON — gürültü tabanı.
    let (noise_med, noise_p10) = noise_floor(&bad_tag);
    println!("GÜRÜLTÜ TABANI (aynı girdi, iki yarı):");
    println!("  medyan farkı = {noise_med:8.1} ns");
    println!("  p10    farkı = {noise_p10:8.1} ns\n");

    // 2) SİNYAL — sınıf çiftleri, dönüşümlü ölçüm.
    let pairs: [(&str, &[u8], &[u8]); 4] = [
        ("kısa vs AEAD-fail", &short, &bad_tag),
        ("format vs AEAD-fail", &bad_fmt, &bad_tag),
        ("h_ext vs AEAD-fail", &bad_hext, &bad_tag),
        ("tag-bozma vs ct-bozma", &bad_tag, &bad_ct),
    ];

    println!("SINIF ÇİFTLERİ:");
    println!(
        "  {:<24} {:>10} {:>10} {:>10}  karar",
        "çift", "Δmedyan", "Δp10", "oran"
    );
    println!("  {}", "─".repeat(70));

    let mut any_distinguishable = false;
    for (name, a, b) in pairs {
        let (mut va, mut vb) = measure_interleaved(a, b, WARMUP + N);
        va.drain(..WARMUP);
        vb.drain(..WARMUP);
        let d_med = (median(&mut va) - median(&mut vb)).abs();
        let d_p10 = (p10(&mut va) - p10(&mut vb)).abs();
        let ratio = if noise_p10 > 0.0 {
            d_p10 / noise_p10
        } else {
            f64::INFINITY
        };
        let verdict = if ratio > K {
            any_distinguishable = true;
            "AYIRT EDİLEBİLİR"
        } else if ratio > 1.0 {
            "gürültüye yakın"
        } else {
            "ÖLÇÜLEMEDİ"
        };
        println!("  {name:<24} {d_med:>10.1} {d_p10:>10.1} {ratio:>10.2}  {verdict}");
    }

    println!("\nYORUM:");
    if any_distinguishable {
        println!("  En az bir sınıf çifti gürültü tabanının {K}× üstünde ayrışıyor.");
        println!("  Bu, YEREL bir ölçümdür — ağ üzerinden gözlemlenebilirliği AYRI bir sorudur:");
        println!("  `open()` yanıt üretmediği için zamanlama ancak SONRAKİ paketin işlenme");
        println!("  anına dolaylı yansır ve pacing gecikmeleri (31 B/s cold-start floor'unda");
        println!("  paket başına ~3,2 s) bu farkı 10^6 kat gömer.");
    } else {
        println!("  Hiçbir çift gürültü tabanını {K}× aşmadı.");
        println!("  DİKKAT: bu 'sızıntı yok' DEMEK DEĞİLDİR — bu ortamda ÖLÇÜLEMEDİ demektir.");
        println!("  Bare-metal, izole çekirdek ve kesme-maskesi ile tekrar edilmelidir.");
    }
    println!("\n[BEKLİYOR — Abkarada] Bu ölçüm bir gereksinim mi (constant-time reddetme),");
    println!("yoksa kabul edilmiş bir risk mi? §2 Cryptex byte kanalını kapatıyor, zaman");
    println!("kanalı hakkında hüküm içermiyor.\n");
}

/// Ortamın ölçüm hassasiyetini tek başına raporlar.
///
/// Yukarıdaki testin ön koşulu: gürültü tabanı, ölçmek istediğimiz farktan küçük olmalı.
/// Bu test tabanı bağımsız olarak kaydeder ki ortam değişince (CI runner, bare metal)
/// karşılaştırma yapılabilsin.
#[test]
#[ignore = "ortam kalibrasyonu — ~20 s; --release --ignored ile koş"]
fn measurement_environment_calibration() {
    let valid = valid_datagram();
    let mut bad = valid.clone();
    let n = bad.len();
    bad[n - 1] ^= 0xFF;

    println!("\n════ ORTAM KALİBRASYONU ════");
    for (name, d) in [("geçerli-open", &valid), ("AEAD-fail", &bad)] {
        let mut v = measure(d, WARMUP + N);
        v.drain(..WARMUP);
        let med = median(&mut v);
        let lo = p10(&mut v);
        v.sort_unstable();
        let p90 = v[v.len() * 9 / 10] as f64;
        println!(
            "  {name:<14} p10={lo:8.1} ns  medyan={med:8.1} ns  p90={p90:8.1} ns  \
             yayılım={:.1}×",
            p90 / lo.max(1.0)
        );
    }
    let (nm, np) = noise_floor(&bad);
    println!("\n  gürültü tabanı: Δmedyan={nm:.1} ns  Δp10={np:.1} ns");
    println!("  (Δp10 küçükse ortam hassas; büyükse ölçüm anlamsız.)\n");
}
