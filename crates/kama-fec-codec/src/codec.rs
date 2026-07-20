//! FecCodec trait + reed-solomon-simd adapter (fec-core §2). OPAQUE-symbol API — Noise/AEAD/nonce/
//! key TİPLERİ SIZMAZ (K-FEC-SOC-1/2). symbol-byte plaintext/ciphertext FARK-ETMEZ.
//!
//! İki motor: [`RsSimdCodec`] (stateless — her-çağrı taze-engine) ve [`RsSimdEngine`] (Sprint-3
//! engine-reuse — profile-başına `reset()` ile allocation-yeniden-kullanım). İkisi de AYNI çekirdek-
//! mantığı paylaşır → byte-eş (golden-teyitli).
//!
//! **SPRINT-3 ÖLÇÜM BULGUSU (D0-hipotezi ÇÜRÜDÜ):** D0, reconstruct'ın ~1000×-encode olmasını
//! "decoder-İNŞA-baskın" varsaymış + reuse-ZORUNLU demişti. Ölçüm (measure.rs) reuse'un **hızlanma
//! VERMEDİĞİNİ** gösterdi (~1.0×): maliyet `new()`/`reset()`-inşa değil, decode-İŞLEMİNİN-kendisinde
//! (reed-solomon-simd FFT-tabanlı decode fixed-cost'u; küçük-K/R'de amortize-olmaz). `RsSimdEngine`
//! yine de KORUNUR (doğru, byte-eş, alloc'u azaltır); decode-fixed-cost ayrı-optimizasyon-takibi
//! (reports/fec-poc/README.md Sprint-3-bulgusu; büyük-E'de amortize: E=1024 enc ~8.7 GB/s).

use crate::error::FecError;
use crate::profile::FecProfile;
use std::cell::RefCell;

use reed_solomon_simd::{ReedSolomonDecoder, ReedSolomonEncoder};

/// Kaynak-shard (encode girdisi). esi ∈ [0, K_shape). bytes.len() == E.
#[derive(Debug, Clone)]
pub struct SourceSymbol<'a> {
    pub esi: u16,
    pub bytes: &'a [u8],
}

/// Repair-shard (encode çıktısı). esi ∈ [0, R_code_max). bytes.len() == E.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairSymbol {
    pub esi: u16,
    pub bytes: Vec<u8>,
}

/// Alıcıya ulaşan authenticated-shard (decode girdisi). is_repair ile source/repair ayrımı.
#[derive(Debug, Clone)]
pub struct ReceivedSymbol<'a> {
    pub esi: u16,
    pub is_repair: bool,
    pub bytes: &'a [u8],
}

/// Reconstruct edilen kayıp-source. esi = source-index; bytes = E-byte canonical symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredSource {
    pub esi: u16,
    pub bytes: Vec<u8>,
}

/// Kripto-agnostik FEC codec sözleşmesi (fec-core §2).
pub trait FecCodec {
    /// K_shape source-symbol → R repair-symbol (R ≤ R_code_max). Model-A: known-zero-slot'lar da
    /// source-set'e girer (çağıran doldurur); bu API tam K_shape source bekler.
    ///
    /// # Errors
    /// Profile-bound / source-count / symbol-len / RS-engine.
    fn encode(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
    ) -> Result<Vec<RepairSymbol>, FecError>;

