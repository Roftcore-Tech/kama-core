# FAZ-3 — RUNTIME TELEMETRİ / AKTİVASYON KANITI · KAPANIŞ

**Kapsam: YALNIZ GÖZLEM.** Hiçbir transport kararı değişmedi, hiçbir algoritma ayarlanmadı,
hiçbir benchmark sonucu iyileştirilmedi. Eklenen alanların HİÇBİRİ bir kontrol yolunda okunmaz.

## 1. Kapatılan borç

Denetim tema C: **sevk edilen ikilide "mekanizma ateşledi mi?" sorusu cevapsızdı.** Tail-probe
emisyonu ve tahminci değerleri yalnız `diag-sched-telemetry` feature'ı altında görünüyordu; ürün
derlemesinde kanıt üretilemiyordu. Ayrıca `SchedDiag.fec_parity_{token_denied, wire_emitted}`
**deklare edilmiş ama hiç yazılmıyordu** (vakum sayaç).

## 2. Beş sorunun ayrı ayrı cevaplanması

`ActivationDiag` (sender) → `FateWindowDiag.activation` (ürün runtime) → bench `KP-ACTIVATION`.
Üç olgu KARIŞTIRILMADAN taşınır:

| soru | alan(lar) |
|---|---|
| 1. yapılandırıldı mı? | `cfg=[tail=… fec=…]` |
| 2. ulaşılabilir mi? (kod yolu koştu) | `reach=[transitions, recover_arq, recover_fwd, probe_up, left_preconv]` |
| 3. fiilen ateşledi mi? | `fired=[tail_probe@ilk-an, tail_cancel, parity, parity_denied, retx, dup]` |
| 4. hangi girdi sebep oldu? | `est=[btlbw, srtt, rtprop, qdelay, pace, tokens, inflight]` + mevcut `arbiter`/`spec_levels`/`gate_opened`/`ledger` |
| 5. hangi eylem tele ulaştı? | `parity`, `retx`, `dup`, `tail_probe` (emisyon sayaçları) + `fec_parity`/`retx` byte'ları |

## 3. Ürün-yolu ölçümü (`scripts/netem/v1_activation_proof.sh`, `profile=kama-v1-full`)

| hücre | cfg | fired | reach | est |
|---|---|---|---|---|
| temiz | tail=true fec=true | parity 0 · retx 0 · dup 0 · **tail 0** | 4 geçiş · left_preconv **217 ms** | btlbw 16.4 kB/s · srtt 204 ms · qdelay 0 |
| kayıp %20 | tail=true fec=true | **parity 39** · parity_denied 657 · retx 50 | 86 geçiş · **recover_arq 41** | btlbw 33.4 kB/s · srtt 209 ms |
| burst %50 | tail=true fec=true | **tail_probe 1 @ 7932 ms** · tail_cancel 2 · parity 11 · parity_denied 7490 · retx 162 · **dup 26** | 25 geçiş · recover_arq 12 | btlbw 14.6 kB/s · qdelay 295 µs · inflight 2387 B |

**Kapanan somut boşluk:** tail-probe'un ateşlemesi artık ürün derlemesinde kanıtlanabiliyor
(burst hücresinde 7.93 s'de bir probe, iki iptal). Faz-1 raporundaki "tail açık diyebiliyorum ama
ateşledi diyemiyorum" cümlesi artık geçerli değil.

**Gözlem (Faz-3'te düzeltilmedi, kayıt):** burst hücresinde `parity_denied=7490` ≫ `parity=11` —
pacer, parity denemelerinin ezici çoğunluğunu reddediyor. Bu bir ölçümdür; politika sorusu
Faz-5'in (T2 burst) konusudur.

## 4. Vakum sayaç düzeltmesi

`SchedDiag.fec_parity_token_denied` / `fec_parity_wire_emitted` artık gerçek emisyon ve gerçek
pacer-reddi noktalarında yazılıyor → feature'lı görünüm ürün sayaçlarıyla tutarlı. (Denetimin
"vakum sayaç, DELETE adayı" bulgusu, silme yerine DOLDURMA ile kapatıldı: alanların adı doğruydu,
yalnız yazılmıyorlardı.)

## 5. Testler (deterministik, vakum korumalı)

- `faz3_yapilandirildi_atesledi_ayrimi` — tail KAPALI → `configured=false`; tail AÇIK ama sürülmedi
  → `configured=true, probes=0`. **Yapılandırma ateşleme değildir.**
- `faz3_ulasilabilirlik_ve_tahminciler_dolu` — ACK'li akıştan sonra `state_transitions>0`,
  `left_preconverge_ns>0`, `srtt_us>0`, `pacer_rate>0`, ARQ kolu ateşlemiş.
- `faz3_fec_yapilandirilmamisken_parity_sayaclari_sifir` — FEC yokken parity sayaçları SIFIR
  (telemetri yanlış kanıt üretmez).

## 6. Kapı

fmt · clippy `--all-features` 0 · clippy `--no-default-features` 0 · PR test kümesi **914/0**
(133 binary) · aktivasyon koşusu ürün profilinde üç hücrede de sayaç üretiyor.

## 7. Faz-3 çıkış ölçütü

> "Her büyük SHIP mekanizması için runtime aktivasyonu KANITLANABİLİR; hiçbir kritik mekanizmanın
> çalıştığı yalnızca VARSAYILMIYOR."

**SAĞLANDI** — aşağıdaki mekanizmalar için ürün derlemesinde ateşleme kanıtı üretilebiliyor:
ARQ retransmit · forward duplication · tail-probe · cold-start (PreConverge çıkışı) · FEC önerisi
(spec seviyeleri + arbiter nedenleri) · FEC parity emisyonu ve pacer reddi · yapı-gate ·
kuruluş guard'ı · kayıp defteri · dönüş-yolu tahmincisi (Faz-1) · faz geçişleri · bant/RTT/pacing.

**Kanıtı ÜRETİLEMEYEN kalan mekanizma yok**; ateşlemediği ölçülen mekanizmalar (`probe_up=0`,
`recover_fwd=0` bu hücrelerde) dürüstçe sıfır olarak raporlanır — bu bir telemetri boşluğu DEĞİL,
o hücrede gerçekten devreye girmediklerinin kanıtıdır.
