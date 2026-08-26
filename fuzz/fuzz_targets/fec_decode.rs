//! Düşman FEC shard çerçevesi: decode asla panik etmez (PARSER-02 bounds-before-read).
#![no_main]
use libfuzzer_sys::fuzz_target;
use adaptive_transport::fec::FecShardFrame;

fuzz_target!(|data: &[u8]| {
    let _ = FecShardFrame::decode(data);
});
