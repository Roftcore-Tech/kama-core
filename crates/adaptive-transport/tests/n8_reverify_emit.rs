//! Sprint-5c ÇIKIŞ-ŞARTI — N8-YENİDEN-DOĞRULAMA (FEC-emisyon AÇIKKEN).
//!
//! D0-N8 ("INV-RATE-1 + recovered≠ACK UYUMLU") kanıtı FEC-emisyon KAPALIYKEN alınmıştı
//! (`TransmitKind::FecParity` emit-edilmiyordu → ihlal-yolu YAPISAL-İMKANSIZDI). DR-S5-1
//! emisyonu AÇTI → ihlal-yolu ilk-kez FİZİKSEL-MÜMKÜN. Bu dosya, "kapalıyken-imkansızdı"nın
//! "açıkken-de-olmuyor"a dönüştüğünün SENDER-tarafı canlı-kanıtıdır (Test-1 = INV-RATE-1).
//!
//! SAF-DOĞRULAMA: mühürlü-koda (bw.rs/send_rate/delivered-EAI/W_BTLBW) DOKUNMAZ; yalnız
//! public API'yi sürer + diag OKUR.

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, TransmitKind};
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 1200;
const ROUNDS: u64 = 80;

fn ack(seqs: &[u64], delay_ms: u64) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: seqs.to_vec(),
        ack_delay: Duration::from_millis(delay_ms),
    })
}

/// Sabit-kanal-BW rejiminde SENDER'ı FEC-emisyon-AÇIK sürer. `loss_mod`==0 → kayıpsız;
/// `loss_mod`>0 → `seq % loss_mod == 3` olan wire-seq'ler ACK'LENMEZ (kayıp = ACK-yokluğu →
/// gap-detection → recovery-kolu). Kanal-BW ikisinde de AYNI (UNIT / 10ms).
/// Döner: (btlbw, send_rate=pacer.rate, loss_pkt, fec_emitted, acked_total).
fn drive(loss_mod: u64) -> (u64, u64, u64, usize, u64) {
    let mut s = Sender::new();
    s.set_fec_enabled(true); // ← emisyon-AÇIK (ihlal-yolu fiziksel-mümkün)
    let mut fec_emitted = 0usize;
    let mut acked_total = 0u64;

    for i in 0..ROUNDS {
        let t = i * 10 * MS;
        // Emisyon-AÇIK: her 4. round bir repair-shard akıt (gerçek FEC-parity wire'a çıkar).
        if i % 4 == 0 {
            s.enqueue_fec_parity(1, i as u16, UNIT);
        }
        let out = s.on_event(Event::AppData(OpaqueUnit { id: i, size: UNIT }), t);
        let mut seqs: Vec<u64> = Vec::new();
        for d in &out {
            if d.kind == TransmitKind::FecParity {
                fec_emitted += 1;
            }
            seqs.push(d.seq);
        }
        // ACK survivors (kanal-BW aynı; kayıp yalnız hangi seq'lerin teslim-edildiğini değiştirir).
        let ack_seqs: Vec<u64> = seqs
            .iter()
            .copied()
            .filter(|&sq| loss_mod == 0 || sq % loss_mod != 3)
            .collect();
        acked_total += ack_seqs.len() as u64;
        if !ack_seqs.is_empty() {
            s.on_event(ack(&ack_seqs, 0), t + 5 * MS);
        }
    }

    let (_, btlbw, _srtt, pacer_rate, _) = s.diag_rate();
    let (loss_pkt, _, _) = s.diag_loss();
    (btlbw, pacer_rate, loss_pkt, fec_emitted, acked_total)
}

#[test]
fn n8_inv_rate_1_loss_does_not_lower_send_rate_emit_on() {
    let (btlbw0, sr0, loss0, fec0, ack0) = drive(0); // kayıpsız
    let (btlbw15, sr15, loss15, fec15, ack15) = drive(7); // ~%14 kayıp (seq%7==3)

    eprintln!("[N8 Test-1] loss=0 : btlbw={btlbw0} send_rate={sr0} loss_pkt={loss0} fec_emit={fec0} acked={ack0}");
    eprintln!("[N8 Test-1] loss=%14: btlbw={btlbw15} send_rate={sr15} loss_pkt={loss15} fec_emit={fec15} acked={ack15}");

    // Emisyon gerçekten AÇIK (ihlal-yolu fiziksel-mümkündü): FEC-parity wire'a çıktı.
    assert!(
        fec0 > 0 && fec15 > 0,
        "FEC-emisyon AÇIK: parity emit edildi"
    );
    // Kayıp gerçekten ENJEKTE edildi (test anlamlı). NOT: emisyon-AÇIK her iki arm'da da az-sayıda
    // benign gap-detection üretir (FEC-parity DATA'dan ayrı-pace → geçici inflight-seq boşluğu; loss0≈4).
    // Kayıp-arm BELİRGİN-DAHA-FAZLA gap-detection tetikler + daha-az-seq ACK'ler → kayıp gerçek.
    assert!(
        loss15 > loss0,
        "kayıp-arm daha-fazla gap-detection (loss15={loss15} > loss0={loss0})"
    );
    assert!(
        loss15 >= 10,
        "kayıp-arm anlamlı-kayıp işaretledi (recovery-kolu beslendi)"
    );
    assert!(ack15 < ack0, "kayıp-arm daha-az-seq ACK'ler (kayıp gerçek)");

    // INV-RATE-1: loss send_rate'i DÜŞÜRMEZ. Kanal-BW ikisinde AYNI → BtlBw-peak korunur →
    // send_rate loss'a HİÇ tepki-vermez. Bu, N8-spot'un (mapping-fires-vs-noop bit-özdeş) canlı-
    // pipeline + emisyon-AÇIK + gerçek-kayıp versiyonudur: kayıp≈%14 iken send_rate BİT-ÖZDEŞ.
    // (Karşı-örnek: loss→rate-coupling send_rate'i ~0.86× yapar → bit-özdeşlik KIRILIR → FAIL.)
    assert_eq!(
        btlbw15, btlbw0,
        "INV-RATE-1: loss BtlBw'yi DEĞİŞTİRMEZ (bit-özdeş: {btlbw15} == {btlbw0})"
    );
    assert_eq!(
        sr15, sr0,
        "INV-RATE-1: loss send_rate'i DEĞİŞTİRMEZ (bit-özdeş: {sr15} == {sr0})"
    );
}
