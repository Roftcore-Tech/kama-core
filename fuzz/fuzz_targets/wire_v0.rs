//! P1 wire-v0 + IdentitySeam v1: open + tüm çerçeve-decode'ları panik-siz (kabuğun ilk savunma hattı).
#![no_main]
use client_core::seam::{IdentitySeam, SecuritySeam};
use client_core::wire::{
    decode_data, decode_fec, decode_param, decode_param_ack, decode_recovered,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut seam = IdentitySeam;
    if let Ok(od) = seam.open(data) {
        let _ = decode_data(&od.plain);
        let _ = decode_fec(&od.plain);
        let _ = decode_recovered(&od.plain);
        let _ = decode_param(&od.plain);
        let _ = decode_param_ack(&od.plain);
    }
    let _ = decode_data(data);
    let _ = decode_fec(data);
    let _ = decode_recovered(data);
    let _ = decode_param(data);
    let _ = decode_param_ack(data);
});
