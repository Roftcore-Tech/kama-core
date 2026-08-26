# P3 — Sertleştirme (KAPANIŞ)

**Statü:** TAMAM. Kapsam: cargo-fuzz altyapısı + CI fuzz-smoke kapısı + read-only parser denetimi
(SR-AUDIT-001) → 3 doğrulanmış kusur düzeltildi. Otorite: roadmap A-hattı P3.

## 1. Fuzz altyapısı (`aec5674b`)
Ayrık `fuzz/` crate'i (nightly+libFuzzer; üretim grafiğine girmez), 5 hedef güvenilmeyen-girdi yüzeyinde:
`ack_decode` (AckFrame::decode + acked_full, u16-sınır tabanları) · `fec_decode` (FecShardFrame) ·
`noise_wire_decode` (outer/inner/control) · `protector_open` (GERÇEK AEAD open, sabit stub anahtar —
handshake iddiası yok) · `wire_v0` (P1 wire + IdentitySeam). CI'ya `fuzz-smoke` job'u (nightly, hedef
başına 60 s; yeni crash = kapı düşer). Yerel: protector_open 300 s'de 21M exec temiz.

## 2. Denetim + düzeltmeler (SR-AUDIT-001)
Read-only workflow (4 lens × verify): 3 CONFIRMED kusur — **hepsi ağ-erişilebilir, panik-SIZ tahsis/
state amplifikasyonu** (bu yüzden fuzzer'lar kaçırdı; libFuzzer default 2GB rss altında kaldılar).

| # | kusur | fix | commit |
|---|---|---|---|
| A1 | `AckFrame::decode` `MAX_ACK_RANGES` zorlamıyordu (u8→255 range) → `acked_full` ~16.7M-giriş (~134MB) Vec | decode typed reddeder (`TooManyAckRanges`) + `acked_full` `MAX_ACKED_SEQS`=65536 tavanı | `16757f38` |
| A2 | (A1'in ikinci yüzü — `acked_full` len-şişirme) | aynı iki-katmanlı fix | `16757f38` |
| A3 | `Receiver.received` BTreeSet insert-only, hiç budanmıyor → düşman ayrık-seq akışında sınırsız rx-state + her SACK O(n log n) | `record_seq` kayan-pencere (`RX_DEDUP_WINDOW`=4096, O(log n) `pop_first`) | `6ddd7747` |

Her fix: typed/bounded, kümülatif-SACK sayesinde işlevsel kayıp yok, regresyon testli. A1 için 600MB
rss-limitli re-fuzz: eski kod OOM verirdi, yeni kod DONE temiz.

## 3. PARSER-02/03 durumu
- **PARSER-02** (bounds-before-read): zaten yürürlükte (`read.rs`, `ack.rs`, `fec.rs`); A1/A3 onu
  tahsis/state-amplifikasyon ekseninde TAMAMLADI.
- **PARSER-03** (cert not_after / zaman-overflow): HEDEF KOD HENÜZ YOK (ccr-codec 3-satır stub, DER
  parse yazılmadı). Kural ön-kayıtlı kalır; ccr-codec implementasyonunda uygulanır (B-hattı/PKI).

## 4. Gerileme
Motor lib 149, client-core 7, clippy 0 (default+all-features), fmt temiz; netem P1 kapısı GREEN.
Bankalar re-run YOK (r5 M: motor fix = yeni commit).

## Kalan (P3 kapsamı dışı, kayıt)
- PARSER-03 → ccr-codec DER parse yazılınca.
- Fuzz corpus'unu CI cache'e seed'lemek (şimdilik cold-start; 60 s/hedef yeterli smoke).
