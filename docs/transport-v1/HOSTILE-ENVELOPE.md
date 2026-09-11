# Kama Transport v1 — İLAN EDİLMİŞ İŞLETİM ZARFI

Bu belge Transport v1'in **NE İDDİA ETTİĞİNİ** sabitler; pazarlama/spec kayması bu tablonun
dışına çıkamaz. Her satır Faz-5'in gerçek hücresine ve hükmüne bağlıdır
(`reports/transport-v1/PHASE5-T0-T7-REPORT.md`, N=30, D=20 s, ölçüm SHA `e3c1e8bb`).
Zarf sessizce GENİŞLETİLMEZ; yeni rejim = yeni ön-kayıtlı hücre.

## 1. DOĞRULANMIŞ ÇEKİRDEK (N=30, frozen metodoloji)

| rejim | hücre (profil kaynağı) | Kama | quinn | TCP | hüküm |
|---|---|---|---|---|---|
| sağlıklı yol (regresyon kontrolü) | T0 `h6_rtt20`: delay 10 ms, loss 1 %, 10 Mbit | 1.000 | 1.000 | 1.000 | PARITY — patolojik retx/blackout yok |
| yüksek rastgele kayıp | T1a `p1_loss50`: loss 50 %, 100 kbit | **1.000** [1.000,1.000] | 0.900 [0.530,1.000] | 0.120 | **KAMA DOMINATES** |
| rastgele kayıp + yüksek RTT | T1c `iid50`: 200 ms, loss 50 % / dönüş 10 % | 1.000 | 1.000 | 0.830 | PARITY (D=5 s: 0.960 ↔ 0.590) |
| korelasyonlu/burst kayıp | T2a `h3b_burst50` gemodel 50 %, 250 kbit | 1.000 | 1.000 | 0.770 | PARITY — süreklilikte Kama belirgin (blackout 4/30 ↔ 25/30, örtüşmeyen CI) |
| korelasyonlu kayıp + RTT | T2b `burst50`: 200 ms, loss 50 % 50 % | 1.000 | 1.000 | 0.430 | PARITY — önceki n=6 üstünlük iddiası GERİ ÇEKİLDİ |
| düşük bant | T3a `h2_cell`: 8 kbit, loss 25 %, jitter | 1.000 (min 0.967) | 1.000 (min 0.000) | 1.000 (min 0.067) | PARITY — Kama felaket koşusu 0/30 |
| yüksek gecikme | T4 `h6_rtt200`: 200 ms RTT, loss 1 % | 1.000 | 1.000 | 1.000 | PARITY |
| kayıp + reorder | T7 `C4`: loss 20 % + reorder 30 %, 1 Mbit | 1.000 | 1.000 | 1.000 | PARITY — `spurious = 1`, fwd_loss 200‰ = enjekte |

## 2. DOĞRULANMIŞ UZMANLAŞMA — bozuk geri-besleme / yönlü kayıp gözlemi

| rejim | hücre | Kama | quinn | TCP | hüküm |
|---|---|---|---|---|---|
| **bozuk dönüş yolu** | T6 `p5_revpath`: ileri loss 5 % / dönüş loss 80 %, 100 kbit | **1.000 [1.000,1.000]** | 0.740 [0.210,0.990] | 0.060 [0.050,0.630] | **KAMA DOMINATES** — CI'lar örtüşmüyor |

Mekanizma kanıtı: enjekte ileri %5 → ölçülen 45.5‰; enjekte dönüş %80 → ölçülen 793‰;
health Dead iken teslim sürdü. quinn 9/30 koşuda bağlantı kuramadı. Bu, ürünün **birincil
iddiasıdır**: *bozuk geri-besleme körce ileri DATA kaybı sayılmaz.*

## 3. KISMEN DOĞRULANMIŞ

| rejim | hücre | ne ölçüldü | ne EKSİK |
|---|---|---|---|
| bant asimetrisi (ileri ≠ dönüş kapasite) | T5 R3: ileri 1 Mbit / dönüş 64 kbit | üç kol 1.000, PARITY; asimetri telde uygulandı (arşivli tc) | dönüş yolu **doyuma girmedi** (ACK trafiği 281 B/s, kapasite 8 000 B/s, %3.5) → runtime asimetriyi GÖZLEMEDİ; doygun-dönüş davranışı doğrulanmadı (LIM-004) |
| aşırı rastgele kayıp | T1b `p2_loss80`: loss 80 % çift yön | Kama göreli üstün (P≈0.73, max 0.900 ↔ 0.130) ama medyan 0.010; üç kol da mutlak başarısız | işletim noktası DEĞİL; 18/30 koşu sıfır-ACK durması (→ §4) |

## 4. DIŞARIDA / GARANTİ EDİLMEZ

| rejim | neden dışarıda | kanıt |
|---|---|---|
| **sıfır-geri-besleme kuruluşu** (ilk uçuşta hiç kullanılabilir ACK yok) | bootstrap mekanizmaları kalibre edilmemiş, bilinçli kapalı; ilerleme garanti edilmez | T1b 18/30 kalıcı durma; ADR-zero-ack-bootstrap; LIM-001 |
| §11 acil-durum köşesi (çift-yön ≥ %80 kayıp ∧ sub-kbps ∧ ack_conf LOW) | yol kablolu ve birim-testli ama T0–T7'de HİÇ ateşlemedi (`recover_fwd_entries = 0`, 330/330) | ADR-minimal-parity-exception; LIM-006 |
| sağlıklı yüksek-bant ağda TCP/QUIC'i geçmek | hedef DEĞİL (ARCHITECTURE §4.1 non-goal) | T0/T4/T5 parite |
| tam sıralı akış semantiği | mesaj-granülü, sırasız (INV-011); sıra üst katmanın | Faz-4 metodoloji §5 |
| QUIC-tarzı çoklu akış / bağlantı göçü / kernel-NIC paritesi | kapsam dışı | — |
| sub-kbps (≤ 1 kbit) rejimde kıyas | yalnız Kama sondası (T3b, n=5); rakip kıyası yok | `SPEC CONFLICT NOT REACHED` |

## 5. Ölçüm koşulları (zarfın geçerlilik sınırı)

Geometri 100 × 256 B @ 50 ms (T3a: 30 × 64 B), D = 20 s, kollar `kama-v1-full` /
`quinn-reliable-stream` / `tcp-default`, kıyas düzeyi tüm hücrelerde NON-NORMALIZED (Kama sırasız
mesaj ↔ rakipler tam sıralı akış; anlam normalize edilmemiştir). Bu geometrinin dışındaki
yükler (büyük mesaj, fragmenter, ACK-ağır trafik) bu zarfla KAPSANMAZ.