    /// Admitted-distinct source+repair → kayıp-source reconstruct. known_zero_count canonical-zero
    /// bilinen-source enjekte edilir (Model-A; §6). Decode-koşulu: admitted + known_zero ≥ K_shape.
    ///
    /// # Errors
    /// Profile-bound / dup-ESI / ESI-range / insufficient / RS-engine.
    fn reconstruct(
        &self,
        profile: &FecProfile,
        received: &[ReceivedSymbol<'_>],
        known_zero_count: usize,
    ) -> Result<Vec<RecoveredSource>, FecError>;

    /// Tek repair-ESI'yi on-demand DETERMİNİSTİK yeniden-hesaplar (§6, on-demand-recompute-netleştirme):
    /// `recompute_repair(esi)` == `encode()[esi]` == golden-vector (aynı-immutable-source-set + aynı-profile).
    /// SERBEST-olan budur; YASAK-olan farklı-profile/matrix/ordering-ile-yeniden-encode.
    ///
    /// **POC-BULGUSU (§6):** reed-solomon-simd tüm-R recovery-shard'ı tek `encode()` çağrısında üretir; bu
    /// backend'de gerçek-tek-ESI-compute-saving YOKtur (encode-then-select). On-demand-repair'in faydası
    /// latency/scheduling/bellek-tutma-ertelemesi, CPU-tasarrufu-DEĞİL (R≤8 küçük). N7-PMTU ile ortak.
    ///
    /// # Errors
    /// esi ≥ R_code_max → [`FecError::EsiOutOfRange`]; ayrıca [`FecCodec::encode`] hataları.
    fn recompute_repair(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
        esi: u16,
    ) -> Result<RepairSymbol, FecError> {
        if usize::from(esi) >= profile.r_code_max as usize {
            return Err(FecError::EsiOutOfRange {
                esi,
                kind: "repair",
            });
        }
        let repairs = self.encode(profile, source)?;
        repairs
            .into_iter()
            .find(|rs| rs.esi == esi)
            .ok_or(FecError::EsiOutOfRange {
                esi,
                kind: "repair",
            })
    }
}

fn rs_err(e: impl core::fmt::Display) -> FecError {
    FecError::RsEngine(e.to_string())
}

// ── Ortak çekirdek (stateless + reuse motorları paylaşır) ──

/// Source'u ESI 0..K sırasına dizer + doğrular (symbol_len==E, range, dup, tam-K_shape).
fn ordered_source<'a>(
    profile: &FecProfile,
    source: &[SourceSymbol<'a>],
) -> Result<Vec<&'a [u8]>, FecError> {
    let k = profile.k_shape_usize();
    let e = profile.e_usize();
    if source.len() != k {
        return Err(FecError::SourceCount {
            got: source.len(),
            expected: k,
        });
    }
    let mut ordered: Vec<Option<&[u8]>> = vec![None; k];
    for s in source {
        if s.bytes.len() != e {
            return Err(FecError::SymbolLen {
                got: s.bytes.len(),
                expected: e,
            });
        }
        let idx = usize::from(s.esi);
        if idx >= k {
            return Err(FecError::EsiOutOfRange {
                esi: s.esi,
                kind: "source",
            });
        }
        if ordered[idx].is_some() {
            return Err(FecError::DuplicateEsi(s.esi));
        }
        ordered[idx] = Some(s.bytes);
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(i, slot)| {
            slot.ok_or(FecError::EsiOutOfRange {
                esi: i as u16,
                kind: "source-gap",
            })
        })
        .collect()
}

/// Hazır (reset-edilmiş) encoder'la R repair üretir.
fn run_encode(
    enc: &mut ReedSolomonEncoder,
    ordered: &[&[u8]],
) -> Result<Vec<RepairSymbol>, FecError> {
    for bytes in ordered {
        enc.add_original_shard(bytes).map_err(rs_err)?;
    }
    let result = enc.encode().map_err(rs_err)?;
    Ok(result
        .recovery_iter()
        .enumerate()
        .map(|(i, rec)| RepairSymbol {
            esi: i as u16,
            bytes: rec.to_vec(),
        })
        .collect())
}

