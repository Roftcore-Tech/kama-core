//! MUTATION-GÜDÜMLÜ — `fusion.rs` karar eşikleri ve domain-örtüsü invariantı.
//!
//! **Kaynak:** `cargo mutants -p adaptive-transport --timeout 90 --jobs 4`
//! [ÖLÇÜLDÜ] `505 mutants tested in 42m: 43 missed` — 10'u `fusion.rs`'te, kalan en büyük
//! dokunulmamış küme.
//!
//! **Kök sebep:** `tv_fusion.rs` 10 test içeriyor ama hepsi golden-vektör: fixture config ile
//! belirli girdilerin belirli çıktılar verdiğini pinliyor. Karar EŞİKLERİNİN kendisi —
//! `emergency > PERMILLE/2`, `bw.low > PERMILLE/2` — hiç sınırdan sürülmemiş. Fixture'ın
//! trapezoidleri o sınırlara tam oturmadığı için mutasyonlar fark yaratmıyor.
//!
//! **Yöntem:** Fixture yerine SENTETİK bir `FuzzyConfig` kurulur. `Trap{a:0, b:1000, …}`
//! rising-edge'i sayesinde `mu(500) == 500` olur, yani `PERMILLE/2` sınırı TAM vurulabilir.
//! Bu, golden vektörlerin yapamadığı şeydir: sınırın iki yanını da adresleyebilmek.
//!
//! ---
//! **[ÖLÇÜLDÜ] `decide`'ın 7 mutantı KAPSANMADI — DOMAIN KARIŞMASI.**
//!
//! `375:33`, `375:44`, `382:31` (×2), `382:42` (×2), `382:46`. Üçünü hedefleyen test yazdım,
//! üçü de kırmızı yandı ve sebebi kodun KENDİ uyarısıydı:
//!
//! ```text
//! // [TUR-4] bw DOMAIN dönüşümü: membership log2_fp-ölçeğinde (eşikler de aynı ölçekte).
//! // DİKKAT domain-karışması: bandwidth eşikleri log-fp (≤~16k), diğer 4 değişken HAM birim.
//! let bw = c.bandwidth.fuzzify(log2_fp(inputs.bandwidth_bps));
//! ```
//!
//! `emergency = min3(loss.high, bw.low, ackc.low)` ve `compress = bw.low > PERMILLE/2 || …`
//! — İKİSİ DE `bw.low`'a bağlı, yani sınırı vurmak için trapezoidleri LOG-FP ölçeğinde
//! kurmak zorunlu. Ham byte/s ölçeğinde kurulan bir rampa sessizce yanlış domaine düşüyor:
//! [ÖLÇÜLDÜ] `bandwidth_bps=10` → `log2_fp(10)≈3400`, 1000'de biten rampada `mu_low=0`.
//!
//! **Bu başlı başına bir bulgudur:** aynı `VarThresholds` tipi iki farklı ÖLÇEKTE kullanılıyor
//! ve tip sistemi bunu ayırt etmiyor. Testte bana çarptı; üretimde bir konfigürasyon
//! hatasında sessizce yanlış davranış üretir (bw membership'i hep 0 ya da hep 1000).
//! [BEKLİYOR — Abkarada] `bandwidth` için ayrı bir newtype (`LogFpThresholds`) gerekli mi?
//!
//! Aşağıdaki `covers_domain` testleri bu karışmadan ETKİLENMEZ — o fonksiyon saf, ölçekten
//! bağımsız bir yapısal invariant kontrolüdür.
//! ---
//!
//! **Neden `covers_domain` önemli:** [KOD] yorumu normatif — komşu setler KESİN-örtüşmeli;
//! tek-nokta zero-weight deliği hiçbir kuralın ateşlenmediği bir girdi değeri demektir
//! (`no_rule` sayacı artar, karar varsayılana düşer).
//!
//! Koş: `cargo test -p adaptive-transport --test fusion_thresholds`

use adaptive_transport::fusion::{
    DeterministicFuzzyController, FusionController, FusionInputs, FuzzyConfig, Mode, Trap,
    VarThresholds, PERMILLE,
};

const MAX: u64 = u64::MAX;

