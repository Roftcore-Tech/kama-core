//! TC-09 — ~~ACK/SACK/~~RECOVERED body fuzz hedefi (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! `fuzz_control_decode` dokuz codepoint'i birden tarar; bu hedef DEĞİŞKEN-UZUNLUKLU üç türe
//! (ACK ranges, SACK gap-blocks, RECOVERED esi-ranges) odaklanır. Ayrım anlamlıdır: bu üç tür
//! wire'dan gelen bir SAYAÇ okuyup ona göre allocate eden tek yüzeydir — yani `CLAUDE.md §10`
//! `PARSER-02` borcunun ("alloc-before-check OOM → bounds-check before alloc") doğrudan
//! muhatabıdır. DR-1 tavanları (~~`ACK_MAX_RANGES`, `SACK_MAX_GAP_BLOCKS`,~~
//! `RECOVERED_MAX_ESI_RANGES`) DoS-guard'dır ve tam da bu hedefle sınanır.
//!
//! [D2 2026-10-08] ACK/SACK kolları kalktı — Ack/Sack ailesi kodda yok (K3: ürün ACK'i engine
//! `ACK_V2`); değişken-uzunluklu kalan tek tür RECOVERED. Hedef adı (`fuzz_ack_decode`)
//! Cargo.toml / fuzz-all.sh / EVIDENCE.md atıfları için KORUNDU. 0x10/0x11 seçicide RESERVED
//! olarak kalır (indeksler korunur → mevcut corpus aynı codepoint'e düşer).
//!
//! **Oracle:** panik/OOM yok; `u16::MAX` sayaç beyan eden kısa bir gövde tipli hata almalı,
//! 65 535 elemanlık bir vektör AYIRMAMALIDIR (`-rss_limit_mb=2048` bunu yakalar). RESERVED
//! 0x10/0x11 HER gövdede `non-control codepoint` ile reddedilmelidir (sayaç hiç okunmaz).
//!
//! Koş:  cargo +nightly fuzz run fuzz_ack_decode -- -rss_limit_mb=2048

#![no_main]

use kama_noise_wire::{decode_control_body, encode_control_body, WireError};
use kama_transport_schema::inner_kind::{RECOVERED, RESERVED_ACK, RESERVED_SACK};
use libfuzzer_sys::fuzz_target;

const VARLEN: [u8; 3] = [RESERVED_ACK, RESERVED_SACK, RECOVERED];

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let cp = VARLEN[usize::from(data[0]) % VARLEN.len()];
    let body = &data[1..];

    // [D2 2026-10-08] RESERVED → reject: gövdeden bağımsız, codepoint kapısında.
    if cp == RESERVED_ACK || cp == RESERVED_SACK {
        assert_eq!(
            decode_control_body(cp, 0x4142_4344, body),
            Err(WireError::ControlDecode("non-control codepoint")),
            "reserved codepoint reddedilmedi (cp={cp:#04x})"
        );
        return;
    }

    // (1) Ham gövde: sayaç-beyanı ile gerçek gövde uzunluğu arasındaki her tutarsızlık.
    if let Ok(c) = decode_control_body(cp, 0x4142_4344, body) {
        let re = encode_control_body(&c);
        assert_eq!(
            decode_control_body(cp, 0x4142_4344, &re).as_ref(),
            Ok(&c),
            "varlen decode→encode→decode idempotan değil (cp={cp:#04x})"
        );
        // Canonical-encoding kilidi: kabul edilen gövde, encoder'ın ürettiğiyle BYTE-EŞ olmalıdır.
        // Aksi hâlde aynı anlamın iki farklı wire-temsili vardır (malleability → replay/dedup riski).
        assert_eq!(
            body, &re[..],
            "non-canonical gövde kabul edildi (cp={cp:#04x})"
        );
    }
});