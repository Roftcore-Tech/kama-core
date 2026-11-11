//! **K4G-05b-R — payload sahipliği: runtime yalnız motorun teyit çıktısıyla siler**
//! (sahip kararı 2026-11-11: #140'ın runtime kısmı ayrı PR; motor kısmı ayrıca karara bağlanır).
//!
//! Envanter: `docs/design/K4G-05b-RUNTIME-ENVANTER.md`. Motor (`Sender::take_confirmed_units`)
//! KARAR-NÖTR bir teyit çıktısı verir: yalnız CANLI kaydı (uçuş, tombstone, un-loss) eşleşen ACK ve
//! FEC RECOVERED teyit üretir. Motorun bir iletim kaydını unuttuğu her yolda (tombstone TTL'i,
//! TOMBSTONE_MAX tahliyesi, kopya kökenli kayıp) o kaydın GEÇ ACK'i teyit üretmez. Taban
//! (`4a9e97f1`) runtime'ı aynı ACK'te payload'ı seq → birim haritasıyla siliyordu: sonraki yeniden
//! gönderim gövdesiz (store-miss) çıkar. Bu dosya gerçek runtime ile (`ClientEndpoint` /
//! `ServerEndpoint`, UDP loopback) her yolu AYRI sınar.
//!
//! **CI.** Testler gerçek zamanlıdır (sırayla; yerelde ≈ 2–2,5 dk) ve `#[ignore]` ile hızlı kümeden
//! (`test-fast`) çıkarılmıştır (sahip kararı 2026-11-11). Her PR'da test-fast'a paralel `test-runtime`
//! işi (`.github/workflows/ci.yml`; test-fast'ın özellik kümesiyle), main'de `core-main`
//! `runtime-ownership` işi (ürün varsayılan derlemesi, all-features, release) `-- --ignored` ile koşar
//! ve koşan test sayısını birebir denetler (vakum bekçisi; sayı `K4G05B_RUNTIME_TESTS`).
//!
//! **Motor.** `RuntimeConfig::default()` motoru #176'dan beri `CcPort`'tur. Tombstone TTL'i ve
//! TOMBSTONE_MAX tahliyesi fikstürleri eski motorun (`Legacy`) zamanlamasına kalibredir: `CcPort`'ta
//! hedefin kaydı PTO probuyla ≈ 0,16 s'de yenilenir (TTL ön koşulu tutmaz) ve tombstone tablosu
//! tavana ulaşmaz (görülen 105) — bu iki test `Legacy`'ye sabitlidir; iki motorda aynı yollar motor
//! düzeyinde `sender.rs` `tests::k4g05br_teyit` ile sınanır. Kopya kökenli kayıp, ESI adaşı ve iki
//! bekçi her iki motorla koşar; FEC bekçisinin parçalamalı (DATA_FRAG) kolu `CcPort` iledir.
//!
//! AĞ: istemci ile sunucu arasında bir röle (gerçek UDP). `IdentitySeam` düz metin taşıdığından
//! röle çerçeveyi okur ama yalnız ağın yapabildiğini yapar: datagramı geçirir, düşürür ya da
//! geciktirir. Çerçeve değiştirilmez, üretilmez. "Geç ACK" = alıcının ürettiği gerçek ACK
//! datagramının ağda tutulup sonradan istemciye verilmesidir. Sonraki ACK'ler o PN'i yeniden
//! bildirmesin diye alıcının ACK aralığı sınırı kullanılır (`MAX_ACK_RANGES`): hedeften sonra
//! tek sayılı dolgu birimlerinin İLK datagramı düşer. Alıcının alım kümesi parçalanır ve hedefin
//! bloğu ACK çerçevesinin dışında kalır (`AckFrame::from_received`, gerçek alıcı davranışı).
//!
//! Her yol için beş iddia:
//! 1. Geç ACK birimi teyit etmez (`acked()` olayı yok).
//! 2. Payload yerinde: birim teyitsiz depoda.
//! 3. Açılıştan sonraki her iletim birimin gerçek gövdesini taşır (store-miss yok, boş gövde yok).
//! 4. O gönderimin ACK'i birimi teyit eder, payload düşer.
//! 5. Runtime haritaları boştur (sızıntı yok).
//!
//! Ve her testte (sahip kararı 2026-11-11, tel bekçisi):
//! 6. Tele gövdesiz datagram ÇIKMAZ: röle istemciden gelen her datagramı sayar; boş gövdeli DATA,
//!    yalnız önekli DATA_FRAG ve boş sembollü FEC çerçevesi sayısı 0'dır. Gövdesi olmayan karar
//!    runtime'da atlanır: `stale_skipped == stale_decisions + fec_repair_missing`.
//! 7. Motor teyitli birime karar ÜRETMEZ (sahip kararı 2026-11-11; motor teyit olayında birimin
//!    bekleyen işini iptal eder): `stale_decisions == 0`. Bunun ters çevrilmiş hâli (iptalden önce)
//!    FEC bekçisinin vakum ön koşuluydu (`stale_skipped ≥ 1`). Bu yüzden iddia 6'nın atlama dalı
//!    burada doğal trafikle koşmaz; dalın kendisi (store-miss ve eksik onarım sembolünde çerçeve
//!    kurulmaz) `runtime.rs` `skip_tests`'te sınanır.
//!
//! TABAN (`4a9e97f1`): iddia 1 KIRMIZI beklenir. İddia 5'in kancası
//! (`SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}`) tabanda yoktur.
//!
//! Yeniden gönderim bekleme tavanı motorun bildirdiği sonraki vadeden türetilir (`resend_wait`):
//! tabandaki geometrik kopya aralığı tavansız katlandığından sabit bir tavan yol yaşına bağlı olurdu.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2, MAX_ACK_RANGES};
use client_core::runtime::{
    ClientEndpoint, ClientHandle, ClientStats, FecRuntimeConfig, RuntimeConfig, RuntimeError,
    SenderRecoveryDiag, ServerEndpoint, ServerHandle, ServerStats, TransportEngine,
};
use client_core::seam::{IdentitySeam, IDENTITY_TAG};
use client_core::wire::{
    decode_data, decode_data_frag, decode_fec, FRAME_DATA, FRAME_DATA_FRAG, FRAME_FEC, HDR_DATA,
    HDR_DATA_FRAG, HDR_FEC,
};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

// ── Sabitler ─────────────────────────────────────────────────────────────────────────────────

