//! MUTATION-GÜDÜMLÜ — kayıp ilanı + tombstone + UN-LOSS yolu (`Sender::on_ack_received`).
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --file .../sender.rs`
//! Kaçan mutantların büyük bir kümesi `on_ack_received`'in kayıp/tombstone kollarındaydı
//! (satır 546, 561, 564, 616, 624, 637, 638, 652, 656) ve `queue_losses`'ta (747).
//!
//! **[ÖLÇÜLDÜ] Kök sebep — kayıp yolu İKİ FAZLI kurulum olmadan ERİŞİLEMEZ:**
//!
//!   1. [KOD] `on_timer(Pto)` §3.3-C gereği `PreConverge`'de `detect_time_losses`'ı ATLAR.
//!   2. `PreConverge`'den çıkış YALNIZ ilk geçerli SACK ile olur (srtt kilidi açılır).
//!   3. Pacer cold-start'ta `min_rate_bps = 31` B/s floor'undadır; ölçüldü ki 400 AppData
//!      olayı / 20 sanal saniye yalnız ~6 paket gönderir. İlk ACK'ten sonra hız 118 B/s'e
//!      çıkar ve anlamlı sayıda paket uçuşa girer.
//!
//! Üçü birleşince: **bir ACK round-trip'i tamamlanmadan tek bir kayıp bile ilan edilemez.**
//! Depodaki hiçbir test bu kurulumu yapmıyordu; `desen1_sim` senaryoyu üretiyor ama
//! gözlemcileri `println!` ile basıp assert ETMİYOR. Bu yüzden tüm kayıp kolu kanıtsızdı.
//!
//! **Ölçülen referans akış** (bu dosyanın dayandığı sayılar):
//! ```text
//! FAZ-1 ACK sonrası    state=Startup     rate=118 (31 floor'undan çıktı)
//! FAZ-3 iki seq atlandı loss=(2,0,2)      tomb_len=2   state=RecoverArq
//! FAZ-4 geç-ACK         loss=(2,0,1)      tomb=(1,hit=1,bw_rec=1)  ledger=(1,0,1)
//! ```
//!
//! **Kapsam dışı (açıkça):** `cadence_loss_floor` (719) ve `expected_ack_interval` (731)
//! mutantları — 10 adet. İkisi de private ve tek gözlemlenebilir etkileri tombstone
//! TTL'idir; [ÖLÇÜLDÜ] TTL ≈ `K_TOMB × max(9/8·srtt, cadence_floor)` ve 6,4 s'de bile
//! düşmüyor (~14 s ufku). Bu zamanlamayı teste sabitlemek tasarımı aşırı kısıtlar.
//! [BEKLİYOR — Abkarada] TTL ufku bir gereksinim mi, yoksa türetilmiş bir değer mi?
//!
//! Koş: `cargo test -p adaptive-transport --test sender_loss_path`

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer};
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 101; // TC-17: 26 outer + 11 inner + E=64

fn ack(v: Vec<u64>) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: v,
        ack_delay: Duration::from_micros(500),
    })
}

/// İki fazlı kurulum: (1) PreConverge'den çık, (2) uçuşa bol paket koy.
/// Döner: (sender, şimdiki-zaman, gönderilmiş-seq'ler).
fn armed() -> (Sender, u64, Vec<u64>) {
    let mut s = Sender::new();
    let mut now = 0u64;
    let mut out: Vec<u64> = Vec::new();
    let mut uid = 0u64;

    // FAZ 1 — floor hızında birkaç paket çıkar, birini ACK'le → srtt kurulur, PreConverge biter.
    for _ in 0..5 {
        now += 200 * MS;
        uid += 1;
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: uid,
                size: UNIT,
            }),
            now,
        ) {
            out.push(d.seq);
        }
    }
    assert!(
        !out.is_empty(),
        "FAZ-1'de hiç paket çıkmadı — pacer floor'u değişmiş olabilir"
    );
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "ilk ACK'ten önce PreConverge'de olunmalı (§3.3-C)"
    );

    now += 50 * MS;
    s.on_event(ack(vec![out[0]]), now);
    assert_ne!(
        s.state(),
        SenderState::PreConverge,
        "ilk geçerli SACK PreConverge'i sonlandırmalı"
    );

    // FAZ 2 — hız yükseldi; uçuşa yeterli paket koy.
    for _ in 0..300 {
        now += 20 * MS;
        uid += 1;
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: uid,
                size: UNIT,
            }),
            now,
        ) {
            out.push(d.seq);
        }
    }
    assert!(
        out.len() >= 10,
        "FAZ-2'de yeterli paket çıkmadı ({}) — kayıp yolu sürülemez",
        out.len()
    );
    (s, now, out)
}

