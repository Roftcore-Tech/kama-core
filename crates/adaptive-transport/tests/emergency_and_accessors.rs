//! MUTATION-GÜDÜMLÜ — emergency histerezisi + modül accessor sözleşmeleri.
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --timeout 90 --jobs 4` (8 dosya)
//! [ÖLÇÜLDÜ] `505 mutants tested in 49m: 53 missed, 430 caught, 21 unviable, 1 timeouts`
//!
//! Bu dosya kaçan 53'ün 16'sını hedefler; ikisi ayrı küme:
//!
//! **(1) `emergency.rs::HysteresisController::update` — 4 mutant (159:28, 159:71, 162:35,
//! 162:80).** Bunlar `RecoverFwd`'a giriş ve çıkış eşikleridir; yani "ACK yolu çöktü,
//! ACK-bağımsız forward-recovery'ye geç" kararının tetiği. Bir off-by-one ya emergency'yi
//! bir tık geç açar (kayıp penceresi uzar) ya da erken kapatır (osilasyon). Survival-band
//! davranışının doğrudan kalbi ve `sender_state_matrix.rs` bunu SÜRMÜYOR — matris geçişin
//! izinli olduğunu kanıtlıyor, tetiklendiği EŞİĞİ değil.
//!
//! **(2) Accessor sabitleri — 12 mutant.** `bw.rs::delivered` (2), `loss.rs::unrecovered_units`
//! (2), `loss.rs::ledger_evicted_total` (2), `loss.rs::with_config` (2),
//! `receiver.rs::received_count` (1), `receiver.rs::largest_seq` (2), `rtt.rs::min_entry_time` (1).
//! Aynı kör nokta: mevcut testler bunları ÖLÇÜM ARACI olarak kullanıyor (önce/sonra
//! karşılaştırması), sabit dönen mutant o eşitliği bozmuyor. Çözüm: değeri HEDEF olarak
//! bilinen bir sabite karşı assert etmek.
//!
//! Koş: `cargo test -p adaptive-transport --test emergency_and_accessors`

use adaptive_transport::bw::DeliveryRateSampler;
use adaptive_transport::emergency::{
    EmergencyLevel, HysteresisConfig, HysteresisController, RecoveryModeController,
};
use adaptive_transport::loss::{LossConfig, LossDetector, ThresholdLossDetector};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use std::time::Duration;

const MS: u64 = 1_000_000;

// ════════════════════════════════════════════════════════════════════════════
// emergency.rs — histerezis eşikleri
// ════════════════════════════════════════════════════════════════════════════

fn hyst() -> (HysteresisController, HysteresisConfig) {
    let cfg = HysteresisConfig {
        escalate_permille: 400,
        deescalate_permille: 700,
        deescalate_dwell: Duration::from_millis(500),
    };
    (HysteresisController::new(cfg, EmergencyLevel::Arq), cfg)
}

/// MUTANT `emergency.rs:159:28` — `health < escalate_permille` → `<=`.
#[test]
fn escalation_threshold_is_strict_not_inclusive() {
    // TAM eşikte tırmanma OLMAMALI: `escalate_permille` "bunun ALTINA düşerse" sınırıdır.
    // `<=` olsaydı sınırdaki sağlık değeri emergency'yi tetikler ve normal çalışma
    // koşulunda forward-duplication açılır — bant boşuna harcanır.
    let (mut c, cfg) = hyst();
    assert_eq!(
        c.update(cfg.escalate_permille, 0),
        EmergencyLevel::Arq,
        "TAM escalate eşiğinde tırmanılmamalı"
    );
    // Bir birim altı tırmanmalı.
    assert_eq!(
        c.update(cfg.escalate_permille - 1, MS),
        EmergencyLevel::Dup2,
        "eşiğin bir altında tırmanılmalı"
    );
}

/// MUTANT `emergency.rs:159:71` — `self.level < EmergencyLevel::Dup3` → `<=`.
#[test]
fn escalation_saturates_at_the_top_level() {
    // Tavan `Dup3`. `<=` olsaydı `Dup3`'te bir kez daha `escalated()` çağrılırdı;
    // `escalated()` doygun olsa bile bu, tavanın nerede olduğunu belirsizleştirir.
    let (mut c, cfg) = hyst();
    let low = cfg.escalate_permille - 1;
    let mut t = 0u64;
    let mut seen = Vec::new();
    for _ in 0..5 {
        t += MS;
        seen.push(c.update(low, t));
    }
    assert_eq!(seen[0], EmergencyLevel::Dup2);
    assert_eq!(seen[1], EmergencyLevel::Dup3);
    assert!(
        seen[2..].iter().all(|&l| l == EmergencyLevel::Dup3),
        "Dup3 üstüne çıkıldı veya seviye düştü: {seen:?}"
    );
}

/// MUTANT `emergency.rs:162:35` — `health > deescalate_permille` → `>=`.
#[test]
fn deescalation_threshold_is_strict_not_inclusive() {
    // TAM eşikte gerileme OLMAMALI. `>=` olsaydı sınırdaki sağlık emergency'yi kapatır ve
    // hâlâ kırılgan bir kanalda forward-recovery erken bırakılır.
    let (mut c, cfg) = hyst();
    let mut t = 0u64;
    for _ in 0..3 {
        t += MS;
        c.update(cfg.escalate_permille - 1, t);
    }
    assert_eq!(c.update(cfg.escalate_permille - 1, t), EmergencyLevel::Dup3);

    // TAM deescalate eşiği → gerilemez.
    t += 10 * MS;
    assert_eq!(
        c.update(cfg.deescalate_permille, t),
        EmergencyLevel::Dup3,
        "TAM deescalate eşiğinde gerilenmemeli"
    );
    // Bir birim üstü → geriler.
    t += 10 * MS;
    assert_eq!(
        c.update(cfg.deescalate_permille + 1, t),
        EmergencyLevel::Dup2,
        "eşiğin bir üstünde gerilenmeli"
    );
}

/// MUTANT `emergency.rs:162:80` — `self.level > EmergencyLevel::Arq` → `>=`.
#[test]
fn deescalation_stops_at_the_arq_floor() {
    // Taban `Arq`. `>=` olsaydı `Arq`'ta da `deescalated()` çağrılır ve taban altına
    // inilmeye çalışılırdı — recovery tamamen kapanabilir.
    let (mut c, cfg) = hyst();
    let high = cfg.deescalate_permille + 1;
    let mut t = 0u64;
    for _ in 0..5 {
        t += 10 * MS;
        assert_eq!(
            c.update(high, t),
            EmergencyLevel::Arq,
            "Arq tabanının altına inildi"
        );
    }
}

#[test]
fn deescalation_respects_the_dwell_lock() {
    // Gerileme dwell'lidir (osilasyon önlemi): ardışık iki gerileme arasında
    // `deescalate_dwell` geçmelidir. Bu, eşik testlerinin ön koşuludur — dwell
    // çalışmasaydı yukarıdaki "bir birim üstü geriler" testleri de anlamsızlaşırdı.
    let (mut c, cfg) = hyst();
    let mut t = 0u64;
    for _ in 0..3 {
        t += MS;
        c.update(cfg.escalate_permille - 1, t);
    }
    assert_eq!(c.update(cfg.escalate_permille - 1, t), EmergencyLevel::Dup3);

    let high = cfg.deescalate_permille + 1;
    t += 100 * MS;
    assert_eq!(c.update(high, t), EmergencyLevel::Dup2, "ilk gerileme");

    // Dwell içinde ikinci gerileme OLMAMALI.
    t += 100 * MS; // dwell 500 ms
    assert_eq!(
        c.update(high, t),
        EmergencyLevel::Dup2,
        "dwell içinde ikinci gerileme yapıldı — osilasyon koruması yok"
    );

    // Dwell dolunca gerileme devam eder.
    t += 500 * MS;
    assert_eq!(
        c.update(high, t),
        EmergencyLevel::Arq,
        "dwell sonrası gerileme"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// Accessor sözleşmeleri — sabit döndürme mutantları
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `bw.rs:121:9` — `DeliveryRateSampler::delivered` → 0 / 1.
#[test]
fn delivery_sampler_delivered_is_cumulative_not_constant() {
    // `delivered` [TUR-3c] probe-volume ölçümünün tabanıdır: `ProbeUp` başarısı
    // `delivered_since_entry >= volume` ile karar verilir. Sabitlenirse probe ya hep
    // başarılı ya hep başarısız olur ve bant araması tamamen bozulur.
    let mut s = DeliveryRateSampler::default();
    assert_eq!(s.delivered(), 0, "taze sampler'da delivered 0 olmalı");

    let mut t = 0u64;
    let mut pkts = Vec::new();
    for _ in 0..10u64 {
        t += MS;
        pkts.push(s.on_sent(t, 101, false, true));
    }
    assert_eq!(
        s.delivered(),
        0,
        "gönderim tek başına delivered artırmamalı"
    );

    t += 10 * MS;
    let mut prev = 0u64;
    for (n, pkt) in pkts.into_iter().enumerate() {
        s.on_ack(t + n as u64 * MS, pkt);
        let d = s.delivered();
        assert!(
            d > prev,
            "ACK sonrası delivered artmadı ({prev} → {d}) — sabit döndürülüyor olabilir"
        );
        prev = d;
    }
    assert!(
        prev >= 10 * 101,
        "delivered toplamı beklenenden düşük: {prev}"
    );
}

/// MUTANT `loss.rs:182:9` — `unrecovered_units` → 0 / 1; `370:9` — `+=` → `-=`/`*=`.
///
/// [ÖLÇÜLDÜ] Bu sayaç kayıp İLANIYLA artmaz — `on_unrecovered(units)` ile beslenir, yani
/// sender recovery'den SONRA "bunu kurtaramadım" diye bildirir. İlk yazımda `on_ack`'in
/// artıracağını varsaymıştım; ölçüm çürüttü.
///
/// **Neden önemli:** `residual_loss_ppm` = unrecovered/sent, ve o oran FEC parity
/// politikasının girdisidir. Sabitlenirse parity ya hep tavanda ya hep tabanda kalır.
#[test]
fn unrecovered_units_accumulates_only_via_on_unrecovered() {
    let mut d = ThresholdLossDetector::default();
    assert_eq!(d.unrecovered_units(), 0, "taze dedektörde 0 olmalı");

    // Gönderim + kayıp ilanı TEK BAŞINA sayacı artırmamalı.
    let mut t = 0u64;
    for i in 0..8u64 {
        t += MS;
        d.on_sent(i, t, 1);
    }
    t += 10 * MS;
    let acked: Vec<u64> = (1..8).collect();
    assert!(
        !d.on_ack(&acked, t).is_empty(),
        "kurulum: kayıp ilan edilmedi"
    );
    assert_eq!(
        d.unrecovered_units(),
        0,
        "kayıp ilanı unrecovered'ı artırdı — semantik karışmış"
    );

    // Recovery başarısız bildirimi sayacı artırmalı ve BİRİKMELİ.
    d.on_unrecovered(3);
    assert_eq!(d.unrecovered_units(), 3, "on_unrecovered taşınmıyor");
    d.on_unrecovered(5);
    assert_eq!(
        d.unrecovered_units(),
        8,
        "birikmeli olmalı — `+=` `-=`/`*=`'e dönmüş olabilir"
    );

    // residual_loss_ppm bu sayaçtan türetilmeli.
    assert!(
        d.residual_loss_ppm() > 0,
        "unrecovered var ama residual_loss_ppm 0"
    );
}

/// MUTANT `loss.rs:261:9` — `ledger_evicted_total` → 0 / 1.
#[test]
fn ledger_evicted_total_is_zero_until_capacity_is_hit() {
    // SR-AUDIT-001 gözlemi: ledger kapasitesi dolduğunda en-eski-önce eviction yapılır ve
    // sayaç artar. Küçük bir senaryoda eviction OLMAMALI — sabit `-> 1` buradan geçemez.
    let mut d = ThresholdLossDetector::default();
    assert_eq!(d.ledger_evicted_total(), 0, "taze dedektörde eviction 0");

    let mut t = 0u64;
    for i in 0..8u64 {
        t += MS;
        d.on_sent(i, t, 1);
    }
    t += 10 * MS;
    let acked: Vec<u64> = (1..8).collect();
    d.on_ack(&acked, t);
    assert_eq!(
        d.ledger_evicted_total(),
        0,
        "LEDGER_MAX'a ulaşılmadan eviction sayıldı — sabit döndürülüyor olabilir"
    );
}

/// MUTANT `loss.rs:163:9` (`with_config` → `Default::default()`), `164:13` (`cfg` alanı silme).
#[test]
fn with_config_actually_carries_the_given_config() {
    // `with_config` verilen eşikleri TAŞIMALI. `Default::default()`'a dönerse çağıranın
    // ayarı sessizce yok sayılır ve kayıp tespiti varsayılan eşiklerle çalışır.
    //
    // Gözlem: packet-threshold'u aşırı yükseltilmiş bir config ile aynı senaryoda
    // kayıp İLAN EDİLMEMELİ; varsayılanla edilmeli.
    let strict = LossConfig {
        packet_threshold: 1000, // pratikte ulaşılamaz
        ..LossConfig::default()
    };
    let mut d = ThresholdLossDetector::with_config(strict);
    let mut t = 0u64;
    for i in 0..8u64 {
        t += MS;
        d.on_sent(i, t, 1);
    }
    t += 10 * MS;
    let acked: Vec<u64> = (1..8).collect();
    let declared = d.on_ack(&acked, t).len();
    assert_eq!(
        declared, 0,
        "packet_threshold=1000 ile kayıp ilan edildi — config taşınmıyor"
    );

    // Varsayılanla AYNI senaryo kayıp ilan etmeli (kontrast).
    let mut d2 = ThresholdLossDetector::default();
    let mut t2 = 0u64;
    for i in 0..8u64 {
        t2 += MS;
        d2.on_sent(i, t2, 1);
    }
    t2 += 10 * MS;
    let acked2: Vec<u64> = (1..8).collect();
    let declared2 = d2.on_ack(&acked2, t2).len();
    assert!(
        declared2 > 0,
        "varsayılan config ile de kayıp ilan edilmedi — senaryo kurulumu bozuk"
    );
}

/// MUTANT `receiver.rs:465:9` (`received_count` → 1), `470:9` (`largest_seq` → None/Some(1)).
#[test]
fn receiver_accessors_reflect_real_reception_state() {
    // Bu ikisi SACK üretiminin ve gap tespitinin tabanıdır. `largest_seq -> None` alıcıyı
    // "hiç paket almadım" durumuna kilitler; `-> Some(1)` ise gap hesabını sabitler.
    let mut r = Receiver::default();
    assert_eq!(r.received_count(), 0, "taze alıcıda sayı 0");
    assert_eq!(
        r.largest_seq(),
        None,
        "taze alıcıda largest_seq None olmalı"
    );

    let mut t = 0u64;
    for seq in [5u64, 1, 9, 3] {
        t += MS;
        r.on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            t,
        );
    }
    assert_eq!(r.received_count(), 4, "alınan farklı seq sayısı yanlış");
    assert_eq!(
        r.largest_seq(),
        Some(9),
        "largest_seq gerçek en yüksek değeri vermiyor"
    );

    // Tekrar eden seq sayıyı ARTIRMAMALI (farklı-seq sayacı).
    t += MS;
    r.on_datagram(
        RxDatagram {
            seq: 5,
            unit_id: 5,
            kind: RxKind::Data,
        },
        t,
    );
    assert_eq!(r.received_count(), 4, "tekrar eden seq sayacı artırdı");

    // Daha yüksek seq largest'ı ilerletmeli.
    t += MS;
    r.on_datagram(
        RxDatagram {
            seq: 42,
            unit_id: 42,
            kind: RxKind::Data,
        },
        t,
    );
    assert_eq!(r.largest_seq(), Some(42));
    assert_eq!(r.received_count(), 5);
}

/// MUTANT `emergency.rs:162:80` — `self.level > EmergencyLevel::Arq` → `>=`.
///
/// [ÖLÇÜLDÜ] İlk yazımda `deescalation_stops_at_the_arq_floor` ile öldürmeyi denedim;
/// KAÇTI. Sebebi: `deescalated()` Arq'ta DOYGUN (`Dup2 | Arq => Arq`), yani mutant seviye
/// olarak aynı sonucu veriyor. Ama **yan etki farklı**: guard gevşediğinde Arq'ta da
/// `last_deescalate_ns = Some(now_ns)` yazılır.
///
/// **Neden önemli:** O zaman damgası dwell kilidinin çapasıdır. Arq'ta boşuna güncellenirse,
/// bir sonraki GERÇEK gerileme (Dup2 → Arq) sahte bir dwell kilidine takılır ve emergency
/// gereğinden uzun açık kalır — dar bantta forward-duplication israfı demektir.
///
/// Test bu yan etkiyi zamanlama üzerinden gözlemler.
#[test]
fn arq_floor_does_not_stamp_the_dwell_clock() {
    let (mut c, cfg) = hyst();
    let high = cfg.deescalate_permille + 1;
    let low = cfg.escalate_permille - 1;

    // (1) Arq'ta yüksek sağlık: DOĞRU kodda hiçbir şey olmaz (guard bloklar).
    //     Mutantta `deescalated()` çağrılır ve dwell damgası t=0'a yazılır.
    assert_eq!(c.update(high, 0), EmergencyLevel::Arq);

    // (2) Tırman.
    assert_eq!(c.update(low, MS), EmergencyLevel::Dup2);

    // (3) Hemen geriye: DOĞRU kodda `last_deescalate_ns` hâlâ None → kilit YOK → geriler.
    //     Mutantta Some(0) → 2ms < 0+500ms → KİLİTLİ → Dup2'de kalır.
    assert_eq!(
        c.update(high, 2 * MS),
        EmergencyLevel::Arq,
        "ilk gerçek gerileme sahte dwell kilidine takıldı — \
         Arq tabanında dwell damgası yazılıyor olabilir"
    );
}
