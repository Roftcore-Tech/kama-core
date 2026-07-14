# b2-rtt-hijyen-v1 — Aralıklı-RTT-Kirliliği: EAI-Breach + FLOW-Only-rtprop + Provizyonel-Yükseliş [TASLAK r4 — ONAY BEKLİYOR]

> **r3→r4 (doğrulama wf_cf75fa72 — 8. hesapsız-fix; ÜÇ kök-bulgu, ikisi r3-kararlarımı vurdu):**
> (K1) **çok-döngülü K_SPAN^m rtprop-ratchet:** GAP-DOWN örnekleri rtprop-min'e girip rezidan
> kalıyor + dürüst-min yaşlanınca rtprop YÜKSELİYOR → tavan (K_SPAN×rtprop) HAREKETLİ → srtt
> 1→4→16→65s (~11.5dk); r3-assert'im vacuous-PASS. **→ rtprop-min YALNIZ FLOW'dan beslenir (§1-C):
> GAP-UP+GAP-DOWN çıkar → rtprop saldırıyla oynatılamaz → tavan SABİT.** (K2) **breach=gap>rtprop
> hedef-rejimi ÖLDÜRÜR** (r3-kararım): sub-kbps'te EAI≫rtprop → her ack breach → committed hiç
> beslenmez. **→ breach r2-EAI-eşiğine GERİ (gap > K_CONF_EAI×EAI); çıplak-rtprop-r3-hatam geri-
> alındı** (hesap: 0.25k/p5 normal-ack FLOW✓, p6b/1k kopma GAP✓). (K3) **rttvar_cand TANIMSIZ** →
> RFC-6298-kalıbı pin (§1-D). + FLOW-EWMA-yakınsama düzeltmesi; S4-tavan-pin-(v); n=1-GAP-DOWN
> gerekçesi. **Provizyonel-çekirdek (Abkarada A-kararı) KORUNDU** — breach-çerçevem düzeltildi +
> rtprop-FLOW-only eklendi = YÖN-DEĞİŞİKLİĞİ (r3-breach-kararı geri), onaya sunulur.

> **Statü:** D0-TASARIM-NOTU (B2, DV-3-öncesi dar-tur). r2 NO-GO → r3 = PATCH-B2-HIGH
> ((A)-yapısal-fren, karar-sabit) + 4 S-kararı işli. ONAYDAN SONRA kod.
> **Kapsam-sınırı:** estimator-dar — `sender.rs` ack-yolu + `rtt.rs` (candidate-track dahil —
> büyüme kapsam-içi, talimat) + testler + TRAJ; **başka her şey = İHLAL, DUR.**
>
> **r2→r3 (talimat b2-r3-talimati.md + 7. hesapsız-fix kaydı):** r2'nin n-koruması ratchet'i
> KIRMIYOR yavaşlatıyordu — n=1-EWMA'da her kopma-örneği eşiğin-hemen-altında seçilirse
> srtt_{i+1} ≈ 1.25·srtt_i ÜSTEL (talimat-aritmetiği; doğrulama-hesabım: 2.10s→19.6s
> **10 adım** — "düzinelerce/pratikte-ölü" hükmüm matematiksel-savunulamazdı; 13-adımlık dizi
> jammer-tasarlanabilir). r3-tasarımı eşik-makinesini TOPYEKÛN atar (kör-bant kavramı kökten
> ölür) ve yükselişi provizyonel-track'e alır: **saldırı-serisi çarpanı 1.25^k → 1.00**
> (commit-tavanı adım-sayısından bağımsız — §2-ispat). n=1 yeniden-adlandırıldı:
> "tek-örnek amplifikasyon-sınırlayıcı" (merdiven-kırıcı DEĞİL — talimat-1).

## 0. HÜKÜM — dört-halkalı bit-exact zincir (r2'den KORUNDU; talimat-onaylı)

1. Kirli-örnek: kopma-öncesi-orijinalin straggler-ack'i, NORMAL-inflight-yolu (raw=19.63s;
   tombstone/dup-adayları sayaç/kodla çürütüldü; sınıf kapı-agnostik).
2. Guard-varsayımı kırık: `apply_ack_delay` yalnız AŞAĞI-yön; "raw = güvenilir kanal-üst-sınırı"
   varsayımı aralıklıda ölür (raw = kopma-bekleyişi).
3. n=64 kuantum-amplifikatörü: tek örnek → tam-yakınsama (bit-exact).
4. rtprop tek-örnek-reseed (prune-on-observe dejeneresi) + Karn-açlığı kalıcılığı.
   Hasar: Little-516, conf-58.8s-körlük, 16.5s-freeze → 18/20.

