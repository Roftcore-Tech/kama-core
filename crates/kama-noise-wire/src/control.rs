//! Secure-control wire-codec (§2.57 exact body-layout'ları + DR-1 count-tavanları).
//!
//! Body = UNIFIED-INNER-HEADER + control_id SONRASI kısım (inner.rs prefix'i ayrı yazar). Bu modül
//! yalnız §2.57-body'yi serialize/parse eder; schema `Control` DTO'suyla köprü kurar (logical↔wire).
//!
//! DR-1 [Abkarada-onaylı]: ~~ACK_MAX_RANGES=32 · SACK_MAX_GAP_BLOCKS=32~~ · RECOVERED_MAX_ESI_RANGES=16.
//! [D2, 2026-10-08] Ack/Sack (0x10/0x11) ailesi KALDIRILDI (K3: ürün ACK'i engine `ACK_V2`); gelen
//! 0x10/0x11 inner_kind sınıflandırmasında reddedilir (`kama_transport_schema::inner_kind::RESERVED_*`).
//! GÖNDERİRKEN tavan-aşımı → REJECT-DEĞİL-BÖL (çok-datagram, her-biri-kendi-control_id). GELEN count>tavan
//! → FAIL-CLOSED reject (DoS-guard). Ayrıca invalid-range (start>end) / kısa-body / reserved → reject.

use crate::error::WireError;
use crate::inner::InnerPlain;
use kama_transport_schema::control::{Control, ControlId};
use kama_transport_schema::inner_kind;

// ── DR-1 count-tavanları ──
/// RECOVERED esi-range üst-sınırı (DR-1).
pub const RECOVERED_MAX_ESI_RANGES: usize = 16;

// ── DR-CS-1 (§2.57-amendman; EXACT — spec-dondurulmuş) ──
/// CS-probe pad uzunluğu (amp≤1: probe_wire=44B ≥ echo_wire=43B).
pub const CS_PROBE_PAD_LEN: usize = 9;
/// CS-probe body = subtype(1)+probe_seq(2)+pad(9) = 12B → wire 26+6+12 = 44B.
pub const CS_PROBE_BODY_LEN: usize = 12;
/// CS-echo body = subtype(1)+probe_seq(2)+seen_count(4)+arrival_gap(4) = 11B → wire 26+6+11 = 43B.
pub const CS_ECHO_BODY_LEN: usize = 11;
/// Tek-geçerli subtype (cold-start); diğerleri reserved → FAIL-CLOSED-reject.
pub const CS_SUBTYPE_COLD_START: u8 = 0x01;

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// §2.57 body serialization (control_id HARİÇ — o inner-header'da). Gönderme-tarafı count'ları
/// tavan-içinde VARSAYAR (çağıran `split_for_send` ile böler); debug-assert ile guard'lı.
#[must_use]
pub fn encode_control_body(c: &Control) -> Vec<u8> {
    let mut out = Vec::new();
    match c {
        Control::Recovered {
            block_id,
            esi_ranges,
            ..
        } => {
            debug_assert!(esi_ranges.len() <= RECOVERED_MAX_ESI_RANGES);
            put_u32(&mut out, *block_id);
            put_u16(&mut out, esi_ranges.len() as u16);
            for (s, e) in esi_ranges {
                put_u16(&mut out, *s);
                put_u16(&mut out, *e);
            }
        }
        Control::BlockComplete {
            block_id,
            final_source_count,
            ..
        } => {
            put_u32(&mut out, *block_id);
            put_u16(&mut out, *final_source_count);
        }
        Control::ParamSet {
            param_hash,
            profile_id,
            code_suite_id,
            ..
        } => {
            out.extend_from_slice(param_hash);
            out.push(*profile_id);
            out.push(*code_suite_id);
        }
        Control::ParamAck { param_hash, .. } => out.extend_from_slice(param_hash),
        Control::BlockAbort {
            block_id,
            reason_code,
            ..
        } => {
            put_u32(&mut out, *block_id);
            out.push(*reason_code);
        }
        Control::SessionClose { reason_code, .. } => out.push(*reason_code),
        Control::SessionRenewRequired {
            reason_code,
            renew_hint,
            ..
        } => {
            out.push(*reason_code);
            out.push(*renew_hint);
        }
        // DR-CS-1 (§2.57-amendman): EXACT-layout — spec-önce-donduruldu, kod-uygular.
        Control::CsProbe {
            subtype, probe_seq, ..
        } => {
            out.push(*subtype);
            put_u16(&mut out, *probe_seq);
            out.extend_from_slice(&[0u8; CS_PROBE_PAD_LEN]); // pad=0x00 (amp≤1: probe≥echo)
        }
        Control::CsEcho {
            subtype,
            probe_seq,
            seen_count,
            arrival_gap,
            ..
        } => {
            out.push(*subtype);
            put_u16(&mut out, *probe_seq);
            put_u32(&mut out, *seen_count);
            put_u32(&mut out, *arrival_gap);
        }
    }
    out
}