/// Genel bekleme tavanı: loopback'te beklenen olayın ÜST sınırı; aşılırsa kırmızı.
const WAIT: Duration = Duration::from_secs(10);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Yoklama adımı.
const STEP: Duration = Duration::from_millis(1);
/// Kapanış boşaltmasının tavanı.
const DRAIN: Duration = Duration::from_millis(500);
/// Yeniden gönderim bekleme payı, hedefin motorda yalnız uçuş kaydı varken (kayıp ilanı bekleniyor):
/// zaman eşiği tabanı (3/2 × EAI ≥ 3.15 s) birkaç kez + pay. Kopya vadesi biliniyorsa vade + `WAIT`.
const RETX_LOSS_MARGIN: Duration = Duration::from_secs(15);
/// Tombstone TTL'inin ALT sınırı: TTL = K_TOMB × T_loss, T_loss ≥ K_c × EAI, EAI ≥ 65 B / 31 B·s⁻¹
/// (`sender.rs` v1 sabitleri K_TOMB = 8, CADENCE_KC = 3/2, EAI_SACK_WIRE_BYTES = 65,
/// EAI_RETURN_BW_PRIOR = 31; bu PR değiştirmez) ⇒ 8 · 3/2 · 65/31 s = 25 161 ms. Yalnız ön koşul
/// denetimidir: TTL testinde kayıt bundan ERKEN düştüyse yol TTL değildir; tahliye testinde
/// bundan GEÇ düştüyse yol tahliye değildir.
const TTL_LOWER_BOUND: Duration = Duration::from_millis(8 * 3 * 65_000 / (2 * 31));
/// TTL bekleme tavanı: alt sınırın birkaç katı (T_loss'un hız terimi loopback'te küçüktür).
const TTL_WAIT: Duration = Duration::from_secs(120);
/// Tahliye bekleme tavanı (tahliye TTL alt sınırından önce olmalıdır; pay ön koşul denetimindedir).
/// Tahliye kolunda geç ACK öncesi trafik boşaltmasının da bütçesidir: tahliye edilen kayıtların
/// geç ACK'i teyit etmez, birimler ek bir yeniden gönderim turuyla teyit edilir (beklenen bedel).
/// Değer #140'ın motoruyla ölçülmüştü; bu PR'ın motorunda süre her koşuda yazdırılır.
const CAP_WAIT: Duration = Duration::from_secs(60);
/// Tahliye kolunda tahliye birimlerinin teyit bekleme tavanı (geç ACK öncesi, bekleme trafiğiyle).
/// Ön koşul değil, kurulum bütçesidir: tahliye ön koşulu bu adımdan önce denetlenir; bu adım TTL'le
/// yarışmaz, iddia taşımaz. Süre CPU kısıtında eski motorun kurtarma hızına bağlıdır (ölçüm, yerel
/// loopback, bu fikstür: sabitlenmemiş beş koşuda 0,7–1,2 s; tek çekirdeğe sabitli iki koşuda
/// 42,1 / 40,6 s, ikincisinde boşaltma başında 1208 birim teyitsiz; aynı çekirdekte meşgul döngüyle
/// iki koşuda 38,9 / 26,1 s). Kısaltma denemeleri (bekleme kapısı yalnız kendi birimlerini sayar;
/// tahliyeden sonra düşüş kalkar; tahliye evresinde teyitsiz birim tavanı 256) CPU kısıtında süreyi
/// kısaltmadı ya da ön koşulu kırdı, bırakıldı. Değer `TTL_WAIT` ile aynıdır.
const CAP_DRAIN_WAIT: Duration = TTL_WAIT;
/// Motorun tombstone tavanı (`sender.rs` TOMBSTONE_MAX = 4096, v1; bu PR değiştirmez).
const TOMBSTONE_CAP: u32 = 4096;
/// Bir yoklama turunda sunulan tahliye dolgusu (teklif kuyruğu 1024'ün altında).
const CAP_BATCH: u64 = 64;
/// Alıcının bir ACK çerçevesinde bildirdiği blok sayısı: en üst blok + `MAX_ACK_RANGES` aralık.
const ACK_BLOCKS: u64 = MAX_ACK_RANGES as u64 + 1;
/// Parçalama dolgusu: hedefin bloğunu ACK çerçevesinin dışına itmek için alıcıda hedefin üstünde
/// `ACK_BLOCKS`'tan fazla blok gerekir; her tek sayılı dolgu bir boşluk açar (pay: 4 blok).
const FRAG_FILLERS: u64 = 2 * (ACK_BLOCKS + 4);
/// Tahliye dolgusunda birim başına düşen ilk datagram sayısı: orijinal ile ilk iki yeniden gönderim
/// düşer. Üçü de kopya OLMAYAN kayıttır (eski motorun ARQ tavanı 3: `arq_retx_cap`) → üç kayıp ilanı
/// → üç tombstone; üçüncü yeniden gönderim geçer ve birimi teyit eder.
///
/// Ön koşul hâlâ duvar saati karşılaştırmasıdır (tahliye süresi ↔ `TTL_LOWER_BOUND`): bu fikstür
/// TTL'e payı büyütür, yarışı kaldırmaz; yavaşlık yanlış yeşil değil, ön koşul kırmızısı üretir.
/// Ölçüm (main `6716eb8d`, yerel loopback): birim başına tek
/// düşüşle (önceki fikstür, 4232 dolgu birimi) tavan geçişten 10–22 s sonra geliyordu ve dört koşunun
/// birinde kayıt 25,30 s'de, tablo 3390'dayken TTL'le düştü (TTL alt sınırı 25,16 s). O fikstürde
/// uçuşta ≈ 1400 yeniden gönderim kaydı ≈ 3 s bekleyip zaman eşiğiyle topluca kayıp ilan ediliyor,
/// teklif alımı ilk ≈ 0,6 s'den sonra ≈ 100 birim/s'ye düşüyordu (alıcıda her yeniden gönderim ayrı
/// blok, ACK aralığı sınırının dışında kalan teyit edilmez — kod okuması). Bu fikstür tavanı 1720
/// tahliye birimiyle, uçuşta birkaç düzine kayıtla kurar; süre her koşuda yazdırılır.
const CAP_DROPS_PER_FILLER: u32 = 3;
/// Taşıyıcı aralığı: tahliye birimlerinden her `CAP_CARRIER_PERIOD`'uncusu düşmez (taşıyıcı). ACK
/// ilerlemesi dolguların yeniden gönderimlerine bağlı kalmaz: taşıyıcının ACK'i kendinden öncekileri
/// paket eşiğiyle kayıp ilan eder. (Taşıyıcısız, birim başına iki düşüşte ACK ilerlemesi durmuştu:
/// 25 s'de en çok 28 tombstone.)
const CAP_CARRIER_PERIOD: u64 = 8;
/// Tahliye dolgusu (taşıyıcılar hariç): `CAP_DROPS_PER_FILLER` tombstone/birim ile tavan + pay.
const CAP_FILLERS: u64 = TOMBSTONE_CAP as u64 / CAP_DROPS_PER_FILLER as u64 + 8 * ACK_BLOCKS;
/// Bekleme boyunca trafik aralığı. Alıcı ACK_FREQ_N = 2 ile ilerleme ≈ 2 × bu aralıkta gelir; bu,
/// tail-recovery PTO'sunun alt sınırının (`max_ack_delay` 25 ms) altındadır. Böylece hedefin
/// kaydı prob emekliliğiyle çoğalmaz (birim başına kayıt sınırına ulaşmaz) ve kopyaları kayıp
/// ilanıyla düşer — TTL testinde kaydı TTL'den başka bir yol düşürmez.
const TRICKLE: Duration = Duration::from_millis(5);
/// Bekleme trafiğinin teyitsiz birim tavanı: trafik yalnız ACK akışını sürdürmek içindir, teklif
/// kuyruğunu doldurmaz. Gönderici bu hızın gerisine düşerse (ölçüm: tahliye kolunda atlama
/// politikasıyla faz B'de ≈ 1100 teyitsiz birim, hedefin vadesi geçmiş kopyası ≈ 10 s çıkmadı) yeni
/// birim sunulmaz; aksi hâlde bekleme tavanı fikstürün kendi yüküyle aşılırdı.
const TRICKLE_OUTSTANDING_MAX: usize = 64;
/// FEC kolları: profil 0x03 (K = 8, R_max = 4, E = 64; `kama_fec_codec` katalogu).
const FEC_PROFILE: u8 = 0x03;
/// Statik parite sayısı = R_max: parite ESI'leri 0..R, kaynak birim 0..R ile ad çakışır.
const PARITY_R: u16 = 4;
/// FEC kolunda birim tavanı: 48 + SOURCE_PREFIX_LEN 11 ≤ E 64.
const FEC_UNIT_BYTES: usize = 48;
/// Parite testinde bloğun tamamlanma tavanı. PRE_CONVERGE kopya takvimi 31 B/s tabanından türer:
/// ilk kopya = 3/2 × (birim + 65 B) / 31 B·s⁻¹ (FEC kolunda birim ≤ ~128 B ⇒ ≤ ~9.4 s), sonraki her
/// aralık ×2 (tavansız). Faz 1 birkaç saniye sürdüğünden en kötü durumda üçüncü kopya
/// (≈ 9.4 + 18.7 + 37.4 s) beklenir; tavan bunu + pay kapsar. Gerçek süre yazdırılır.
const ESI_WAIT: Duration = Duration::from_secs(90);
/// Bekçi: teyitten sonra motorun kalan kayıtlarının akıbetinin belirlendiği pencere — zaman
/// eşiği kayıp ilanının tabanı (3/2 × EAI ≥ 3.15 s) + pay.
const SETTLE: Duration = Duration::from_secs(5);
/// Bekçi kanalı: birim sayısı, bekleme tavanı, tohum ve olasılıklar (milyonda).
const GUARD_UNITS: u64 = 400;
const GUARD_WAIT: Duration = Duration::from_secs(60);
const GUARD_SEED: u64 = 0x4B34_4730_3562;
const GUARD_DATA_DROP_PPM: u64 = 200_000;
const GUARD_DATA_DELAY_PPM: u64 = 100_000;
const GUARD_ACK_DROP_PPM: u64 = 100_000;
/// Bekçi gecikmesi: paket eşiği kayıp ilanından (loopback'te ms) çok uzun, tombstone TTL'inden
/// (≥ 25 s) çok kısa → geç orijinal, yeniden gönderimden SONRA ve kaydı hâlâ canlıyken gelir.
const GUARD_DELAY: Duration = Duration::from_millis(300);
/// Bekçi FEC kolunda kalıcı silinen birimler (DATA'ları retransmit dahil hep düşer).
const GUARD_ERASED: [u64; 2] = [2, 5];
/// Knuth MMIX LCG (belirlenimli sözde-rastgele; yalnız test kanalı).
const LCG_MUL: u64 = 6_364_136_223_846_793_005;
const LCG_INC: u64 = 1_442_695_040_888_963_407;
const PPM: u64 = 1_000_000;
/// `IdentitySeam` etiketinde PN'in kapladığı önek (u64 BE; seam.rs S4).
const PN_PREFIX: usize = 8;
/// DATA_FRAG gövdesinin başındaki kaynak öneki (`SourcePrefix`, 11 B): iki başlık boyunun farkı.
const FRAG_PREFIX: usize = HDR_DATA_FRAG - HDR_DATA;
/// Rölenin gecikme kuyruğu tick'i ve gecikmiş datagram enjeksiyon kuyruğu.
const RELAY_TICK: Duration = Duration::from_millis(1);
const INJECT_QUEUE: usize = 64;
/// UDP datagram tamponu.
const DATAGRAM_MAX: usize = 65_536;
/// Tel bekçisi: gövdesiz datagram örneklerinin tavanı (sayım tavansız).
const BODYLESS_LOG_CAP: usize = 8;
/// Kapanıştan sonra rölenin soket kuyruğunu işlemesi için bekleme (loopback; üst sınır).
const RELAY_SETTLE: Duration = Duration::from_millis(300);
/// FEC bekçisi: teyitten sonra `stale_skipped` ve motor kuyruklarının izlendiği pencere. İptal
/// öncesi bu pencerede atlanan karar görülüyordu (ilk atlama teyitten ≤ 0.42 s sonra; zaman eşiği
/// kayıp ilanı + kuyruk kurtarması); şimdi pencere sonunda `stale_decisions == 0` iddia edilir.
const SKIP_OBSERVE: Duration = Duration::from_secs(10);
/// İzleme örnek aralığı.
const SKIP_SAMPLE: Duration = Duration::from_secs(1);