/// Alınan-shard'ları doğrular + Model-A hazırlar. Döner: (real_src, rep, k_actual, herhangi-kayıp-var-mı).
#[allow(clippy::type_complexity)]
fn prepare_received<'a>(
    profile: &FecProfile,
    received: &[ReceivedSymbol<'a>],
    known_zero_count: usize,
) -> Result<(Vec<(usize, &'a [u8])>, Vec<(usize, &'a [u8])>, usize, bool), FecError> {
    let k = profile.k_shape_usize();
    let e = profile.e_usize();
    let r = profile.r_code_max as usize;

    // K_actual = K_shape − known_zero_count (§6 Model-A; B4: 1 ≤ K_actual ≤ K_shape).
    if known_zero_count >= k {
        return Err(FecError::KnownZeroTooLarge {
            known_zero: known_zero_count,
            k_shape: k,
        });
    }
    let k_actual = k - known_zero_count;

    let mut src_seen = vec![false; k_actual];
    let mut rep_seen = vec![false; r];
    let mut src: Vec<(usize, &[u8])> = Vec::new();
    let mut rep: Vec<(usize, &[u8])> = Vec::new();
    for s in received {
        if s.bytes.len() != e {
            return Err(FecError::SymbolLen {
                got: s.bytes.len(),
                expected: e,
            });
        }
        let idx = usize::from(s.esi);
        if s.is_repair {
            if idx >= r {
                return Err(FecError::EsiOutOfRange {
                    esi: s.esi,
                    kind: "repair",
                });
            }
            if rep_seen[idx] {
                return Err(FecError::DuplicateEsi(s.esi));
            }
            rep_seen[idx] = true;
            rep.push((idx, s.bytes));
        } else {
            if idx >= k {
                return Err(FecError::EsiOutOfRange {
                    esi: s.esi,
                    kind: "source",
                });
            }
            // Real-source YALNIZ [0, K_actual)'da yaşar (§6 fail-closed).
            if idx >= k_actual {
                return Err(FecError::SourceInZeroRegion {
                    esi: s.esi,
                    k_actual,
                });
            }
            if src_seen[idx] {
                return Err(FecError::DuplicateEsi(s.esi));
            }
            src_seen[idx] = true;
            src.push((idx, s.bytes));
        }
    }
    // Decode-koşulu (§6): admitted_source + admitted_repair + known_zero ≥ K_shape.
    let admitted = src.len() + rep.len();
    if admitted + known_zero_count < k {
        return Err(FecError::Insufficient {
            admitted,
            known_zero: known_zero_count,
            k_shape: k,
        });
    }
    let any_missing = src.len() < k_actual;
    Ok((src, rep, k_actual, any_missing))
}

/// Hazır (reset-edilmiş) decoder'la kayıp-source reconstruct eder (Model-A zero-inject [K_actual,K_shape)).
fn run_decode(
    dec: &mut ReedSolomonDecoder,
    k: usize,
    e: usize,
    k_actual: usize,
    src: &[(usize, &[u8])],
    rep: &[(usize, &[u8])],
) -> Result<Vec<RecoveredSource>, FecError> {
    for (idx, bytes) in src {
        dec.add_original_shard(*idx, bytes).map_err(rs_err)?;
    }
    // Known-zero injection: [K_actual, K_shape) canonical-zero bilinen-source (§6 Model-A).
    let zero = vec![0u8; e];
    for idx in k_actual..k {
        dec.add_original_shard(idx, &zero).map_err(rs_err)?;
    }
    for (idx, bytes) in rep {
        dec.add_recovery_shard(*idx, bytes).map_err(rs_err)?;
    }
    let result = dec.decode().map_err(rs_err)?;
    let mut out: Vec<RecoveredSource> = result
        .restored_original_iter()
        .filter(|(idx, _)| *idx < k_actual) // zero-injected slot'ları app'e verme
        .map(|(idx, bytes)| RecoveredSource {
            esi: idx as u16,
            bytes: bytes.to_vec(),
        })
        .collect();
    out.sort_by_key(|r| r.esi);
    Ok(out)
}

/// reed-solomon-simd tabanlı STATELESS codec (her-çağrı taze-engine; Alpha CODE_SUITE_RS_SIMD_V1).
#[derive(Debug, Default, Clone, Copy)]
pub struct RsSimdCodec;

