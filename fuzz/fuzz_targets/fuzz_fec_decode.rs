//! TC-10 — `FecCodec::reconstruct()` fuzz hedefi (SR-STR-CORE-001 §5.2, BUL-04).
//!
//! `codec.rs` region kapsamı %89,24 ile workspace'in en düşüğüdür (STR §3.3). Bu hedef
//! `INV-FEC-1`'in DECODER tarafındaki savunmasını ölçer: authenticate edilmiş ama tutarsız
//! `(k, r, esi, is_repair, known_zero)` kombinasyonlarında motorun fail-closed olduğu.
//!
//! **Neden `Arbitrary` ile yapısal girdi:** ham byte akışı reconstruct'ın imza-uyumlu bir
//! `&[ReceivedSymbol]` üretmesini beklemez; `symbol_len == E` kapısı ham-byte'ta neredeyse her
//! zaman ilk adımda düşer ve derinlik oluşmaz. Yapılandırılmış girdi ESI-uzayını, dup'ları ve
//! known_zero sınırını fiilen tarar.
//!
//! **Oracle:** panik/OOM yok; dönüş ya tipli `FecError` ya da `Ok(Vec<RecoveredSource>)`.
//! `Ok` dönerse her recovered ESI `[0, K_actual)` içinde olmalı ve `bytes.len() == E` olmalıdır —
//! zero-inject edilmiş `[K_actual, K_shape)` slot'ları uygulamaya SIZMAMALIDIR (§6 Model-A).
//!
//! Koş:  cargo +nightly fuzz run fuzz_fec_decode -- -rss_limit_mb=2048

#![no_main]

use arbitrary::Arbitrary;
use kama_fec_codec::codec::{FecCodec, ReceivedSymbol, RsSimdCodec};
use kama_fec_codec::profile::PROFILES;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    profile_idx: u8,
    known_zero: u8,
    /// (esi, is_repair, symbol_is_correct_len) — symbol içeriği decode-kabulünü etkilemez,
    /// UZUNLUĞU etkiler; bu yüzden içerik yerine uzunluk-doğruluğu fuzzlanır.
    shards: Vec<(u16, bool, bool)>,
}

fuzz_target!(|input: Input| {
    // E=1024 profilleri ×64 shard'a kadar tampon ayırmak fuzz bütçesini yakar; küçük-E'ye sınırla.
    let small: Vec<_> = PROFILES.iter().filter(|p| p.e <= 64).collect();
    if small.is_empty() || input.shards.len() > 64 {
        return;
    }
    let p = small[usize::from(input.profile_idx) % small.len()];
    let e = p.e_usize();

    let full = vec![0xA5u8; e];
    let short = vec![0xA5u8; e.saturating_sub(1)];
    let received: Vec<ReceivedSymbol> = input
        .shards
        .iter()
        .map(|&(esi, is_repair, correct_len)| ReceivedSymbol {
            esi,
            is_repair,
            bytes: if correct_len { &full } else { &short },
        })
        .collect();

    let Ok(out) = RsSimdCodec.reconstruct(p, &received, usize::from(input.known_zero)) else {
        return; // tipli hata = beklenen fail-closed davranış
    };

    // §6 Model-A: known-zero slot'ları uygulamaya verilmez, her recovered TAM-E'dir.
    let k_actual = p.k_shape_usize().saturating_sub(usize::from(input.known_zero));
    for rec in &out {
        assert!(
            usize::from(rec.esi) < k_actual,
            "zero-pad bölgesindeki slot uygulamaya sızdı: esi={} k_actual={k_actual}",
            rec.esi
        );
        assert_eq!(rec.bytes.len(), e, "recovered symbol TAM-E değil");
    }
    // ESI'ler artan ve ayrık olmalı (run_decode sort_by_key sözleşmesi).
    assert!(
        out.windows(2).all(|w| w[0].esi < w[1].esi),
        "recovered ESI'ler artan-ayrık değil"
    );
});