/// §2.57 body byte-uzunluğu (control_id hariç) — sizer için (seal ÖNCESİ kesin-boyut, §0.5 P0-3).
///
/// # Errors
/// Tavan-aşan count (gönderme-öncesi `split_for_send` yapılmalıydı) → [`WireError::ControlCountExceedsCap`].
pub fn control_body_len(c: &Control) -> Result<usize, WireError> {
    let n = match c {
        Control::Recovered { esi_ranges, .. } => {
            cap_check(
                inner_kind::RECOVERED,
                esi_ranges.len(),
                RECOVERED_MAX_ESI_RANGES,
            )?;
            4 + 2 + esi_ranges.len() * 4
        }
        Control::BlockComplete { .. } => 6,
        Control::ParamSet { .. } => 34,
        Control::ParamAck { .. } => 32,
        Control::BlockAbort { .. } => 5,
        Control::SessionClose { .. } => 1,
        Control::SessionRenewRequired { .. } => 2,
        Control::CsProbe { .. } => CS_PROBE_BODY_LEN, // DR-CS-1: 12B (wire 44B)
        Control::CsEcho { .. } => CS_ECHO_BODY_LEN,   // DR-CS-1: 11B (wire 43B)
    };
    Ok(n)
}

fn cap_check(kind: u8, count: usize, cap: usize) -> Result<(), WireError> {
    if count > cap {
        return Err(WireError::ControlCountExceedsCap { kind, count, cap });
    }
    Ok(())
}