impl FecCodec for RsSimdCodec {
    fn encode(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
    ) -> Result<Vec<RepairSymbol>, FecError> {
        profile.validate()?;
        let ordered = ordered_source(profile, source)?;
        let mut enc = ReedSolomonEncoder::new(
            profile.k_shape_usize(),
            profile.r_code_max as usize,
            profile.e_usize(),
        )
        .map_err(rs_err)?;
        run_encode(&mut enc, &ordered)
    }

    fn reconstruct(
        &self,
        profile: &FecProfile,
        received: &[ReceivedSymbol<'_>],
        known_zero_count: usize,
    ) -> Result<Vec<RecoveredSource>, FecError> {
        profile.validate()?;
        let k = profile.k_shape_usize();
        let e = profile.e_usize();
        let (src, rep, k_actual, any_missing) =
            prepare_received(profile, received, known_zero_count)?;
        if !any_missing {
            return Ok(Vec::new());
        }
        let mut dec = ReedSolomonDecoder::new(k, profile.r_code_max as usize, e).map_err(rs_err)?;
        run_decode(&mut dec, k, e, k_actual, &src, &rep)
    }
}

/// reed-solomon-simd ENGINE-REUSE codec (Sprint-3): tek encoder + tek decoder tutar, profile-başına
/// `reset()` ile allocation-yeniden-kullanır (D0: reconstruct decoder-inşa-baskındı ~1000× encode).
/// `RefCell` interior-mutability → `FecCodec` (&self) API korunur; tek-yön/tek-thread kullanım.
/// **Byte-eş garantisi:** aynı çekirdek fonksiyonlar (golden-teyitli: reuse == stateless == golden).
#[derive(Default)]
pub struct RsSimdEngine {
    enc: RefCell<Option<ReedSolomonEncoder>>,
    dec: RefCell<Option<ReedSolomonDecoder>>,
}

impl core::fmt::Debug for RsSimdEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // RS-engine'ler Debug değil; yalnız-kurulu-olup-olmadığını göster (anahtar/materyal-yok zaten).
        f.debug_struct("RsSimdEngine")
            .field("enc_cached", &self.enc.borrow().is_some())
            .field("dec_cached", &self.dec.borrow().is_some())
            .finish()
    }
}

impl RsSimdEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl FecCodec for RsSimdEngine {
    fn encode(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
    ) -> Result<Vec<RepairSymbol>, FecError> {
        profile.validate()?;
        let ordered = ordered_source(profile, source)?;
        let (k, r, e) = (
            profile.k_shape_usize(),
            profile.r_code_max as usize,
            profile.e_usize(),
        );
        let mut slot = self.enc.borrow_mut();
        match slot.as_mut() {
            Some(enc) => enc.reset(k, r, e).map_err(rs_err)?,
            None => *slot = Some(ReedSolomonEncoder::new(k, r, e).map_err(rs_err)?),
        }
        run_encode(slot.as_mut().expect("reset/new sonrası Some"), &ordered)
    }

    fn reconstruct(
        &self,
        profile: &FecProfile,
        received: &[ReceivedSymbol<'_>],
        known_zero_count: usize,
    ) -> Result<Vec<RecoveredSource>, FecError> {
        profile.validate()?;
        let (k, r, e) = (
            profile.k_shape_usize(),
            profile.r_code_max as usize,
            profile.e_usize(),
        );
        let (src, rep, k_actual, any_missing) =
            prepare_received(profile, received, known_zero_count)?;
        if !any_missing {
            return Ok(Vec::new());
        }
        let mut slot = self.dec.borrow_mut();
        match slot.as_mut() {
            Some(dec) => dec.reset(k, r, e).map_err(rs_err)?,
            None => *slot = Some(ReedSolomonDecoder::new(k, r, e).map_err(rs_err)?),
        }
        run_decode(
            slot.as_mut().expect("reset/new sonrası Some"),
            k,
            e,
            k_actual,
            &src,
            &rep,
        )
    }
}
