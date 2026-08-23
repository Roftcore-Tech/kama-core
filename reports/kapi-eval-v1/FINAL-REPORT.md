# kapi-eval-v1 — FAZ-1 Evaluation Kampanyası SONUÇ RAPORU

**Statü:** PRIMARY + SEKONDER TAMAM (2026-08-23). Exploratory Grup-B/C partisi koşuyor
(~4-5 gün; bitince EK-A olarak eklenecek — "unadjusted exploratory"). Bu belge kampanyanın
TEK sonuç raporudur.

**Provenance:** tasarım `KAPI-1-V2-BENCHMARK-TASARIM.md` (FINAL) · ön-kayıt `KAPI-2-ON-KAYIT.md`
(onay 2026-08-21, F1–F4) · freeze `775750a2` · binary sha `provenance.txt` · tohumlar drand
round `DRAND.txt` (Model A; configs-freeze SONRASI) · manifest 240 satır / 960 yürütme ·
calibration bankası `2d6e94a3` · ham veri `ab946a33` · hükümler `1b39a4f0` (byte-eş tekrarlanabilir).

## 1. Bütünlük

960/960 koşu tamam; süreç sızıntısı yok; bozuk dosya yok. Ön-kayıt §5 realized-loss taraması
(756 yeterli-örneklemli koşu): **2 ihlal** — ikisi de SEKONDER burst hücrelerinde q-dgram koşuları
(p2-burst rep6: 0.625 vs 0.80; p5-burst rep8: 0.141 vs 0.05) → exogenous-INVALID, veri korunarak
agregasyondan düşürüldü (hücreler n=9 raporlar). Primary hücrelere dokunulmadı. Üç makine-uykusu
ve bir CLI-çökmesi yaşandı; sürücü yetim sürerek tamamladı, uykuyu kapsayan 3 koşu tolerans-içi
doğrulandı (`SUSPEND-NOTE.txt`); tüm saatler CLOCK_MONOTONIC olduğundan ölçüm-çerçevesi uykuyu
görmez. Capture katmanı (167 MB) `CAPS-SHA256SUMS.txt` ile mühürlü, diskte.

## 2. Ön-kayıtlı PRIMARY sonuçlar

Test (değiştirilemez, KAPI-2 §2): rep-düzeyi paired Δcompletion (`kama-tail-v1` − `q-dgram-arq`),
paired-bootstrap %95 CI (10 000, hücre-türevi tohum), **Holm-3**; iddia kuralı: adj-p<0.05 ∧ CI>0 ∧
Δ̂ ≥ +15pp. Endpoint: eşit kanal-türevli paced bütçede (u=0.75) mesaj-başı D=120 s içinde teslim oranı.

| hücre | n | Δ̂ | medyan | %95 CI | holm-p | **HÜKÜM** |
|---|---|---|---|---|---|---|
| p2_loss80 | 50 | **+0.384** | +0.470 | [+0.315, +0.449] | 0.0003 | **KAMA-SUPERIOR** |
| p3_loss95 | 30 | +0.033 | 0.000 | [+0.008, +0.062] | 0.0062 | SEPARATION-NO-CLAIM |
| p5_revpath | 60 | +0.120 | 0.000 | [+0.069, +0.177] | 0.0003 | SEPARATION-NO-CLAIM |

Yan kollar (rapor zorunluluğu; medyan completion):

| hücre | kama-tail-v1 | kama-arq | q-dgram-arq | q-stream |
|---|---|---|---|---|
| p2_loss80 | 0.505 | 0.495 | 0.040 | 0.020 |
| p3_loss95 | 0.000 | 0.000 | 0.000 | 0.000 |
| p5_revpath | 1.000 | 1.000 | 1.000 | 1.000 |

**İkincil maliyet ucu** (sıfır-teslim-güvenli `delivered_payload/injected_ip_bytes`, paired;
tek başına iddia taşımaz): p2 Δ̂=+0.054 CI [+0.040, +0.068] · p3 Δ̂=+0.002 CI [−0.009, +0.011] ·
**p5 Δ̂=+0.323 CI [+0.300, +0.347]** — p5'te iki sistem de tamamlıyor; Kama aynı işi belirgin
biçimde daha az IP-byte ile yapıyor (r5 D.5 düşük-maliyet gözlemi).

### İzinli iddia (FAZ-1 tavanı, r5 C.3)

> Kama, preregistered Quinn 0.11.11 baseline'larını (Q-STREAM, Q-DGRAM-ARQ) p2_loss80 profilinde
> eşit kanal-türevli IP-byte bütçesinde, Holm-düzeltmeli ön-kayıtlı testle geçti (+38.4pp,
> %95 CI [+31.5, +44.9]). FAZ-1 FEC'siz olduğundan bu sonuç yalnız **recovery/transport-entegrasyon**
> farkına atfedilir; "QUIC mimarisine üstünlük" iddiası FAZ-2 (ISOLATION+REFERENCE conjunctive)
> koşulmadan YAPILAMAZ.

Dış araştırma-sorusu cevabı (r5 C.3 metni bağlamında): "The campaign tests whether Kama retains
useful message-delivery capability in synthetic high-loss/high-delay/asymmetric/sub-kilobit IP
channels where preregistered Quinn configurations may experience connection-establishment or
recovery limitations." — p2'de EVET (0.505 vs ≤0.040 medyan); p3'te hiçbir sistem D=120'de anlamlı
teslim yapamıyor (dürüst taban); p5'te kapasite eşit, byte-maliyeti belirgin düşük.

