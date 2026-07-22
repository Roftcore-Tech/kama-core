# FP — COLD-START LIVENESS CLASSIFICATION (Phase-A)

> **Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün kollarda aktiftir; ölçülen şey ARQ üzerine eklenen
> proaktif FEC'in marjinal faydasıdır.** (Bu faz UTILITY değil LIVENESS sınıflandırır.)

## HÜKÜM

> **COLD-START LIVENESS DEFECT CONFIRMED — PROGRESS DR REQUIRED**
>
> Nitelik: kalıcı-silent-deadlock DEĞİL (ACK-bağımsız §3.3-B geometrik-kopya-kurtarması var), AMA iki
> DEFECT-göstergesi LİTERAL olarak karşılanır — **(i) PreConverge'de PTO arm/fire OLMUYOR** (donuk),
> **(ii) ACK olmadan state-machine ilerleyemiyor** (yapısal-ACK-kapılı). Buna eklenen 31 B/s PreConverge-
> dispatch-floor + geometrik-kurtarma-timescale'i (~61.2s@1200B) birlikte, hedef-kullanım (hostile-ağ;
> bağlantı-başında first-flight-loss / reverse-blackout SIK) için gerçek bir cold-start-liveness-zaafıdır.
> Düzeltme bu turda UYGULANMADI (ayrı kullanıcı-onayı; §Seçenekler+Riskler).

## 1. PROGRESS INVARIANT (PRE-DECLARED — koddan, sonuç-görülmeden)

Sender saf-reaktif state-machine'dir: tek-mutasyon-girişi `on_event`; **kendi-arm-eden-timer YOK, sonraki-
uyanış-çıktısı YOK** (tüm public-API 173–2952). Bu, invariant'ın çekirdeğidir.

- **PreConverge çıkışı YAPISAL-ACK-KAPILI:** çıkış yalnız `rtt.srtt().is_some()` iken Startup'a (sender.rs
  1382–1388); `srtt` yalnız boş-olmayan-ACK'in RTT-örneğiyle set-edilir (`observe`, 1136; boş-ACK erken-
  döner 987–989). ACK-yok → PreConverge'de kalıcı-pinli.
- **PTO PreConverge'de DONUK:** time-threshold-loss + retransmit gate'lenir (1177 `if state != PreConverge`).
  ack_confidence FULL-pinli (2800–2801) → emergency/RecoverFwd de bypass.
- **Tek ACK-bağımsız ilerleme = §3.3-B geometrik-kopya:** ilk-flight-unit `dran_new_data`'da kuyruklanır
  (2727–2742); `first_delay = (3/2)·(unit_size + PC_SACK_WIRE=65)/PRE_CONVERGE_FLOOR_BW=31` saniye; sonra
  ×2-backoff, sınırsız, ACK-iptaline-kadar. `schedule_transmit` her `on_event`'te koşar (982) → caller-
  clocked (Sender kendisi-uyanmaz).
- **Dispatch PreConverge'de 1-birim-floor + 31 B/s refill** (968–975, 1622–1623): initial-burst 1-unit;
  sonraki-birim ~38.7s (1200B/31) refill-bekler.

**PRE-DECLARED BOUND:** ilk-eligible-send'den sonra ACK-bağımsız bir Duplicate en-geç **2×first_delay**
içinde emit-edilmeli (aksi = SilentStall). State-ilerleme ACK-YOKKEN yapısal-imkansız (bu bir liveness-
özelliği; harness-artefaktı-değil).

## 2. SENARYO MATRİSİ + SONUÇLAR

### Direct-integration (GERÇEK Sender code-path; simüle-zaman now_ns elle-ilerletilir; gerçek-uyku-YOK)
Test: `crates/adaptive-transport/tests/cold_start_liveness.rs` (4 test yeşil).

| Senaryo | Sonuç | Sınıf |
|---|---|---|
| İlk-source-kaybı + ACK-yok | t=0 Data(1-birim); ilk-Duplicate first_delay'de; state PreConverge-pinli | ProgressViaProactiveRepair (caller-clocked) |
| İlk-ACK-kaybı (Sender-perspektifi = ACK-yok) | Yukarıyla-özdeş (Sender ACK-görmüyor) | ProgressViaProactiveRepair |
| Hayatta-kalan-ACK | ACK-teslim → state PreConverge→Startup | ACK-mekanizması-çalışır (kontrol) |
| Zaman-tek-başına (5×first_delay, ACK-yok) | state hep PreConverge; Duplicate'ler var | ACK-gated (timer-gated-değil) |

**Ölçülen geometrik-kopya timeline (evidence `data/cold-start/summary.csv`):**

