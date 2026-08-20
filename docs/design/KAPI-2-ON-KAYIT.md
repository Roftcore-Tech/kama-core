# KAPI-2 — v2 Benchmark Evaluation ÖN-KAYIT (FAZ-1)

**Statü:** TASLAK → Abkarada **KAPI-1 FINAL + KAPI-2** onayı bekliyor (r5 §A: evaluation/headline
koşuları ancak ikisi tamamlanınca). Bu belge commit edildiği anda configs-FREEZE başlar; onay
sonrası drand-adımı (§6) çalıştırılır ve evaluation başlar. Tasarım otoritesi:
`docs/design/KAPI-1-V2-BENCHMARK-TASARIM.md` (TASLAK-ONAYLI + §13 amendment'ları).

## 1. Donmuş konfigürasyon (freeze)

| öğe | değer |
|---|---|
| kaynak commit | bu belgenin commit'i (öncesi: harness `80245662`, calibration bankası `2d6e94a3`) |
| bench binary | `cargo build --release -p bench` @ freeze-commit; sha256 koşu-provenance'ına yazılır (calibration'da `f94dcaf0…`) |
| Kama kolları | `kama-arq` = `Sender::new()` defaults; `kama-tail-v1` = + `TailRecoveryConfig::validated_v1()` (pin-testli sabit) |
| quinn kolları | `q-stream` (mesaj-başı bi-stream) · `q-dgram-arq` (DATAGRAM + PTO app-ARQ: clamp(2×rtt,100ms,10s), ×2 backoff; ACK aynası N=2+20ms+R-kota) |
| quinn pinleri | 0.11.11 · PMTUD off · initial_mtu 1452 · GSO off · idle 300 s · bi-stream kredisi 512 |
| bütçe | u=0.75 (D2) · paced token-bucket refill=C·u/8, bucket=max(3·MTU, 2 s·refill) (D6), ceiling=C·W·u/8 · packet-atomic · yön-başına, aktarım yok · handshake+warmup pre-arm muaf · reverse arm gate-watch ile t0-senkron |
| warmup / quiescence | 10 mesaj, aynı-sınıf paced; sınırlı drain (pif=0 hedefi, 3 s tavan); residue her kolda `warmup_residue`'ya kaydedilir (KAPI-1 Amendment-1) |
| düzlem | A2 primary (D3): temiz-ağ kuruluş → netem @ establish-gate → impaired warmup → barrier → t0. A1 sekonder: netem @ ölçüm-gate |
| topoloji/capture | 3-netns, netem orta-ns; AF_PACKET üç-düzlem + PACKET_STATISTICS; pin_neigh 4 arayüz; ölümcül çift-yön barrier (Amendment-3) |
| tohumlar | YALNIZ §6 drand türetimi (`SHA256(drand ‖ "KAMA-V2-EVAL" ‖ cell ‖ rep)`); calibration-uzayı evaluation'da YASAK |

Freeze'den sonra motor/harness koduna dokunulmaz (r5 M); dokunulursa kampanya partisi yeni
commit'le BAŞTAN (cherry-pick yasak).

## 2. Primary endpoint + test (değiştirilemez)

- **Endpoint:** `E(B,D)` = eşit kanal-türevli paced bütçede, mesaj-başı deadline (D=120 s) içinde
  teslim edilen mesaj oranı (completion; latency=teslim−sunum≤D).
- **Primary hücreler (3):** {p2_loss80, p3_loss95, p5_revpath} × paced × A2 × D=120.
- **Primary karşılaştırma:** `kama-tail-v1` − `q-dgram-arq` (FAZ-1 primary comparator; `kama-arq`
  ve `q-stream` her hücrede yanında raporlanır — el-devri kuralı).
- **Test:** rep-düzeyi eşleştirilmiş Δcompletion; paired-bootstrap %95 CI (10 000 yeniden-örnekleme,
  bootstrap-tohumu = hücre-tohumu; n = rep, mesaj-bazlı örneklem YASAK); **Holm (3 hücre)**.
- **Başarı iddiası kuralı:** bir hücrede "Kama üstün" YALNIZ Holm-sonrası CI>0 VE Δ̂ ≥ +15pp ise.
  Maliyet iddiası: sıfır-güvenli `delivered_payload/injected_ip_bytes` (governor düzlemi) paired
  farkı, aynı Holm ailesinde İKİNCİL (iddiaya tek başına kaynak olmaz).
- **İddia tavanı (r5 C.3):** FAZ-1 FEC'siz → yalnız *recovery/transport-entegrasyon* dili;
  "QUIC mimarisine üstünlük" DENMEZ (o iddia FAZ-2 conjunctive kuralına kilitli).

