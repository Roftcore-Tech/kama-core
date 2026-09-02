//! Düşman datagram → gerçek AEAD protector.open: asla panik etmez, her zaman typed hata/Ok.
//! Sabit stub anahtar (TEST-ONLY; handshake iddiası yok) — deterministik.
#![no_main]
use libfuzzer_sys::fuzz_target;
use kama_noise_wire::protector::{KamaNoiseWireProtector, KEY_LEN};
use zeroize::Zeroizing;

fuzz_target!(|data: &[u8]| {
    let k = Zeroizing::new([0x11u8; KEY_LEN]);
    let mut p = KamaNoiseWireProtector::new(1, &k, &k);
    let _ = p.open(data);
});
