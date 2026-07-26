# KAMA TRANSPORT — BİRLEŞİK ROADMAP VE YAŞAYAN STATUS-BOARD

**Statü:** ONAYLI — **ROADMAP GO** (Abkarada, 2026-07-26) · **Belge tipi:** resmî repo belgesi
(`docs/roadmap/`) · **Güncelleme yetkisi:** yalnız DUR-SON hükümleriyle; her satır kanıt-linki taşır.
**Bağlayıcı çerçeve:** Bu program ARQ ile FEC'i yarıştırmaz; ARQ tüm kollarda aktiftir. Ölçülen şey
ARQ substratı üzerine eklenen yapıların **marjinal** değeridir. "ARQ FEC'i yendi / FEC ARQ'dan iyi"
türü ifadeler YASAKTIR.

---

## 0. İKİ KONTROLLÜ PROGRAM

```text
Program 1 — COLD-START:  uçtan uca kapat ve FREEZE et            (A1 → A2 → B)
Program 2 — FEC:         gerçekten faydalı bir çalışma bölgesi
                         var mı — ölç, tasarla, tekrar ölç,
                         gerekirse DÜRÜSTÇE geri çek             (C0 → C1 → C2 → C3 → D)
```

Sıra SERİDİR: `A1 → A2 → B → C0 → C1 → C2 → C3 → D`. Tek-DR-tek-konu: cold-start ile FEC asla
aynı DR'da birleştirilmez. Bu bir yeniden-tasarım arayışı değil, iki kontrollü programın
yürütülmesidir.

---

## 1. YAŞAYAN STATUS-BOARD (2026-07-26 itibarıyla)

| Alan | Durum | Hüküm | Sonraki adım |
|---|---|---|---|
| Cold-start kök nedeni | Çözüldü (workload-vs-initial-cwnd) | GO | — |
| Sender external wakeup / batch API | Uygulandı | GO | B'de mühürlenecek |
| Probe/echo bootstrap RTT | Uygulandı | GO | B'de mühürlenecek |
| PreConverge→Startup + PTO binding (+re-arm) | Doğrulandı | GO | B'de mühürlenecek |
| Typed timeout + epoch semantiği | Uygulandı | GO | B'de mühürlenecek |
| W-1 first-source-loss recovery | Decisive PASS (300/1200B) | GO | — |
| W-2A exact-once first-SACK self-heal | PASS 27/27 | GO | — |
| **W-2B faithful ACK-starvation — probe→PTO dalı** | Tam doğrulanmadı | **OPEN** | **A1** |
| **W-3 restore=61 @ 65B** | UNOBSERVED (harness-horizon) | **PARTIAL** | **A1** |
| DI-2B sustained reverse-blackout | PASS (3 boyut) | GO | — |
| Receiver wire resource shape (O(1)/bounded) | Doğrulandı | PARTIAL GO | A2 |
| **Product receiver patience** | Uygulanmadı | **NO-GO** | **A2** |
| **Product receiver resource lifecycle** | Uygulanmadı | **NO-GO** | **A2 (iki-seviye cap)** |
| Mixed-version codec güvenliği | Doğrulandı (g9) | GO | flip-öncesi rollout-politikası ayrı |
| Runtime/deployment interoperability | Kısmi | PARTIAL | flip-öncesi |
| FEC codec | Çalışıyor | GO | — |
| **FEC emission timing** | Çözülmedi (parity-at-block-close) | **NO-GO** | **C0→C3** |
| **FEC marginal utility** | Kanıtlanmadı (tek istisna: MarginalFecBeneficial@RTT250) | **NO-GO** | **C1→D** |
| Hostile-network readiness | Kanıtlanmadı | NO-GO | Program 1+2 |
| Feature default flip (cold-start) | Yasak | NO-GO | A2 + rollout + §4.4-benchmark sonrası karar |
| PR #44 | DRAFT | NO-GO (merge) | Abkarada kararı |

---

## 2. PROGRAM 1 — COLD-START

### A1 — Kalan faithful hücreler *(harness/test-only; PRODUCTION CODE DEĞİŞİKLİĞİ SIFIR)*
Kapsam:
- **(1) W-3-65** — settle-exit kapalı + observation-horizon ≥120 s; restore=61; beklenen zincir
  pre-registered (TimedOut@60 → restore@61 → sonraki-PTO → retransmit-wire → DATA-ACK →
  PostTimeoutRecovery).