/// Birime özgü, SABİT boylu gövde: alıcıdaki bayt eşliği içerikle doğrulanır; boy eşitliği
/// PRE_CONVERGE kuruluş probunun hedef seçimini belirlenimli kılar (eşit vade → önce sunulan).
fn body(unit: u64) -> Vec<u8> {
    format!("k4g05b-birim-{unit:06}").into_bytes()
}

/// FEC kapalı, canlı ayna açık (teşhis aynası salt gözlemdir; ürün varsayılanı kapalı). Motor
/// açıkça seçilir (bkz. dosya başı "Motor").
fn cfg_no_fec(engine: TransportEngine) -> RuntimeConfig {
    RuntimeConfig {
        engine,
        fec: None,
        drain_cap: DRAIN,
        live_diag: true,
        ..RuntimeConfig::default()
    }
}

/// Statik-r FEC (Manual): parite her blokta R adet, belirlenimli.
fn cfg_fec_static(engine: TransportEngine) -> RuntimeConfig {
    RuntimeConfig {
        engine,
        max_unit_bytes: FEC_UNIT_BYTES,
        fec: Some(FecRuntimeConfig {
            profile_id: FEC_PROFILE,
            manual_r: Some(PARITY_R),
        }),
        drain_cap: DRAIN,
        live_diag: true,
        ..RuntimeConfig::default()
    }
}

/// Statik-r FEC + parçalama açık (`max_message_bytes` kurulu → DATA_FRAG, gövde `önek ‖ payload`).
/// Gövdeler birim tavanından küçüktür: her mesaj tek parçadır, mesaj kimliği = birim kimliği
/// (iki sayaç da 0'dan başlar), alıcı teslimi birim kimliğiyle eşleşir.
fn cfg_fec_static_frag(engine: TransportEngine) -> RuntimeConfig {
    RuntimeConfig {
        max_message_bytes: Some(FEC_UNIT_BYTES),
        ..cfg_fec_static(engine)
    }
}

/// Profil 0x03'ün blok boyu K (katalogdan).
fn block_k() -> u64 {
    u64::from(
        kama_fec_codec::by_id(FEC_PROFILE)
            .expect("profil 0x03 katalogda")
            .k_shape,
    )
}

/// İddia 5 — runtime haritaları boş (sızıntı yok) ve teyit çıktısı taşmadı.
///
/// [TABAN] `SenderRecoveryDiag::{seq_to_unit_len, unit_seqs_len}` tabanda (`4a9e97f1`) yoktur.
fn assert_runtime_maps_empty(r: &SenderRecoveryDiag, path: &str) {
    assert_eq!(
        (r.seq_to_unit_len, r.unit_seqs_len),
        (0, 0),
        "[{path}] (5) teyitsiz birim kalmadığında runtime haritaları boş olmalı: {r:?}"
    );
    assert_eq!(
        r.confirmed_units_overflow, 0,
        "[{path}] teyit çıktısı her olaydan sonra boşaltılır, taşmaz: {r:?}"
    );
}

/// İddia 6 — tel bekçisi (sahip kararı 2026-11-11): gövdesi olmayan karar datagram üretmez ve
/// atlanan karar sayacı neden sayaçlarının toplamıdır.
fn assert_no_bodyless(w: &WireSummary, cs: &ClientStats, path: &str) {
    eprintln!(
        "[{path}] gözlem: istemci datagramı {} (gövdesiz {}); stale_skipped={} \
         (stale_decisions={} fec_repair_missing={})",
        w.frames, w.bodyless, cs.stale_skipped, cs.stale_decisions, cs.fec_repair_missing
    );
    assert!(
        w.frames > 0,
        "[{path}] ön koşul: röle istemci datagramı gördü"
    );
    assert_eq!(
        w.bodyless, 0,
        "[{path}] (6) tele gövdesiz datagram çıkmamalı (boş DATA / yalnız önekli DATA_FRAG / boş \
         sembollü FEC); ilk örnekler (PN, tür) {:?}",
        w.examples
    );
    assert_eq!(
        cs.stale_skipped,
        cs.stale_decisions + cs.fec_repair_missing,
        "[{path}] (6) atlanan karar = neden sayaçlarının toplamı: stale_skipped={} \
         stale_decisions={} fec_repair_missing={}",
        cs.stale_skipped,
        cs.stale_decisions,
        cs.fec_repair_missing
    );
    assert_eq!(
        cs.stale_decisions, 0,
        "[{path}] (7) motor teyitli birime karar üretmez (teyitte bekleyen iş iptal): {cs:?}"
    );
}

/// Faz B'nin bekleme tavanı: motorun hedef için bildirdiği sonraki iletim kaynağından türetilir.
/// Kopya vadesi varsa vade + `WAIT`; kayıp kuyruğundaysa `WAIT`; yalnız uçuş kaydı varsa kayıp
/// ilanı + `RETX_LOSS_MARGIN`. Hiçbiri yoksa motor birimi bırakmıştır → açık kırmızı.
fn resend_wait(rec: &SenderRecoveryDiag, unit: u64, path: &str) -> Duration {
    let u = rec
        .oldest_unacked
        .filter(|u| u.unit_id == unit)
        .unwrap_or_else(|| panic!("[{path}] hedef {unit} en eski teyitsiz birim değil: {rec:?}"));
    let wait = if let Some(due_in) = u.dup_due_in_ns {
        Duration::from_nanos(u64::try_from(due_in).unwrap_or(0)) + WAIT
    } else if u.engine.pending_lost_retx.is_some() {
        WAIT
    } else if u.engine.inflight_records > 0 {
        WAIT + RETX_LOSS_MARGIN
    } else {
        panic!(
            "[{path}] motor hedef {unit} için sonraki iletim kaynağı bildirmiyor (kopya vadesi, \
             kayıp kuyruğu ya da uçuş kaydı yok) — birim bırakılmış: {u:?}"
        )
    };
    eprintln!(
        "[{path}] faz B bekleme tavanı {wait:?} (motor: {:?})",
        u.engine
    );
    wait
}

/// Hedefin rölede geçen iletiminin motor kaydı düştü mü: hedef teyitsiz depodaki en eski birimdir
/// ve motorun o birim için tuttuğu en eski kayıt (uçuş, tombstone, kayıp kuyruğu), o iletimin
/// rölede görüldüğü andan SONRA gönderilmiştir — ya da birimin hiç kaydı kalmamıştır.
fn record_gone(r: &SenderRecoveryDiag, origin: Instant, unit: u64, passed_at: Instant) -> bool {
    let Some(u) = r.oldest_unacked.filter(|u| u.unit_id == unit) else {
        return false;
    };
    u.engine
        .earliest_record_send_ns
        .is_none_or(|ns| origin + Duration::from_nanos(ns) > passed_at)
}

// ── Ağ (röle) ────────────────────────────────────────────────────────────────────────────────

/// `IdentitySeam` teli: `çerçeve ‖ etiket`, etiketin ilk 8 baytı PN (BE). Döner: (çerçeve, PN).
fn split_wire(wire: &[u8]) -> Option<(&[u8], u64)> {
    let cut = wire.len().checked_sub(IDENTITY_TAG)?;
    let pn = u64::from_be_bytes(wire.get(cut..cut + PN_PREFIX)?.try_into().ok()?);
    Some((&wire[..cut], pn))
}

/// Tel bekçisi: istemcinin tele çıkardığı çerçeve gövdesiz mi — boş gövdeli DATA, yalnız önekli
/// DATA_FRAG (canonical önek, payload yok) ya da boş sembollü FEC. Gövdesiz değilse `None`.
/// Sınıflandırma yalnız bu fikstürlerde geçerlidir: burada her gövde boş değildir. Geçerli bir BOŞ
/// uygulama mesajı (`send` boş payload'ı kabul eder) de tam başlık boyunda çıkar ve burada
/// gövdesiz sayılırdı; o, gövdesi olmayan motor kararı DEĞİLDİR.
fn bodyless_kind(frame: &[u8]) -> Option<&'static str> {
    match *frame.first()? {
        FRAME_DATA if frame.len() <= HDR_DATA => Some("DATA boş gövde"),
        FRAME_DATA_FRAG if frame.len() <= HDR_DATA_FRAG => Some("DATA_FRAG yalnız önek"),
        FRAME_FEC if frame.len() <= HDR_FEC => Some("FEC boş sembol"),
        _ => None,
    }
}

