//! MUTATION-GÜDÜMLÜ — `receiver.rs` ACK üretimi, kota ve gap tespiti.
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --timeout 90 --jobs 4`
//! [ÖLÇÜLDÜ] `505 mutants tested in 49m: 53 missed` — 19'u `receiver.rs`'te, tek dosyadaki
//! en büyük küme.
//!
//! **Kök sebep:** Depoda alıcı tarafını süren tek test `tv_rx.rs` (golden vektör). ACK
//! ÜRETİM POLİTİKASI — frequency tetiği, cadence tabanı, epizot kotası, gap tespiti —
//! hiç assert edilmemiş. Bu, dönüş yolunun (`p5_revpath`) davranışını belirleyen katman:
//! ACK çok seyrek üretilirse sender ack-açlığına düşer ve emergency'ye geçer; çok sık
//! üretilirse zaten dar olan dönüş bandı boğulur.
//!
//! **[ÖLÇÜLDÜ] Referans davranış** (bu dosyanın dayandığı ölçümler):
//! ```text
//! Sıralı varış:      her 2. DATA bir ACK tetikler (seq 1,3,5 → ack)
//! Gap sonrası timer: +1000 ms'de ACK YOK, +5000 ms'de ACK VAR (cadence tabanı arada)
//! set_quota_r(0):    quota_burn 0 kalır;  set_quota_r(4/8): burn oluşur
//! ```
//!
//! **Kapsam dışı (açıkça) — [ÖLÇÜLDÜ] gerekçelerle:**
//!
//!   `episode_baseline` (337/338/344, 4) ve `push_gap` (353): private; yalnız kota kararları
//!   üzerinden dolaylı gözlemleniyor. Medyan penceresini (EPISODE_WINDOW=3) dışarıdan
//!   deterministik doldurmak ayrı bir harness işi.
//!
//!   `has_gap` (401, `-` → `+`): İlk yazımda "kesintisiz alımda epizot açılmaz" varsayımıyla
//!   test yazdım; ÖLÇÜM ÇÜRÜTTÜ. [KOD] "Oturum-ilk epizot new()+ilk-varışta açılır" — tek
//!   datagramlık senaryoda bile `episode_count=1`. Yani epizot sayacı gap tespitinin
//!   göstergesi DEĞİL; `has_gap`'in etkisi başka bir yoldan gözlemlenmeli.
//!
//!   `set_quota_r`'ın açık-epizot hizalaması (363, 3 mutant): epizot açıldıktan SONRA R'yi
//!   değiştirmek ölçülebilir fark üretmiyor — [ÖLÇÜLDÜ] R=1/4/8/16 dördü de `quota_burn=3`
//!   veriyor, çünkü timer emisyonları kotayla değil CADENCE ile sınırlanıyor. Aşağıdaki
//!   `quota_zero_disables_episode_credit` yalnız R=0 kolunu kapatır.
//!
//! [BEKLİYOR — Abkarada] Bu üç grup için gözlemlenebilirlik mi eksik, yoksa kod mu ölü?
//!
//! Koş: `cargo test -p adaptive-transport --test receiver_ack_policy`

use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;

fn dg(seq: u64) -> RxDatagram {
    RxDatagram {
        seq,
        unit_id: seq,
        kind: RxKind::Data,
    }
}

/// Sıralı `n` datagram besler; (alıcı, son-zaman, üretilen-ACK-sayısı) döner.
fn feed(n: u64) -> (Receiver, u64, usize) {
    let mut r = Receiver::default();
    let mut t = 0u64;
    let mut acks = 0usize;
    for seq in 0..n {
        t += 10 * MS;
        if r.on_datagram(dg(seq), t).ack.is_some() {
            acks += 1;
        }
    }
    (r, t, acks)
}

// ════════════════════════════════════════════════════════════════════════════
// Frequency tetiği
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `receiver.rs:208:28` — `on_datagram` içindeki DATA-sayacı karşılaştırması `>` → `>=`.
#[test]
fn ack_frequency_trigger_fires_every_second_data() {
    // §2.1 ACK-tetik: DATA sayacı ACK_FREQ'e ulaşınca ACK üretilir. [ÖLÇÜLDÜ] ACK_FREQ=2 —
    // seq 1, 3, 5 ACK üretir; 0, 2, 4 üretmez.
    //
    // Bir off-by-one ACK'i her pakette (dönüş bandı boğulur) ya da her üçüncüde (sender
    // ack-açlığına yaklaşır) üretir. İkisi de `p5_revpath` davranışını doğrudan değiştirir.
    let mut r = Receiver::default();
    let mut t = 0u64;
    let mut pattern = Vec::new();
    for seq in 0..6u64 {
        t += 10 * MS;
        pattern.push(r.on_datagram(dg(seq), t).ack.is_some());
    }
    assert_eq!(
        pattern,
        vec![false, true, false, true, false, true],
        "ACK frequency deseni değişti — tetik eşiği kaymış olabilir"
    );
}

#[test]
fn every_datagram_is_delivered_regardless_of_ack_emission() {
    // ACK üretimi ile TESLİM ayrı yollardır. Frequency mutantı teslimi etkilememeli;
    // etkiliyorsa katman karışmış demektir.
    let mut r = Receiver::default();
    let mut t = 0u64;
    let mut delivered = 0usize;
    for seq in 0..6u64 {
        t += 10 * MS;
        delivered += r.on_datagram(dg(seq), t).delivered.len();
    }
    assert_eq!(delivered, 6, "her DATA bir birim teslim etmeli");
    assert_eq!(r.received_count(), 6);
}

