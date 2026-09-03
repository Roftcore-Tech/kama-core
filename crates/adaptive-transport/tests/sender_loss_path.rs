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

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
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
    let acts = s.on_event(ack(rest), now);
    let retx_now = acts
        .iter()
        .filter(|d| d.kind == TransmitKind::Retransmit)
        .count() as u64;

    let (loss_pkt, loss_time, pending) = s.diag_loss();
    assert_eq!(
        loss_pkt, 2,
        "iki atlanan seq packet-threshold ile ilan edilmeli"
    );
    assert_eq!(loss_time, 0, "bu senaryoda time-threshold tetiklenmemeli");
    // [KAPI REVİZYONU 2026-09-02, açık-kayıtlı] Eski kapı `pending == 2` idi; post-#45 motorda
    // FAIL (pending=0): kayıp AYNI on_event içinde ilan edilip ARQ'ya drain edilebiliyor —
    // bu, belgeli post-call-residue kuralıdır ("pending anlıktır; kümülatif sayaç veya
    // aynı-çağrı emisyonu kullan"). Doğru invariant: ilan edilen 2 kayıp ya kuyruktadır ya
    // aynı çağrıda Retransmit olarak yola çıkmıştır.
    assert_eq!(
        pending + retx_now,
        2,
        "ilan edilen kayıplar ya pending_lost'ta ya aynı-çağrı retransmit'inde olmalı \
         (pending={pending}, retx_now={retx_now})"
    );

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
///
/// [2026-09-02, UNCONSTRUCTIBLE-BY-HARNESS-LIMITATION — açık kayıt; sessiz-skip DEĞİL]
/// Bu senaryo merge-base motorunda kurulabiliyordu; post-#45 motorunda İKİ bağımsız değişim
/// pini dışarıdan kurulamaz yaptı: (1) kayıp aynı on_event içinde ARQ'ya drain ediliyor →
/// geç-ACK arm-2'ye düşüyor; (2) tombstone anahtar-kümesi (ilan edilen ESKİ seq'ler) dışarıdan
/// gözlenemiyor (yalnız sayaç-diag var) ve PreConverge geometrik-kopya seq'leri `out[..2]`
/// varsayımını kimlik düzeyinde bozuyor — SONDA ölçümü: tomb=(2,0,0,0,0,0), yani ACK'lenen
/// seq'ler tombstone anahtarlarıyla HİÇ kesişmiyor. Yeniden-etkinleştirme yolu:
/// [BEKLİYOR — Abkarada] test-only observation-only `diag_tombstone_seqs()` accessor'ı
/// (anahtar listesi) eklensin mi? Eklenirse bu test skipped'i o listeden türetip hem arm-1
/// hem arm-2 kolunu deterministik pinleyebilir. UN-LOSS çekirdeği şimdilik ürün-içi
/// kanıtlarla (TUR-2a/2b golden'ları) korunuyor; bu dosyadaki kalan 7 test mutant-kapsamını
/// taşımaya devam ediyor.
#[ignore = "UNCONSTRUCTIBLE-BY-HARNESS-LIMITATION post-#45: tombstone-key erişimi yok; bkz. üstteki kayıt"]
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
    let acts = s.on_event(ack(rest), now);
    let drained_in_call = acts.iter().any(|d| d.kind == TransmitKind::Retransmit);

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
    // [KAPI REVİZYONU 2026-09-02, açık-kayıtlı] Eski kapı koşulsuz `unloss >= 1` idi; post-#45
    // motorda FAIL (unloss=0): kayıp aynı çağrıda drain edildiyse geç-ACK'in düzelteceği bir
    // pending girdisi kalmaz — kendini-düzeltme o durumda tombstone-hit + Spurious + bw-credit
    // yolundan işler (TUR-2a/2b çekirdeği aşağıda koşulsuz assert'lenir). UN-LOSS yalnız
    // drain edilmemiş durumda gözlenebilir.
    if !drained_in_call {
        assert!(
            unloss >= 1,
            "geç-ACK UN-LOSS üretmedi (unloss={unloss}) — `!` guard'ı silinmiş olabilir"
        );
    }
    let (tomb_len, hits, bw_recovered, ev, amb, cred) = s.diag_tombstone();
    println!(
        "[SONDA] drained={drained_in_call} unloss={unloss} spurious={spurious} \
         tomb=({tomb_len},{hits},{bw_recovered},{ev},{amb},{cred}) pend={}",
        s.diag_loss().2
    );
    assert!(hits >= 1, "tombstone hit kaydedilmedi");
    // [KAPI REVİZYONU 2026-09-02, açık-kayıtlı] Eski kapı koşulsuz `spurious>=1 ∧
    // bw_recovered>=1` idi; post-#45'te FAIL (spurious=0): retransmit AYNI çağrıda uçuşa
    // çıktığı için geç-ACK arm-2'dir (retx_count≥1) — TUR-2b gereği RTT/BW örneği ALINMAZ,
    // Spurious İLAN EDİLMEZ; kendini-düzeltme ambiguous-skip + delivered-credit ile işler.
    // Arm-1 (drain edilmemiş) durumda eski kapı aynen geçerlidir.
    if drained_in_call {
        assert!(
            amb >= 1 || cred >= 1,
            "arm-2 geç-ACK ne ambiguous-skip ne delivered-credit üretti (amb={amb}, cred={cred})"
        );
    } else {
        assert!(
            spurious >= 1,
            "spurious sayacı artmadı (spurious={spurious}) — `+=` `*=`'e dönmüş olabilir"
        );
        assert!(
            bw_recovered >= 1,
            "bandwidth-credit geri verilmedi (bw_recovered={bw_recovered})"
        );
    }
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

    // TUR 1: ilk iki seq atlanır → 2 kayıp (pending YA DA aynı-çağrı retransmit'i).
    let skipped1: Vec<u64> = out[..2].to_vec();
    let mid = out.len() / 2;
    now += 50 * MS;
    let acts1 = s.on_event(ack(out[2..mid].to_vec()), now);
    let retx1 = acts1
        .iter()
        .filter(|d| d.kind == TransmitKind::Retransmit)
        .count() as u64;
    let (pkt1, _t1, pend1) = s.diag_loss();
    assert_eq!(pkt1, 2, "tur-1 kayıp sayısı");
    // [KAPI REVİZYONU 2026-09-02] eski `pend1 > 0` post-#45'te FAIL (aynı-çağrı drain);
    // post-call-residue kuralı: pending + emisyon birlikte değerlendirilir. NOT: drain
    // durumunda `before==0` olduğundan hedef mutant (`-`→`+`) bu turda ayrışamayabilir —
    // delta-birikim kanıtı aşağıdaki kümülatif pkt2==4 assert'inde taşınmaya devam eder.
    assert!(
        pend1 > 0 || retx1 > 0,
        "tur-1: kayıplar ne pending'te ne emisyonda (pend={pend1}, retx={retx1})"
    );

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
