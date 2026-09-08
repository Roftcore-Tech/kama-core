# v1 AKTİVASYON KANITI — config → öneri → arbiter → encoder → TEL

Koşum: `scripts/netem/v1_activation_proof.sh` (kapi3 iki-netns netem, 200 mesaj × 256 B, RTT 200 ms,
1 mbit). Amaç performans DEĞİL, **ateşleme**: her mekanizmanın fiilen çalıştığını sayaçtan kanıtlamak
(performanstan çıkarım yasak). Ham: `$S/v1act2`.

Üç hücrede de manifest: `profile=kama-v1-full` · `DELTA-vs-v1: (yok)` — yani ölçülen şey ürün profili.

| hücre | parity B | blok | repair | spec L0/L1/L2 | arbiter (None/…/Gate/Boot/Pers) | gate_opened | retx B | dup B | ledger tepe | durum |
|---|---|---|---|---|---|---|---|---|---|---|
| temiz | **0** | 0 | 0 | 4564/0/0 | 4564/0/0/0/0/0/0 | 0 | 0 | 0 | 0 | Idle |
| kayıp %20 | 48 392 | 23 | 46 | 447/124/4129 | 447/…/0/124/**4129** | 0 | 10 054 | 0 | 30 000 ppm | Steady |
| burst %50 | 48 392 | 23 | 46 | 477/109/5340 | 477/…/**1236**/109/4104 | **1236** | 52 290 | 6 132 | 260 000 ppm | Steady |

**Kanıtlananlar:**
1. **FEC alt-sistemi ürün profilinde AÇIK ve adaptif** (`fec=ON adaptive p0x05`): temiz yolda parity **0**,
   kayıp altında parity **> 0** — "otomatik tetiklenme" tam olarak bu.
2. **Spekülatif kol → arbiter → encoder → tel** zinciri tamam: L2 önerileri (4129/4104 tick) arbiter'da
   `SpecPersistent` olarak kazanıyor, latch 23 blok kapatıyor, 46 repair TELE çıkıyor.
3. **Yapı-gate (canonical kol) burst'te ayrı ateşliyor** (`gate_opened=1236`, arbiter `StructureGate=1236`)
   → iki karar kolu da canlı ve arbiter ikisini birden görüyor.
4. **ARQ + forward duplication** aynı anda çalışıyor (retx 10–52 kB, dup 6.1 kB burst'te) — FEC ARQ'nun
   yerine geçmiyor, yanında koşuyor.
5. **Kayıp defteri** canlı (tepe 30 000 → 260 000 ppm), spurious 0 (reorder yok, beklenen).

**Kanıtlanamayanlar (telemetri açığı, ayrı kalem):** tail-probe emisyonu ürün yolunda sayaçsız
(`diag-sched-telemetry` feature'ı gerekiyor) — mekanizma açık (`tail=ON(validated_v1)`) ama ateşlemesi
default derlemede görünmüyor. Aynı şekilde bw/srtt/pacing tahmincileri ürün istatistiklerine
export edilmiyor (SPEC→CODE `bw-rtt-pacing` kümesi bunu WIRE olarak işaretledi).

## C-1 (KALİBRASYON KALEMİ — blocker değil) — symbol boyu E ile mesaj boyu uyumu

Profil 0x05'te her repair TAM E = 1024 B'dir (§2.6 symbol_len kilidi). 256 B'lik test yükünde:
46 repair × 1052 B ≈ 48 kB parity, 51 kB payload karşılığı → **~%94 ek yük**. Aynı iş profil 0x03
(E=64) ile ~%30 olurdu ama birim 53 B'ye iner ve fragmenter (her DATA'ya +11 B) zorunlu olur.

Yani **FEC profili bir dağıtım parametresidir**: E, beklenen birim boyuna eşleşmelidir. v1 default'u
0x05'tir çünkü ürünün birim tavanı MTU ölçeğindedir (1013 B) ve tam dolu symbol'de sabit ek yük %3.5'e
iner. Küçük-mesajlı dağıtımlar (≈256 B) için 0x03 + fragmenter ölçülmelidir — **açık kalibrasyon kalemi**,
kablolama kusuru değil. T-matrisi hem symbol-dolu (≈1000 B) hem tarihsel (256 B) yükte raporlanmalı.
