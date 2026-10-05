//! P1 wire-v0 + IdentitySeam (S3 PacketProtection): open + tüm çerçeve-decode'ları panik-siz (kabuğun ilk savunma hattı).
#![no_main]
use client_core::seam::IdentitySeam;
use client_core::wire::{
    decode_data, decode_fec, decode_param, decode_param_ack, decode_recovered,
};
use kama_contracts::s3::PacketProtection;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut seam = IdentitySeam::new();
    if let Some(od) = seam.open(data) {
        let _ = decode_data(&od.frame);
        let _ = decode_fec(&od.frame);
        let _ = decode_recovered(&od.frame);
        let _ = decode_param(&od.frame);
        let _ = decode_param_ack(&od.frame);
    }
    let _ = decode_data(data);
    let _ = decode_fec(data);
    let _ = decode_recovered(data);
    let _ = decode_param(data);
    let _ = decode_param_ack(data);
});
