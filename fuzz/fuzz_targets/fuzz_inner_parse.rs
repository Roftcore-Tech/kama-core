//! TC-08 — `parse_inner()` doğrudan fuzz hedefi (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! **Bu grubun en kritik maddesi.** STR §3.5'te ölçülen `fuzz_open` 6,9 M çalıştırma ve 592 edge
//! üretmiş, ancak `inner.rs`'in yedi hata yolunun HİÇBİRİNE ulaşamamıştır — ulaşamaz da:
//! `open()` AEAD doğrulamasını inner-parse'tan ÖNCE yapar (§4-ORDER adım-5 → adım-7) ve rastgele
//! byte'ın geçerli Poly1305 tag üretme olasılığı 2⁻¹²⁸'dir. Yani `fuzz_open`'ın tüm kapsamı
//! outer-parse + AEAD-reddi bölgesindedir.
//!
//! Bu hedef AEAD'i BAYPAS eder ve authenticate-edilmiş-ama-iyi-biçimli-OLMAYAN plaintext'i
//! doğrudan parser'a besler. `CLAUDE.md §10`'daki `PARSER-02` borcu ("alloc-before-check OOM →
//! bounds-check before alloc, take(MAX 4096)") tam olarak bu katmandadır.
//!
//! **Tehdit modeli gerekçesi:** Bu katmana ulaşan byte'lar authenticate edilmiştir — ancak
//! authenticate edilmiş olmak İYİ BİÇİMLİ olmak demek değildir. Bozuk bir gönderen, uyumsuz bir
//! sürüm ya da gelecekteki bir format değişikliği bu yolları tetikler; ve gönderen, anahtarı
//! paylaşan kimliği doğrulanmış bir eş olabilir (compromised-peer senaryosu).
//!
//! **Oracle:** `-rss_limit_mb=2048` altında panik/OOM/timeout yok. Dönen değer YALNIZ
//! `Ok(InnerPlain)` veya `Err(WireError::InnerParse(_))` olabilir — başka variant sızarsa
//! katman sınırı ihlal edilmiştir.
//!
//! Koş:  cargo +nightly fuzz run fuzz_inner_parse -- -rss_limit_mb=2048

#![no_main]

use kama_noise_wire::error::WireError;
use kama_noise_wire::inner::{parse_inner, InnerPlain};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match parse_inner(data) {
        Ok(inner) => {
            // Kabul edilen her plaintext için yapısal tutarlılık (silent-truncation kapısı):
            // parse edilen alanların uzunlukları girdi uzunluğunu AŞMAMALIDIR.
            match inner {
                InnerPlain::FecShard { symbol, .. } => {
                    assert!(
                        symbol.len() <= data.len(),
                        "symbol girdiden uzun: {} > {}",
                        symbol.len(),
                        data.len()
                    );
                }
                InnerPlain::Control { body, .. } => {
                    assert!(
                        body.len() <= data.len(),
                        "control body girdiden uzun: {} > {}",
                        body.len(),
                        data.len()
                    );
                }
            }
        }
        Err(WireError::InnerParse(_)) => {}
        // Katman sınırı: parse_inner YALNIZ InnerParse üretmelidir. Başka bir variant
        // (ör. AeadFail, EpochNotAccepted) buraya sızarsa hata-taksonomisi bozulmuştur.
        Err(other) => panic!("parse_inner beklenmeyen hata variantı üretti: {other:?}"),
    }
});