## 3. Sekonder eksenler (ön-kayıtlı; **unadjusted exploratory** etiketli — iddia taşımaz)

| eksen | hücre | Δ̂ (ktv1−qd) | ktv1 med | qd med | not |
|---|---|---|---|---|---|
| burst workload | p2 burst | +0.640 (n=9) | 0.720 | 0.090 | yön korunuyor, etki büyüyor |
| burst workload | p3 burst | +0.025 (n=10) | 0.000 | 0.000 | taban |
| burst workload | p5 burst | **−0.111** (n=9) | 1.000 | 1.000 | yüksek-varyans hücre (ön-kayıt dürüstlük kaydı); medyanlar tavanda, ortalamayı birkaç kötü kama-rep'i çekiyor |
| A1 düzlemi (geçiş-şoku) | p2 A1 | +0.480 (n=10) | 0.615 | 0.080 | temiz-converge sonrası şokta da ayrım sürüyor |
| A1 düzlemi | p3 A1 | +0.240 (n=10) | 0.230 | 0.000 | A1'de Kama %95 kayıpta bile teslim ediyor (temiz-kanal converge etkisi) |
| A1 düzlemi | p5 A1 | +0.122 (n=10) | 1.000 | 1.000 | |
| fixed-B, D=60 | p2 bd120 | +0.335 (n=10) | 0.400 | 0.020 | byte sabit, süre kısa |
| fixed-B, D=300 | p2 bd120 | +0.522 (n=10) | 0.610 | 0.045 | byte sabit, süre uzun — fark süreyle büyüyor |
| u=0.50 | p2 | +0.266 (n=10) | 0.395 | 0.055 | bütçe yarıya inince de yön korunuyor |
| u=0.90 | p2 | +0.430 (n=10) | 0.575 | 0.045 | |

## 4. Wire accounting + kanal

"The primary byte budget measures IP bytes injected by the endpoints into the emulated channel,
including transmissions subsequently erased by the impairment model; it is not a measurement of
physical RF airtime." (r5 H, aynen.) Enjeksiyon düzlemi governor sayaçları (`budget.injected_*`,
her kolda simetrik); `udp_tx_bytes` üç kolda da transport-DENEMESİ düzlemidir. Realized-kanal:
netem sayaç-farkları barrier→post; ön-kayıt toleransı yalnız yukarıdaki 2 koşuda aşıldı.
pin_neigh 4 arayüzde; ARP-drop=0. Capture AF_PACKET üç-düzlem, PACKET_STATISTICS drop=0.

## 5. Hizmet-semantiği (r5 C.4)

| boyut | KAMA | Q-STREAM | Q-DGRAM-ARQ |
|---|---|---|---|
| güvenilirlik | ARQ (+tail-v1); bütçe/deadline sınırlı | QUIC reliable | app-ARQ (PTO, ×2 backoff) |
| sıralama | yok (unit bağımsız) | stream-içi FIFO (mesaj-başı-stream) | yok |
| mesaj-sınırları | unit=mesaj | stream=mesaj | datagram=mesaj |
| deadline-sonrası | harness D'de keser; geç teslim stale | aynı | aynı |
| dup-suppression | receiver unit_id | QUIC | app msg_id |
| app-ACK | SACK (N=2 + 20 ms) | stream-FIN | kümülatif aralık-ACK (N=2 + 20 ms + R-kota aynası) |
| connection-state | yok (motor durmaz) | QUIC conn (idle 300 s) | aynı |

## 6. Sınırlılıklar (dürüstlük kaydı)

1. **Kripto stub:** Kama data-frame'i gerçek AEAD değil +16B maket → hiçbir CPU/throughput iddiası
   yapılmaz (r5 L). Byte-eşdeğerlik korunur.
2. **Aynı-host veth:** "loopback değil ama bağımsız-fiziksel-host da değil" (r5 K caveat'i aynen).
3. **Warmup residue:** severe-loss'ta t0'da teslim-edilmemiş warmup birimleri kalır; her kolda
   simetrik ölçülür (`warmup_residue`) ve post-t0 residue trafiği tüm kollarda ölçüme dahildir.
4. **Tail-v1'in payı:** random-loss rejimlerinde kama-arq ≈ kama-tail-v1 (p2: 0.495 vs 0.505) —
   tail'in doğrulanmış alanı kuyruk-senaryolarıdır; buradaki ayrımın ana taşıyıcısı motorun
   ARQ/pacing/rate bütünü. Bu, iddiaya değil atfa dair bir netleştirmedir.
5. p5-burst sekonder hücresi negatif-işaretli ve yüksek-varyanslı (ön-kayıtta öngörülen güç sınırı).
6. Q-DGRAM-ARQ app-ARQ'sunu biz yazdık (politika ön-kayıtlı ve kaynaktan aynalı); "knowingly
   disadvantaging" yasağına uygunluk W1 tasarım kaydında.

## 7. Sonraki adımlar

Exploratory EK-A (koşuyor) · white-paper taslağı (r5 §C kilitleriyle) · FAZ-2 GO kararı
(FEC kolları; "mimari" iddiasının tek yolu) · default-flip kararları bu kanıtla ayrıca sunulacak.
