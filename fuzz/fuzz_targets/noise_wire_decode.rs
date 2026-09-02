//! Düşman kama-noise-wire datagram'ı: outer parse + inner parse + control decode panik-siz.
//! (AEAD-öncesi/sonrası parse yüzeyleri; doğrulama-öncesi parse SIKI sınırlı olmalı.)
#![no_main]
use libfuzzer_sys::fuzz_target;
use kama_noise_wire::control::decode_control;
use kama_noise_wire::inner::parse_inner;
use kama_noise_wire::outer::ExtendedOuter;

fuzz_target!(|data: &[u8]| {
    let _ = ExtendedOuter::parse(data);
    if let Ok(inner) = parse_inner(data) {
        let _ = decode_control(&inner);
    }
});
