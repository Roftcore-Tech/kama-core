//! P1 wire-v0 + IdentitySeam: open + decode_data panik-siz (kabuğun ilk savunma hattı).
#![no_main]
use libfuzzer_sys::fuzz_target;
use client_core::seam::{IdentitySeam, SecuritySeam};
use client_core::wire::decode_data;

fuzz_target!(|data: &[u8]| {
    if let Some(plain) = IdentitySeam.open(data) {
        let _ = decode_data(&plain);
    }
    let _ = decode_data(data);
});
