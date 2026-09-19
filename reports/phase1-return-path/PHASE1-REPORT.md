# FAZ-1 — DÖNÜŞ-YOLU GÖZLEMLENEBİLİRLİĞİ · KAPANIŞ

**Çıkış ölçütü:** *"forward impairment != return impairment runtime durumunda kanıtlanabilmeli."*
**Sonuç: SAĞLANDI.** Ölçüm: `scripts/netem/r_returnpath_proof.sh`, ham koşular `runs-R0-R5/`.
Beklenti kaydı koşudan ÖNCE yazıldı (`EXPECTATION.md`).

## 1. Ne eklendi (yalnız GÖZLEM)

`crates/adaptive-transport/src/return_path.rs` — sabit bellekli, tamsayı-EWMA, RNG'siz gözlemci
(ADR-17 / SR-AUDIT-001). **Hiçbir kontrol yolu (pacing, ARQ, FEC, congestion, confirm ufku) bundan
beslenmez.** Girdi: telde zaten var olan `ACK_FRAME_V2.seen_count` (§2.2) + ACK'in teldeki boyu +
göndericinin kümülatif TX datagram sayısı. **Yeni wire alanı YOK.**

Kablolama: `Sender::observe_ack_feedback()` ← client-core runtime ACK-decode noktası (ürün yolu) →
`Sender::diag_return_path()` → `FateWindowDiag.return_path` → bench `KP-ACTIVATED return_path=[…]`.

Ölçülen büyüklükler (hepsi ayrı fiziksel nicelik — INV-RETURN-1):

| alan | anlam |
|---|---|
| `measured_return_bw_bps` | ACK telinin gözlenen debisi. **31 B/s ÖNSELİ DEĞİL** — o en-kötü-durum priorı yerinde duruyor, bu ölçümdür |
| `ack_interval_ns` | geri-bildirim ritmi (EWMA) |
| `silence_ns` | son ACK'ten beri geçen süre |
| `feedback_continuity_permille` | kümülatif kaçırılmış pencere oranı |
| **`forward_loss_permille`** | `(sent_{n-1} − seen_n)/sent_{n-1}` — bir-ACK gecikmeli kümülatif |
| **`return_ack_loss_permille`** | `1000 × (1 − ACK_FREQ_N/dpa)` — alıcının **nominal ritmine** (2 datagram/ACK) göre MUTLAK ölçü |
| `datagrams_per_ack_x100` | gözlenen ACK başına datagram |
| `health` | Unknown / Healthy / Degraded / Dead — sessizlik **ve** mutlak ACK kaybı, iki bağımsız kanıt |

## 2. Ölçüm — R0–R5 (ürün profili, `profile=kama-v1-full`, 300 mesaj × 256 B)

| hücre | fwd / rev bozulma | `fwd_loss` | `ret_ack_loss` | dpa | `ack_int` | health | öngörü |
|---|---|---:|---:|---:|---:|---|---|
| R0 sağlıklı simetrik | — | 6‰ | **0‰** | 1 | 48 ms | Healthy | ✓ |
| R1 **yalnız ileri** | fwd loss %20 | **210‰** | 75‰ | 2 | 52 ms | Healthy | ✓ (150–350‰) |
| R2 **yalnız dönüş** | rev loss %40 | **6‰** | **424‰** | 3 | 64 ms | Degraded | ✓ |
| R3 asimetrik bant | rev 64 kbit | 6‰ | 0‰ | 1 | 49 ms | Healthy | ✓ |
| R4 asimetrik gecikme | fwd 20 / rev 200 ms | 6‰ | 0‰ | 1 | 49 ms | Healthy | ✓ |
| R5 ölü geri bildirim | rev loss %95 | 89‰ | **956‰** | 44 | 478 ms | **Dead** | ✓ |

**AYRIM (Faz-1 çıkış ölçütü):** ileri bozulma R1'de `fwd_loss=210‰` iken `ret_ack_loss=75‰`;
dönüş bozulmasında (R2/R5) `fwd_loss=6–89‰` iken `ret_ack_loss=424–956‰`. İki eksen **birbirine
karışmıyor**; hiçbir hücrede dönüş bozulması ileri-yön kaybına yazılmadı.

## 3. Beklenti sapması — implementasyon ÇÜRÜTÜLDÜ ve düzeltildi

İlk uygulama iki **DELTA**'yı kıyaslıyordu (Δseen [t_{n-1},t_n] vs Δsent [t_{n-2},t_{n-1}]);
pencereler hizasızdı. Ölçüm bunu anında yakaladı: **temiz ileri yolda** R2 %75, R3 %80, R5 %74
yapay "ileri kayıp" — yani tam olarak beklenti kaydındaki çürütme koşulu (INV-RETURN-1 ihlali).
Düzeltme: bir-ACK gecikmeli **kümülatif** oran. Bu form yapısal olarak muhafazakârdır — geri bildirim
seyrekleştikçe ileri kaybı olduğundan AZ gösterir, dönüş bozulmasını asla ileri yola yazmaz.

İkinci sapma: R5'te `health=Healthy` çıkıyordu, çünkü EWMA yeni (yavaş) ritme uyum sağlıyordu.
Düzeltme: alıcının **belgelenmiş ACK sıklığına** (`ACK_FREQ_N=2`, receiver.rs:76) göre MUTLAK
ACK-kaybı ölçüsü; EWMA uyumundan etkilenmez. Bu ölçü kendi birim-test fixture'larımı da yakaladı
(ACK başına 20 datagram = zaten %90 dönüş kaybı) — fixture'lar nominal ritme çekildi.

**Kalan dürüst sınır:** R5'te `fwd_loss=89‰` küçük bir yanlış-pozitif; aşırı geri-bildirim kaybında
bir-ACK gecikmesi 44 datagram'a çıktığı için taban bayatlıyor. Büyüklük sırası ayrımı bozmuyor
(89‰ ↔ 210‰) ve yön muhafazakâr. Kayıtlı sınır, gizlenmiş değil.

## 4. Kapsam dışı (bilinçli, spec'e uygun)

Ölçülen `return_bw`'nin **tüketilmesi** — EAI/confirm-ufkundaki 31 B/s önselinin yerine geçmesi ve
`ACK_FREQ`/ACK-bütçesi decimation'ı — spec'in kendisi tarafından ADIM-6'ya ertelenmiştir
(engine-spec §3.3-E: "FULL adaptif decimation … ADIM-6 ana gövdesi"). Faz-1 o borcun **ölçüm
yarısını** kapatır: nicelik artık ölçülüyor, kablolu ve test edilmiş durumda. Tüketim v1 dışıdır ve
kapanış raporunda açıkça beyan edilecektir.

## 5. Testler

Birim (deterministik, `fec_loss_risk.rs`): `r0_…` ölçülen bandın önselden bağımsızlığı ·
`r0b_…` nominal ritimde yanlış alarm yok · `r1_…` ileri kayıp dönüşe yazılmaz ·
`r2_…` dönüş bozulması ileri yola yazılmaz (üç ayırt edici) · `r5_…` sessizlik → Dead ·
`r5b_…` mutlak ACK kaybı EWMA uyumundan etkilenmez · `v1_…` V2 yoksa yön kanıtı üretilmez.
34/34 yeşil. Uçtan uca: `scripts/netem/r_returnpath_proof.sh` (6 hücre).