/// Sol-omuz LOW: x=0'da 1000, x=1000'de 0. Yani `mu(500) == 500`.
fn shoulder_low() -> Trap {
    Trap {
        a: 0,
        b: 0,
        c: 0,
        d: 1000,
    }
}
/// Sağ-omuz HIGH: x=0'da 0, x=1000'de 1000. Yani `mu(500) == 500`.
fn shoulder_high() -> Trap {
    Trap {
        a: 0,
        b: 1000,
        c: MAX,
        d: MAX,
    }
}
/// Hiç tetiklenmeyen MED (test edilen kuralları izole eder).
fn dead_mid() -> Trap {
    Trap {
        a: MAX,
        b: MAX,
        c: MAX,
        d: MAX,
    }
}

fn var() -> VarThresholds {
    VarThresholds {
        low: shoulder_low(),
        med: dead_mid(),
        high: shoulder_high(),
    }
}

fn linear_cfg() -> FuzzyConfig {
    let f = FuzzyConfig::fixture();
    FuzzyConfig {
        loss: var(),
        bandwidth: var(),
        jitter: var(),
        queue_delay: var(),
        ack_confidence: var(),
        ..f
    }
}

fn inputs_for_emergency(e: u64) -> FusionInputs {
    FusionInputs {
        loss_ppm: e,
        bandwidth_bps: 1000 - e,
        jitter_ppm: 0,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000 - e,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// decide() — dejenere sabit kontrolü (domain karışmasından etkilenmeyen kısım)
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `fusion.rs:375:44` — `PERMILLE / 2` → `PERMILLE % 2`.
#[test]
fn emergency_threshold_is_half_permille_not_a_degenerate_constant() {
    // `PERMILLE % 2` = 0 → eşik sıfıra iner ve HERHANGİ bir emergency derecesi
    // (1 permille bile) forward-redundancy'yi açar.
    let c = DeterministicFuzzyController::new(linear_cfg());
    for small in [1u64, 10, 100, 250] {
        let d = c.decide(&inputs_for_emergency(small));
        assert_eq!(
            d.mode,
            Mode::Normal,
            "emergency={small} permille ile mod Emergency oldu — eşik sıfıra inmiş olabilir"
        );
    }
    assert_eq!(c.decide(&inputs_for_emergency(0)).mode, Mode::Normal);
}

/// MUTANT `fusion.rs:382:42` — `PERMILLE / 2` → `%` (=0) / `*` (=2000).
#[test]
fn compress_threshold_constant_is_not_degenerate() {
    // `%` olursa HER bw.low>0 compress açar; `*` olursa bw.low asla aşamaz (max 1000)
    // ve compress YALNIZ emergency'ye kalır.
    let c = DeterministicFuzzyController::new(linear_cfg());

    // (a) Çok düşük bw.low + emergency yok → compress KAPALI olmalı (`%` mutantını öldürür).
    let almost_full_bw = FusionInputs {
        loss_ppm: 0,
        bandwidth_bps: 990,
        jitter_ppm: 0,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    };
    let d1 = c.decide(&almost_full_bw);
    assert_eq!(d1.mode, Mode::Normal);
    assert!(
        !d1.compress,
        "bw.low düşükken compress açıldı — eşik sabiti sıfıra inmiş olabilir"
    );

    // (b) Çok yüksek bw.low + emergency yok → compress AÇIK olmalı (`*` mutantını öldürür).
    let very_low_bw = FusionInputs {
        loss_ppm: 0,
        bandwidth_bps: 0,
        jitter_ppm: 0,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    };
    let d2 = c.decide(&very_low_bw);
    assert_eq!(d2.mode, Mode::Normal, "kurulum: emergency olmamalı");
    assert!(
        d2.compress,
        "bw.low tavandayken compress açılmadı — eşik sabiti tavanın üstüne çıkmış olabilir"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// covers_domain — TUR-4 domain-örtüsü invariantı
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `fusion.rs:141:45` (`low.a == 0 && low.b == 0` → `||`),
/// `142:54` (`high.c == MAX && high.d == MAX` → `||`).
#[test]
fn covers_domain_requires_both_shoulder_conditions() {
    // [KOD] Sol-omuz LOW ve sağ-omuz HIGH YAPISAL şarttır. `&&` → `||` olursa YARIM omuz da
    // geçerli sayılır ve uçlarda μ=0 deliği oluşur — o girdi değerinde hiçbir kural
    // ateşlenmez (`no_rule` artar, karar varsayılana düşer).
    let good = VarThresholds {
        low: Trap {
            a: 0,
            b: 0,
            c: 100,
            d: 400,
        },
        med: Trap {
            a: 300,
            b: 400,
            c: 600,
            d: 800,
        },
        high: Trap {
            a: 700,
            b: 900,
            c: MAX,
            d: MAX,
        },
    };
    assert!(good.covers_domain(), "geçerli örtü reddedildi");

    // Sol omuz YARIM: a==0 ama b!=0 → `&&` reddeder, `||` kabul eder.
    let half_left = VarThresholds {
        low: Trap { b: 50, ..good.low },
        ..good
    };
    assert!(
        !half_left.covers_domain(),
        "yarım sol-omuz kabul edildi — `&&` `||`'a dönmüş olabilir"
    );

    // Sağ omuz YARIM: c==MAX ama d!=MAX.
    let half_right = VarThresholds {
        high: Trap {
            d: 5000,
            ..good.high
        },
        ..good
    };
    assert!(
        !half_right.covers_domain(),
        "yarım sağ-omuz kabul edildi — `&&` `||`'a dönmüş olabilir"
    );
}

/// MUTANT `fusion.rs:143:82` — `self.med.d > self.high.a` → `>=` (ve simetrik `low.d > med.a`).
#[test]
fn covers_domain_requires_strict_overlap_between_neighbours() {
    // [KOD] yorumu normatif: "Komşu setler KESİN-örtüşmeli (`>`; `≥` YETMEZ — trap kenarları
    // dışlayıcı: low.d==med.a noktasında x=low.d için μL=0 ∧ μM=0 = tek-nokta zero-weight
    // deliği, workflow-bulgusu)."
    let base = VarThresholds {
        low: Trap {
            a: 0,
            b: 0,
            c: 100,
            d: 400,
        },
        med: Trap {
            a: 300,
            b: 400,
            c: 600,
            d: 800,
        },
        high: Trap {
            a: 700,
            b: 900,
            c: MAX,
            d: MAX,
        },
    };
    assert!(base.covers_domain());

    // med.d == high.a → TAM DEĞME, örtüşme YOK → reddedilmeli.
    let touching = VarThresholds {
        med: Trap { d: 700, ..base.med },
        ..base
    };
    assert!(
        !touching.covers_domain(),
        "med.d == high.a tek-nokta deliği kabul edildi — `>` `>=`'e dönmüş olabilir"
    );

    // Bir birim örtüşme yeterli.
    let barely = VarThresholds {
        med: Trap { d: 701, ..base.med },
        ..base
    };
    assert!(barely.covers_domain(), "bir birimlik örtüşme reddedildi");

    // low.d == med.a de aynı şekilde reddedilmeli (simetrik kol).
    let touching_left = VarThresholds {
        low: Trap { d: 300, ..base.low },
        ..base
    };
    assert!(
        !touching_left.covers_domain(),
        "low.d == med.a tek-nokta deliği kabul edildi"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// Vakumsuzluk
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn linear_config_actually_produces_the_intended_memberships() {
    // Bu dosyadaki her test, sentetik config'in `mu(500) == 500` verdiği varsayımına
    // dayanıyor. Rampa tanımı değişirse tüm sınır testleri sessizce anlamsızlaşır.
    let v = var();
    assert_eq!(v.low.mu(0), PERMILLE, "sol-omuz x=0'da tavan olmalı");
    assert_eq!(v.low.mu(500), PERMILLE / 2, "sol-omuz x=500'de yarı olmalı");
    assert_eq!(v.low.mu(1000), 0, "sol-omuz x=1000'de sıfır olmalı");
    assert_eq!(v.high.mu(0), 0, "sağ-omuz x=0'da sıfır olmalı");
    assert_eq!(
        v.high.mu(500),
        PERMILLE / 2,
        "sağ-omuz x=500'de yarı olmalı"
    );
    assert_eq!(v.high.mu(1000), PERMILLE, "sağ-omuz x=1000'de tavan olmalı");
    assert_eq!(v.med.mu(500), 0, "MED bu testlerde ölü olmalı");
}