/// Tel bekçisinin özeti: rölenin istemciden gördüğü datagramlar.
#[derive(Debug, Default, Clone)]
struct WireSummary {
    /// İstemciden gelen bütün datagramlar.
    frames: u64,
    /// Gövdesiz datagram sayısı ve ilk örnekleri (PN, tür).
    bodyless: u64,
    examples: Vec<(u64, &'static str)>,
}

/// Hedef DIŞI DATA datagramlarının kapısı.
#[derive(Debug, Clone, Default)]
enum DataGate {
    #[default]
    All,
    Nothing,
    Except(BTreeSet<u64>),
}

impl DataGate {
    fn admits(&self, unit: u64) -> bool {
        match self {
            Self::All => true,
            Self::Nothing => false,
            Self::Except(units) => !units.contains(&unit),
        }
    }
}

/// Bekçi kanalı: belirlenimli sözde-rastgele kayıp ve gecikme.
#[derive(Debug, Clone)]
struct Chaos {
    state: u64,
    data_drop_ppm: u64,
    data_delay_ppm: u64,
    delay: Duration,
    ack_drop_ppm: u64,
}

impl Chaos {
    fn roll(&mut self, ppm: u64) -> bool {
        self.state = self.state.wrapping_mul(LCG_MUL).wrapping_add(LCG_INC);
        (self.state >> 33) % PPM < ppm
    }
}

/// İleri yön kararı.
enum Forward {
    Pass,
    Drop,
    Delay(Instant),
}

/// Rölenin kuralları (test yazar) ve gözlemleri (röle yazar).
#[derive(Default)]
struct NetState {
    // ── İleri yön (istemci → sunucu) kuralları
    /// Hedef birim: geç ACK'i üretilen birim.
    target: Option<u64>,
    /// Hedefin YALNIZ bu sıradaki (0 tabanlı) DATA datagramı alıcıya geçer, diğerleri düşer.
    target_pass_nth: u32,
    /// Faz B: hedefin bütün datagramları geçer (ve gövdeleri kaydedilir).
    target_open: bool,
    /// İlk DATA datagram(lar)ı düşen birimler (orijinal kaybolur, yeniden gönderimler geçer).
    drop_first: BTreeSet<u64>,
    /// `drop_first` birimlerinde düşen ilk datagram sayısı (0 ya da 1 = yalnız orijinal).
    drop_first_n: u32,
    /// Hedef dışı DATA kapısı.
    data_gate: DataGate,
    /// Bekçi kanalı (hedef dışı DATA ve bütün ACK'ler).
    chaos: Option<Chaos>,
    // ── Dönüş yönü (sunucu → istemci) kuralları
    /// Hedefin geçen datagramını bildiren ACK datagramları gecikme hattına alınır.
    hold_target_acks: bool,
    /// Bütün dönüş datagramları tutulur (PRE_CONVERGE'i uzatmak için).
    hold_all: bool,
    // ── Gözlemler
    /// Tel bekçisi (her datagram, kurallardan ÖNCE sayılır).
    wire: WireSummary,
    max_fwd_pn: Option<u64>,
    data_seen: BTreeMap<u64, u32>,
    data_passed: BTreeMap<u64, u32>,
    /// Hedefin alıcıya geçen datagramının PN'i ve rölede görüldüğü an.
    hold_pn: Option<u64>,
    target_passed_at: Option<Instant>,
    /// Faz B'de alıcıya geçen hedef gövdeleri.
    target_after_open: Vec<Vec<u8>>,
    /// Hedefin PN'ini bildiren, ağda tutulan ACK datagramları (geliş sırasıyla).
    held: Vec<Vec<u8>>,
    /// `hold_all` süresince tutulan dönüş datagramları.
    pre_held: Vec<Vec<u8>>,
    parity_pns: BTreeSet<u64>,
    acks_forwarded: u64,
    acks_with_all_parity: u64,
    delayed: VecDeque<(Instant, Vec<u8>)>,
    /// Gecikmiş datagram enjeksiyonu: istemciye gönderilen / gönderilemeyen (salt gözlem).
    injected: u64,
    inject_failed: u64,
}

impl NetState {
    fn on_forward(&mut self, wire: &[u8], now: Instant) -> Forward {
        let Some((frame, pn)) = split_wire(wire) else {
            return Forward::Pass;
        };
        self.wire.frames += 1;
        if let Some(kind) = bodyless_kind(frame) {
            self.wire.bodyless += 1;
            if self.wire.examples.len() < BODYLESS_LOG_CAP {
                self.wire.examples.push((pn, kind));
            }
        }
        self.max_fwd_pn = Some(self.max_fwd_pn.map_or(pn, |m| m.max(pn)));
        if decode_fec(frame).is_some() {
            self.parity_pns.insert(pn);
            return Forward::Pass;
        }
        // DATA ya da DATA_FRAG (parçalamalı kol: gövdenin öneki atlanır, payload karşılaştırılır).
        let Some((unit, payload)) = decode_data(frame).map(|(_, u, p)| (u, p)).or_else(|| {
            decode_data_frag(frame).and_then(|(_, u, _, b)| b.get(FRAG_PREFIX..).map(|p| (u, p)))
        }) else {
            return Forward::Pass; // PARAM / CS-probe: kontrol düzlemi geçer.
        };
        let seen = self.data_seen.entry(unit).or_insert(0);
        let nth = *seen;
        *seen += 1;
        if self.target == Some(unit) {
            if self.target_open {
                self.target_after_open.push(payload.to_vec());
                return Forward::Pass;
            }
            if nth == self.target_pass_nth {
                self.hold_pn = Some(pn);
                self.target_passed_at = Some(now);
                return Forward::Pass;
            }
            return Forward::Drop;
        }
        if nth < self.drop_first_n.max(1) && self.drop_first.contains(&unit) {
            return Forward::Drop;
        }
        if !self.data_gate.admits(unit) {
            return Forward::Drop;
        }
        if let Some(c) = self.chaos.as_mut() {
            let (drop_ppm, delay_ppm, delay) = (c.data_drop_ppm, c.data_delay_ppm, c.delay);
            if c.roll(drop_ppm) {
                return Forward::Drop;
            }
            if c.roll(delay_ppm) {
                return Forward::Delay(now + delay);
            }
        }
        *self.data_passed.entry(unit).or_insert(0) += 1;
        Forward::Pass
    }

    /// `true` = istemciye geçir.
    fn on_back(&mut self, wire: &[u8]) -> bool {
        let Some((frame, _)) = split_wire(wire) else {
            return true;
        };
        let is_ack = frame
            .first()
            .is_some_and(|&t| t == FRAME_TYPE_ACK || t == FRAME_TYPE_ACK_V2);
        let acked: Vec<u64> = if is_ack {
            AckFrame::decode(frame)
                .map(|f| f.acked_full(self.max_fwd_pn.map_or(0, |m| m + 1)))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if self.hold_target_acks && self.hold_pn.is_some_and(|p| acked.contains(&p)) {
            self.held.push(wire.to_vec());
            return false;
        }
        if self.hold_all {
            self.pre_held.push(wire.to_vec());
            return false;
        }
        if let Some(c) = self.chaos.as_mut() {
            let ppm = c.ack_drop_ppm;
            if is_ack && c.roll(ppm) {
                return false;
            }
        }
        if is_ack {
            self.acks_forwarded += 1;
            if !self.parity_pns.is_empty() && self.parity_pns.iter().all(|p| acked.contains(p)) {
                self.acks_with_all_parity += 1;
            }
        }
        true
    }

    fn take_due(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut due = Vec::new();
        while self.delayed.front().is_some_and(|(at, _)| *at <= now) {
            if let Some((_, wire)) = self.delayed.pop_front() {
                due.push(wire);
            }
        }
        due
    }
}

fn lock(m: &Mutex<NetState>) -> MutexGuard<'_, NetState> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// İstemci ↔ röle ↔ sunucu. İstemci rölenin ön adresine bağlanır; röle arka soketiyle sunucuya.
struct Net {
    front: SocketAddr,
    state: Arc<Mutex<NetState>>,
    inject: mpsc::Sender<Vec<u8>>,
}

impl Net {
    async fn start(server: SocketAddr) -> Self {
        let front = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("röle ön soketi");
        let back = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("röle arka soketi");
        back.connect(server).await.expect("röle → sunucu");
        let addr = front.local_addr().expect("röle adresi");
        let state = Arc::new(Mutex::new(NetState::default()));
        let (inject, rx) = mpsc::channel(INJECT_QUEUE);
        tokio::spawn(relay(front, back, Arc::clone(&state), rx));
        Self {
            front: addr,
            state,
            inject,
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut NetState) -> R) -> R {
        f(&mut lock(&self.state))
    }

