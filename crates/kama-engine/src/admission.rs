//! 5d-2 decoder-admission köprüsü (INV-FEC-1 production-kapanışı).
//!
//! `kama-integration/tests/pipeline.rs::receiver_open_decode`'un ÜRETİM-halefi: `OpenedTransportShard`
//! (noise-wire AEAD-witness) → FEC-decode. **TYPE-GATE:** bu fonksiyonun girdisi YALNIZ
//! `OpenedTransportShard`'tır; onu ÜRETMEnin tek-yolu `protector.open()` (AEAD-verify). Forge/unauthenticated
//! datagram open()'da `AeadFail` alır → `OpenedTransportShard` ÜRETİLEMEZ → bu köprüye ULAŞAMAZ (authenticate-
//! before-decode, INV-FEC-1). Ek: **profil-gate** — shard.profile_id ≠ bound-profil → fail-closed (PARAM-bağı).
//!
//! **N8-firewall:** bu köprü decode-başarısını (RECOVERED) BtlBw/delivered-EAI/credit'e ASLA beslemez —
//! yalnız app-bytes döndürür (logical-teslim). Kredi yalnız gerçek-wire-ACK (sender.bw.rs, mühürlü).

use crate::error::EngineError;
use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec};
use kama_noise_wire::{InnerPlain, OpenedTransportShard, SensitiveSymbolBytes};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;

/// Authenticated-shard'ları (bound-profil) FEC-decode + reassemble → app-bytes.
///
/// Type-gate: girdi `Vec<OpenedTransportShard>` (open()-üretimi; forge erişemez). Profil-gate:
/// her shard.profile_id == `bound_profile_id` (aksi fail-closed). `known_zero = k_shape − k_actual`.
///
/// # Errors
/// [`EngineError::UnknownProfile`] by_id-miss · [`EngineError::ProfileMismatch`] shard-profil≠bound ·
/// [`EngineError::FecDecode`] reconstruct fail-closed · [`EngineError::Reassembly`] eksik-source/prefix/`k_actual>k_shape`.
pub fn admit_and_decode(
    shards: Vec<OpenedTransportShard>,
    bound_profile_id: u8,
    k_actual: u8,
) -> Result<Vec<u8>, EngineError> {
    let profile = by_id(bound_profile_id).ok_or(EngineError::UnknownProfile(bound_profile_id))?;
    let k_shape = profile.k_shape as usize;
    let known_zero = k_shape
        .checked_sub(usize::from(k_actual))
        .ok_or(EngineError::Reassembly("k_actual > k_shape"))?;

    // Open-edilmiş (authenticated) shard'lardan symbol topla (owned; sonra ReceivedSymbol referansları).
    // A-5.1 (DR-CS-1b/A2; Abkarada dar type-propagation): SensitiveSymbolBytes MOVE edilir (KOPYALANMAZ);
    // decode-sonrası bu Vec'ler drop'ta zeroize olur. TYPE-GATE/profil/INV-FEC-1/N8 DEĞİŞMEZ.
    let mut source_syms: Vec<(u16, SensitiveSymbolBytes)> = Vec::new();
    let mut repair_syms: Vec<(u16, SensitiveSymbolBytes)> = Vec::new();
    for shard in shards {
        match shard.into_inner() {
            InnerPlain::FecShard { header, symbol } => {
                // Profil-gate (PARAM-bağı fail-closed): yanlış-profil-shard decode'a giremez.
                if header.profile_id != bound_profile_id {
                    return Err(EngineError::ProfileMismatch {
                        shard: header.profile_id,
                        bound: bound_profile_id,
                    });
                }
                match header.kind {
                    ShardKind::Source => source_syms.push((header.esi, symbol)),
                    ShardKind::Repair => repair_syms.push((header.esi, symbol)),
                }
            }
            InnerPlain::Control { .. } => {} // control decode-yolunda değil (yok-say)
        }
    }

    // FEC reconstruct (Model-A known-zero-inject; dup-esi/yetersiz → fail-closed FecError).
    let codec = RsSimdCodec;
    let mut recv: Vec<ReceivedSymbol> = Vec::new();
    for (esi, bytes) in &source_syms {
        recv.push(ReceivedSymbol {
            esi: *esi,
            is_repair: false,
            bytes: bytes.as_ref(),
        });
    }
    for (esi, bytes) in &repair_syms {
        recv.push(ReceivedSymbol {
            esi: *esi,
            is_repair: true,
            bytes: bytes.as_ref(),
        });
    }
    let recovered = codec.reconstruct(&profile, &recv, known_zero)?;

    // Tüm K_actual source'u topla (alınan + kurtarılan) → reassemble (canonical prefix).
    // A-5.1: all_source de SensitiveSymbolBytes taşır (reconstructed-symbol da zeroize-sahibi olur;
    // recovered codec-Vec'i SensitiveSymbolBytes'a SARILIR — ham-Vec dışarı-akmaz). Reassembly/prefix-
    // parse borrow'la (&[u8] deref) çalışır; decode-control-flow DEĞİŞMEZ.
    let mut all_source: Vec<Option<SensitiveSymbolBytes>> = Vec::new();
    all_source.resize_with(usize::from(k_actual), || None);
    for (esi, bytes) in source_syms {
        if (esi as usize) < usize::from(k_actual) {
            all_source[esi as usize] = Some(bytes);
        }
    }
    for r in recovered {
        if (r.esi as usize) < usize::from(k_actual) {
            all_source[r.esi as usize] = Some(SensitiveSymbolBytes::from(r.bytes));
        }
    }

    let mut app = Vec::new();
    for slot in all_source {
        let sym = slot.ok_or(EngineError::Reassembly("missing source after decode"))?;
        let pfx = SourcePrefix::parse(&sym).map_err(|_| EngineError::Reassembly("prefix parse"))?;
        app.extend_from_slice(pfx.payload_slice(&sym));
    }
    Ok(app)
}
