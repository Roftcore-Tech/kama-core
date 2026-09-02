//! DR-FEC-WORKLOAD-1 (RATIFIED) — canonical 8×300B application workload → 48 E=64 source-symbols.
//! HARNESS-ONLY (experimental FS path; production application-fragmenter YOK — §3). Production SourcePrefix
//! FORMAT'ı kullanılır (kama-transport-schema; prefix 11B + payload≤53 + canonical_zero_pad = E=64).
//! global_source_fragment_id (0..47) arm-local codec block/esi'den BAĞIMSIZ (RULE-4; §5).
#![allow(dead_code)]

use kama_transport_schema::prefix::{SourcePrefix, SOURCE_FLAG_FINAL};
use kama_transport_schema::SOURCE_PREFIX_LEN;
use std::collections::BTreeMap;

pub const APP_UNIT_BYTES: usize = 300;
pub const APP_UNITS: usize = 8;
pub const TOTAL_APP_BYTES: usize = APP_UNIT_BYTES * APP_UNITS; // 2400
pub const E: usize = 64;
pub const APP_CAP: usize = E - SOURCE_PREFIX_LEN; // 53
pub const FRAGS_PER_UNIT: usize = 6; // ceil(300/53) = 6
pub const TOTAL_SOURCE_SYMBOLS: usize = APP_UNITS * FRAGS_PER_UNIT; // 48

/// Bir canonical source-symbol (global-kimlikli; E=64 sealed-öncesi plaintext-symbol).
#[derive(Clone, Debug)]
pub struct CanonicalSymbol {
    /// RULE-4: partition-BAĞIMSIZ global-kimlik (0..47). Arm-local (block_id,esi) = codec-layout-metadata (AYRI).
    pub global_source_fragment_id: u16,
    pub app_unit_index: u8,   // 0..7
    pub fragment_in_unit: u8, // 0..5
    pub message_id: u32,
    pub chunk_offset: u32,
    pub actual_payload_len: u16,
    pub is_final: bool,
    /// E=64 canonical symbol (prefix 11 + payload + canonical_zero_pad).
    pub symbol_bytes: Vec<u8>,
}

/// 8 DISTINCT app-unit (deterministik-içerik; message_id = 0xA0000000 + unit_index; §3 "eight distinct identities").
#[must_use]
pub fn app_units() -> Vec<Vec<u8>> {
    (0..APP_UNITS)
        .map(|u| {
            (0..APP_UNIT_BYTES)
                .map(|b| (u.wrapping_mul(131).wrapping_add(b.wrapping_mul(7))) as u8)
                .collect()
        })
        .collect()
}

#[must_use]
pub fn message_id_of(unit: usize) -> u32 {
    0xA000_0000u32 + unit as u32
}

/// **RATIFIED §2 fragmentation:** her 300B app-unit BAĞIMSIZ fragmentlenir → 6 symbol. offsets {0,53,106,159,212,265};
/// len {53,53,53,53,53,35}; son-fragment SOURCE_FLAG_FINAL. Padding coded+authenticated-symbol İÇİNDE. Global-order
/// unit-major (unit0→gid 0..5, unit1→gid 6..11, …).
#[must_use]
pub fn canonical_workload() -> Vec<CanonicalSymbol> {
    let units = app_units();
    let mut out = Vec::with_capacity(TOTAL_SOURCE_SYMBOLS);
    let mut gid: u16 = 0;
    for (u, app) in units.iter().enumerate() {
        for f in 0..FRAGS_PER_UNIT {
            let offset = f * APP_CAP; // 0,53,106,159,212,265
            let len = (APP_UNIT_BYTES - offset).min(APP_CAP); // 53×5 + 35
            let is_final = f == FRAGS_PER_UNIT - 1;
            let pfx = SourcePrefix {
                message_id: message_id_of(u),
                chunk_offset: offset as u32,
                actual_payload_len: len as u16,
                source_flags: if is_final { SOURCE_FLAG_FINAL } else { 0 },
            };
            let mut sym = vec![0u8; E];
            pfx.serialize_symbol(&app[offset..offset + len], &mut sym)
                .expect("canonical fragment serialize");
            out.push(CanonicalSymbol {
                global_source_fragment_id: gid,
                app_unit_index: u as u8,
                fragment_in_unit: f as u8,
                message_id: message_id_of(u),
                chunk_offset: offset as u32,
                actual_payload_len: len as u16,
                is_final,
                symbol_bytes: sym,
            });
            gid += 1;
        }
    }
    out
}

/// **RATIFIED §2 reassembly:** symbol'lerden app-unit'leri geri-kur (message_id-grup + chunk_offset-sıra + payload-concat).
/// SADECE production SourcePrefix::parse/payload_slice kullanır. Döner: reassembled app-unit'ler (message_id-sırasında).
#[must_use]
pub fn reassemble(symbols: &[CanonicalSymbol]) -> Vec<Vec<u8>> {
    let mut by_msg: BTreeMap<u32, BTreeMap<u32, Vec<u8>>> = BTreeMap::new();
    for s in symbols {
        let pfx = SourcePrefix::parse(&s.symbol_bytes).expect("parse canonical symbol");
        let payload = pfx.payload_slice(&s.symbol_bytes).to_vec();
        by_msg
            .entry(pfx.message_id)
            .or_default()
            .insert(pfx.chunk_offset, payload);
    }
    by_msg
        .into_values()
        .map(|chunks| chunks.into_values().flatten().collect())
        .collect()
}

/// Canonical source-sequence deterministik hash (48-symbol + global-id; FNV-1a; RNG-YOK).
#[must_use]
pub fn sequence_hash(symbols: &[CanonicalSymbol]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for s in symbols {
        for &b in &s.symbol_bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= u64::from(s.global_source_fragment_id);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