// ════════════════════════════════════════════════════════════════════════════

/// PreConverge kapısının kendisi — kayıp yolunun ön koşulu.
#[test]
fn loss_declaration_is_frozen_until_the_first_sack() {
    // §3.3-C yasak: cold-start'ta sessizlik reverse-path çöküşü DEĞİL, taban-bw
    // serialization gecikmesidir. Bu yüzden PreConverge'de kayıp İLAN EDİLMEZ.
    // [ÖLÇÜLDÜ] 50 paket + 40 × 500 ms PTO → loss=(0,0,0), state=PreConverge.
    let mut s = Sender::new();
    let mut now = 0u64;
    for i in 0..50u64 {
        now += 50 * MS;
        s.on_event(Event::AppData(OpaqueUnit { id: i, size: UNIT }), now);
    }
    for _ in 0..40 {
        now += 500 * MS;
        s.on_event(Event::TimerFired(Timer::Pto), now);
    }
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "ACK olmadan PreConverge'den çıkılmamalı"
    );
    assert_eq!(
        s.diag_loss(),
        (0, 0, 0),
        "PreConverge'de kayıp ilan edildi — §3.3-C dondurması çalışmıyor"
    );
    assert_eq!(
        s.diag_tombstone().0,
        0,
        "PreConverge'de tombstone oluştu — kayıp ilanına bağlı olmalı"
    );
}

/// MUTANT `sender.rs:546:68`, `561:72` (×3), `564:*` (×4) — packet-threshold kayıp kolu.
#[test]
fn packet_threshold_declares_loss_and_creates_tombstones() {
    let (mut s, mut now, out) = armed();
    assert_eq!(s.diag_loss(), (0, 0, 0), "FAZ-2'de henüz kayıp olmamalı");

    // İlk İKİ seq'i ATLA, kalanını ACK'le → PACKET_THRESHOLD=3 aşılır.
    let skipped: Vec<u64> = out[..2].to_vec();
    let rest: Vec<u64> = out[2..].to_vec();
    now += 50 * MS;
    s.on_event(ack(rest), now);

    let (loss_pkt, loss_time, pending) = s.diag_loss();
    assert_eq!(
        loss_pkt, 2,
        "iki atlanan seq packet-threshold ile ilan edilmeli"
    );
    assert_eq!(loss_time, 0, "bu senaryoda time-threshold tetiklenmemeli");
    assert_eq!(pending, 2, "ilan edilen kayıplar pending_lost'a girmeli");

    // MUTANT 747 (`queue_losses`): her kayıp bir tombstone üretmeli.
    assert_eq!(
        s.diag_tombstone().0,
        2,
        "kayıp başına tombstone oluşmadı — queue_losses atlanmış olabilir"
    );

    // Kayıp ilanı ARQ recovery'yi tetiklemeli.
    assert_eq!(
        s.state(),
        SenderState::RecoverArq,
        "kayıp ilan edildi ama ARQ recovery'ye geçilmedi"
    );

    // Atlanan seq'ler uçuştan düşürülmüş olmalı (queue_losses `inflight.remove`).
    assert!(skipped.len() == 2, "kurulum bozuk: iki seq atlanmalıydı");
}