## 3. n-gerekçesi (calibration `kapi-cal-v1`, rep-düzeyi paired sd)

| hücre (paced) | cal Δ̂ | sd | öneri n | +15pp gücü | gözlenen-etki gücü |
|---|---|---|---|---|---|
| p2_loss80 | +0.392 | 0.209 | **50** | >%99 | >%99 |
| p3_loss95 | +0.028 | 0.064 | **30** | >%99 (sd küçük) — D=120 taban-etkisi belgeleme hücresi | — |
| p5_revpath | +0.313 | 0.395 | **60** | ~%84 | >%99 (+31pp için) |

α=0.05 (Holm-öncesi), hedef güç %80, min-relevant-effect +15pp (r5 I). p3-D120 calibration'da TÜM
kollar için taban (median 0.000) — ön-kayıtlı beklenti: "her iki sistem de D=120'de anlamlı teslim
yapamaz" da GEÇERLİ ve raporlanacak sonuçtur (başarısız-hücre-düşürme yasağı). p5-burst yüksek-varyans
(sd 0.464) → burst sekonder ekseninde kalır ve yalnız ≥+20pp etkiler için güçlüdür (dürüstlük kaydı).

## 4. Evaluation matrisi + duvar-saati (ölçülen 240 s/paced-rep-kol, 150 s/burst)

| parti | içerik | rep-kol | süre |
|---|---|---|---|
| PRIMARY | 3 hücre × 4 kol × paced × A2 × n={50,30,60} | 560 | ~37 h |
| SEC-1 burst | 3 hücre × 4 kol × burst × A2 × n=10 | 120 | ~5 h |
| SEC-2 A1 | 3 hücre × 4 kol × paced × A1 × n=10 | 120 | ~8 h |
| SEC-3 fixed-B | p2 × paced × D∈{60,300} × n=10 (B=B(120) sabit) | 80 | ~7 h |
| SEC-4 fixed-D | p2 × paced × u∈{0.50,0.90} × n=10 | 80 | ~5 h |
| **toplam FAZ-1 evaluation** | | **960** | **~62 h ≈ 3 gece (seri)** |

Exploratory Grup-B/C süpürmesi (n=10) primary-parti sonuçları bankalandıktan sonra ayrı partide;
"unadjusted exploratory" etiketiyle. Tüm sekonder/exploratory çıktılar da aynı etiketle raporlanır.

## 5. Geçerlilik/invalidation kuralları (yalnız EXOGENOUS — r5 J)

INVALID (hücreden düşer, nedeni raporlanır): INFRA-FAIL sağlık-probu · server-ölümü ·
barrier-doğrulama başarısızlığı (çift-yön) · gate-timeout · ARP-drop>0 (pin_neigh ihlali) ·
capture `drops>0` · realized fwd-loss sapması: configured p'den, enjekte-N üstünden binom %99
CI dışı VE |sapma|>6pp (calibration gözlem maks ~4.5pp). İki transportun realized-loss FARKI
(endogenous) invalidation DEĞİL. MISSING satır = açıklanmadan hücre raporlanamaz (survivorship).

## 6. Model A kör-ayrım — drand prosedürü (onay SONRASI, tek sefer)

1. Bu belge + freeze-commit push edilir; Abkarada FINAL+KAPI-2 onayı verir.
2. Onay anından SONRAKİ ilk drand mainnet round'u `R` seçilir
   (`https://api.drand.sh/public/latest` → `round`, `randomness`); `R` ve `randomness` hex'i
   `reports/kapi-eval-v1/DRAND.txt`'ye yazılır ve commit edilir.
3. `kapi_manifest.py evaluation --drand-randomness <hex>` manifesti üretir (üretici drand'sız
   ÜRETMEYİ yapısal olarak reddeder); manifest sha'sı provenance'a girer; koşu başlar.
   Configs bu belgeyle zaten kilitli olduğundan "tohumları görüp ayar" yapısal olarak imkânsız.

## 7. Onay istenen kararlar (Abkarada — FINAL kapısı)

| # | karar | öneri |
|---|---|---|
| F1 | KAPI-1 FINAL (tasarım + §13 amendment'ları) | onay |
| F2 | Bu ön-kayıt (endpoint/test/n/matris/kurallar) | onay |
| F3 | n tahsisi: p2=50 · p3=30 · p5=60 | onay (alternatif: hepsi 50 — p5 gücü %77'ye düşer) |
| F4 | Exploratory Grup-B/C partisinin primary-sonrası koşulması | onay |