/// GÖNDERME-tarafı bölme (DR-1): count > tavan ise en-güncel-durumu birden-fazla control'e böler;
/// her-parça yeni-control_id alır (`next_id`). Tavan-içiyse tek-elemanlı Vec döner (aynı id).
/// RECOVERED bölünür (~~ACK/SACK~~ D2'de kaldırıldı); diğerleri sabit-boy (bölme-gerektirmez).
pub fn split_for_send(c: &Control, mut next_id: impl FnMut() -> ControlId) -> Vec<Control> {
    match c {
        Control::Recovered {
            id,
            block_id,
            esi_ranges,
        } if esi_ranges.len() > RECOVERED_MAX_ESI_RANGES => esi_ranges
            .chunks(RECOVERED_MAX_ESI_RANGES)
            .enumerate()
            .map(|(i, ch)| Control::Recovered {
                id: if i == 0 { *id } else { next_id() },
                block_id: *block_id,
                esi_ranges: ch.to_vec(),
            })
            .collect(),
        other => vec![other.clone()],
    }
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }
    fn u8(&mut self) -> Result<u8, WireError> {
        let v = *self
            .b
            .get(self.pos)
            .ok_or(WireError::ControlDecode("body too short"))?;
        self.pos += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16, WireError> {
        let end = self.pos + 2;
        let s = self
            .b
            .get(self.pos..end)
            .ok_or(WireError::ControlDecode("body too short"))?;
        self.pos = end;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, WireError> {
        let end = self.pos + 4;
        let s = self
            .b
            .get(self.pos..end)
            .ok_or(WireError::ControlDecode("body too short"))?;
        self.pos = end;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn hash32(&mut self) -> Result<[u8; 32], WireError> {
        let end = self.pos + 32;
        let s = self
            .b
            .get(self.pos..end)
            .ok_or(WireError::ControlDecode("body too short"))?;
        self.pos = end;
        let mut a = [0u8; 32];
        a.copy_from_slice(s);
        Ok(a)
    }
    /// Tüm-body tüketildi mi (trailing-byte → reject, §2.57 exact).
    fn finish(&self) -> Result<(), WireError> {
        if self.pos == self.b.len() {
            Ok(())
        } else {
            Err(WireError::ControlDecode("trailing bytes"))
        }
    }
}

/// §2.57 body parse (fail-closed: DR-1 tavan / invalid-range / kısa-body / trailing → reject).
/// `control_id` inner-header'dan gelir (schema `Control.id` alanına yerleşir).
///
/// # Errors
/// [`WireError::ControlDecode`] / [`WireError::ControlCountExceedsCap`].
pub fn decode_control_body(
    codepoint: u8,
    control_id: ControlId,
    body: &[u8],
) -> Result<Control, WireError> {
    use inner_kind::*;
    let mut r = Reader::new(body);
    let c = match codepoint {
        RECOVERED => {
            let block_id = r.u32()?;
            let count = usize::from(r.u16()?);
            cap_check(RECOVERED, count, RECOVERED_MAX_ESI_RANGES)?;
            let mut esi_ranges = Vec::with_capacity(count);
            for _ in 0..count {
                let s = r.u16()?;
                let e = r.u16()?;
                if s > e {
                    return Err(WireError::ControlDecode("recovered esi start>end"));
                }
                esi_ranges.push((s, e));
            }
            Control::Recovered {
                id: control_id,
                block_id,
                esi_ranges,
            }
        }
        BLOCK_COMPLETE => {
            let block_id = r.u32()?;
            let final_source_count = r.u16()?;
            Control::BlockComplete {
                id: control_id,
                block_id,
                final_source_count,
            }
        }
        PARAM_SET => {
            let param_hash = r.hash32()?;
            let profile_id = r.u8()?;
            let code_suite_id = r.u8()?;
            Control::ParamSet {
                id: control_id,
                param_hash,
                profile_id,
                code_suite_id,
            }
        }
        PARAM_ACK => Control::ParamAck {
            id: control_id,
            param_hash: r.hash32()?,
        },
        BLOCK_ABORT => {
            let block_id = r.u32()?;
            let reason_code = r.u8()?;
            Control::BlockAbort {
                id: control_id,
                block_id,
                reason_code,
            }
        }
        SESSION_CLOSE => Control::SessionClose {
            id: control_id,
            reason_code: r.u8()?,
        },
        SESSION_RENEW_REQUIRED => {
            let reason_code = r.u8()?;
            let renew_hint = r.u8()?;
            Control::SessionRenewRequired {
                id: control_id,
                reason_code,
                renew_hint,
            }
        }
        CTRL_CS_PROBE => {
            // DR-CS-1 §2.57: subtype(=0x01) + probe_seq + pad[9]=0x00 (reserved-nonzero → reject).
            let subtype = r.u8()?;
            if subtype != CS_SUBTYPE_COLD_START {
                return Err(WireError::ControlDecode("cs-probe reserved subtype"));
            }
            let probe_seq = r.u16()?;
            for _ in 0..CS_PROBE_PAD_LEN {
                if r.u8()? != 0 {
                    return Err(WireError::ControlDecode("cs-probe nonzero pad"));
                }
            }
            Control::CsProbe {
                id: control_id,
                subtype,
                probe_seq,
            }
        }
        CTRL_CS_ECHO => {
            let subtype = r.u8()?;
            if subtype != CS_SUBTYPE_COLD_START {
                return Err(WireError::ControlDecode("cs-echo reserved subtype"));
            }
            let probe_seq = r.u16()?;
            let seen_count = r.u32()?;
            let arrival_gap = r.u32()?;
            Control::CsEcho {
                id: control_id,
                subtype,
                probe_seq,
                seen_count,
                arrival_gap,
            }
        }
        _ => return Err(WireError::ControlDecode("non-control codepoint")),
    };
    r.finish()?;
    Ok(c)
}

/// Opened-inner'ı schema `Control`'e çevirir (§4-ORDER-8 sonrası). FEC-shard → None.
///
/// # Errors
/// Control-body-decode hatası (fail-closed).
pub fn decode_control(inner: &InnerPlain) -> Option<Result<Control, WireError>> {
    match inner {
        InnerPlain::Control {
            inner_kind,
            control_id,
            body,
        } => Some(decode_control_body(*inner_kind, *control_id, body)),
        InnerPlain::FecShard { .. } => None,
    }
}