| unit_size | first_delay | gözlenen-ilk-Duplicate | 2×bound-içinde | dup-sayısı | state-pinli |
|---|---|---|---|---|---|
| 65 B | 6.3 s | **6.3 s** | ✓ | 2 | ✓ |
| 300 B | 17.7 s | **17.7 s** | ✓ | 2 | ✓ |
| 1200 B | 61.2 s | **61.2 s** | ✓ | 2 | ✓ |

1200B timeline: `t=0.00 Data(seq0)` → `t=61.25 Duplicate(unit1,fresh-seq)` → `t=183.70 Duplicate` (×2-backoff).
→ Bound (2×first_delay) KARŞILANIR; kalıcı-silent-deadlock YOK. Ama recovery ~61s@1200B.

### Wire-arm (UDP-runner over FAITHFUL harness; RTT 100ms; block-0 first-source-loss; ext 90s)
`data/../coldstart-wire`: block-0 **90s içinde TAMAMLANMADI** (pending); rx 8s-idle-sonrası-terk-etti
(cold-start-dispatch-aralığı ~38s ≫ rx-sabrı 8s). receiver_events sadece-1.

### Harness-vs-production ayrımı (§4.5)
Direct-integration = GERÇEK Sender-kod-yolu → davranış Sender'da (üretim), harness'te-değil. Wire-arm
uç-uca-sonucu doğrular (etkin-stall). **İki-arm-uyumlu → TRANSPORT-STATE-MACHINE liveness-özelliği,
harness-artefaktı-DEĞİL.**

## 3. DEFECT-vs-PASS DEĞERLENDİRMESİ

| Kriter | Gözlem | Yön |
|---|---|---|
| Bounded-progress (recoverable) | Geometrik-kopya 2×first_delay-içinde | PASS-yönü |
| PTO arm/fire | PreConverge'de DONUK (1177) | **DEFECT-göstergesi** |
| Initial-pacing-floor kalıcı-blok | Hayır (31 B/s-refill; glacial-ama-kalıcı-değil) | borderline |
| ACK'siz state-ilerleme | İMKANSIZ (yapısal-ACK-kapılı) | **DEFECT-göstergesi** |
| Silent-stall bound-aşımı | Hayır (geometrik-kopya-var) ama bound ~61s@1200B | borderline |

İki DEFECT-göstergesi (PTO-arm-yok, ACK'siz-state-ilerlemez) LİTERAL karşılanır → **DEFECT CONFIRMED**
(silent-deadlock-nitelemesi-değil; "yavaş-bounded + yapısal-ACK-bağımlılık").

## 4. MİNİMAL REPRODUCER

`crates/adaptive-transport/tests/cold_start_liveness.rs::cold_start_first_loss_recovers_ack_free_but_state_pinned`
— GERÇEK Sender, ilk-paket-kaybı, ACK-yok, simüle-zaman → state-pinli + geometrik-kopya first_delay'de.
(Wire-tekrarı: faithful-harness RTT-100ms + `--deficit 1 --deficit-from-block 0`.)

## 5. DÜZELTME SEÇENEKLERİ (RAPORLANDI; UYGULANMADI — ayrı-onay)

1. **PreConverge'de PTO'yu INITIAL_RTT(=100ms) ile aç:** ilk-flight-loss bounded-retransmit-alır
   (~birkaç×100ms, ~61s-yerine). *Risk:* gerçek-RTT≫100ms (2.5s-linkler) → prematüre-retransmit / self-
   congestion; recovery-obsesif-INV-RATE-1 ile etkileşir.
2. **PreConverge minimum-progress-timer:** first_delay'i bounded-tavana (birkaç×INITIAL_RTT) kırp. *Risk:*
   yavaş-kanal-koruması (31 B/s-floor'un-VARLIK-nedeni: bilinmeyen-kapasiteyi-boğmama) zayıflar.
3. **State-çıkışını bounded-probe ile (kesin-ACK-yerine):** konservatif-initial-RTT'li-probe. *Risk:*
   RTT-örneği-olmadan-ramp → aşırı-gönderim.
4. **PreConverge-dispatch-floor'u yükselt (31 B/s):** cold-start-throughput-artar. *Risk:* yavaş-kanal-
   flooding (tam-bu-yüzden-düşük).

**Ortak-risk:** cold-start-liveness ↔ yavaş-kanal-koruması gerilimi. Bu bir TASARIM-kararı (DR), trivial-
fix-değil. Öneri: PROGRESS DR — bounded-cold-start-recovery + slow-channel-safety birlikte-optimize.

## 6. KAPSAM
Üretim-yolu DEĞİŞMEDİ (yalnız test + mevcut `state()` accessor). Fix UYGULANMADI. Production-fuzzy-config +
LeftoverOnly + F15/P15 DOKUNULMADI.
