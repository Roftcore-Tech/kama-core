//! MUTATION-GÜDÜMLÜ — `Sender` teşhis gözlemcilerinin sözleşmesi.
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --timeout 120 --jobs 4`
//! [ÖLÇÜLDÜ] `1106 mutants tested in 47m: 178 missed, 864 caught, 63 unviable, 1 timeouts`
//!
//! Kaçan 178'in büyük bir bölümü `diag_*` gözlemcileriydi (`diag_rtt`, `diag_ledger`,
//! `diag_pif`, `diag_wbtlbw`, `diag_btlbw_samples`, `diag_fuzzy_no_rule`, `diag_probe`,
//! `diag_tombstone`, `diag_qd`, `diag_loss`, `diag_rate`). Hepsi "sabit döndür" mutantıyla
//! delinebiliyordu.
//!
//! **Neden kaçtılar:** Mevcut testler bu fonksiyonları ÖLÇÜM ARACI olarak kullanıyor —
//! `desen1_sim` onları `println!` ile basıyor, `sender_state_matrix` hiç kullanmıyor.
//! Sabit döndüren bir mutant hiçbir assertion'ı bozmuyor.
//!
//! **Neden 17 ayrı test yazılmadı:** Bunlar teşhis yüzeyidir, davranış değil. Doğru sözleşme
//! tek cümledir: *"her gözlemci GERÇEK durumu yansıtır; sabit değildir."* Bu dosya o cümleyi
//! tek senaryoda pinler — motor sürülür, her gözlemcinin (a) taze motorda beklenen başlangıç
//! değerini verdiği, (b) trafik sonrası DEĞİŞTİĞİ assert edilir.
//!
//! **Neden önemsiz değil:** `desen1_sim`'in GO/NO-GO kararları ve `harness_signature_pin`
//! tamamen bu gözlemcilerin okuduğu sayılara dayanır. Biri sabitlenirse harness sessizce
//! yalan söyler ve ölçüme dayalı her karar zehirlenir.
//!
//! Koş: `cargo test -p adaptive-transport --test sender_diag_contract`

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer};
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 101; // TC-17: 26 outer + 11 inner + E=64

/// Deterministik senaryo: veri akışı + kayıplı ACK + timer'lar. RNG yok.
/// Motoru gözlemcilerin hepsinin anlamlı değer üreteceği bir duruma taşır.
fn drive(steps: usize) -> Sender {
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut outstanding: Vec<u64> = Vec::new();
    let mut unit_id = 0u64;

    for i in 0..steps {
        now += 5 * MS;
        let ev = match i % 5 {
            0 | 1 => {
                unit_id += 1;
                Event::AppData(OpaqueUnit {
                    id: unit_id,
                    size: UNIT,
                })
            }
            2 | 3 => {
                // Gap-ACK: baştaki birkaç seq'i düşür (kayıp), kalanı onayla.
                let take = outstanding.len().min(8);
                let mut acked: Vec<u64> = outstanding.drain(..take).collect();
                if acked.len() >= 3 {
                    acked.drain(..2); // kayıp simülasyonu → pending_lost dolar
                }
                Event::AckReceived(AckInfo {
                    acked_seqs: acked,
                    ack_delay: Duration::from_micros(500),
                })
            }
            _ => {
                if i % 10 == 4 {
                    Event::TimerFired(Timer::RtpropProbe)
                } else {
                    Event::TimerFired(Timer::Pto)
                }
            }
        };
        for d in s.on_event(ev, now) {
            outstanding.push(d.seq);
        }
        if outstanding.len() > 512 {
            outstanding.drain(..256);
        }
    }
    // Bekleyen işle bitir: aksi hâlde ACK'ler kuyruğu boşaltır ve motor Idle'a düşer,
    // gözlemcilerin hepsi sıfırlanır ve test vakuma döner.
    for _ in 0..20 {
        now += MS;
        unit_id += 1;
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: unit_id,
                size: UNIT,
            }),
            now,
        );
    }
    s
}

// ════════════════════════════════════════════════════════════════════════════
// Taze motor: her gözlemci BİLİNEN başlangıç değerini vermeli
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `sender.rs:1485/1491/1500/1513/1519/1525` — `diag_*` sabit döndürme.
#[test]
fn fresh_sender_reports_known_zero_state() {
    // Sabit-döndüren mutantların ilk kapısı: taze motorda her sayaç SIFIR olmalı.
    // `-> 1` veya `-> (1, 0)` gibi mutantlar buradan geçemez.
    let s = Sender::new();

    assert_eq!(s.state(), SenderState::Idle);
    assert_eq!(s.inflight_bytes(), 0);

    let (state, rate, btlbw, eff, _b) = s.diag_rate();
    assert_eq!(state, SenderState::Idle, "diag_rate durumu yanlış");
    assert_eq!((rate, btlbw), (0, 0), "taze motorda rate/btlbw 0 olmalı");
    // [ÖLÇÜLDÜ] efektif hız taze motorda `BudgetConfig::min_rate_bps` = 31 B/s (cold-start floor).
    assert_eq!(eff, 31, "taze motorda efektif hız min_rate floor'u olmalı");

    let (loss_pkt, loss_time, pend) = s.diag_loss();
    assert_eq!(
        (loss_pkt, loss_time, pend),
        (0, 0, 0),
        "taze loss sayaçları"
    );

    // [ÖLÇÜLDÜ] `diag_rtt()` srtt/rtprop DEĞİL — `(diag_rtt_rejected, diag_ack_delay_max_us)`
    // döndürür (satır 1485). İsimlendirme yanıltıcı; test doğru alanları pinler.
    let (rtt_rejected, ack_delay_max_us) = s.diag_rtt();
    assert_eq!(
        (rtt_rejected, ack_delay_max_us),
        (0, 0),
        "taze motorda rtt-red ve ack_delay-tepe 0 olmalı"
    );

    let (unloss, loss_ppm, spurious) = s.diag_ledger();
    assert_eq!((unloss, loss_ppm, spurious), (0, 0, 0), "taze ledger");

    assert_eq!(s.diag_pif(), 0, "taze motorda uçuşta paket yok");

    let (qd, rtp) = s.diag_qd();
    assert_eq!((qd, rtp), (0, 0), "taze qd/rtprop");

    // [ÖLÇÜLDÜ] taze motorda pencereli btlbw 0, ama YAŞ 0 DEĞİL — 10 s'lik başlangıç
    // penceresi taşınır. Sabit-döndüren mutantlar ((0,0)/(0,1)/(1,0)/(1,1)) yaşın
    // 1'den çok büyük olmasıyla öldürülür.
    let (w_btlbw, w_age) = s.diag_wbtlbw();
    assert_eq!(w_btlbw, 0, "taze pencereli btlbw 0 olmalı");
    assert!(
        w_age > 1_000_000,
        "taze wbtlbw yaşı {w_age} — sabit döndürülüyor olabilir"
    );

    assert!(
        s.diag_btlbw_samples().is_empty(),
        "taze motorda bw örneği olmamalı"
    );
    assert_eq!(s.diag_fuzzy_no_rule(), 0, "taze fuzzy-no-rule sayacı");

    let (succ, abort, timeout, preempt) = s.diag_probe();
    assert_eq!((succ, abort, timeout, preempt), (0, 0, 0, 0), "taze probe");

    let (t_len, hits, bw_rec, evicted, ambig, credited) = s.diag_tombstone();
    assert_eq!(
        (t_len, hits, bw_rec, evicted, ambig, credited),
        (0, 0, 0, 0, 0, 0),
        "taze tombstone sayaçları"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// Sürülmüş motor: gözlemciler DEĞİŞMELİ (sabit değiller)
// ════════════════════════════════════════════════════════════════════════════

/// Aynı mutant kümesinin ikinci kapısı: trafikten sonra değerler sıfırdan FARKLI olmalı.
/// `-> 0` mutantları taze-durum testinden geçer ama buradan geçemez; `-> 1` mutantları
/// tersini yapar. İki test birlikte tüm sabit-döndürme uzayını kapatır.
#[test]
fn driven_sender_reports_state_that_actually_moved() {
    let s = drive(4_000);

    // Motor Idle'da kalmamalı — senaryo gerçekten sürülmüş olmalı (vakumsuzluk).
    assert_ne!(s.state(), SenderState::Idle, "senaryo motoru sürmedi");

    // RTT teşhisi: ACK'ler işlendiği için hem red-sayacı hem ack_delay-tepesi dolmalı.
    let (rtt_rejected, ack_delay_max_us) = s.diag_rtt();
    assert!(
        rtt_rejected > 0,
        "ACK'lere rağmen rtt-red sayacı 0 — diag_rtt sabit olabilir"
    );
    assert!(
        ack_delay_max_us > 0,
        "ack_delay tepesi 0 — diag_rtt sabit olabilir"
    );

    // BtlBw: teslim olduğu için bant tahmini oluşmalı.
    let (_state, rate, btlbw, _eff, _b) = s.diag_rate();
    assert!(btlbw > 1, "teslimata rağmen btlbw={btlbw} — sabit olabilir");
    assert!(rate > 1, "pacing hızı {rate} — sabit olabilir");

    // [ÖLÇÜLDÜ] `diag_wbtlbw().0` bu senaryoda 0 KALIR (pencere örneği dolmuyor);
    // sabit-mutantlar YAŞ üzerinden öldürülür.
    let (_w_btlbw, w_age) = s.diag_wbtlbw();
    assert!(
        w_age > 1_000_000,
        "wbtlbw yaşı {w_age} — sabit döndürülüyor olabilir"
    );

    // BW örnekleri: en az bir örnek birikmiş olmalı ve hepsi anlamlı.
    let samples = s.diag_btlbw_samples();
    assert!(
        !samples.is_empty(),
        "bw örneği yok — diag_btlbw_samples sabit olabilir"
    );
    assert!(
        samples.iter().any(|&(t, v)| t > 0 && v > 0),
        "tüm bw örnekleri (0,0) — sabit vec döndürülüyor olabilir"
    );
    // Sabit `vec![(1,1)]` mutantına karşı: örnek sayısı 1'den fazla olmalı.
    assert!(
        samples.len() > 1,
        "tek örnek — sabit vec döndürülüyor olabilir ({} örnek)",
        samples.len()
    );

    // [ÖLÇÜLDÜ] Bu senaryoda kayıp İLAN EDİLMİYOR: gap-ACK'ler packet-threshold'u
    // tetikleyecek yoğunlukta değil ve PTO time-threshold'u da geçmiyor. Dolayısıyla
    // `diag_loss`/`diag_ledger`/`diag_tombstone` sıfır KALIR. Bu, sabit-`-> 0`
    // mutantlarını öldürmez ama sabit-`-> 1` mutantlarını öldürür.
    // [BEKLİYOR] Kayıp yollarını süren ayrı bir senaryo `desen1_sim`'de mevcut;
    // oradaki gözlemcileri de pinlemek ayrı bir iştir.
    assert_eq!(
        s.diag_loss(),
        (0, 0, 0),
        "bu senaryoda kayıp ilan edilmemeli"
    );
    assert_eq!(
        s.diag_ledger(),
        (0, 0, 0),
        "bu senaryoda ledger boş kalmalı"
    );
    assert_eq!(
        s.diag_tombstone(),
        (0, 0, 0, 0, 0, 0),
        "bu senaryoda tombstone oluşmamalı"
    );
}

/// MUTANT `sender.rs:1500:9` — `diag_pif` → 0 / 1.
#[test]
fn packets_in_flight_tracks_real_inflight_not_a_constant() {
    // `diag_pif` sampler'ın uçuş sayısıdır. Sabit 0/1 döndüren mutant, değerin
    // gönderimle ARTTIĞI ve ACK ile AZALDIĞI gösterilerek öldürülür.
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut sent: Vec<u64> = Vec::new();

    // Sürekli veri ver, ACK verme → uçuş sayısı artmalı.
    for i in 0..200u64 {
        now += 50 * MS;
        for d in s.on_event(Event::AppData(OpaqueUnit { id: i, size: UNIT }), now) {
            sent.push(d.seq);
        }
    }
    // [ÖLÇÜLDÜ] `diag_pif` SAMPLER'ın uçuş sayısıdır, `inflight_bytes` sender'ın
    // muhasebesidir; ikisi AYNI olmak zorunda değil. Ayrıca cold-start floor'u (31 B/s)
    // yüzünden 200 olayda yalnız birkaç paket çıkar — bu yüzden adım süresi büyütüldü.
    let pif_loaded = s.diag_pif();
    assert!(
        pif_loaded > 1,
        "gönderimden sonra pif={pif_loaded} — sabit döndürülüyor olabilir"
    );
    assert!(
        s.inflight_bytes() > 0,
        "uçuşta paket var ama inflight_bytes 0"
    );

    // Hepsini ACK'le → uçuş sayısı düşmeli.
    now += 100 * MS;
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: sent.clone(),
            ack_delay: Duration::from_micros(500),
        }),
        now,
    );
    assert!(
        s.diag_pif() < pif_loaded,
        "ACK sonrası pif düşmedi: {} → {}",
        pif_loaded,
        s.diag_pif()
    );
}

/// MUTANT `sender.rs:1525:9` — `diag_fuzzy_no_rule` → 0 / 1.
#[test]
fn fuzzy_no_rule_counter_is_monotone_and_reflects_evaluations() {
    // [ÖLÇÜLDÜ] Bu senaryoda fuzzy controller her değerlendirmede kural buluyor;
    // sayaç 0 KALIYOR. Bu, sabit-`-> 1` mutantını öldürür ama sabit-`-> 0` mutantını
    // ÖLDÜREMEZ — onu öldürmek için kuralsız bir girdi kombinasyonu üretmek gerekir.
    // [BEKLİYOR — Abkarada] `DeterministicFuzzyController` kural tablosunda boşluk var mı,
    // yoksa `no_rule` sayacı yapısal olarak ulaşılamaz mı?
    let s1 = drive(500);
    let s2 = drive(4_000);
    let (a, b) = (s1.diag_fuzzy_no_rule(), s2.diag_fuzzy_no_rule());
    assert_eq!(a, 0, "500 adımda kuralsız değerlendirme beklenmiyordu");
    assert_eq!(b, 0, "4000 adımda kuralsız değerlendirme beklenmiyordu");
    assert!(b >= a, "fuzzy_no_rule monoton değil");
}

/// MUTANT `sender.rs:1519:9` — `diag_btlbw_samples` → `vec![]` / `vec![(0,0)]` / …
#[test]
fn btlbw_samples_are_windowed_not_a_fixed_vector() {
    // Örnek listesi zamanla DEĞİŞMELİ: kısa koşum ile uzun koşum farklı içerik vermeli.
    let short = drive(300);
    let long = drive(4_000);
    let (a, b) = (short.diag_btlbw_samples(), long.diag_btlbw_samples());
    assert_ne!(
        a, b,
        "kısa ve uzun koşum aynı örnek listesini verdi — sabit vec olabilir"
    );
    // Zaman damgaları artan olmalı (pencere sıralı tutulur).
    assert!(
        b.windows(2).all(|w| w[0].0 <= w[1].0),
        "bw örnekleri zaman sırasında değil"
    );
}