    /// Ağda tutulmuş bir datagramı istemciye şimdi verir (gecikmiş datagram).
    async fn deliver_late(&self, wire: Vec<u8>) {
        self.inject.send(wire).await.expect("röle açık");
    }
}

/// Röle döngüsü. Kilit hiçbir `await` boyunca tutulmaz. Enjeksiyon kanalı kapanınca biter.
async fn relay(
    front: UdpSocket,
    back: UdpSocket,
    state: Arc<Mutex<NetState>>,
    mut inject: mpsc::Receiver<Vec<u8>>,
) {
    let mut client: Option<SocketAddr> = None;
    let mut up = vec![0u8; DATAGRAM_MAX];
    let mut down = vec![0u8; DATAGRAM_MAX];
    let mut tick = tokio::time::interval(RELAY_TICK);
    loop {
        tokio::select! {
            r = front.recv_from(&mut up) => {
                let Ok((n, from)) = r else { continue };
                client = Some(from);
                let verdict = lock(&state).on_forward(&up[..n], Instant::now());
                match verdict {
                    Forward::Pass => {
                        let _ = back.send(&up[..n]).await;
                    }
                    Forward::Drop => {}
                    Forward::Delay(at) => {
                        let wire = up[..n].to_vec();
                        lock(&state).delayed.push_back((at, wire));
                    }
                }
            }
            r = back.recv(&mut down) => {
                let Ok(n) = r else { continue };
                let pass = lock(&state).on_back(&down[..n]);
                if let (true, Some(to)) = (pass, client) {
                    let _ = front.send_to(&down[..n], to).await;
                }
            }
            _ = tick.tick() => {
                let due = lock(&state).take_due(Instant::now());
                for wire in due {
                    let _ = back.send(&wire).await;
                }
            }
            m = inject.recv() => {
                let Some(wire) = m else { break };
                let sent = match client {
                    Some(to) => front.send_to(&wire, to).await.is_ok(),
                    None => false,
                };
                if sent {
                    lock(&state).injected += 1;
                } else {
                    lock(&state).inject_failed += 1;
                }
            }
        }
    }
}

// ── Düzenek ──────────────────────────────────────────────────────────────────────────────────

/// Gerçek zamanlı (loopback, zamanlama duyarlı) testler SIRAYLA koşar: test koşucusu testleri
/// paralel iş parçacıklarında başlatır; CPU yarışı TTL/tahliye ön koşullarının zaman payını yer.
fn serial() -> Arc<tokio::sync::Mutex<()>> {
    static SERIAL: OnceLock<Arc<tokio::sync::Mutex<()>>> = OnceLock::new();
    Arc::clone(SERIAL.get_or_init(|| Arc::new(tokio::sync::Mutex::new(()))))
}

struct Rig {
    /// Serileştirme kilidi (`serial`); düzenekle birlikte bırakılır.
    _serial: tokio::sync::OwnedMutexGuard<()>,
    client: ClientHandle,
    server: ServerHandle,
    net: Net,
    next_unit: u64,
    /// İstemcinin `acked()` olayları (sırasıyla) ve kümesi.
    acked: Vec<u64>,
    acked_set: BTreeSet<u64>,
    /// Sunucunun ilk teslimleri (birim → gövde).
    delivered: BTreeMap<u64, Vec<u8>>,
}

impl Rig {
    async fn new(cfg: RuntimeConfig) -> Self {
        let serial = serial().lock_owned().await;
        let server = ServerEndpoint::bind(
            "127.0.0.1:0".parse().expect("adres"),
            cfg.clone(),
            Box::new(IdentitySeam::new()),
        )
        .await
        .expect("sunucu");
        let net = Net::start(server.local_addr()).await;
        let client = ClientEndpoint::connect(net.front, cfg, Box::new(IdentitySeam::new()))
            .await
            .expect("istemci");
        Self {
            _serial: serial,
            client,
            server,
            net,
            next_unit: 0,
            acked: Vec::new(),
            acked_set: BTreeSet::new(),
            delivered: BTreeMap::new(),
        }
    }

    /// Olay akışlarını `d` boyunca boşaltır (teyit kanalı taşmasın, teslim kanalı dolmasın).
    async fn pump(&mut self, d: Duration) {
        let until = tokio::time::Instant::now() + d;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(until) => return,
                Some(u) = self.client.acked() => {
                    self.acked.push(u);
                    self.acked_set.insert(u);
                }
                Some(dl) = self.server.next_delivery() => {
                    self.delivered.entry(dl.unit_id).or_insert(dl.payload);
                }
            }
        }
    }

    /// Bir birim sunar (dolu kuyrukta boşaltıp yeniden dener); birim kimliğini döner. Fragmentasyon
    /// kapalı: birim kimliği sunum sırasıdır.
    async fn send(&mut self) -> u64 {
        let unit = self.next_unit;
        let deadline = Instant::now() + WAIT;
        loop {
            match self.client.send(body(unit)) {
                Ok(()) => break,
                Err(RuntimeError::QueueFull) if Instant::now() < deadline => {
                    self.pump(STEP).await;
                }
                Err(e) => panic!("birim {unit} sunulamadı: {e}"),
            }
        }
        self.next_unit += 1;
        unit
    }

    async fn wait(&mut self, what: &str, limit: Duration, mut done: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + limit;
        loop {
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {limit:?} içinde olmadı — {}",
                self.describe()
            );
            self.pump(STEP).await;
        }
    }

    /// `wait` + bekleme boyunca `TRICKLE` aralığıyla trafik (geçen birimler).
    async fn wait_trickle(
        &mut self,
        what: &str,
        limit: Duration,
        mut done: impl FnMut(&Self) -> bool,
    ) {
        let deadline = Instant::now() + limit;
        let mut next = Instant::now();
        loop {
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {limit:?} içinde olmadı — {}",
                self.describe()
            );
            if Instant::now() >= next && self.client.outstanding() < TRICKLE_OUTSTANDING_MAX {
                self.send().await;
                next = Instant::now() + TRICKLE;
            }
            self.pump(STEP).await;
        }
    }

    /// Şu andan SONRA alınmış kurtarma görüntüsü (görev `RECOVERY_DIAG_PERIOD`'da bir yeniler).
    async fn fresh_recovery(&mut self) -> SenderRecoveryDiag {
        let since = Instant::now();
        let mut got = None;
        self.wait("taze kurtarma görüntüsü", WAIT, |r| {
            got = r.client.live_diag().recovery.filter(|x| x.taken_at > since);
            got.is_some()
        })
        .await;
        got.expect("koşul sağlandı")
    }

    /// Kırmızı mesajı için durum özeti (gövde baytı içermez).
    fn describe(&self) -> String {
        let live = self.client.live_diag();
        let net = lock(&self.net.state);
        format!(
            "outstanding={} son_teyitler={:?} rtt_samples={} retransmits={} dups={} \
             recovery={:?} hedef={:?} hedef_görülen={:?} hedef_pn={:?} tutulan_ack={} \
             ön_tutulan={} parite_pn={} ack_geçen={} enjekte={} enjekte_hata={} \
             ack_çerçeve={} son_ack_çerçeveleri(index, en büyük PN, mapped)={:?}",
            self.client.outstanding(),
            self.acked.iter().rev().take(8).collect::<Vec<_>>(),
            live.rtt_samples,
            live.retransmits,
            live.dups,
            live.recovery,
            net.target,
            net.target.and_then(|t| net.data_seen.get(&t).copied()),
            net.hold_pn,
            net.held.len(),
            net.pre_held.len(),
            net.parity_pns.len(),
            net.acks_forwarded,
            net.injected,
            net.inject_failed,
            live.ack.frames,
            live.acks
                .recent()
                .map(|a| (a.index, a.largest_pn, a.mapped_seqs))
                .collect::<Vec<_>>(),
        )
    }

    /// Kapanış: istemci ve sunucu kapanır, röle kuyruğunu işler; tel bekçisinin özeti döner.
    async fn finish(self) -> (ClientStats, ServerStats, WireSummary) {
        let Self {
            client,
            server,
            net,
            ..
        } = self;
        let cs = client.shutdown().await;
        let ss = server.shutdown().await;
        tokio::time::sleep(RELAY_SETTLE).await;
        let wire = net.with(|s| s.wire.clone());
        (cs, ss, wire)
    }

    /// TTL ve tahliye kollarının ortak kurulumu: kuruluş birimi W teyitli; hedef T'nin YALNIZ ilk
    /// datagramı alıcıya geçer ve onu bildiren ACK'ler ağda tutulur; parçalama dolgusunun tek sayılı
    /// birimlerinin ilk datagramı düşer → alıcıda T'nin bloğu ACK çerçevesinin dışına itilir, ACK
    /// akışı (T hariç) sürer ve T'nin orijinali kayıp ilan edilir. Döner: T.
    async fn target_with_held_ack(&mut self) -> u64 {
        let w = self.send().await;
        self.wait("kuruluş birimi teyit edilmeli", WAIT, |r| {
            r.acked_set.contains(&w)
        })
        .await;
        let t = self.next_unit;
        let fillers = (t + 1)..=(t + FRAG_FILLERS);
        self.net.with(|s| {
            s.target = Some(t);
            s.target_pass_nth = 0;
            s.hold_target_acks = true;
            s.drop_first.extend(fillers.clone().filter(|u| u % 2 == 1));
        });
        assert_eq!(self.send().await, t, "fikstür: hedef kimliği");
        for _ in fillers {
            self.send().await;
        }
        self.wait("dolgu teyit edilmeli (yalnız hedef teyitsiz)", WAIT, |r| {
            r.client.outstanding() == 1
        })
        .await;
        let want = body(t);
        self.wait("alıcı hedefi orijinalinden teslim etmeli", WAIT, |r| {
            r.delivered.get(&t).is_some_and(|b| *b == want)
        })
        .await;
        self.net.with(|s| {
            assert!(s.hold_pn.is_some(), "ön koşul: hedefin ilk datagramı geçti");
            assert!(
                !s.held.is_empty(),
                "ön koşul: hedefi bildiren ACK ağda tutuldu"
            );
        });
        t
    }

    /// Faz A (geç ACK) + faz B (yeniden gönderim): beş iddia. Ağda tutulan İLK ACK (hedefin PN'ini
    /// ilk bildiren datagram) istemciye şimdi verilir. `drain_budget`: geç ACK öncesi trafik
    /// boşaltmasının yol başına bütçesi (yalnız bütçe; iddialar yoldan bağımsız).
    async fn late_ack_keeps_payload_then_resend_confirms(
        mut self,
        t: u64,
        path: &str,
        drain_budget: Duration,
    ) {
        let drain_started = Instant::now();
        self.wait(
            "trafik teyit edilmeli (yalnız hedef teyitsiz)",
            drain_budget,
            |r| r.client.outstanding() == 1,
        )
        .await;
        eprintln!(
            "[{path}] geç ACK öncesi trafik boşaltması: {:?} (bütçe {drain_budget:?})",
            drain_started.elapsed()
        );
        let late = self
            .net
            .with(|s| {
                s.hold_target_acks = false;
                s.held.first().cloned()
            })
            .expect("ağda tutulan geç ACK");
        // Geç ACK'in teyit ettiği en büyük PN (rölenin gördüğü tel; istemcinin çözümüyle aynı).
        let late_largest = self.net.with(|s| {
            let expected = s.max_fwd_pn.map_or(0, |m| m + 1);
            split_wire(&late)
                .and_then(|(frame, _)| AckFrame::decode(frame).ok())
                .and_then(|f| f.acked_full(expected).into_iter().max())
        });
        let frames_before = self.client.live_diag().ack.frames;
        let mark = self.acked.len();
        self.net.deliver_late(late).await;
        // Boşluk bekçisi: geç ACK çerçevesi istemcide İŞLENDİ ve PN'leri motor seq'ine ÇEVRİLDİ
        // (aksi hâlde iddia 1 boş geçerdi — çerçeve seam'de reddedilmiş ya da ufuk dışı olurdu).
        // Kayıt halkası sınırlı olduğundan çerçeve işlenir işlenmez yoklanır.
        let mut late_rec = None;
        self.wait(
            &format!(
                "[{path}] ön koşul: geç ACK çerçevesi istemcide işlenmeli (en büyük PN \
                 {late_largest:?}; teslimden önceki çerçeve sayısı {frames_before})"
            ),
            WAIT,
            |r| {
                late_rec = r
                    .client
                    .live_diag()
                    .acks
                    .recent()
                    .find(|a| a.index > frames_before && a.largest_pn == late_largest)
                    .copied();
                late_rec.is_some()
            },
        )
        .await;
        assert!(
            late_rec.is_some_and(|a| a.mapped_seqs > 0),
            "[{path}] ön koşul: geç ACK çerçevesi (en büyük PN {late_largest:?}) motor seq'ine \
             çevrilmeli: {late_rec:?}"
        );
        eprintln!("[{path}] geç ACK çerçevesi: {late_rec:?}");
        // Bariyer: sonra sunulan birimin teyidi geldiğinde geç ACK'in teyit drenajı da bitmiştir.
        let barrier = self.send().await;
        self.wait("bariyer birimi teyit edilmeli", WAIT, |r| {
            r.acked_set.contains(&barrier)
        })
        .await;
        self.pump(QUIET).await;

        // (1) Geç ACK teyit üretmez.
        assert!(
            !self.acked[mark..].contains(&t),
            "[{path}] (1) geç ACK hedefi TEYİT ETMEMELİ — motor o kaydı unutmuştu; geç ACK \
             sonrası olaylar: {:?}",
            &self.acked[mark..]
        );
        assert_eq!(
            self.client.outstanding(),
            1,
            "[{path}] (1) hedef teyitsiz kalmalı"
        );
        // (2) Payload yerinde.
        let rec = self.fresh_recovery().await;
        assert_eq!(
            (rec.unacked_units, rec.oldest_unacked.map(|u| u.unit_id)),
            (1, Some(t)),
            "[{path}] (2) payload teyitsiz depoda kalmalı: {rec:?}"
        );

        // Faz B: hedefin sonraki iletimleri alıcıya geçer.
        let retx_wait = resend_wait(&rec, t, path);
        self.net.with(|s| s.target_open = true);
        let opened = Instant::now();
        self.wait_trickle(
            &format!("[{path}] (4) yeniden gönderimin ACK'i hedefi teyit etmeli"),
            retx_wait,
            |r| r.acked_set.contains(&t),
        )
        .await;
        eprintln!("[{path}] faz B teyit süresi {:?}", opened.elapsed());
        // (3) Açılıştan sonraki her iletim gerçek gövdeli: store-miss yok; teyitten sonra motor birime
        // karar vermez (bekleyen işi teyitte iptal edilir, iddia 7), verse de tele çıkmaz (iddia 6).
        let (lens, all_ok) = self.net.with(|s| {
            let want = body(t);
            (
                s.target_after_open.iter().map(Vec::len).collect::<Vec<_>>(),
                !s.target_after_open.is_empty() && s.target_after_open.iter().all(|b| *b == want),
            )
        });
        assert!(
            all_ok,
            "[{path}] (3) açılıştan sonraki her iletim birimin gerçek gövdesini taşımalı \
             (store-miss yok, boş gövde yok): gövde boyları {lens:?}, beklenen {}",
            body(t).len()
        );
        // (4) Teyit sonrası payload düştü; (5) haritalar boş.
        self.wait("trafik teyit edilmeli", WAIT, |r| {
            r.client.outstanding() == 0
        })
        .await;
        let rec = self.fresh_recovery().await;
        assert_eq!(
            (rec.unacked_units, rec.oldest_unacked.map(|u| u.unit_id)),
            (0, None),
            "[{path}] (4) teyitten sonra payload düşmeli: {rec:?}"
        );
        assert_runtime_maps_empty(&rec, path);

        let total = self.next_unit;
        let (cs, _ss, wire) = self.finish().await;
        assert_no_bodyless(&wire, &cs, path);
        assert_eq!(cs.undelivered, 0, "[{path}] {cs:?}");
        assert_eq!(cs.acked, total, "[{path}] her birim bir kez teyit: {cs:?}");
    }
}

