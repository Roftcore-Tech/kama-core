# Kama Transport v1 — MİMARİ (donmuş, Faz-6 2026-09-11)

Bu belge MEVCUT donmuş mimariyi anlatır; deney günlüğü değildir. Normatif kaynaklar repo kökü
`specs/adaptive-transport-engine-spec.md` (HOW) ve `specs/transport-layer-tdd.md` (WHAT);
çelişkide spec kazanır. Kod: `crates/adaptive-transport` (motor), `crates/client-core`
(runtime/endpoint), `crates/kama-transport-schema` (tel DTO'ları), `crates/kama-fec-codec`.

## 4.1 Tasarım hedefleri ve hedef-olmayanlar

**Hedefler:** hostile/zayıflamış ağlarda uygulama verisini ULAŞTIRMAK — yüksek rastgele kayıp,
burst/korelasyonlu kayıp, düşük bant, yüksek gecikme, asimetrik yol, bozuk geri-besleme;
veri-öncelikli (zorunlu ön-DATA el sıkışması yok, INV-010); mesaj-granülü teslim (INV-011);
recovery-obsesif taban (INV-RATE-1: kayıp hızı düşürmez).

**Hedef-olmayanlar:** sağlıklı yüksek-bant ağda TCP/QUIC'i geçmek (T0/T4/T5 parite yeterli);
olgun çoklu-akış QUIC semantiğini ikame etmek; bağlantı göçü; çekirdek/NIC optimizasyon paritesi;
tam sıralı akış; kripto (INV-016 — güvenlik katmanının müşterisi).

## 4.2 Veri yolu

```
app  ──send(payload ≤ 1013 B)──►  ClientSender (bounded offer_queue 1024)
     ─►  ClientSession/Sender: unit_id (u64 monoton) atanır, birim = mesaj (fragmenter KAPALI)
     ─►  FEC source-symbol yuvası (SOURCE_PREFIX 11 B + payload + zero_pad = E 1024; blok K=8)
     ─►  Sender state (PreConverge → Startup → Steady / RecoverArq / RecoverFwd / Idle)
     ─►  pacer (TokenBucket; PreConverge ilk pencere 2400 B; hız = BtlBw + queue_delay)
     ─►  DATA (0x00) / FEC_SHARD (0x20) / kuruluş probu  ──UDP──►  alıcı
alıcı ─► dedup (RX_DUP_SUPPRESS) → FEC blok toplama/decode → Delivery{peer, unit_id, payload}
     ─► ACK_V2 (0x14): kümülatif + ≤16 aralık + seen_count + ack_delay  ──UDP──►  gönderici
```
Teslim sırasız; `acked()` göndericiye ilk-ACK-kazanır birim id akışı verir.

## 4.3 ACK/SACK modeli

* Yalnız **pozitif bilgi**: alıcı ne ALDIĞINI söyler (kümülatif + aralıklar, `MAX_ACK_RANGES 16`);
  NACK YOKTUR. Bilgi monotondur (INV-003).
* Cadence: her `ACK_FREQ_N = 2` DATA'da + zamanlayıcı; sessizlik re-ACK bütçesi `quota_r = 4`
  (epizot başına); SACK ≈ 65 B; dönüş-bant önseli 31 B/s (en-kötü-durum önseli, ölçüm değil).
* **Eksik ACK ≠ kayıp** (INV-001): ACK yokluğu yalnız `Suspected` üretir; dönüş yolu bozulduğunda
  gönderici bunu `return_path` gözlemcisinden AYRI ölçer (§4.8).
* `seen_count` (ACK_V2): alıcının gördüğü TÜM datagram sayısı → göndericinin ileri-kayıp
  tahmininin kaynağı (Faz-1).

## 4.4 Fate / kayıp modeli

Kayıp bir HİPOTEZDİR: `LedgerState::{Suspected, Confirmed, Spurious}` (`loss.rs`).
`Suspected` → confirm ufku kanıtsız geçince `Confirmed` (pencere-içi `loss_ppm`'e sayılır) →
geç ACK gelirse `Spurious` (geri alınır, sayılmaz). Confirm ufku `CadenceLegacy`.

İki ayrı tüketici: **ARQ kesinliği** (hangi birim yeniden gönderilecek — `pending_lost`,
reorder-toleranslı `ReorderTracker` k=3) ve **FEC risk tahmini** (`loss_ppm`, yapı-gate, spekülatif
kol). Kayıp `send_rate`'e GİRMEZ (INV-RATE-1/§5.4); reorder kayıp sayılmaz
(`reorder_suppress 300‰`, INV-012).

## 4.5 ARQ

* Yeniden iletim `pending_lost`'tan planlanır (`plan_arq_retransmits`), `RetxConfig`: retx cap 3,
  reorder_k 3, ρ 2/1; cap aşılırsa `HandToForward` (forward-recovery'ye devir).
* Onay: `ConfirmSource::WireAck` (gerçek ACK) veya `Recovered` (FEC decode → RECOVERED kontrol;
  kredi üretmez). RECOVERED, kuyruktaki DATA retransmit'ini ve bekleyen parity'yi iptal eder.
* Kimlik: birim `unit_id`/seq (u64 monoton); repair sembolleri AYRI ESI namespace'inde ve
  kaybolan repair ARQ'ya GİRMEZ (INV-004/005, `repair_loss_not_arqed`).
* Kurtarma yolu: `RecoverArq` durumu her aktif fazdan girilir (AMENDMENT-1); PreConverge'de
  recovery DONDURULUR (§3.3-C).

## 4.6 Tail probe

`TailRecoveryConfig::validated_v1()` (ClientSession kurar): PTO 10 ms..10 s, ilk 1 s (RTT yokken),
backoff tavanı 30 s, `max_ack_delay 25 ms`, **en fazla 3 probe**. Amaç: akışın kuyruğunda (uçuşta
başka veri yokken) kaybolmuş son birimin ACK'sız beklemesini kesmek — probe, orijinali tombstone'a
çekip taze seq ile kopyalar (Karn-dışlama korunur). Sıradan retransmit'ten farkı: kayıp İLANI
gerektirmez, zaman-tetiklidir; ACK yarışında iptal (`tail_cancelled`). Faz-5 T0: 100 mesaj / 33
probe (retx == probes, tamamı tail) ama toplam tel quinn +%4 — 50 ms'lik paced yük her mesajdan
sonra doğal kuyruk yaratır; **sayıdan patoloji çıkarılmaz**, tel maliyetine bakılır.

## 4.7 FEC (mevcut tasarım)

* **Geometri:** profil 0x05 — `K_shape 8`, `R_code_max 4`, `E 1024`; source-symbol =
  `SOURCE_PREFIX 11 + payload ≤ 1013 + zero_pad`; sealed shard 1 061 B < 1 280 PMTU.
* **Kısmi/son blok (Model-A):** `k_actual ≤ 8` telde taşınır; alıcı `known_zero = K_shape −
  k_actual` sıfır sembol enjekte ederek decode eder; kurtarılan birim id'si K_shape tabanlı
  (INV-013). Gönderici drain öncesi kısmi bloğu sıfır-dolguyla kapatır.
* **Repair üretimi:** `compute_requested_r(ratio‰, k_actual, r_max)` → r (yuvarlama; r=0 ise
  çerçeve kurulmaz — `rounded_to_zero_count`), profil r-cap; witness-otoriteli kuyruk
  (`push_parity_authorized`); pacer token reddi `parity_token_denied`.
* **Adaptif politika (Controller, `manual_r: None`):** üç kol ÖNERİR — canonical/confirmed
  (`loss_ppm`), yapı-gate (`Q18Iid`, seviye 250‰), spekülatif `bounded_v1` (L1/L2 250‰,
  persistence P2 öğrenilen Q95, C4 guard G1 reorder_suppress 300‰) — **tek arbiter**
  `max(...)` (toplama yok) → **§8.5 bant bütçesi** (`cap = BtlBw·1000/125 kB/s`, profil tavanı
  900‰; yalnız KÜÇÜLTÜR, sıfırlamaz — AMENDMENT-3) → KURULUŞ-GUARD (PreConverge'de 0) → latch
  (INV-006).
* **§11 MinimalParity istisnası:** fuzzy emergency ∧ hysteresis ≥ Dup2 ∧ SegmentBlock → sabit
  100/200‰, profil r-cap, §8.5 bütçesine tabi değil, normal oranın YERİNE (ADR-minimal-parity-
  exception). ~~T0–T7'de hiç ateşlemedi~~ **[ERRATUM-2 (2026-10-18)]** T0–T7'de 15/330 koşuda
  `RecoverFwd` girişi var (T1a 2, T1b 7, T6 6); MinimalParity emisyonu ayrı sayaçla ölçülmedi
  (LIM-006).
* **Repair ≠ DATA:** kaybolan repair ARQ'ya girmez (INV-005).
* **Ürün kimliği:** `fec = Some{0x05, None}`; değiştirmek profil kimliğini değiştirir.

> **Faz-5, FEC'i gözlenen hostile üstünlüğün nedensel kaynağı olarak KURMAZ.** T1a
> (DOMINATES) `parity = 0`; T6 (DOMINATES) 3 sembol. FEC katılan, nedensel olmayan bileşendir
> (ADR-fec-v1-role, LIM-002). "FEC avantajı sağlar" ifadesi kanıtsızdır ve kullanılmaz.

## 4.8 Dönüş-yolu gözlenebilirliği (Faz-1)

`return_path.rs` `ReturnPathObserver` her ACK'te: dönüş ACK cadence'i (`ack_int`, sessizlik),
ölçülen dönüş bandı, **ileri kayıp** (`seen_count`'a dayalı bir-ACK-gecikmeli kümülatif oran —
yapısal olarak muhafazakâr) ve **dönüş ACK kaybı** (`ACK_FREQ_N = 2` referanslı mutlak ölçü);
EWMA α 250‰; yön kanıtı ≥ 8 datagram, ACK örneği ≥ 4; health `Unknown → Healthy → Degraded
(boşluk ×3) → Dead (×12)`. INV-RETURN-1/INV-002: düşük dönüş bandı ileri kayıp DEĞİLDİR.
Doğrulama (tanım değil): T6'da enjekte ileri %5 → 45.5‰, dönüş %80 → 793‰, health Dead iken
teslim 1.000.

## 4.9 Pacing / zamanlama

Tek pacer (`TokenBucket`, burst cap 2 400 B): hız = delivered-rate (`BtlBw` max-filtre, 10 s
pencere) + queue_delay; PreConverge'de ilk pencere kredisi (IW), `MIN_RATE` tabanı PreConverge'de
UYGULANMAZ (R8). Sınıflar aynı token havuzundan: **DATA** (orijinal) · **ARQ** retransmit ·
**tail probe** · **FEC repair** (token reddi → `parity_token_denied`; kuruluş probu token-muaf).
Öncelik/tahkim mevcut kod sırasıdır; yeni tahkim SPEKÜLE EDİLMEZ. Emergency'de ACK decimation
(§11) tanımlı, T0–T7'de ateşlememiş.

## 4.10 Runtime telemetri

`CONFIGURED → REACHABLE → FIRED → ACTION OBSERVED` ayrımı (Faz-3):
`ActivationDiag` (her zaman açık, feature'sız): `tail_configured/fec_configured` (configured) ·
`state_transitions, recover_arq/fwd_entries, probe_up_entries, left_preconverge_ns` (reachable) ·
`tail_probes, parity_emitted, parity_token_denied, retransmits, dups` (fired) · tahminciler
`btlbw, srtt, rtprop, queue_delay, pacer_rate/tokens, inflight` (neden). `FecActuationDiag`:
latch zinciri `requested → computed → clamped → planned → enqueued → emitted`, `rounded_to_zero_count`
(Faz-6). `ReturnPathState` (§4.8). Bench manifesti `ActivationSummary` (19 alan, şema v1).
Yalnız `diag-sched-telemetry` feature'ı ek alan açar (`KP-ACTIVATED tail_probe=`, ürün derlemesinde
`n/a` — vakum sayaç uyarısı).

**Sıkıştırma transport'ta YOKTUR** (TDD §8 AMENDMENT-4/6; sahibi Application mesaj-hazırlama — gerekçe tip bilgisi; kanonik katman grafiği App → Transport → Crypto(tel kenarı) → UDP, INV-NEIGHBOR-1).
`FusionDecision.compress` (`fusion.rs:444`) üretilen ama hiçbir yerde tüketilmeyen kalıntı bir
çıktıdır; zstd bağımlılığı yoktur. ADR-17 golden-vektör kararlılığı için v1'de bırakıldı;
kaldırılması v1.x temizliği (EVOLUTION §9.8).

**Telemetri SALT GÖZLEMDİR ve transport kontrolüne KATILMAZ** (INV-007/015): accessor'lar
`&self`, hiçbir karar `diag_*` okumaz; `sim_determinism`/`sender_diag_contract` testleri kilitler.
