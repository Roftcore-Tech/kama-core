//! A2-F1.6 — MEMORY-MODEL DOĞRULAMA KAPISI (freeze-r2 §8; Abkarada F1-şartı).
//!
//! YÖNTEM (yalnız size_of DEĞİL — Abkarada-şartı): per-shard retained-cost bileşen-bileşen ölçülür:
//! (a) symbol-buffer GERÇEK `Vec::capacity()` + konservatif allocator-modeli (16B malloc-chunk-header
//!     + 16B'ye-yukarı-yuvarlama — glibc-sınıfı; gerçek-allocator bundan-küçükse model konservatif),
//! (b) shard-DTO (`OpenedTransportShard`) gerçek `size_of` (header+Bytes-üçlüsü+proof dahil),
//! (c) hold-slot (VecDeque-eleman = DTO-boyutu),
//! (d) dedup/ESI-girdisi: anahtar + konservatif BTree-düğüm-payı (48B),
//! (e) blok-metadata AYRI-modellidir (BLOCK_META_COST=512B; C-3'te ayrı-kalem — per-shard'a girmez).
//! KAPI: ölçülen(1024) > 1536B ise bu test KIRMIZI → DUR-B → cap'ler AŞAĞI yeniden-türetilir;
//! SEALED-cap sessizce korunmaz. GEVŞEME YÖNÜ YOK (ölçülen küçükse cap BÜYÜTÜLMEZ).

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::OpenedTransportShard;
use std::mem::size_of;

/// Konservatif allocator-sınıfı: 16B chunk-header + 16B hizalama-yuvarlaması (glibc-malloc-sınıfı).
fn alloc_class(bytes: usize) -> usize {
    (bytes + 16).div_ceil(16) * 16
}

/// Konservatif BTree-düğüm-payı (amortize; düğüm-başına-11-girdi × ~512B-düğüm ≈ 47 → 48).
const BTREE_NODE_SHARE: usize = 48;

/// Model-sabitleri (freeze-r2 §2 — SEALED-türetimin girdileri).
const MODEL_SYMBOL_1024: usize = 1280; // 1024 × 1.25
const MODEL_H_SHARD: usize = 256;
const MODEL_TOTAL_1024: usize = 1536;
const MODEL_SYMBOL_64: usize = 80; // 64 × 1.25

#[test]
fn f1_6_memory_model_gate_e1024() {
    // (a) symbol-buffer: open()-ürünü Vec (len=E); GERÇEK capacity ölçülür.
    let symbol: Vec<u8> = vec![0u8; 1024];
    let measured_symbol = alloc_class(symbol.capacity());
    assert!(
        measured_symbol <= MODEL_SYMBOL_1024,
        "symbol-kolu modeli aştı: {measured_symbol} > {MODEL_SYMBOL_1024} → DUR-B (cap-AŞAĞI)"
    );

    // (b)+(c)+(d) H_SHARD bileşenleri:
    let dto = size_of::<OpenedTransportShard>();
    let hold_slot = dto; // VecDeque<OpenedTransportShard> eleman-yuvası
    let dedup_entry = size_of::<(u16, bool)>() + BTREE_NODE_SHARE;
    let measured_h = dto + hold_slot + dedup_entry;
    assert!(
        measured_h <= MODEL_H_SHARD,
        "H_SHARD-kolu modeli aştı: dto={dto} hold={hold_slot} dedup={dedup_entry} → {measured_h} > {MODEL_H_SHARD} → DUR-B"
    );

    // TOPLAM-KAPI:
    let measured_total = measured_symbol + measured_h;
    assert!(
        measured_total <= MODEL_TOTAL_1024,
        "SHARD_RETAINED_COST(1024) modeli aştı: {measured_total} > {MODEL_TOTAL_1024} → DUR-B (TOTAL_SHARD_CAP ve bağımlı bound'lar AŞAĞI yeniden-türetilir)"
    );
}

#[test]
fn f1_6_memory_model_e64_component() {
    let symbol: Vec<u8> = vec![0u8; 64];
    let measured = alloc_class(symbol.capacity());
    assert!(
        measured <= MODEL_SYMBOL_64,
        "E=64 symbol-kolu: {measured} > {MODEL_SYMBOL_64} (bilgi-kalemi; cap'ler worst-case-1024'ten)"
    );
}