## 1. r3-FIX: **ÜÇ-SINIF ÖRNEK-KAPISI + PROVİZYONEL-YÜKSELİŞ** (eşiksiz; kanal-türevli)

**Breach-damgası (r4 — r2-EAI-eşiğine GERİ; K2-fix):** ack-boşluğu > **K_CONF_EAI×EAI** olan
sessizliğin bitiş-anı `breach_ns` damgalanır (ölçüm last_ack_ns-güncellemesinden ÖNCE; çıpa =
max(last_ack_ns, first_send_ns)). EAI = beklenen-ACK-cadence (mevcut sender-hesabı, kanal-türevi)
→ hedef-rejim (sub-kbps, EAI≫rtprop) KORUNUR: normal-ack gap≈EAI < K×EAI → FLOW (0.25k/p5-açlık
hesap-teyitli); kopma (≫EAI) → GAP (p6b/1k). **Sensing-sınırı (BEYANLI):** 0.25k'da EAI-mertebeli
kopmalar (≤K×EAI) FLOW-kalır — return-supply T_SPACING-ön-şartıyla AYNI sınıf (yerel-veri kopma
vs yavaş-cadence ayırt-edemez; kesin-ayrım E-FULL/DV-3); zararsız (o örnekler ~cadence-RTT).
r3-çıplak-rtprop eşiği hedef-rejimi öldürüyordu (K2 — geri-alındı). Örnek-sınıfı = uçuş-penceresi
breach'le örtüşüyor mu; kümülatif-SACK-sarkması bu tanımla kapalı.