- **(2) W-2B** — faithful ACK-starvation; normatif suppression protokolü ve 25-alan ledger
  (bkz. TUR-A1 talimatı §2): release-condition = ilk existing-PTO retransmit wire-emission;
  fallback T_MAX → release + FAIL/PARTIAL; suppressed_ack_count sınırlı. §2.4-a spurious
  maliyetinin İLK gerçek-tel doğrulaması.
- **(3) H1-grep** — tüm harness/script'lerde `$( … & … )` sınıfı desen denetimi; bulgu →
  etkilenen kanıt INSTRUMENT-SUSPECT + DUR-B.

Çıkış hükmü: `DR-CS-1 SENDER-SIDE ACCEPTANCE: FULL` veya `PARTIAL — [EXACT]`.

### A2 — Ürün-alıcı patience/resource entegrasyonu *(talimatı A1-DUR-SON'dan SONRA yazılır)*
Kapsam: durum-analizi §12 on-bir maddesi + DI-10 bağlayıcı kısıtları + **İKİ-SEVİYE CAP
(Abkarada şartı):**
```text
per-session / per-flow cold block+byte cap
global process-wide cold-state block+byte cap
cap REZERVASYONU allocation'dan ÖNCE
session close/timeout'ta global bütçe İADE edilir
cap aşımı → typed diagnostic; sessiz eviction YOK
probe/echo InboundControlDedup.admit() yoluna KESİNLİKLE girmez (stateless-echo zorunlu)
flood altında MEMORY PLATEAU testi
```
`RX_COLD_BLOCK_CAP=256 / RX_COLD_BYTE_CAP≈2.46MB` = **[CAL] başlangıç değeridir**; ürün sabiti
ancak gerçek eşzamanlı-session bütçesiyle doğrulanınca donar. Patience: EK4-A1 profil-türevi
formül + tablo. Kabul: DI-10 ürün-varyantları + **W-3-65'in ürün-alıcı üstünde reruni** + plateau.
Çıkış hükmü: `END-TO-END PRODUCT COLD-START FIX VALIDATED` *(default-flip AYRI: coordinated
rollout / min-peer-version / federation-policy + §4.4-benchmark)*.

### B — COLD-START FREEZE *(yalnız A1+A2 tamamlandıktan sonra)*
Freeze belgesi şunları MÜHÜRLER:
```text
probe codepoint/layout (0x19=CTRL_CS_PROBE 44B · 0x1A=CTRL_CS_ECHO 43B · 0x12=RECOVERED kilidi)
probe ladder (N=4; fire 3/9/21/45s; ≤176B nominal / 200B hard)
batch ordering (on_event_batch 4-faz; Tick≡boş-batch; adapter per-iterasyon damgalama)
PTO binding (bootstrap-sonrası + re-arm yaşam döngüsü; next_wakeup ∈ {None, gelecek})
timeout semantics (epoch-başına-1; Seçenek-A; PostTimeoutRecovery)
receiver patience formula (EK4-A1 max-formülü; profil-türevi tablo)
receiver resource path (stateless-echo; iki-seviye cap; bounds-before-alloc)
acceptance pins (F4 + A1 + A2 test setleri = kalıcı regresyon kapıları)
```
Freeze sonrası cold-start üzerinde yalnız **correctness / security / kanıtlanmış-regresyon** DR'ı
açılabilir. **Tuning amaçlı v8 AÇILMAZ.** Ladder-tuning, ACK-refresh, CS-içine-FEC, fuzzy
değişikliği YASAK.

---

## 3. PROGRAM 2 — FEC (DR-FEC-ET-1: PARITY AVAILABILITY / EMISSION TIMING)

**Araştırma sorusu:** *Parity, mevcut ARQ retransmitten önce receiver'a ulaşabilecek kadar erken
wire'a çıkıyor mu?* Önce baseline ölçülür; çözüm doğrudan yazılmaz.

### C0 — Enstrümantasyon + P2 deadline-corrigendum *(koşusuz + ledger kodu)*
- **9 zorunlu timestamp — TEK monotonic clock domain:** `t_source_loss · t_parity_eligible ·
  t_parity_enqueued · t_parity_wire · t_parity_arrival · t_arq_eligible · t_arq_wire ·
  t_arq_arrival · t_block_complete`
