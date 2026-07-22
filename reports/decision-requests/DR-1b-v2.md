# DR-1b-v2 — Parity token/budget politikası (REVİZE) [TASLAK — NO PRODUCTION IMPLEMENTATION]

> v1-blocker'larının (7) kapanışı. Taban: [problem-formalizasyonu](DR-1b-problem-formal.md) (imkânsızlık-
> üçgeni + L/R/B) · [kernel-sim](../d1-exit/1b-policy-kernel-sim.md) (Commit-R/S) · [MS1](../d1-exit/1b-micro-sweep-1.md)
> (corrigendum'lu). Üretim-`plan_fec_parity`-wiring'i YOK; canlı-motor-prototipi AYRI-KAPI (§23).

## 24-zorunlu-artefakt (talimat-§17)

**1. Resource-entitlement-kararı:** **R-sınıfı — Protected-parity-entitlement** (bounded-rezervasyon;
DATA-korunan-payı-tüketemez; work-conserving). Mimari-aday: **FloorCap** (R + F2-ticket + C1-cap).
Kernel-sim: saturated'da L-yapısal-starvation vs R-bounded-akış (üçgen-kanıtı). SEÇİM=candidate;
nihai-politika canlı-prototip-kapısına-bağlı.

**2. Reservation/borrowing-statüsü:** Rezervasyon **VAR ve açıkça-böyle-adlandırılır** (bounded-
entitlement-credit; pay=%20-[CAL]; kuantum-havuzu-bucket'ta-birikir). Borçlanma **birincil-adayda-YOK**;
B-sınıfı (BoundedDebtFloor, C2-dar: max_debt=2×UNIT, due-floor-only, accrual-önce-borç-kapatır)
**ikincil-varyant** — kernel-sim'de FloorCap'ten-AYRIŞMADI (debt=0-hep) → marjinal-değeri-kanıtsız.

**3. DATA-ordering vs token-entitlement (bağlayıcı):** "DATA wire-ordering korunur (DATA→ARQ→parity;
mühürlü); DATA'nın token-entitlement üzerindeki MUTLAK-önceliği FloorCap-modunda korunmaz." Eski-sınır
"reservation-yok" SUPERSEDE: "bounded-parity-entitlement 1b-kapsamında; queue-preemption + wire-order-
değişikliği kapsam-DIŞI." **Kullanıcı-onayı-olmadan-production'da-uygulanmaz** (§24).

**4. Floor-KOŞULLU-servis-sözleşmesi:** "**Admitted-floor-ticket**, (a) her-scheduling-opportunity'de-
kernel-çağrısı VE (b) token-feasibility (entitlement/leftover ≥ psize ∧ hardcap-izinli) koşulları-
altında, `N_floor`-eligible-opportunity-içinde [birincil-form] — T_floor-event-saatinde-karşılığıyla —
servis-edilir." Wall-clock-SLA-YOK (scheduler-wakeup'sız-garanti-yazılmaz; idle-sender'da tek-mekanizma
`next_wakeup_hint_ns` — çağıran-timer'ına-öneri; wakeup-yoksa-sözleşme-askıda-ve-miss-teşhisli).
Unconditional-"her-blok-T_floor'da-parity-alır" İFADESİ-YASAK (requested≠admitted≠serviced üçlüsü).

**5. Admission-feasibility (kernel-`admit()`-uygulandı; 9-typed-sonuç):** global-item-cap ✓ global-
byte-cap ✓ floor-ticket-cap ✓ per-block-admitted-cap ✓ hard-cap/debt-izni (grant-anında) ✓ parity-
boyut-desteği ✓ blok-canlı/¬moot (RECOVERED) ✓ duplicate-ticket-reddi ✓ profile-bound (boyut-cap'i-
temsil-eder) ✓. Her-red typed + sayaç (sessiz-yok).

**6. Floor-miss-semantiği:** admitted-ticket T_floor-aşımında (geç-servis-veya-hiç): `floor_deadline_miss`
teşhisi = {block_id, generation, reason (cap_suppressed|token_infeasible|no_opportunity|canceled),
token-opportunity-sayısı, displacement-o-ana-kadar, queue-state (pending-items/bytes/tickets)}.
Sim-kanıtı: many-blocks 19-miss-SAYILDI; low-bw 22-miss — garanti-ihlali-sessiz-DEĞİL.

**7. T_floor-modeli (kernel-`t_floor_ns`-uygulandı+testli):**
`T_floor = clamp(mult×srtt, T_min, T_max).max(serialization(psize,rate) + sched_granularity)`.
Kapalı-durumlar: srtt-yok/0→prior(300ms) · düşük-srtt→T_min(50ms)-veya-serialization-tabanı ·
RTT-5s→T_max(10s)-clamp · 30kbps→serialization-tabanı-üstün (330ms) · E64-vs-E1024→taban-farkı-testli ·
rate=0→saturating-taban · timer-overflow→saturating (2^63-trace-panik-yok) · saat=event-clock
(determinizm; wall-clock-değil) · partial-block/idle/blackout→opportunity-koşulu (§4).

**8. Per-block-fairness:** **F2 — floor-ticket-kuyruğu** SEÇİLDİ: her-admitted-blok-için-İLK-parity
ayrı-bounded-ticket-FIFO'sunda; parity-bütçesi-önce-ticket'ları-servis-eder; kalan-bulk-FIFO'ya.
DATA-ordering-değişmez. F1-RED (per-block-garanti-veremez — talimat-tespiti); F3-alternatif-admission-
biçimi-olarak-notlu (engine-realizasyonunda-değerlendirilebilir); F4-RED (cerrahi-sınır-aşımı).

**9. Parity-queue-ordering-kararı:** F2 = **parity-İÇİ-seçim-değişikliği** (ticket'lar-bulk'un-önüne).
Eski-"queue-ordering-değişmez"-sınırıyla-ilişki: DATA/ARQ/parity-ARASI-sıra-AYNEN; değişen-yalnız-
parity-alt-kuyruk-seçimi. **Bu-scope-değişikliği-açık-kullanıcı-onayına-tabidir** (§24).

**10. Floor-vs-cap-önceliği:** **C1 — hard-cap-ABSOLUTE** seçildi (cap-asla-aşılmaz → floor-miss-
mümkün + zorunlu-teşhis; "garanti"-koşullu). Karar-tablosu: {ticket-due ∧ cap-exhausted ∧ DATA-pending
∧ parity-pending ∧ deadline-yakın} → parity=hardcap_credit(=0)→miss+`cap_suppressed`-kaydı; DATA-normal-
akar; ticket-kuyrukta-kalır (sonraki-credit'te-öncelikli). C2-dar = yalnız-B-varyantında (due-floor,
max_debt=2×UNIT, birikimsiz); C3-RED (unbounded-multi-block-arrival).

**11. Hard-cap-accounting (bounded-credit state-machine; kernel-uygulandı):**
`credit += hardcap_share×pacing_rate×dt; credit = min(credit, credit_cap); settle: credit −= spent`.
Cevaplar: sliding-window-DEĞİL/leaky-bounded-credit-EVET · epoch-reset-YOK · **connection-lifetime-
kümülatif-YASAK** · idle'da-birikir-AMA-cap'li (max-credit=credit_cap=8×UNIT-[CAL]; sim: idle-sonrası-
burst-bounded) · rate=**pacing-rate** (BtlBw/delivered-DEĞİL — accrual-kaynağıyla-aynı-saat) ·
rate-düşüşünde-mevcut-credit-korunur (cap-sabit-byte) · mode-transition/rollback: `reset()` (kalıntı-
taşınmaz; sim-trace'li) · Manual/Controller-ORTAK-state (talep-tarafı-farklı-yalnız) · integer:
u64-byte, floor-division, saturating · update-order: accrual→debt-repay→limit→(çağıran-drain)→settle ·
long-pause-burst: cap-sınırlı ✓.

**12. Idle-credit-bound:** entitlement-cap=4×UNIT, hardcap-cap=8×UNIT [CAL] — testli
("idle-birikim-CAP'li") + sim-trace (credit-max-after-idle).

**13. Pending-bound'ları:** `pending_fec_items_cap`(64-[CAL]) + `pending_fec_bytes_cap`(64×UNIT-[CAL])
+ per-block-admitted-cap(8) + floor-ticket-cap(8) + parity-size-max(2048) + **stale-age-bound**:
oldest-pending-age > K_stale×T_floor → age-eviction+teşhis (stale-blok-kuyruğu-sonsuz-tutamaz);
RECOVERED-cleanup-mevcut (Commit-K-iv); alias-koruması=generation (DR-1f-binding). Aşım-politikası:
**newest-reject** (eski-blok-kararı-korunur) + age-eviction-tamamlayıcısı.

**14. manual_r-typed-semantiği:** dış-API **TYPED-REJECT** — `manual_r > r_code_max` →
`Err(FecAuthorityError::ManualRExceedsProfile)` (sessiz-clamp-RED: yanlış-konfig-gizler). İç/test-
migrasyonu: explicit-pre-clamp-helper (`min(r, r_code_max)`-çağıran-tarafında-görünür). Legacy-harness-
uyumu clamp-gerekçesi-DEĞİL (talimat). Telemetri (reject'te): requested + bound + sayaç.

**15. Mode-transition-state:** politika/mode-değişiminde `BudgetPolicyState::reset()` — credit/debt-
kalıntısı-taşınmaz (sim-trace: mode-transition ✓). Blok-stable-emisyon-semantiği (Commit-K) ÜSTTE-aynen.

**16. Rollback-state:** flag→`LeftoverOnly` tek-adım + `reset()`; kuyruk/karar-state-etkilenmez;
davranış=bugünkü-baseline-birebir.

**17. Telemetri (exact):** `admitted_r` · `floor_ticket_admitted/serviced/missed_total` (saturating) ·
`last_floor_miss` (snapshot; §6-alanları; tam-overwrite) · `cap_suppressed_events` · `entitlement_credit/
hardcap_credit/debt_bytes` (gauge) · `displacement_bytes_total` (=entitlement-spend; saturating) ·
mevcut-Commit-N-denial-attribution AYNEN · semantik-tablo-güncellemesi-zorunlu.

**18. Property-testler (kernel-UYGULANDI + engine-fazı-planı):** exhaustive-küçük-uzay-13.8k
(token-icadı-yok / cap-uyum / state-bound / work-conserving / klon-determinizm / leftover-formülü /
settle-bound) ✓ · T_floor-mekanik ✓ · admission-9-typed ✓ · kuantum-altı-birikim ("harcanmamış-grant-
eritmez") ✓ · [engine-fazı: DATA-sıra-birebir-flag-ON/OFF + N8 + displacement-bound + golden].

**19. Adversarial-trace'ler (20; sim-UYGULANDI):** offered=C/>C · no-data · bursty · one-block ·
many-blocks>floor · interleaved · rtt-50ms/1s/5s · low-bw-serialization · scheduler-gap · mode-
transition · RECOVERED-cancel · cap-exhausted · idle-credit-max · timer-wrap · size-change ·
item-cap · byte-cap — invariant'lar-tick-başı-assert + determinizm-çift-koşu.

**20. Reliable-vs-unreliable kanıt-tablosu:**
| kanıt | statü | kullanım |
|---|---|---|
| MS1-ölçümü (baseline-davranış; 10-hücre) | GÜVENİLİR | leftover-karakterizasyonu; collapse-mekanizması |
| MS1-counterfactual temiz-hücreler (fid ≤0.04) | GÜVENİLİR-göreli | model-yön-karşılaştırması |
| MS1-counterfactual contended (loss20/rev99/d1000/d2500/offered-c1) | **UNRELIABLE** | yalnız-yön-işareti; exact-hüküm-YASAK |
| Kernel-sim (Commit-S) | GÜVENİLİR (kernel-matematiği) | sınıf-semantiği/invariant/fairness-kanıtı; **kanal-feedback-İÇERMEZ** |
| Canlı-motor-prototipi | YOK (gelecek-kapı) | contended-hedef-rejimin-NİHAİ-kanıtı |

**21. Policy-kernel-sonuçları:** [1b-policy-kernel-sim.md](../d1-exit/1b-policy-kernel-sim.md) —
saturated: L=0-parity (üçgen), R=bounded-akış (displaced==spend ≤pay); FloorCap: per-block-ilk-parity
+ dürüst-miss (91-servis/19-miss); idle/wrap/transition/cancel/cap-trace'leri-temiz; kernel-revizyon-
bulguları (iki-faz-settle; metrik-artefaktı) belgeli.

**22. Candidate-karşılaştırması:** L=baseline (saturated-starvation) · ProtectedShare=bounded-pay,
fairness'siz · **FloorCap=BİRİNCİL-ADAY** (pay+fairness+cap+miss-dürüstlüğü) · BoundedDebt=ikincil-
varyant (sim'de-ayrışmadı) · Model-isimleri-mekanizmadan-ayrıldı (eski-"Model-B"-parametre-reddi
bounded-credit-KAVRAMINI-reddetmez — hard-cap-zaten-bounded-credit).

**23. CANLI-MOTOR-PROTOTİP-KAPISI (bu-turda-UYGULANMADI; öneri-§hüküm):** kurallar — default-
`LeftoverOnly`-birebir; non-default-experimental-flag; product/runtime-activation-YOK; DR-1f-bağımsız;
sabit-manual-demand-arm'ları (Controller-arm-sonuçları-kalibrasyonsuz-policy-evidence-SAYILMAZ);
contended-hücrelerde-GERÇEK-feedback-loop (MS1-harness-tabanı); ≥10-paired-seed (bimodal-30);
before/after-ledger; DATA-displacement/tail-latency; N8; tüm-mevcut-kapılar.

**24. KULLANICI-ONAYI-GEREKTİREN-scope-değişiklikleri:** (a) bounded-parity-entitlement (rezervasyon —
"reservation-yok"-sınırının-süpersedesi; production'da-uygulama-onayı) · (b) F2-parity-içi-ordering-
değişikliği · (c) manual_r-typed-reject (dış-API-davranış-değişikliği) · (d) canlı-prototip-koşusu ·
(e) [CAL]-sabitlerinin-default-seçimi. Hiçbiri-bu-turda-uygulanmadı.
