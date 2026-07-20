# DR-S5-1 — sender.rs: FecParity-emit-noktası + SealedPacketMeta-üretim-hook'u [ONAY BEKLİYOR]

- **Sprint:** 5c (cerrahi; §2-protokol). **Statü:** ÖNERİ — Abkarada-onayı-öncesi UYGULANMAZ.
- **Non-regression referansı:** `reports/d1-exit/baseline-pre-5c.md` (A exact-count + B aralık-pin + C FEC-off-yapısal).
  ⚠ Metodoloji §D-onayına bağlı (desen1 bit-for-bit GEÇERSİZ; A+B+C-gate önerildi).

## 1. Dokunulacak dosya:satır (±3-bağlam)

**(a) sender.rs:124-135** — TransmitKind (FecParity şu-an "Emit EDİLMEZ"):
```
124  /// Transmit türü.
125  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
126  pub enum TransmitKind {
127      Data,
...
133      /// FEC parity shard (§10) — POLİTİKA; transport üretmez ... Emit EDİLMEZ.
134      FecParity,
135  }
```
**(b) sender.rs:113-122** — TransmitDecision (send-record-hook eklenecek alan):
```
113  pub struct TransmitDecision {
114      pub kind: TransmitKind,
115  ... seq / unit_id / size
122  }
```
**(c) sender.rs:1140-1196** — schedule_transmit (emit-noktası; şu-an DATA + retransmit + duplicate; FecParity YOK).

## 2. Değişiklik diff-taslağı

- **(a)** `TransmitKind::FecParity` yorumu güncellenir: "koşullu-emit (FEC-etkinse); FEC-kapalıyken emit-EDİLMEZ".
  Enum-variant AYNEN kalır (byte/ABI-değişimi yok).
- **(b)** TransmitDecision'a **opsiyonel send-record alanı** eklenir (yeni-alan, mevcut-alanlar-değişmez):
  ```rust
  /// Send-record hook'u (§0.5 P0-4): FEC/control için logical_ref + packet_class + send_generation.
  /// DATA için None (mevcut-davranış korunur). noise-wire::SealedPacketMeta ile eşlenir (engine seal-eder).
  pub send_record: Option<SendRecordHint>,
  ```
  `SendRecordHint { logical_ref: (block_id,esi)|control, packet_class, send_generation }` — YENİ-tip.
- **(c)** schedule_transmit'e **FEC-etkinse** FecParity-emit-kolu eklenir: DATA-öncelik-sırası (§3 "DATA ÖNCE")
  KORUNUR; FecParity yalnız DATA+recovery-sonrası pacing-bütçesinde. FEC-kapalıyken kol-çalışmaz (allocation-yok).

## 3. Hangi mühürlü-davranışa KOMŞU

- **KOMŞU (dokunmuyorum):** `TransmitKind::Data` yolu + INV-RATE-1 (DATA send_rate'i sürer) + schedule_transmit
  "DATA ÖNCE" sıralaması (§3) + PreConverge-CC-cold-start (KESİN-YASAK). FecParity-kolu bunların SONRASINA eklenir;
  DATA-scheduling/rate-mantığı BYTE-DEĞİŞMEZ.
- **DOKUNMUYORUM (KESİN-YASAK):** bw.rs/send_rate-loss/ZAMAN-MODELİ/W_BTLBW/qd/TokenBucket/SACK-V2. Send-record
  yalnız-METADATA üretir; rate/muhasebe-mantığına GİRMEZ.

## 4. Non-regression kanıt-planı (AYNI COMMIT'te)

1. baseline-A: 5-crate exact-test-count AYNEN (noise-wire 90 / fec-codec 25 / schema 9 / integration 9 / adaptive-transport 118).
2. baseline-B: desen1_sim 5 aralık-pin PASS (qd>1s · btlbw<2100 · <600 · sub-kbps-yakınsama).
3. baseline-C: **FEC-KAPALI-yol** (flag-off) FecParity-emit-ETMEZ → hot-path allocation-yok KORUNUR (yeni-test: fec-off→emit-yok assert).
4. INV-RATE-1 spot: FecParity-emit-AÇIKKEN DATA-send_rate DEĞİŞMEZ (loss→rate-düşüş-yok; N8-kısmi, tam-doğrulama 5c-çıkışında).
5. fmt/clippy temiz.

## 5. Kapsam-sınırı

Bu-DR **yalnız emit-noktası + send-record-hook-iskeleti** (metadata-üretimi). Gerçek ACK→logical-eşleme-akışı +
N8-tam-yeniden-doğrulama 5c-çıkış-şartı (DR-S5-1 + DR-S5-2 + emisyon-açık-pipeline birlikte).
