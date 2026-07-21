# DR-S5-6 — DATA-source send-record kapanışı (M1-adalet ön-koşulu; 5e-1) [ONAYLANDI + UYGULANDI]

- **Sprint:** 5e-1. **Statü:** ONAYLANDI (Abkarada, 2026-07-21; Çözüm-A + 2-ek-şart) — UYGULANDI.
  - **Şart-1 (N8-DATA-bit-özdeşlik):** `dr_s5_6_n8_data_register_does_not_change_btlbw` — register-fires-vs-noop
    BtlBw BİT-ÖZDEŞ + fires'ta WireAck-girişleri + dejenerasyon-guard (btlbw>0). Aynı-commit.
  - **Şart-2 (CAP/M1-doğruluk):** SENT_LOGICAL_CAP=16384 gerekçeli (120s×%15×~910pkt/s'e SIFIR-eviction;
    M1-profilleri-altında) + çift-artefakt-sayaç (`diag_sent_logical`: evicted + ack_miss). **M1-hücre-işaretleme
    YALNIZ evicted>0** — adversarial-ÇÜRÜT-1-bulgusu: miss>0∧evicted==0 = beklenen Recovered→geç-ACK yarışı
    (RECOVERED-önce-teyit, geciken-wire-ACK-sonra; artefakt-DEĞİL → miss işaretleme-kuralı-dışı, salt-teşhis).
    Gerekirse özgüllük-seti (recovered_then_acked) M1-sonrası.
  - Adversarial-verify (3-çürütücü): 0-blocker; ÇÜRÜT-1-note yorum/kural-düzeltmesiyle kapatıldı; ÇÜRÜT-2
    (vacuity) redli — register-ACK-öncesi + koşular-ayrışıyor + bit-özdeşlik-gerçek; ÇÜRÜT-3 sealed-untouched
    (mühürlü-bölge byte-diff-özdeş; else-dalı btlbw.update'in-önüne-girmedi).
- **Amaç (N8-v2 Not-2 kapanışı):** `sent_logical` şu-an YALNIZ FEC-parity-emisyonunda doluyor → RECOVERED bir
  **DATA-source**-birimini kurtardığında retransmit-CANCEL çalışır (unit_id) AMA defter-girişi (`ConfirmSource::
  Recovered`) NO-OP kalır → M1'de Kama-arm'ın recovered-teslim-sayımı EKSİK (eğik-zemin). **Bu kapanmadan M1 koşulmaz.**
- **Neden mini-DR (değerlendirme-sonucu):** dar-çözüm bile sender.rs'e dokunuyor VE salt-additive-diag DEĞİL —
  (i) ACK-defter-semantiği genişliyor (DATA-teyitleri artık WireAck-girişi üretir), (ii) S5-3-latent `sent_logical`-
  unbounded-riski DATA-hacmiyle kritikleşiyor → bounded-cap dokunuşu. "Beyan-yeterli"-kategorisinde saymadım.

## 0. ÖNEMLİ-TESPİT: wire-yarısı ZATEN-TAM; eksik yalnız TRANSPORT-defter-yarısı

SealedPacketMeta (noise-wire, seal-anı) DATA-source için ZATEN üretiliyor — pipeline'da source-shard'lar
`FecShard{kind: Source}` olarak seal-ediliyor, meta logical_ref/packet_class taşıyor. Yani "SealedPacketMeta hem
DATA hem FEC" wire-katmanında MEVCUT. Eksik olan **transport-defteri**: DATA-source-emisyonlarının
`sent_logical(seq→SrLogicalRef)`'e kaydı. Bu-DR yalnız o-yarıyı kapatır (wire-katmanına dokunuş YOK).

## 1. ÇÖZÜM-A [ÖNERİLEN] — engine-post-emit kayıt; mühürlü-yola SIFIR-dokunuş

**İlke:** transport DATA'nın block/esi'sini BİLMEZ (SoC; OpaqueUnit agnostik). Çevrim-bilgisi ENGINE'de
(DR-S5-4-şart-2 emsali: RecoveredMap zaten (block,esi)→unit_id tutuyor). Engine, `TransmitDecision`
(pub seq/unit_id) gördüğünde kendi block/esi-bilgisiyle sender-defterine **post-emit** kaydettirir:

```rust
/// DR-S5-6: engine, emit-edilmiş bir seq'in logical-kimliğini deftere kaydeder (DATA-source;
/// FEC-parity plan_fec_parity'de zaten-kayıtlı). SALT-DEFTER — bw/pacing/schedule'a DOKUNMAZ.
pub fn register_sent_logical(&mut self, seq: u64, lref: SrLogicalRef) {
    self.insert_sent_logical(seq, lref); // bounded-helper (aşağıda)
}
```

**Dokunuş-listesi (sender.rs; TÜMÜ defter-bölgesi ~1280-1390, record_*/take_* komşuluğu):**
- **(a) `SENT_LOGICAL_CAP` (yeni-sabit, örn. 8192) + bounded-insert-helper** `insert_sent_logical` (cap-dolunca
  en-eski-seq düşer + `diag_sent_logical_evicted` audit; SR-AUDIT-001). `plan_fec_parity`'nin mevcut-insert'i
  (@1376) bu-helper'a yönlenir (tek-satır-değişim; cap-altında davranış-özdeş).
- **(b) `pub fn register_sent_logical`** (additive; yukarıda).
- **(c) diag:** `diag_sent_logical_len/evicted` (test/M1-teşhis).
- **DEĞİŞMEYENLER:** `drain_new_data` (pacer/track_sent-komşulu — SIFIR-dokunuş; DATA-decision `send_record: None`
  KALIR, wire-hint-gerekmez), `OpaqueUnit`, `TransmitDecision`, on_ack/on_recovered yolları, bw.rs yasak-6-blok
  (güncel: 555-575 W_BTLBW · 577-582 W_qd · 604-605 + 619-620 btlbw.update · 633-634 credit_delivered · ~1530 on_sent).

## 2. DAVRANIŞ-GENİŞLEMESİ (dürüst-beyan)

1. **ACK-yolu:** DATA-seq kayıtlıysa `record_confirmed_logical` artık NO-OP-değil → `(seq, lref, WireAck)` girişi.
   BtlBw-yolu DEĞİŞMEZ (S5-3-kanıtı: record_* SALT-LOGICAL; @604-605 satırları dokunulmaz).
2. **RECOVERED-yolu:** DATA-source-kurtarmaları artık `(seq, lref, Recovered)` girişi üretir → M1-simetri TAM
   (Not-2'nin özü).
3. **Cap-baskısı:** DATA-hacmiyle `confirmed_logical`(4096) hızlı-dolar → **M1-harness'ında düzenli
   `take_confirmed_logical`-drenajı ZORUNLU** (engine-sorumluluğu; nota-geçirilir).

## 3. N8-BİT-ÖZDEŞLİK (zorunlu-test; S5-3-spot'un DATA-versiyonu, AYNI-COMMIT)

```
warmup_data_register(fires: bool): aynı DATA+ACK senaryosu; fires → her-emisyon register_sent_logical'lı.
  ASSERT btlbw_fires == btlbw_noop   (BİT-ÖZDEŞ — DATA-defter-kaydı BtlBw'ye SIFIR-etki)
  ASSERT confirmed_fires > confirmed_noop (fires: DATA-teyitleri deftere girdi — WireAck)
+ bounded-test: CAP+N insert → len==CAP + evicted==N.
```

## 4. ALTERNATİF-B (değerlendirildi; önerilmedi) — tamamen-engine-yan-defter

Engine kendi `seq→lref` defterini tutar; sender.rs'e HİÇ dokunulmaz. **Reddi:** defter PARÇALANIR — sender-defteri
(FEC-parity, ConfirmSource'lu) + engine-defteri (DATA) ayrışır; RECOVERED/ACK-teyit-anları sender-içinde olduğundan
engine-defteri teyit-anını göremez (take_confirmed_logical'dan türetme = dolaylı+kırılgan); M1 iki-kaynaktan toplar
(muhasebe-bütünlüğü bozulur). Tek-defter (Çözüm-A) ConfirmSource-bütünlüğünü korur.

## 5. Non-regression (AYNI-COMMIT)

{A}: mevcut-sayılar AYNEN + yeni-testler (+2~3) · {B} desen1 5-pin · {C} FEC-off yapısal · fmt/clippy ·
N8-bit-özdeşlik (§3). Mühürlü-yasak-listeye dokunuş YOK (defter-bölgesi-only).

## 6. Kapsam-sınırı

Yalnız transport-defter-yarısı + bounded-cap. Engine-tarafı M1-harness'ının register-çağrı-örüntüsü (hangi-emisyonda
hangi-lref) M1-tasarımında (Abkarada'yla bütçe-eşitliği-gözden-geçirmesinde) netleşir. SealedPacketMeta/wire DEĞİŞMEZ.