// ── Testler: yol başına bir ──────────────────────────────────────────────────────────────────

/// Envanter satır 1 — **tombstone TTL'i**. Hedefin orijinali alıcıya ulaşır, ACK'i ağda tutulur;
/// motor orijinali kayıp ilan eder (tombstone) ve TTL (K_TOMB × T_loss ≥ 25.2 s) dolunca kaydı
/// unutur. Sonra tutulan ACK gelir. Taban: runtime payload'ı siler + `acked()` → (1) KIRMIZI.
///
/// Eski motora sabit (dosya başı "Motor"; `CcPort`'ta TTL ön koşulu tutmaz).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn late_ack_after_ttl_keeps_payload() {
    let mut rig = Rig::new(cfg_no_fec(TransportEngine::Legacy)).await;
    let t = rig.target_with_held_ack().await;
    let passed_at = rig
        .net
        .with(|s| s.target_passed_at)
        .expect("hedefin orijinali geçti");
    let mut gone_at = None;
    rig.wait_trickle(
        "hedefin orijinal kaydı tombstone TTL'iyle düşmeli",
        TTL_WAIT,
        |r| {
            let live = r.client.live_diag();
            match (live.clock_origin, live.recovery) {
                (Some(origin), Some(rec)) if record_gone(&rec, origin, t, passed_at) => {
                    gone_at = Some(rec.taken_at);
                    true
                }
                _ => false,
            }
        },
    )
    .await;
    let gone = gone_at.expect("koşul sağlandı").duration_since(passed_at);
    assert!(
        gone >= TTL_LOWER_BOUND,
        "ön koşul: kayıt TTL alt sınırından ({TTL_LOWER_BOUND:?}) önce düştü ({gone:?}) — yol \
         TTL değil (birim başına kayıt budaması ya da tahliye?)"
    );
    rig.late_ack_keeps_payload_then_resend_confirms(t, "ttl", WAIT)
        .await;
}