/// MUTANT `sender.rs:652:*` — UN-LOSS kolu (`unlossed` drain + tombstone hit).
#[test]
fn late_ack_converts_a_declared_loss_into_spurious() {
    // [TUR-2a D-1/3] Kayıp ilan edilmiş bir seq'e ACK gelirse hipotez ÇÜRÜR: ledger'da
    // Spurious'a döner, tombstone'a HIT yazılır ve bandwidth-credit geri verilir.
    // Bu, kayıp ölçümünün kendini düzeltme mekanizmasıdır; bozulursa loss_ppm kalıcı
    // olarak şişer ve FEC/emergency politikası aşırı tepki verir.
    let (mut s, mut now, out) = armed();
    let skipped: Vec<u64> = out[..2].to_vec();
    let rest: Vec<u64> = out[2..].to_vec();
    now += 50 * MS;
    s.on_event(ack(rest), now);

    let tomb_before = s.diag_tombstone().0;
    assert_eq!(tomb_before, 2);
    assert_eq!(
        s.diag_ledger(),
        (0, 0, 0),
        "geç-ACK öncesi ledger boş olmalı"
    );

    // Geç-ACK: atlanan seq'ler nihayet geldi.
    now += 50 * MS;
    s.on_event(ack(skipped), now);

    let (unloss, _loss_ppm, spurious) = s.diag_ledger();
    assert!(
        unloss >= 1,
        "geç-ACK UN-LOSS üretmedi (unloss={unloss}) — `!` guard'ı silinmiş olabilir"
    );
    assert!(
        spurious >= 1,
        "spurious sayacı artmadı (spurious={spurious}) — `+=` `*=`'e dönmüş olabilir"
    );

    let (tomb_len, hits, bw_recovered, _ev, _amb, _cred) = s.diag_tombstone();
    assert!(hits >= 1, "tombstone hit kaydedilmedi");
    assert!(
        bw_recovered >= 1,
        "bandwidth-credit geri verilmedi (bw_recovered={bw_recovered})"
    );
    assert!(
        tomb_len < tomb_before,
        "hit alan tombstone düşürülmedi: {tomb_before} → {tomb_len}"
    );

    // pending_lost azalmalı — kurtarılan seq retransmit kuyruğundan çıkmalı.
    assert!(
        s.diag_loss().2 < 2,
        "UN-LOSS sonrası pending_lost azalmadı: {}",
        s.diag_loss().2
    );
}

/// MUTANT `sender.rs:616:*`, `624:*` — ACK işleme sırasındaki epoch/generation guard'ları.
#[test]
fn duplicate_ack_of_the_same_seq_is_idempotent() {
    // Aynı ACK iki kez gelirse sayaçlar İKİ KEZ artmamalı. `==`→`!=` veya `&&`→`||`
    // mutantları guard'ı gevşetir ve muhasebe çift sayar → loss_ppm ve BtlBw bozulur.
    let (mut s, mut now, out) = armed();
    let rest: Vec<u64> = out[2..].to_vec();
    now += 50 * MS;
    s.on_event(ack(rest.clone()), now);

    let snapshot = (s.diag_loss(), s.diag_ledger(), s.diag_tombstone());

    // Aynı ACK'i üç kez daha besle.
    for _ in 0..3 {
        now += 10 * MS;
        s.on_event(ack(rest.clone()), now);
    }
    assert_eq!(
        (s.diag_loss(), s.diag_ledger(), s.diag_tombstone()),
        snapshot,
        "tekrar ACK sayaçları değiştirdi — idempotans bozuk"
    );
}

/// Vakumsuzluk: kurulum gerçekten kayıp yolunu sürüyor mu?
#[test]
fn setup_actually_reaches_the_loss_branch() {
    // Bu dosyadaki her testin ön koşulu `armed()`'ın çalışmasıdır. Kurulum sessizce
    // bozulursa (pacer floor'u değişir, PreConverge çıkışı kayar) diğer testler
    // "kayıp yok" diye yeşil kalabilir. Bu test kurulumu tek başına doğrular.
    let (s, _now, out) = armed();
    assert!(out.len() >= 10, "yeterli paket gönderilmedi: {}", out.len());
    assert!(
        s.diag_pif() > 1,
        "uçuşta yeterli paket yok: {}",
        s.diag_pif()
    );
    assert_ne!(s.state(), SenderState::PreConverge);
    assert_ne!(s.state(), SenderState::Idle);
    // Hız cold-start floor'undan (31 B/s) çıkmış olmalı.
    let (_st, _rate, _btlbw, eff, _b) = s.diag_rate();
    assert!(eff > 31, "efektif hız hâlâ floor'da: {eff}");
}

