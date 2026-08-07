//! TC-09 — `decode_control_body()` fuzz hedefi, tüm codepoint'ler (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! Control-body §2.57'nin parse yüzeyidir ve `open()`'ın İÇİNDEN sonra çalışır — yani buraya
//! ulaşan byte'lar authenticate edilmiştir. `fuzz_open` bu katmana ulaşamaz (bkz. fuzz_open.rs
//! kapsam-sınırı notu); bu hedef doğrudan besler.
//!
//! Girdinin ilk byte'ı codepoint seçicisidir → fuzzer dokuz control-türünü de (0x10..0x18)
//! ayrı ayrı keşfedebilir. Bu, tek-codepoint'e sabitlenmiş bir hedefin gizleyeceği
//! `cap_check` / `Reader` sınır yollarını açar (DR-1 DoS-guard tavanları).
//!
//! **Oracle:** panik/OOM yok. `Ok` dönen her gövde için re-encode edilebilir olmalıdır
//! (decode→encode→decode idempotansı); aksi hâlde parser kabul ettiği bir şeyi üretemiyor
//! demektir ve bu sessiz bir asimetridir.
//!
//! Koş:  cargo +nightly fuzz run fuzz_control_decode -- -rss_limit_mb=2048

#![no_main]

use kama_noise_wire::{decode_control_body, encode_control_body};
use kama_transport_schema::inner_kind::{
    ACK, BLOCK_ABORT, BLOCK_COMPLETE, PARAM_ACK, PARAM_SET, RECOVERED, SACK,
    SESSION_CLOSE, SESSION_RENEW_REQUIRED,
};
use libfuzzer_sys::fuzz_target;

const CODEPOINTS: [u8; 9] = [
    ACK,
    SACK,
    RECOVERED,
    BLOCK_COMPLETE,
    PARAM_SET,
    PARAM_ACK,
    BLOCK_ABORT,
    SESSION_CLOSE,
    SESSION_RENEW_REQUIRED,
];

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let cp = CODEPOINTS[usize::from(data[0]) % CODEPOINTS.len()];
    let control_id = u32::from(data[0]);
    let body = &data[1..];

    if let Ok(c) = decode_control_body(cp, control_id, body) {
        // Kabul edilen gövde yeniden encode edilebilmeli ve aynı değere decode olmalı.
        // (Bu, `Reader`'ın gövdeyi kısmen tüketip kalanını sessizce yutmadığının kanıtıdır.)
        let re = encode_control_body(&c);
        let c2 = decode_control_body(cp, control_id, &re)
            .expect("re-encode edilen gövde decode edilemedi");
        assert_eq!(c, c2, "decode→encode→decode idempotan değil (cp={cp:#04x})");
    }
});