- **Per-block zorunlu metadata:** `block_id · profile_id · source_count · repair_count ·
  usable_rate · configured_rtt · actual_rtt · loss_injection_id · completion_cause`
- `deadline_hit` YALNIZ block-timestamp'lerinden hesaplanır; runner-kapanışı/toplu-test-süresi
  deadline vekili OLAMAZ. P2: `deadline_hit = completion ≤ deadline` per-block + dört ayrık
  metrik; MarginalFecEssential ya doğrulanır ya resmen geri çekilir.
- **K-kurallarının sayısal alanlarının DONDURULMASI (C1 verisi görülmeden; hash+timestamp):**
  `N_REPETITIONS · JITTER_BUDGET · MIN_MEANINGFUL_HEADSTART · MAX_CLEAN_LINK_OVERHEAD ·
  MAX_LOW_RTT_REGRESSION · MIN_DEADLINE_SUCCESS_DELTA` — değerler ölçüm-altyapısı + ürün
  gereksiniminden seçilir, outcome'dan SEÇİLEMEZ.

### C1 — DAR BASELINE *(mevcut mimari; amaç TANI — utility haritası DEĞİL)*
```text
Çekirdek matris:  RTT {250, 1000, 2500 ms} × loss {exact-1, exact-2}
                  × mode {ARQ-only, current ARQ+FEC}
Kontrol:          clean/no-loss @ RTT250 · clean/no-loss @ RTT2500
M-A ekseni:       sustained reverse-ACK blackout · forward açık ·
                  cold-start önceden kurulmuş / SRTT mevcut
```
**Zorunlu dört-aşama ayrımı (her parity-gecikmesi için):** hiç-eligible-olmadı mı? /
eligible-ama-enqueue-edilmedi mi? / enqueue-ama-wire'a-çıkmadı mı? /
wire'a-çıktı-ama-geç-mi-ulaştı? — bu ayrım olmadan "FEC geç kaldı" tek kök-neden gibi görünür.
**Ana metrik:** `FEC_HEADSTART = ARQ_REPAIR_ARRIVAL − FEC_REPAIR_AVAILABILITY`.
8-RTT'lik tam matris C1'de KOŞULMAZ (Δ3 kararı: dar baseline).