/// Envanter satır 2 — **TOMBSTONE_MAX tahliyesi**. Kurulum TTL koluyla aynı; ardından her birinin
/// ilk üç datagramı düşen dolgu birimleri (aralarında taşıyıcılar; bkz. `CAP_DROPS_PER_FILLER`)
/// bir TTL içinde 4096+ tombstone üretir ve en eski kayıt (hedefin orijinali) tavandan tahliye
/// edilir. Sonra tutulan ACK gelir.
///
/// Eski motora sabit (dosya başı "Motor"; `CcPort`'ta tablo tavana ulaşmaz).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn late_ack_after_cap_keeps_payload() {
    let mut rig = Rig::new(cfg_no_fec(TransportEngine::Legacy)).await;
    let t = rig.target_with_held_ack().await;
    let passed_at = rig
        .net
        .with(|s| s.target_passed_at)
        .expect("hedefin orijinali geçti");
    let first = rig.next_unit;
    // Her `CAP_CARRIER_PERIOD`'uncu birim taşıyıcıdır (düşmez); diğerleri dolgu.
    let periods = CAP_FILLERS.div_ceil(CAP_CARRIER_PERIOD - 1);
    let end = first + periods * CAP_CARRIER_PERIOD;
    let is_carrier = |u: u64| (u - first) % CAP_CARRIER_PERIOD == CAP_CARRIER_PERIOD - 1;
    rig.net.with(|s| {
        s.drop_first
            .extend((first..end).filter(|&u| !is_carrier(u)));
        s.drop_first_n = CAP_DROPS_PER_FILLER;
    });
    let deadline = Instant::now() + CAP_WAIT;
    let mut max_tombs = 0u32;
    // Tablonun tavana İLK ulaştığı görüntü (hedef kaydı o an canlı): tavan en eski kaydı tahliye
    // eder, hedefin orijinali en eski kayıtlardandır.
    let mut cap_reached_at: Option<Instant> = None;
    let (gone_at, tombs_at_gone) = loop {
        let live = rig.client.live_diag();
        if let (Some(origin), Some(rec)) = (live.clock_origin, live.recovery) {
            max_tombs = max_tombs.max(rec.timers.tombstones_len);
            if record_gone(&rec, origin, t, passed_at) {
                break (rec.taken_at, rec.timers.tombstones_len);
            }
            if cap_reached_at.is_none() && rec.timers.tombstones_len >= TOMBSTONE_CAP {
                cap_reached_at = Some(rec.taken_at);
            }
        }
        assert!(
            Instant::now() < deadline,
            "hedefin orijinal kaydı tahliye edilmeli ({CAP_WAIT:?}; en büyük tombstone \
             {max_tombs}) — {}",
            rig.describe()
        );
        let mut batch = 0;
        while rig.next_unit < end && batch < CAP_BATCH {
            rig.send().await;
            batch += 1;
        }
        rig.pump(STEP).await;
    };
    let gone = gone_at.duration_since(passed_at);
    let cap_at = cap_reached_at.map(|c| c.duration_since(passed_at));
    eprintln!(
        "[cap] hedef kaydı geçişten {gone:?} sonra düştü (o an tombstone {tombs_at_gone}); tablo \
         hedef kaydı canlıyken tavana {cap_at:?} sonra ulaştı; en büyük {max_tombs}; tahliye \
         birimi {} (dolgu {}, birim başına {CAP_DROPS_PER_FILLER} düşüş; taşıyıcı {periods})",
        end - first,
        end - first - periods
    );
    // Tahliye: kayıt TTL alt sınırından önce düştü YA DA tablo, hedef kaydı canlıyken o sınırdan
    // önce tavana ulaştı (gözlem çözünürlüğü RECOVERY_DIAG_PERIOD; TTL, geçişten değil kayıp
    // ilanından başlar — yalnız alt sınır olarak kullanılır).
    assert!(
        gone < TTL_LOWER_BOUND || cap_at.is_some_and(|c| c < TTL_LOWER_BOUND),
        "ön koşul: kayıt tahliyeyle düşmeli — TTL alt sınırından ({TTL_LOWER_BOUND:?}) önce düşmeli \
         ya da tablo hedef kaydı canlıyken o sınırdan önce tavana ulaşmalı; düştü {gone:?}, tavan \
         {cap_at:?}"
    );
    assert!(
        max_tombs >= TOMBSTONE_CAP,
        "ön koşul: tombstone tablosu tavana ({TOMBSTONE_CAP}) ulaşmalı; görülen {max_tombs}"
    );
    while rig.next_unit < end {
        rig.send().await;
    }
    // Kuyruk: son dolguların düşen yeniden gönderimlerinden sonra tele geçen datagram kalmazsa ACK
    // ilerlemesi durur — paket/zaman eşiği kayıp ilan etmez, kuyruk probları da düşer ve tavana
    // ulaşır (ölçüm: 20 koşunun 10'uncusunda 21 dolgu 60 s teyitsiz kaldı, prob 3/3). Bekleme
    // trafiği (`TRICKLE`; birimleri tahliye aralığının dışında, düşmez) ACK ilerlemesini sürdürür.
    // Bütçe `CAP_DRAIN_WAIT`'tir (CPU kısıtında süre eski motorun kurtarma hızına bağlıdır).
    let drained_from = Instant::now();
    eprintln!(
        "[cap] boşaltma başında istemcide teyitsiz {}",
        rig.client.outstanding()
    );
    rig.wait_trickle("tahliye birimleri teyit edilmeli", CAP_DRAIN_WAIT, |r| {
        r.acked_set.range(first..end).count() as u64 == end - first
    })
    .await;
    eprintln!(
        "[cap] tahliye birimlerinin teyidi (bekleme trafiğiyle): {:?} (bütçe {CAP_DRAIN_WAIT:?})",
        drained_from.elapsed()
    );
    rig.late_ack_keeps_payload_then_resend_confirms(t, "cap", CAP_WAIT)
        .await;
}

/// Envanter satır 3 — **kopya kökenli kayıp** (kayıt tombstone bırakmadan düşer). Hedef ilk
/// birimdir; RTT örneği yokken (PRE_CONVERGE) motorun tek yeniden gönderim yolu kopya
/// makinesidir, dolayısıyla hedefin İKİNCİ iletimi bir kopyadır (kuruluş probu). Orijinal düşer,
/// kopya alıcıya ulaşır; kopyayı bildiren ACK ağda tutulur. Kuruluştan sonra kopya kayıp ilan
/// edilir (kaydı silinir). Sonra tutulan ACK gelir.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn late_ack_after_dup_loss_keeps_payload() {
    run_dup_loss(TransportEngine::Legacy, "dup_loss").await;
}

/// [`late_ack_after_dup_loss_keeps_payload`], port motoruyla (ürün varsayılanı, #176).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn late_ack_after_dup_loss_keeps_payload_cc_port() {
    run_dup_loss(TransportEngine::CcPort, "dup_loss/cc_port").await;
}

async fn run_dup_loss(engine: TransportEngine, path: &str) {
    let mut rig = Rig::new(cfg_no_fec(engine)).await;
    let t = rig.next_unit;
    rig.net.with(|s| {
        s.target = Some(t);
        s.target_pass_nth = 1;
        s.hold_target_acks = true;
        s.hold_all = true;
    });
    assert_eq!(rig.send().await, t, "fikstür: hedef kimliği");
    let w = rig.send().await;
    rig.wait(
        "hedefin ikinci iletimi (kopya) alıcıya geçmeli",
        WAIT,
        |r| r.net.with(|s| s.hold_pn.is_some()),
    )
    .await;
    rig.wait("motorun kopya kararı aynada görünmeli", WAIT, |r| {
        r.client.live_diag().dups >= 1
    })
    .await;
    let live = rig.client.live_diag();
    assert_eq!(
        (live.rtt_samples, live.retransmits),
        (0, 0),
        "ön koşul: hedefin ikinci iletimi RTT örneği ve ARQ yokken çıktı → kopya (Duplicate) — \
         dups={}",
        live.dups
    );
    // Dönüş yolu açılır: kopyayı BİLDİRMEYEN tutulmuş ACK'ler (W'nin) istemciye verilir → kuruluş.
    let pre = rig.net.with(|s| {
        s.hold_all = false;
        std::mem::take(&mut s.pre_held)
    });
    assert!(!pre.is_empty(), "ön koşul: W'nin ACK'i tutulmuş olmalı");
    for wire in pre {
        rig.net.deliver_late(wire).await;
    }
    rig.wait("W teyit edilmeli", WAIT, |r| r.acked_set.contains(&w))
        .await;
    // Parçalama: kopyanın bloğu ACK çerçevesinin dışına itilir, ilerleme sürer → kopya kayıp ilan
    // edilir (paket eşiği; dolgunun seq'leri kopyanınkinden büyük).
    let first = rig.next_unit;
    let fillers = first..first + FRAG_FILLERS;
    rig.net
        .with(|s| s.drop_first.extend(fillers.clone().filter(|u| u % 2 == 1)));
    for _ in fillers {
        rig.send().await;
    }
    rig.wait("dolgu teyit edilmeli (yalnız hedef teyitsiz)", WAIT, |r| {
        r.client.outstanding() == 1
    })
    .await;
    let want = body(t);
    rig.wait("alıcı hedefi kopyadan teslim etmeli", WAIT, |r| {
        r.delivered.get(&t).is_some_and(|b| *b == want)
    })
    .await;
    rig.net.with(|s| {
        assert!(
            !s.held.is_empty(),
            "ön koşul: kopyayı bildiren ACK ağda tutuldu"
        );
    });
    rig.late_ack_keeps_payload_then_resend_confirms(t, path, WAIT)
        .await;
}

/// Envanter satır 5 — **ESI ad çakışması**: parite ACK'i (`unit.id = esi`, ayrı ad-uzayı) teyit
/// üretmez ve payload'ı silmez. Blok 0'ın kaynakları alıcıya ulaşmaz, R parite ulaşır ve ACK'lenir
/// (alıcı çözemez: kaynak yok). İstemci PRE_CONVERGE'dedir. Teyit çıktısındaki `FecParity` süzgeci
/// olmasaydı runtime 0..R birimlerinin payload'ını silerdi.
///
/// Faz 2 (#140'tan farklı): bütün DATA geçer; blok kopyalar/yeniden gönderimler ve RECOVERED ile
/// tamamlanır. Esi adaşı birimlerin kopyalarının parite ACK'inden sonra sürmesi #140'ın motor
/// kısmına aittir (tabanda parite ACK'i o kopyaları iptal eder); burada iddia EDİLMEZ.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn parity_ack_no_confirmation_payload_kept() {
    run_esi(TransportEngine::Legacy, "esi").await;
}

/// [`parity_ack_no_confirmation_payload_kept`], port motoruyla (ürün varsayılanı, #176).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn parity_ack_no_confirmation_payload_kept_cc_port() {
    run_esi(TransportEngine::CcPort, "esi/cc_port").await;
}