```
SINIF-1  FLOW      (breach-örtüşmesi YOK):
         tam-güven — mevcut kuantum-EWMA + rtprop-min + qd-min AYNEN (0 değişiklik).
SINIF-2  GAP-DOWN  (breach-örtüşmeli ∧ raw ≤ srtt):
         n=1 EWMA'ya girer (GEREKÇE: FLOW-dışı Δt kuantum-güvenilirliği kırık → tek-adım,
         blackout-Δt'sini 64× amplifiye etmez; aşağı-hareket gerçeğe-doğru → güvenli).
         **rtprop-min'e GİRMEZ (§1-C; K1-fix — GAP-DOWN rezidanları çok-döngülü ratchet'in
         yakıtıydı).** qd-min girer.
SINIF-3  GAP-UP    (breach-örtüşmeli ∧ raw > srtt):
         COMMITTED-track'e HİÇBİR katkı YOK (srtt/rttvar/rtprop-min/qd-min/
         drain_rtprop_qualifying — hiçbiri) → CANDIDATE-track:
           cand = EWMA₁/₈(raw);  uyum-bandı: |raw − cand| ≤ 4×rttvar_cand
           rttvar_cand [PİN, K3 — RFC-6298]: ilk-GAP-UP init=cand/2; uyumlu-örnek
             rttvar_cand:=¾·rttvar_cand+¼·|raw−cand|; uyumsuz-reset init=raw/2
             (init=0→band=0→SAHA-ölü/deterministik-test-geçer TUZAĞI, o yüzden raw/2)
           uyumlu → cand_streak++ ; uyumsuz → cand=raw, rttvar_cand=raw/2, streak=1
           FLOW veya GAP-DOWN örneği → streak=0 (sürekli-yüksek hipotezine karşı-kanıt)
         COMMIT: cand_streak ≥ K_CAND [CAL {2,3}] →
           srtt := min(cand, K_SPAN × rtprop)   ← YAPISAL-TAVAN [CAL {4,8}]
           rttvar := cand/2-sınıfı reseed; streak=0; diag_cand_commit++
         her GAP-UP: diag_rtt_stale++ (TRAJ "stale="), commit'ler ayrı-sayaç.
```
Sabit-hijyeni: K_CAND, K_SPAN **boyutsuz kanal-türevli çarpanlar** [CAL, golden'a girmez]
(K_EPISODE-emsali izinli-sınıf); mutlak-zaman sabiti SIFIR; uyum-bandı mevcut rttvar-mekaniği.

## 2. RATCHET-KIRILMA ARİTMETİK-İSPATI (talimat-zorunlu-1; her CAL-kolunda)

**ÖN-KOŞUL (K3-beyan):** önerme rtprop0-DÜRÜST varsayar (zehirli-bootstrap = S4-(v)).
**Yapısal önerme:** `srtt_committed ≤ max(flow-dürüst, K_SPAN × rtprop)`; **rtprop GAP-örnekleriyle
(UP+DOWN) yükselTİLEMEZ** (§1-C: GAP hiç girmez; yalnız FLOW → FLOW-yaşlanması meşru-yükseliş,
saldırı-değil) → tavan SABİT, çok-döngülü K_SPAN^m ratchet YAPISAL-ölü.
∴ Saldırı-serisi (her adım tek doğrulanmamış-GAP-örneği, değer serbest-seçimli):
- committed-EWMA'ya katkı: **0** (SINIF-3 hiç değmiyor) → adım-çarpanı 1.00 (r2: 1.25).
- candidate-commit yolu: K_CAND ardışık-uyumlu örnek gerekir VE commit-değeri ≤ K_SPAN×rtprop —
  rtprop saldırıyla oynatılamadığından tavan SABİT: **k-adımlı seri için srtt ≤ K_SPAN×rtprop₀,
  k'dan BAĞIMSIZ** → "sınırsız-yukarı-taşıma" YAPISAL-imkânsız (talimat-invariantı).

| CAL-kolu | tavan (rtprop=1.019s, p6b) | kalıntı-hasar-sınırı (conf-eşiği 3×tavan) | agreement-saldırı maliyeti |
|---|---|---|---|
| K_SPAN=4 | srtt ≤ 4.08s | ≤ 12.2s (p6b-kirli 58.8s'e karşı) | — |
| K_SPAN=8 | srtt ≤ 8.15s | ≤ 24.5s | — |
| K_CAND=2 | — | — | kopma-başına-1-örnekle commit için 2 ARDIŞIK uyumlu (±4·rttvar_cand) kopma-değeri; araya tek flow/gap-down girse sıfırlanır |
| K_CAND=3 | — | — | 3 ardışık-uyumlu (daha pahalı; commit-gecikmesi ↑) |

r2-karşı-doğrulaması: eski-tasarımda aynı seri 2.10→19.6s'i **10 adımda** alıyordu (1.25^k);
r3'te aynı seri srtt'yi K_SPAN×1.019'da tavanlar — **üstel-büyüme yapısal-imkânsız.**

## 3. ADAPTİF-MERDİVEN KARŞI-VEKTÖRÜ (talimat-zorunlu-2; ≥20-adım; assert-invariant)

Vektör (birim-test): kararlı 1.02s-kanal; 20 adım: adım-i'de `gap_i > rtprop` ∧ raw_i =
saldırgan-seçimli-artan-seri (eski-eşik-takipçisi dizi DAHİL: raw_i = 3×srtt_anlık − 1µs;
ve uyum-bandı-oyunlu dizi: K_CAND-blok-başına sabit-değer merdiveni L, 3L, 9L...).
**ASSERT-INVARIANT (her adım-sonu):** `srtt_committed ≤ K_SPAN × rtprop` ∧ `rtprop == 1.019s
(dürüst-değer; hiç oynamadı)`. PASS-hesabı: uyum-oyunlu dizi en-iyi-halde commit'i
min(L-serisi, K_SPAN×1.019)'a taşır → 20. adımda da srtt ≤ 4.08s (K_SPAN=4-kolu) — p6b-hasar-
sınıfı (conf-58.8s / freeze-16.5s) OLUŞAMAZ (conf ≤ 12.2s; dup-aralık ≤ 4.1s). Ek-vektörler:
r2-seti (p6b-zinciri, sarkma-breach, deadlock-yok, sürekli-uzun-RTT-muafiyeti, blackout-bootstrap
S4-pinli) + reseed-guard vektörü (boş-pencere + GAP → keep-last korunur).

## 4. PROVİZYONELİN KÖR-NOKTASI + KALICI-YÜKSEK-RTT AYRIMI (talimat-zorunlu-3 + KÖK-varsayım)

- **Meşru-yükseliş (YOĞUN-akışlı kanal, 1→20s):** pipelined ack'lerde örnekler-arası boşluk ≈
  gönderim-aralığı ≪ RTT → breach-örtüşmesi YOK → FLOW-sınıfı → normal-EWMA tırmanır.
  Gecikme-hesabı: 1→19s = 23 EWMA-ADIMI ama FLOW kuantum-döngüyü korur (n=Δt/T) → ~1 ack/s'te
  n≈3-4 → **~6-8 FLOW-örneği** (r3 "23 örnek" n≡1-varsayımı, düzeltildi) — sınırlı ve
  YÖNLÜ-GÜVENLİ (alt-tahmin döneminde false-loss'u kanıt-şartı+cadence-taban frenler; TUR-1
  makinesi). **Sürekli-20s-RTT kanal (DV-1-emsal-muafiyet): SAĞLANIR** — yoğun-akışta sınıf
  FLOW'dur, kapı hiç dokunmaz; bootstrap'ta ilk-örnek-bypass (S3) zaten 20s'i tohumlar.
- **SPARSE-kalıcı-yüksek kanal (KÖK-varsayım cevabı — BEYANLI-KÖR-NOKTA):** her örnek breach-
  örtüşmeli (ack-boşlukları > rtprop) → hepsi GAP-UP → commit-tavanı K_SPAN×rtprop_eski —
  gerçek-20s'e TAM adaptasyon imkânsız (srtt en-çok 4-8×rtprop'u izler). Bu desen YEREL-veriyle
  kopma-tekrarından AYIRT-EDİLEMEZ (T_SPACING-sensing-ön-şartıyla aynı sınıf; kesin-ayrım
  E-FULL-ölçümü/DV-3-rejim-tespiti işi — S2-kararıyla bekleyiş-istatistiği oraya). Tavanlı-izleme
  ara-durumu: sistem 20s-kanalı ≥4×rtprop-doğruluğunda izler; kalan sapmanın maliyeti false-loss
  değil (kanıt-şartı) fazla-kopya sınıfı — sınırlı, DV-3'te ölçülür. **Tradeoff-cetveli:**
  K_CAND↑ → saldırı-pahalı ∧ meşru-sparse-commit-gecikmesi↑; K_SPAN↑ → sparse-izleme-doğru ∧
  saldırı-tavanı-yüksek. Beklenti-kaydına iki-uç dengesi (başlangıç: K_CAND=2, K_SPAN=4 —
  saldırı-tavanı-dar; sparse-desen DV-3-verisiyle yeniden-kalibre).

## 5. Dokuz-tüketici (r2'den; anlatım-düzeltmesi talimattan)

**Formül 0/9 DEĞİŞMEZ; giriş-trajektorisi 9/9 değişir** (dürüst-saat beslemesi). Tek-boğaz
(rtt.observe) kod-doğrulanmış; drain_rtprop_qualifying GAP-sınıflarında SET edilmez (r2-pin);
diag-muhasebe kapı-öncesi. F2/F3-RED gerekçeleri aynen (245<258 karşı-örneği dahil).

## 6. S-KARARLARI (talimat-işli)

- **S1 GİRMESİN** ✓ — Duplicate-kendi-send_ns örneği B1-masasına açık-madde.
- **S2 SALT-SAYAÇ** ✓ — `stale=` + `cand=`/`commit=` TRAJ-alanları; bekleyiş-dağılımı DV-3'e.
- **S3 ONAY** ✓ — srtt-None ilk-örnek-bypass YENİ-kod olarak yazılır (r1-atıf-hatası kabul;
  PreConverge-çıkışı için zorunlu-bileşen).
- **S4 KABUL+BEYAN — "bilinen bootstrap-failure-mode"** (nadir-desen ifadesi düzeltildi).
  Birim-test KESİN-pinler: (i) ilk-örnek zehirlenebilir; (ii) kapı o-oturumda-etkisiz-olabilir;
  (iii) B2 bu-vakayı-çözme-iddiasında-DEĞİL; (iv) DV-3'te izlenecek; **(v) [K3] TAVAN-ölümü:**
  zehirli-bootstrap rtprop0'ı tohumlar → commit-tavanı K_SPAN×zehir (80-160s) → kapı VE tavan
  birlikte ölür; §2 rtprop0-DÜRÜST-varsayar (BEYANLI-ön-koşul); kurtuluş: ilk-FLOW-örneği
  rtprop-min'i indirir (sparse'ta gecikir). DV-3 her ikisini kapsar.

## 7. Ölçüm-planı (kod-GO sonrası; beklenti-kaydı ölçümden-önce)

Birim: §3-vektör-seti (merdiven-assert dahil). DV-2-seti yeniden (ana-sınav p6b: dal-A srtt-dürüst
→ freeze'ler kalkar → ≥19/20; dal-B teslim-aynı-ama-zehir-kesildi → dürüst-kayıt, asıl-amaç
DV-3-temizliği). Sürekli-smoke: p5×2 (İKİNCİ+ kurtuluş-örnekleri artık GAP-UP-karantinalı —
srtt tohum-değerinde donar; bugünkü-şişmeden İYİLEŞME-yönlü; dal-yapılı izleme) + temiz-1k(=45)
+ 0.25k (n=1-sınır + sınıf-dağılımı izlenir). TRAJ: stale=/cand=/commit=.
