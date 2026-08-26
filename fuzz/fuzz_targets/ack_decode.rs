//! Düşman ACK çerçevesi: decode + acked_full asla panik etmez, sınırsız tahsis yapmaz.
#![no_main]
use libfuzzer_sys::fuzz_target;
use adaptive_transport::ack::AckFrame;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = AckFrame::decode(data) {
        let _ = frame.ack_delay_duration();
        // acked_full farklı expected tabanlarıyla (u16 sınır bölgeleri dahil).
        for expected in [0u64, 1, 65_535, 65_536, u64::MAX / 2, u64::MAX] {
            let _ = frame.acked_full(expected);
        }
    }
});