async fn run_esi(engine: TransportEngine, path: &str) {
    let k = block_k();
    let r = u64::from(PARITY_R);
    let mut rig = Rig::new(cfg_fec_static(engine)).await;
    rig.net.with(|s| s.data_gate = DataGate::Nothing);
    for _ in 0..k {
        rig.send().await;
    }
    rig.wait(
        "R parite alıcıya ulaşmalı ve ACK'i istemciye geçmeli",
        WAIT,
        |x| {
            x.net
                .with(|s| (s.parity_pns.len() as u64) >= r && s.acks_with_all_parity >= 1)
        },
    )
    .await;
    let forwarded = rig.net.with(|s| s.acks_forwarded);
    rig.wait("istemci geçen ACK çerçevelerini işlemeli", WAIT, |x| {
        x.client.live_diag().ack.frames >= forwarded
    })
    .await;
    rig.pump(QUIET).await;
    // Faz 1: parite ACK'i payload'ı silmez, teyit üretmez.
    assert!(
        rig.acked.is_empty(),
        "parite ACK'i teyit ÜRETMEMELİ: {:?}",
        rig.acked
    );
    assert_eq!(rig.client.outstanding() as u64, k, "payload'lar yerinde");
    let rec = rig.fresh_recovery().await;
    assert_eq!(u64::from(rec.unacked_units), k, "{rec:?}");
    let live = rig.client.live_diag();
    assert_eq!(
        live.rtt_samples, 0,
        "ön koşul: PRE_CONVERGE (parite ACK'i RTT örneği vermez)"
    );
    // Faz 2: bütün DATA geçer; blok tamamlanır (ACK ya da RECOVERED).
    rig.net.with(|s| s.data_gate = DataGate::All);
    let opened = Instant::now();
    rig.wait(
        "blok tamamlanmalı (K/K teyit: ACK ya da RECOVERED)",
        ESI_WAIT,
        |x| x.client.outstanding() == 0,
    )
    .await;
    eprintln!("[{path}] faz 2 tamamlanma süresi {:?}", opened.elapsed());
    rig.wait("bütün birimler teslim edilmeli", WAIT, |x| {
        (x.delivered.len() as u64) == k
    })
    .await;
    let delivered_ok = (0..k).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u)));
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, path);
    let (cs, ss, wire) = rig.finish().await;
    eprintln!(
        "[{path}] gözlem: acked={} fec_recovered_ctrl_units={} sunucu fec_recovered_units={}",
        cs.acked, cs.fec_recovered_ctrl_units, ss.fec_recovered_units
    );
    assert_no_bodyless(&wire, &cs, path);
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(
        cs.acked + cs.fec_recovered_ctrl_units,
        k,
        "her birim bir kez teyit (ACK ya da RECOVERED): {cs:?}"
    );
    assert!(
        delivered_ok,
        "alıcı K birimi gerçek gövdeleriyle teslim etmeli"
    );
}

/// Store-miss bekçisi — kayıp + yeniden sıralama (ARQ, FEC kapalı): DATA'nın %20'si düşer, %10'u
/// 300 ms gecikir (geç orijinal yeniden gönderimden sonra gelir), ACK'lerin %10'u düşer. Bütün
/// birimler teslim + teyit (her biri tam bir kez: store varlığıyla tekilleştirme), haritalar boş.
/// Geç orijinalle teyit edilen birimin kaybolan yeniden gönderimi (envanter satır 7) motorda
/// teyitte iptal edilir: motor ona karar vermez (iddia 7), tele gövdesiz datagram çıkmaz (iddia 6).
/// İptal öncesi port motoruyla bu bekçide 12 atlanan karar ölçülmüştü.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn store_miss_guard_loss_and_reorder() {
    run_guard_loss(TransportEngine::Legacy, "bekçi-kayıp").await;
}

/// [`store_miss_guard_loss_and_reorder`], port motoruyla (ürün varsayılanı, #176).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn store_miss_guard_loss_and_reorder_cc_port() {
    run_guard_loss(TransportEngine::CcPort, "bekçi-kayıp/cc_port").await;
}

async fn run_guard_loss(engine: TransportEngine, path: &str) {
    let mut rig = Rig::new(cfg_no_fec(engine)).await;
    rig.net.with(|s| {
        s.chaos = Some(Chaos {
            state: GUARD_SEED,
            data_drop_ppm: GUARD_DATA_DROP_PPM,
            data_delay_ppm: GUARD_DATA_DELAY_PPM,
            delay: GUARD_DELAY,
            ack_drop_ppm: GUARD_ACK_DROP_PPM,
        });
    });
    for _ in 0..GUARD_UNITS {
        rig.send().await;
    }
    rig.wait("bütün birimler teyit edilmeli", GUARD_WAIT, |r| {
        r.client.outstanding() == 0
    })
    .await;
    rig.wait("bütün birimler teslim edilmeli", WAIT, |r| {
        (r.delivered.len() as u64) == GUARD_UNITS
    })
    .await;
    assert!(
        (0..GUARD_UNITS).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı gerçek gövdeleri teslim etmeli"
    );
    rig.pump(SETTLE).await;
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, path);
    let (cs, _ss, wire) = rig.finish().await;
    assert_no_bodyless(&wire, &cs, path);
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(cs.acked, GUARD_UNITS, "her birim tam bir kez teyit: {cs:?}");
}

/// Store-miss bekçisi — FEC RECOVERED (statik-r): iki birimin DATA'sı kalıcı düşer, alıcı bloğu
/// çözer ve RECOVERED bildirir. RECOVERED teyittir: motor kurtarılan birimin kayıp ve kopya işini
/// iptal eder, uçuştaki kayıtlarını kararsız kılar (uçuşta kalırlar; ACK ya da kayıp ilanıyla ya da
/// sessiz kanal sınırında düşerler, yeniden kuyruğa girmezler — envanter satır 8) ve teyitli birime
/// karar üretmez (iddia 7); tele gövdesiz datagram çıkmaz (iddia 6), runtime haritalarına kayıt
/// açılmaz. Atlama dallarının kendisi `runtime.rs` `skip_tests`'te sınanır (bu trafikte koşmazlar).
///
/// Ters çevrilmiş vakum bekçisi (sahip kararı 2026-11-11): iptal öncesi silinen birimlerin
/// kayıtları teyitten sonra motorda kalıyor, zaman eşiğiyle kayıp ilan ediliyor ve yeniden karar
/// üretiliyordu (`stale_skipped ≥ 1` ön koşuldu). Şimdi teyitten sonra `SKIP_OBSERVE` boyunca
/// `stale_skipped` ve motor kuyrukları yazdırılır ve sonunda `stale_decisions == 0` iddia edilir.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn store_miss_guard_fec_recovered() {
    run_guard_fec(cfg_fec_static(TransportEngine::Legacy), "bekçi-fec").await;
}

/// [`store_miss_guard_fec_recovered`], port motoruyla (ürün varsayılanı, #176).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn store_miss_guard_fec_recovered_cc_port() {
    run_guard_fec(cfg_fec_static(TransportEngine::CcPort), "bekçi-fec/cc_port").await;
}

/// [`store_miss_guard_fec_recovered`], parçalama açık (DATA_FRAG), port motoruyla: teyitli birime
/// karar yoktur, gövdesiz DATA_FRAG çıkmaz (eski davranış 11 B sıfır önekli DATA_FRAG'dı; tel
/// bekçisi onu "DATA_FRAG yalnız önek" diye sayar). İddialar FEC bekçisiyle aynıdır.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "gerçek zamanlı runtime sahipliği (UDP loopback): CI işi test-runtime, `-- --ignored`"]
async fn store_miss_guard_fec_recovered_frag_cc_port() {
    run_guard_fec(
        cfg_fec_static_frag(TransportEngine::CcPort),
        "bekçi-fec/frag/cc_port",
    )
    .await;
}

async fn run_guard_fec(cfg: RuntimeConfig, path: &str) {
    let k = block_k();
    let mut rig = Rig::new(cfg).await;
    rig.net
        .with(|s| s.data_gate = DataGate::Except(GUARD_ERASED.into_iter().collect()));
    for _ in 0..k {
        rig.send().await;
    }
    rig.wait(
        "blok teyit edilmeli (silinenler RECOVERED ile)",
        WAIT,
        |r| r.client.outstanding() == 0,
    )
    .await;
    rig.wait("bütün birimler teslim edilmeli", WAIT, |r| {
        (r.delivered.len() as u64) == k
    })
    .await;
    assert!(
        (0..k).all(|u| rig.delivered.get(&u).is_some_and(|b| *b == body(u))),
        "alıcı gerçek gövdeleri teslim etmeli (silinenler FEC ile)"
    );
    // Gözlem: teyitten sonra `stale_skipped` ve motor kuyrukları, saniyede bir.
    let confirmed_at = Instant::now();
    let mut series = Vec::new();
    let observe_until = Instant::now() + SKIP_OBSERVE;
    while Instant::now() < observe_until {
        rig.pump(SKIP_SAMPLE).await;
        let live = rig.client.live_diag();
        let q = live.recovery.map(|r| {
            (
                r.timers.inflight_len,
                r.timers.pending_lost_len,
                r.timers.dup_queue_len,
            )
        });
        series.push((
            confirmed_at.elapsed().as_secs(),
            live.stale_skipped,
            live.retransmits,
            live.dups,
            q,
        ));
    }
    eprintln!(
        "[{path}] gözlem: teyitten sonra seyir (s, stale_skipped, retransmits, dups, motor (uçuş, \
         kayıp, kopya)) {series:?}"
    );
    let rec = rig.fresh_recovery().await;
    assert_eq!(rec.unacked_units, 0, "{rec:?}");
    assert_runtime_maps_empty(&rec, path);
    let erased = GUARD_ERASED.len() as u64;
    let (cs, ss, wire) = rig.finish().await;
    assert_no_bodyless(&wire, &cs, path);
    assert_eq!(cs.undelivered, 0, "{cs:?}");
    assert_eq!(
        (cs.acked, cs.fec_recovered_ctrl_units),
        (k - erased, erased),
        "{cs:?}"
    );
    assert_eq!(ss.fec_recovered_units, erased, "{ss:?}");
}
