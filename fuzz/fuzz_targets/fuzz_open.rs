//! TC-09 — `KamaNoiseWireProtector::open()` fuzz hedefi (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! **Kapsam sınırı (NORMATİF — bu hedefin ne KANITLAMADIĞI):** `open()` AEAD doğrulamasını
//! inner-parse'tan önce yapar. Rastgele byte'ın geçerli Poly1305 tag üretme olasılığı 2⁻¹²⁸
//! olduğundan bu hedef `inner.rs`'e YAPISAL OLARAK ULAŞAMAZ. STR §3.5'te ölçülen 6,9 M çalıştırma
//! / 592 edge tamamen outer-parse + h_ext-guard + epoch-kabul + PN-precheck + AEAD-reddi
//! bölgesindedir. Inner katman `fuzz_inner_parse` (TC-08) ile ayrıca fuzzlanır.
//! "Fuzz hedefimiz var" demek "parser fuzzlandı" demek DEĞİLDİR; bu ayrım yapılmazsa yanlış
//! güvenlik hissi üretir (BUL-04).
//!
//! **Bugünkü değeri** bir hata bulmak değil, ölçülen temiz durumu KİLİTLEMEKTİR: corpus commit
//! edilmezse her koşum sıfırdan başlar ve regresyon koruması oluşmaz.
//!
//! **Oracle:** panik/OOM/timeout yok; `Ok` dönerse — ki anahtarsız girdiyle dönmemelidir —
//! bu bir forgery'dir ve hedef düşer.
//!
//! Koş:  cargo +nightly fuzz run fuzz_open -- -rss_limit_mb=2048

#![no_main]

use kama_noise_wire::KamaNoiseWireProtector;
use libfuzzer_sys::fuzz_target;
use std::cell::RefCell;
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;

thread_local! {
    // Protector recv-window state TAŞIR; her çağrıda yeniden kurmak PN-precheck yollarını
    // (too-old / duplicate / future-gap) körleştirirdi. Thread-local ile state korunur →
    // fuzzer ardışık girdilerle pencere geçmişi biriktirebilir.
    static PROTECTOR: RefCell<KamaNoiseWireProtector> = RefCell::new(new_protector());
}

fn new_protector() -> KamaNoiseWireProtector {
    let mut p = KamaNoiseWireProtector::new(
        H_EXT,
        &Zeroizing::new([0x11u8; 32]),
        &Zeroizing::new([0x22u8; 32]),
    );
    // Epoch-1 anahtarlarını da kur: aksi hâlde dh_epoch=1 taşıyan her girdi EpochNotAccepted ile
    // erken düşer ve epoch-1 PN-penceresi hiç fuzzlanmaz.
    p.install_epoch1_keys(
        &Zeroizing::new([0x33u8; 32]),
        &Zeroizing::new([0x44u8; 32]),
    );
    p
}

fuzz_target!(|data: &[u8]| {
    PROTECTOR.with(|cell| {
        let mut p = cell.borrow_mut();
        if let Ok(opened) = p.open(data) {
            // Anahtarsız girdiyle open() BAŞARILI OLAMAZ. Olduysa AEAD forgery'dir.
            panic!("FORGERY: open() rastgele girdiyi kabul etti: {opened:?}");
        }
        // Pencere-durumu şişerse (uzun koşumda bellek) yeniden kur — OOM'un state-birikiminden
        // değil, gerçek bir alloc hatasından gelmesini sağlar.
        if data.first() == Some(&0xFF) && data.len() == 1 {
            *p = new_protector();
        }
    });
});