/// MUTANT `sender.rs:656:60` — `self.diag_loss_pkt += (self.pending_lost.len() - before)`
/// içindeki `-` → `+`.
///
/// [ÖLÇÜLDÜ] Bu mutant İLK kayıp turunda ayrışmaz: orada `before == 0` olduğu için
/// `len - 0 == len + 0`. Ancak `pending_lost` DOLUYKEN ikinci bir kayıp turu gelirse
/// mutant sayacı çift sayar. Bu yüzden test iki turlu kurulmalıdır.
///
/// Sayaç `desen1_sim`'in loss-atıflamasını besler; şişerse "packet-threshold baskın"
/// yanlış sonucu çıkar ve FEC/emergency politikası yanlış gerekçeyle ayarlanır.
#[test]
fn loss_counter_accumulates_the_delta_not_the_total() {
    let (mut s, mut now, out) = armed();

    // TUR 1: ilk iki seq atlanır → 2 kayıp, pending_lost = 2.
    let skipped1: Vec<u64> = out[..2].to_vec();
    let mid = out.len() / 2;
    now += 50 * MS;
    s.on_event(ack(out[2..mid].to_vec()), now);
    let (pkt1, _t1, pend1) = s.diag_loss();
    assert_eq!(pkt1, 2, "tur-1 kayıp sayısı");
    assert!(pend1 > 0, "tur-1 sonrası pending_lost dolu olmalı");

    // TUR 2: pending_lost DOLUYKEN yeni kayıp ilan ettir — sonraki seq'leri atlayıp
    // daha ilerisini ACK'le.
    let skipped2: Vec<u64> = out[mid..mid + 2].to_vec();
    now += 50 * MS;
    s.on_event(ack(out[mid + 2..].to_vec()), now);

    let (pkt2, _t2, _pend2) = s.diag_loss();
    // Doğru: 2 + 2 = 4 (delta birikimi). Mutant `+`: 2 + (len + before) → şişer.
    assert_eq!(
        pkt2, 4,
        "loss_pkt delta yerine toplam biriktiriyor olabilir (tur-1={pkt1}, tur-2={pkt2})"
    );
    assert!(
        !skipped1.is_empty() && !skipped2.is_empty(),
        "kurulum bozuk"
    );
}

/// MUTANT `sender.rs:637:16` (`delete !`), `638:40` (`+=` → `*=`) — RTT ack_delay
/// düzeltme-reddi sayacı.
///
/// [KOD] `apply_ack_delay` düzeltmeyi güvensiz bulursa RAW örnek kullanılır ve
/// `diag_rtt_rejected` artar (§7.3 ack_delay_untrusted). Örnek ATLANMAZ — atlamak
/// estimator açlığı üretirdi (K-1 sınıfı). Sayaç `diag_rtt().0` olarak dışarı çıkar.
///
/// **Neden önemli:** bu sayaç, srtt'nin ne kadarının düzeltilmemiş ham örnekten geldiğini
/// söyler. Sabitlenirse ya da guard tersine dönerse, RTT tahmininin güvenilirliği hakkında
/// yanlış bilgi verir ve `desen1_sim`'in `rtt_rej` teşhisi anlamsızlaşır.
#[test]
fn rtt_correction_rejection_counter_reflects_the_guard() {
    // Güvenilir ack_delay (küçük, rtprop içinde) → düzeltme kabul → sayaç ARTMAZ.
    let (mut s, mut now, out) = armed();
    let rejected_before = s.diag_rtt().0;
    now += 50 * MS;
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: out[2..].to_vec(),
            ack_delay: Duration::from_micros(100),
        }),
        now,
    );
    let rejected_small = s.diag_rtt().0;

    // Güvenilmez ack_delay (ölçülen raw'dan BÜYÜK) → düzeltme reddedilir → sayaç ARTAR.
    let (mut s2, mut now2, out2) = armed();
    now2 += MS; // çok kısa raw
    s2.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: out2[2..].to_vec(),
            ack_delay: Duration::from_secs(10), // raw'dan çok büyük → guard'a takılır
        }),
        now2,
    );
    let rejected_big = s2.diag_rtt().0;

    assert!(
        rejected_big > rejected_small || rejected_big > rejected_before,
        "güvenilmez ack_delay reddi sayaca yansımadı \
         (öncesi={rejected_before}, küçük={rejected_small}, büyük={rejected_big})"
    );
}
