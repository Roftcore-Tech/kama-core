//! TC-10 — `SourcePrefix::parse()` fuzz hedefi (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! `prefix.rs` satır kapsamı %78,33 ile workspace'in en düşük ikinci değeridir (STR §3.3) ve
//! saldırgan-etkisindeki byte alan bir parser yüzeyidir: kayıp bir source RS ile reconstruct
//! edildiğinde metadata bu 11 byte'tan geri okunur — yani ATTACKER-INFLUENCED bir symbol'ün
//! prefix'i, uygulamaya sunulan `message_id`/`chunk_offset` değerlerini belirler.
//!
//! **Oracle:** panik yok; `Ok` dönen her prefix için `payload_slice()` symbol sınırları içinde
//! kalmalı ve `actual_payload_len` ile TAM uyuşmalıdır (silent-truncation kapısı —
//! `payload_slice` bugün `end.min(symbol.len())` ile kırpar; kırpma FİİLEN olursa parse
//! bounds-check'i yetersiz demektir).
//!
//! Koş:  cargo +nightly fuzz run fuzz_prefix -- -rss_limit_mb=2048

#![no_main]

use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::SOURCE_PREFIX_LEN;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(p) = SourcePrefix::parse(data) else {
        return;
    };
    let want = usize::from(p.actual_payload_len);
    let slice = p.payload_slice(data);

    // (1) parse() kabul ettiyse bounds-check GEÇMİŞ olmalı → dilim TAM beyan edilen uzunlukta.
    assert_eq!(
        slice.len(),
        want,
        "payload_slice sessizce kırptı: beyan {want}, dönen {} (symbol_len={})",
        slice.len(),
        data.len()
    );

    // (2) Dilim symbol'ün içinde ve prefix'ten SONRA başlıyor.
    assert!(SOURCE_PREFIX_LEN + want <= data.len(), "dilim symbol'ü taşıyor");

    // (3) Roundtrip: aynı payload ile aynı-boy tampona serialize → parse aynı prefix'i vermeli.
    let mut out = vec![0u8; data.len()];
    if p.serialize_symbol(slice, &mut out).is_ok() {
        assert_eq!(
            SourcePrefix::parse(&out),
            Ok(p),
            "serialize→parse roundtrip bozuk"
        );
    }
});