### C2 — Tasarım seçenekleri + KARAR KAPISI (Abkarada)
Aday aileler: incremental-parity · kısmi-blok parity · dispatch'i tam-ACK-clock'a bağlamama ·
time/byte/deadline-tabanlı block-close · sınırlı parity-scheduling-önceliği · (enstrümantasyon
zaten C0'da). **Mühür-çarpışma haritası ZORUNLU.** İlk-kademe adayı: logical-close@enqueue +
kısmi-blok/adaptif-K (wire-değişikliği-yok, K_actual-header-mevcut, mühür-çarpışması-yok);
parity-önceliği DATA-önce mührüne dokunur → koşullu, Abkarada onayına kapılı; dispatch'i
ACK-clock'tan koparma → DR-FEC-ET-2 adayı (ayrı DR).

### C3 — Seçilen modelin flag'li implementasyonu + dar head-to-head
Davranışsal-özdeşlik (flag-OFF byte-özdeş) + C1 hücrelerinin AYNI matrisle reruni →
K-kuralları ön-değerlendirmesi.

### D — TAM CROSSOVER / UTILITY HARİTASI *(yalnız seçilen model üstünde)*
```text
RTT:  100 · 250 · 350 · 500 · 650 · 750 · 1000 · 2500 ms
loss: exact-1-source · exact-2-source · burst · reverse-ACK-loss (AYRI)
```
9-timestamp + metadata tam sette. Utility haritası = white-paper'ın gerçek veri seti.
§15-GO-kriterleri değerlendirmesi → FEC GO **veya** K4 dürüst geri çekilme.
Controller/fuzzy kalibrasyonu (P5b/P7) yalnız D'den SONRA.

---

## 4. KARAR KURALLARI — PRE-REGISTERED *(C0'da donar; üçüncü/bulanık sonuç YASAK)*

- **K1 (head-start restorasyonu):** yüksek-RTT predeclared hücrelerde: tekrarların ÇOĞUNLUĞUNDA
  `FEC_HEADSTART > 0` **ve** median headstart > `JITTER_BUDGET` (ve ≥ `MIN_MEANINGFUL_HEADSTART`)
  **ve** completion/deadline sonucu aynı yönde iyileşiyor.
- **K2 (reverse-blackout nişi — revize):** reverse-blackout altında seçilen FEC modeli, ARQ-only
  kola karşı **önceden dondurulmuş completion/deadline metriğinde anlamlı ve tekrarlanabilir
  avantaj** üretmelidir. `OFF-imkânsız` gözlemi ÇOK GÜÇLÜ niş kanıtıdır fakat zorunlu başarı
  kriteri DEĞİLDİR.
- **K3 (zararsızlık):** clean-link parity israfı ≤ `MAX_CLEAN_LINK_OVERHEAD` · low-RTT regresyonu
  ≤ `MAX_LOW_RTT_REGRESSION` · düşük bantta source-DATA boğulmaz.
- **K4 (dürüst geri çekilme — exact hüküm metni):**
  > **EMISSION TIMING FIXED — MARGINAL UTILITY NOT DEMONSTRATED — ARQ-ONLY REMAINS DEFAULT;
  > FEC RETAINED ONLY AS AN EXPERIMENTAL NICHE FEATURE.**

## 5. FEC GO KRİTERLERİ (§15 — yedi kanıt; D'de değerlendirilir)
Aynı workload/loss/RTT/ARQ/pacing altında ARQ+FEC completion/deadline başarısını anlamlı artırır
VE wire/CPU/memory maliyeti kabul edilebilir. Zorunlu kanıtlar: temiz linkte gereksiz parity yok ·
kayıplı linkte parity ARQ'dan önce · düşük bantta DATA boğulmaz · reverse-blackout hakkında yanlış
iddia yok · controller gerçekten faydalı rejimde açar · block-close/parity timing ölçülebilir ve
bounded · deadline başarısı istatistiksel anlamlı artar.

## 6. PROGRAM-BOYU YASAKLAR (§16)
fuzzy threshold tuning · loss-sensor eşik değişikliği · daha fazla parity üretmek · codec
optimizasyonuna gömülmek · FEC profil sayısını artırmak · high-RTT'de FEC-kapatma kuralı yazmak ·
cold-start ile FEC'i aynı DR'da birleştirmek · PR-merge (karar Abkarada) · Sprint-6 · DR-1f ·
DR-1d-full.

## 7. WORKFLOW (kanıtlanmış döngünün kodifikasyonu)
1. **Tasarım döngüsü:** Claude taslak → Abkarada acımasız inceleme → dar-yetkili revizyon
   (amendman > gövde; EK-deseni; sürümsüz iç başlık) → bağlayıcı talimat. Her tur yalnız kalan
   deltayı işler.
2. **İcra döngüsü (Claude Code turu):** F-fazlar + DUR-A/B/SON · her ölçüm öncesi ENSTRÜMAN
   ZİNCİRİ: faithful-acceptance 6/6 → overlap-gate `receiver_events>0` → binary-provenance
   byte-check → argüman-paritesi denetimi · bound/beklentiler veri görülmeden hash+timestamp'le
   donar · hüküm-öncesi adversarial-verify (skeptik ajanlar) · dürüst-partial hüküm formatları.
3. **Kayıt döngüsü:** repo DR-doc + arşiv-deseni (sessiz silme yok; CANCELLED/SUPERSEDED üst-not)
   + tek-cümle hüküm satırı + BU status-board'un güncellenmesi. DCP/müşteri teknik
   değerlendirmesine giren artefakt bu panodur.
4. **Rol ayrımı:** Abkarada = karar/inceleme otoritesi (DUR kapıları, mühür amendmanları) ·
   Claude (chat) = tasarım + talimat + hakem-sentez · Claude Code = icra + ölçüm · adversarial
   ajanlar = hüküm-öncesi çürütme.
5. **Tek-DR-tek-konu** ve ölçüm-hüküm-kapsam kuralı: bir ölçüm hükmü yalnız o ölçüm tasarımının
   taşıyabileceği iddiayı taşır.

## 8. GOVERNANCE
DUR-A = rapor-et-ve-devam · DUR-B = bekle-onay · DUR-SON = tam rapor + dur. Mühürlü bölge
değişikliği yalnız açık, dar, isimli amendmanla. Her hüküm satırı kanıt-linkiyle. Pre-registered
değerler outcome'dan seçilemez. Bu belge yalnız DUR-SON hükümleriyle güncellenir.