// ════════════════════════════════════════════════════════════════════════════
// Cadence tabanı — on_ack_timer
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `receiver.rs:300:20` (`elapsed < cadence_eff` → `<=`),
/// `303:24` (`cadence_eff < ACK_CADENCE_FLOOR_NS` → `<=`).
#[test]
fn ack_cadence_floor_brackets_the_timer_emission() {
    // §3.3-E MİNİMAL-TABAN: kota tükendiğinde ACK'ler statik bir cadence tabanına düşer.
    // [ÖLÇÜLDÜ] gap sonrası +1000 ms'de ACK YOK, +5000 ms'de ACK VAR — taban arada.
    //
    // Bu iki nokta tabanı KUŞATIR: taban aşağı kayarsa 1000 ms'de ACK çıkar (dönüş bandı
    // israfı), yukarı kayarsa 5000 ms'de bile çıkmaz (sender ack-açlığı → emergency).
    let mk = || {
        let mut r = Receiver::default();
        let mut t = 0u64;
        for seq in 0..3u64 {
            t += 10 * MS;
            r.on_datagram(dg(seq), t);
        }
        t += 6 * MS;
        r.on_datagram(dg(9), t); // gap aç
        (r, t)
    };

    for early in [1u64, 10, 50, 100, 500, 1000] {
        let (mut r, t) = mk();
        assert!(
            r.on_ack_timer(t + early * MS).is_none(),
            "+{early} ms'de ACK üretildi — cadence tabanı aşağı kaymış olabilir"
        );
    }
    let (mut r, t) = mk();
    assert!(
        r.on_ack_timer(t + 5000 * MS).is_some(),
        "+5000 ms'de ACK üretilmedi — cadence tabanı yukarı kaymış olabilir"
    );
}

#[test]
fn timer_without_any_arrival_emits_nothing() {
    // Vakumsuzluk + fail-safe: hiç veri gelmemişken timer ACK üretmemeli. Aksi hâlde
    // sessiz oturumlarda boş ACK seli oluşur.
    let mut r = Receiver::default();
    for k in 1..10u64 {
        assert!(
            r.on_ack_timer(k * 1000 * MS).is_none(),
            "varış yokken ACK üretildi (t={k}s)"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Epizot kotası
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `receiver.rs:363:23` — `set_quota_r` içindeki `if self.quota > 0` → `==`/`<`/`>=`.
#[test]
fn quota_zero_disables_episode_credit() {
    // [r3 §2] Kota, bir epizotta kaç dekorele ACK kopyası gönderileceğini belirler —
    // dönüş yolu kayıplıysa kurtarıcı, temizse israf. R=0 kotayı tamamen kapatmalı.
    // [ÖLÇÜLDÜ] R=0 → quota_burn 0; R=4/8 → burn oluşur.
    let run = |r_val: u32| {
        let mut r = Receiver::default();
        r.set_quota_r(r_val);
        let mut t = 0u64;
        for seq in [0u64, 1, 5, 6, 7, 20] {
            t += 10 * MS;
            r.on_datagram(dg(seq), t);
        }
        for k in 1..8u64 {
            r.on_ack_timer(t + k * 100 * MS);
        }
        r.diag_return_supply()
    };

    let (_ep0, burn0, _acc0, _emit0) = run(0);
    assert_eq!(burn0, 0, "R=0 iken kota yakıldı — kota kapatılmıyor");

    let (_ep4, burn4, _acc4, _emit4) = run(4);
    assert!(burn4 > 0, "R=4 iken kota hiç yakılmadı — kota açılmıyor");
}

// ════════════════════════════════════════════════════════════════════════════
// Seq muhasebesi
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `receiver.rs:389:20` — `record_seq` içindeki `seq > self.largest_seq` → `>=`.
#[test]
fn largest_seq_advances_only_on_strictly_higher_arrivals() {
    // `largest_seq` ack_delay ölçümünün çapasıdır (`largest_recv_ns` onunla güncellenir).
    // `>=` olsaydı AYNI seq'in tekrarı da zaman damgasını tazeler ve ack_delay olduğundan
    // küçük ölçülür — sender'ın RTT tahmini sistematik olarak sapar.
    let mut r = Receiver::default();
    let mut t = 0u64;

    t += 10 * MS;
    r.on_datagram(dg(5), t);
    assert_eq!(r.largest_seq(), Some(5));

    // Daha DÜŞÜK seq largest'ı değiştirmemeli.
    t += 10 * MS;
    r.on_datagram(dg(2), t);
    assert_eq!(r.largest_seq(), Some(5), "düşük seq largest'ı geriletti");

    // AYNI seq largest'ı değiştirmemeli ve sayımı artırmamalı.
    let before = r.received_count();
    t += 10 * MS;
    r.on_datagram(dg(5), t);
    assert_eq!(r.largest_seq(), Some(5));
    assert_eq!(before, r.received_count(), "tekrar eden seq sayıldı");

    // Kesin YÜKSEK seq ilerletmeli.
    t += 10 * MS;
    r.on_datagram(dg(9), t);
    assert_eq!(r.largest_seq(), Some(9));
}

// ════════════════════════════════════════════════════════════════════════════
// Vakumsuzluk
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn baseline_scenario_actually_emits_acks() {
    // Bu dosyadaki testlerin çoğu "ACK üretildi mi" sorusuna dayanıyor. Alıcı hiç ACK
    // üretmese hepsi yanlış nedenle yeşil kalabilirdi.
    let (r, _t, acks) = feed(6);
    assert_eq!(acks, 3, "6 sıralı datagramda 3 ACK bekleniyordu");
    assert_eq!(r.received_count(), 6);
    let (_ep, _burn, _acc, emit_total) = r.diag_return_supply();
    assert!(emit_total > 0, "hiç ACK emisyonu kaydedilmedi");
}
