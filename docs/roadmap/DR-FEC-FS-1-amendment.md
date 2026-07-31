# ROADMAP AMENDMENT — DR-FEC-FS-1 (FS-ROADMAP-AMEND)

**Statü:** Program 2 (FEC) yürütme-yolu resmî değişikliği. Eski `C0→C1→C2→C3→D` yolu SİLİNMEZ; tarihsel
kanıt korunur, kalan-yürütme `DR-FEC-FS-1 v2` (full-scale FEC utility/emission/adaptation characterization)
ile **SUPERSEDE** edilir. Bu amendman FS-PREREG ile birlikte/öncesinde dondurulur (§4/§27-6).

## 1. TARİHSEL KANIT — BASELINE (yeniden-keşfedilmeyecek; provenance-doğrulamaya-tabi)

```text
FS-wallclock (faithful two-netns; 48 koşu)  → RTT≤250ms POZİTİF marjinal-FEC (Beneficial); RTT≥1000 nötr
   §emisyon-modeli-sınırı: parity@blok-kapanış + ACK-clocked-source → yüksek-RTT'de head-start yok
   §DEADLINE (FS-CORRIGENDUM): MarginalFecEssential yalnız deficit≤2 (d1=100%,d2=93.3%); d4=13.3% GERİ-ÇEKİLDİ
Tarama-Kesit-1  → EXPLORATORY SCREENING (NOT calibration); 24/24-nokta en-iyi-goodput bir-ARQ-arm'ında
   sınır: 3-seed, requested-vs-realized-r, arm-collapse, censoring, knobs≠FusionInputs
DR-1a  → FEC-aktüasyon MEKANİZMASI kapandı (fusion→R→authority→latch→enqueue); PRODUCT-INTEGRATION-DEFERRED
   (default Manual{false}; Controller yalnız experimental — bkz. FS-REALITY-AUDIT §3.2)
DR-1b  → parity-SCHEDULING-policy (Leftover vs F15): netem-koşuldu → outcome-policy-bağımsız → LEFTOVER-RETAINED
   (deterministik-F15 sim-artefaktıydı; F15 experimental-flag-arkası korunur)
```
Bu kanıtlar **yeniden-koşulmaz** (provenance/enstrümantasyon gerektirmedikçe). FS bunların üzerine
eksik-nedensel-soruları kapatır (aynı-byte-duplication kontrolü, emisyon-aileleri, oracle, observability).

## 2. SUPERSESSION — C0→D → DR-FEC-FS-1

| Eski C-fazı | Niyet | DR-FEC-FS-1 karşılığı |
|---|---|---|
| C0 enstrümantasyon + P2 deadline-corrigendum | 9-timestamp ledger + deadline-düzeltme | **FS-PREREG §8.1 timing-chain + FS-CORRIGENDUM (yapıldı)** |
| C1 dar baseline | ilk-ölçüm | **FS-1 mechanism-screening** |
| C2 karar-gate | crossover-var-mı | **FS-2 response-surface + FS-4 oracle-gate** |
| C3 flag'li impl | family-impl | **FS-1..3 F0–F5 aileleri (experimental-flag; §11 safety)** |
| D tam crossover-map + GO/K4 | nihai-hüküm | **FS-4..7 + FS-DUR-SON (FULL-GO/NICHE-GO/RESEARCH-ONLY/K4-RETREAT)** |

**DR-1b faithful-revalidation** ayrı-tutulmaz: **FS-3 (profile/scheduler characterization)** eksenine katlanır;
eski "Leftover-kazandı" gerekçesi faithful-kanıt desteklemedikçe korunmaz (§15).

## 3. K4 KORUNUR
`K4-RETREAT` (dürüst geri-çekme) olası-nihai-sonuç olarak **korunur** ama artık §17 minimum-kanıt-barajına
bağlı (oracle + aynı-byte-duplication + confirmatory-niche + ≥1-alternatif-emisyon-aile + observability-
dışlaması). Barajsız-K4 YASAK; onun yerine `INCONCLUSIVE / RESEARCH-ONLY-PENDING` ara-statüsü.

## 4. STATUS-BOARD ETKİSİ (silinmeden)
`docs/roadmap/kama-transport-roadmap-status-board.md`:
- §3 (C0→D metni) KORUNUR + üst-not "**SUPERSEDED-by DR-FEC-FS-1**" eklenir (tarihsel).
- FEC-satırları (emission-timing NO-GO / marginal-utility NO-GO) DEĞİŞMEZ; "Sonraki adım" → DR-FEC-FS-1 fazları.
- Nihai-hüküm ancak FS-DUR-SON + Abkarada-onayı ile değişir (self-ratifying-değil).

## 5. SINIRLAR (§0 tekrarı)
Merge/flip/PR-merge YOK · sealed-crypto/Noise AÇILMAZ · A1 cold-start AÇILMAZ · cold-start'a-FEC-sokma YOK ·
güvenlik/kaynak-invariant gevşetme YOK · sealed-ordering/wire sessizce-değiştirme YOK. Yeni-in-stack-FEC-aile
wire/protokol-genişletme gerektirirse → DUR-B.
