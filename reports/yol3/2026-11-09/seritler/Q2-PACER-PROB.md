# Q2: kova tavanı (A-3) ve ProbeUp (A-4), bayrak arkasında

- Kaynak belge: `QUINN-FARKI-KOD-KARSILASTIRMA-2026-11-09.md` (sha256 85c4d27e…), §3-A'nın 3. ve 4. maddeleri.
- Dal `yol3/q2-quinn-pacer-prob` @ **c8450809**. Taban `yol2/o1` 5fdad83b. Dal origin'de; main'e dokunulmadı.
- Worktree: `/home/ryuzaki/kama-wt/yol3-q2`.
- Commit'ler:
  - c34e6d6c `feat`: bayraklar ve motor kodu;
  - c8450809 `test`: birim testleri.
- Etiketler:
  - [kod]: kaynakta açık;
  - [ölçüldü]: `E1Sim` deterministik birim düzeneği (netem değil);
  - [türetilmiş]: aritmetik;
  - [çıkarım].
- Release derleme ve netem koşulmadı. s3.rs, handles.rs ve protector.rs'e dokunulmadı. todo!() yok. Tel baytı değişmez.

## Bayraklar ve taşındıkları yol

- `adaptive_transport::rate::E1Flags`'e iki alan eklendi: **`a3_burst_floor`** ve **`a4_probe_srtt`** (`rate.rs:217-222`; tablo `rate.rs:198-204`). `Default` ikisini de kapalı tutar.
- Yol E1 ile aynı: `RuntimeConfig.e1_flags` (`client-core/src/runtime.rs:192`) → `Sender::set_e1_flags`.
- Harness kollarına eklenmedi. bench `E1Set::flags` (`bench/src/e1arm.rs:107-111`), f0_flows `E1Set::flags` (`:295-299`) ve testleri iki alanı `false` yazıyor; K0…K2−e kolları bit-aynı kalıyor.
- **Ölçüm için K1+A3+A4 kolu ayrı bir iş.**
- Kapalıyken bit-aynılık [ölçüldü]: `tests/e1_goster.rs::e1_flags_off_trace_digest_reference`'ın dört özeti E1-TESHIS-G2'deki referansla aynı çıktı: 5903368f0467da80 / 3efa627a0ccb8e95 / dd81553968598d65 / 4be8d90ae77aedb9.

## A-3: kova tavanı (`sender.rs:2403-2415`)

**Değişiklik**

- `burst_lo = if a3 ∧ state ≠ PreConverge { max(front, 2·MTU) } else { max(front, iw) }`.
- `burst_hi = max(front, 2·MTU, iw)` değişmedi.
- PreConverge dışında iw = 0 olduğundan açıkken `lo == hi == max(front, 2400)`. Böylece clamp geçerli kalıyor (lo ≤ hi).
- PreConverge'deki ilk-uçuş davranışı aynen duruyor.

**Etki** (`q2_a3_burst_floor_rescue_not_starved_by_small_front`) [ölçüldü]

- Düzenek: 12,5 kB/s, 150 ms. 1100 B'lık birimin ilk iletimi düşüyor; ardından 18 s boyunca her 20 ms'de 93 B'lık birim geliyor.
- Kurtarma iletimi kapalıda **20,12 s**'de çıkıyor (küçük birim akışı bitince), açıkta **2,45 s**'de (kayıp ilanında).
- Mekanizma: küçük birim ön sıradayken her olayda tavan = clamp(rate·40 ms, 93, 2400) < 1100 oluyor ve `set_burst_cap` jetonu bu değere kırpıyor (`pacer.rs:70-73`).
- Keşif ızgarası (testte yok):

  | hız | küçük birim aralığı | fark |
  |---|---|---|
  | 5 kB/s | 20 ve 40 ms | aynı desen |
  | 12,5 kB/s | 40 ms | yok |
  | 5 ve 12,5 kB/s | 80 ms | yok |

  Fark çıkmayan hücrelerde jeton, küçük birimler arasında 1100'e doluyor.

**Bilinen sınır** (`q2_a3_residual_data_first_still_holds_larger_rescue`) [ölçüldü]

- Düzenek: 5 kB/s; 1100 B'lık birimin arkasında 80 × 1000 B.
- Kurtarma iki kolda da son DATA'dan sonra çıkıyor:

  | kol | kurtarma | son DATA |
  |---|---|---|
  | kapalı | 18,10 s | 17,93 s |
  | açık | 18,04 s | 17,86 s |

- Neden [kod]: `schedule_transmit` DATA-önce çalışıyor (`drain_new_data` `:4421`, ardından `plan_arq_retransmits`). Kova ön birim boyunda (1000 B) tükeniyor, jeton 1100'e hiç çıkmıyor.
- Belge §0-5'teki "1058 B parite, 1024 B DATA'nın önünde" vakası bu sınıfta [türetilmiş]. A-3 "asla gönderilemez" yolunu kaldırıyor, ama DATA kuyruğu sürerken parite ve büyük kurtarma yine bekliyor.
- Kapatmak için jeton rezervi ya da sıra kararı gerekir (§3-B-9 kapsamı).

**Etkileşimler**

- [çıkarım] (d)'nin tur mandalı "kova dolu" koşulunu (`tokens == burst_cap`, `:4389`) 2400 B'da sağlar. Daha geç dolar, daha az tur app-limited işaretlenebilir.
- [kod] PreConverge çıkış olayında durum artık Startup olduğu için `fill_to_cap` (`:2419`) 2400 B veriyor: kalkışta 2 birimlik burst.
- [türetilmiş] 31 B/s köşesinde 2400 B jeton 77 s'de birikir; uzun beklemeden sonra 2 birimlik burst çıkar. Belgenin "rate < 1 kB/s'te önsel" rejim kapısı bu bayrakta **yok**: talimat aynen uygulandı.
- INV-RATE-1: hız değişmiyor, yalnız kova kapasitesi değişiyor. Kayıp girdi değil.

## A-4: ProbeUp (`sender.rs:4126-4131`, `:4147-4158`)

**(a)** `loss_abort = attributed ∧ ¬e1_probe_keep ∧ ¬a4_probe_srtt`. İptal artık yalnız qd > 500‰ ile.

**(b)** Steady'de `epoch = srtt` (bayrak açık) ya da `max(srtt, EAI)` (kapalı, eski ifade). Aynı `epoch` girişte `probe_epoch_entry_ns`'e donuyor. Böylece:

- periyot = 4·srtt;
- dwell = `PROBE_MAX_DWELL_EPOCH`·srtt = 4·srtt;
- volume = BtlBw·2·srtt.

**K'nin gerekçesi.** K = `K_PROBE_PERIOD_EPOCH` = 4 aynen alındı; yeni sabit yok ve değer [CAL {2,4,8}] matrisinin ortasında.

- Sabit "epoch" birimiyle tanımlı; epoch'un amacı kanal saati (ZM-2/3).
- EAI'nin 65 B/31 B/s = 2,097 s'lik dönüş önseli, srtt < 2,1 s olan her hatta epoch'u EAI'ye bağlıyordu.
- Bu hücrede eski periyot ≈ 4·2,3 = 9,4 s; yenisi 4·0,324 ≈ 1,3 s [türetilmiş].
- BBRv1 döngüsünde 1,25 fazı ≈ 8·min_rtt'de bir gelir. K = 4 bunun iki katı sık, ama dwell de 4·srtt ile sınırlı; görev çevrimi ≤ %50 [türetilmiş].

**(e-dar) ile etkileşim** [kod + ölçüldü]

- (a) (e-dar)'ın 2. kuralının alt kümesi. `e1_probe_keep` açıkken `loss_abort` zaten false; K1/K2'de (a) etkisiz.
- (e-dar) kapalıyken (a) yalnız atıflı iptali kaldırıyor; bekleyen kayıp ProbeUp'ı yine RecoverArq'a kesiyor (`pending_preempts_probe`, `:4014`, `:4038`). %20 kayıpta (a)'nın tek başına etkisi küçük [çıkarım].
- `q2_a4_attributed_loss_does_not_abort_probe_up` dört birleşimi sınıyor: iptal yalnız ikisi de kapalıyken.
- (b) (e-dar)'dan bağımsız. `q2_a4_more_probe_entries_under_e_dar` (iki kolda e-dar açık; 12,5 kB/s, 150 ms; deterministik %20; 40 s):
  - ProbeUp girişi 2 → **8**;
  - kapanışlar: zaman aşımı 2 / 7, başarı 0 / 0.
- Başarı 0, çünkü 1,25 × 0,8 = 1,0 ve BtlBw +%2,5 eşiğini geçmiyor. Belgedeki §1 "(1−p) telafisi" satırıyla tutarlı.
- [çıkarım] DrainRtprop yalnız Steady'den girer. Prob sıklaşınca Steady'de geçen süre ve DrainRtprop fırsatı azalır.
- INV-RATE-1: (a) kaybın faz kazancını 1,25'ten 1'e indirdiği bir bağı kaldırıyor; (b) kaybı girdi yapmıyor.

## Testler ve kapılar

Testler (`sender.rs:9758-10030`; `rate.rs` varsayılan testi iki alanla genişledi):

| test | ölçtüğü |
|---|---|
| `q2_a3_burst_floor_admits_unit_larger_than_front` | tavan < MTU, `ns_until_sendable` None ↔ tavan 2·MTU, `Some` |
| `q2_a3_burst_floor_rescue_not_starved_by_small_front` | kurtarma anı, küçük birim akışı altında |
| `q2_a3_residual_data_first_still_holds_larger_rescue` | bilinen sınır, DATA-önce sırası |
| `q2_a4_attributed_loss_does_not_abort_probe_up` | (a), dört birleşim |
| `q2_a4_probe_clock_is_k_srtt_without_eai` | (b) 4·srtt'de giriş, epoch = srtt, 4·srtt'de zaman aşımı; kapalıda giriş yok |
| `q2_a4_more_probe_entries_under_e_dar` | (b) giriş sayısı |

Kapılar (hepsi `cargo-lock.sh`, 1.96.0, `--locked`):

| komut | sonuç |
|---|---|
| `test -p adaptive-transport --lib` | **171/171** |
| `clippy -p adaptive-transport --all-targets -- -D warnings` | **rc 0** (ilk koşuda tek `manual_is_multiple_of`; düzeltildi) |
| `fmt -p adaptive-transport -p bench -p kama-integration -- --check` | **rc 0** |
| `test --test e1_goster` | 2/2 |
| `check -p bench -p kama-integration --all-targets` | rc 0 (bench ve kama-integration testleri koşulmadı) |

## Belgede olmayan bulgular (Q2 analizinden)

1. quinn 0.11.18'de BBR `pacing_rate` pacer'a bağlı değil. Pacer pencere·1,25/srtt kullanıyor (`pacing.rs:92`); BBR hızı yalnız `metrics()`'e gidiyor (`bbr/mod.rs:494-499`). Gain döngüsü (`:159-200`) telde etkisiz, yani belge §3-B-6'daki "quinn gain döngüsü" quinn'de gözlenen davranışın kaynağı değil [kod].
2. quinn Startup'ta `cwnd_gain < target_window as f32` karşılaştırması (`bbr/mod.rs:327`) kazancı bayta kıyaslıyor ve hep doğru çıkıyor. cwnd += acked, üst sınırsız; app-limited'da da büyüyor. F0 koşu sonu cwnd 323 861–348 264 B ≈ 240 000 + ACK'lenen bayt; quinn koşu boyunca Startup'ta [kod + türetilmiş].
3. quinn bant örneği BBR'ın delivered/interval'ı değil: paket başına min(son iki gönderim hızı, ardışık iki ACK hızı) (`bw_estimation.rs:41-67`). Max-filtre boşluksuz çiftleri seçtiği için (1−p) yanlılığı yok. Belge §1'deki "Teslim hızı örneği: Eşdeğer" satırı bu yönden düzeltilmeli [kod].
4. quinn ProbeBw'de (bu hücrelerde erişilmiyor) (1−p) telafisi 1,25'ten değil, cwnd_gain 2'den geliyor. inflight ≤ 2·BDP olduğundan p ≤ 0,5'e kadar darboğazı doldurur; RTT = 2(1−p)·rtprop'ta dengelenir. BBRv1'in pacing döngüsünün sınırı p ≤ 0,2 [türetilmiş].
5. quinn'de `probe_rtt_last_started_at = None` iken ilk app-limited olmayan ACK ProbeRtt'ye sokuyor (`bbr/mod.rs:213-219`). min_rtt ömür boyu minimum (`paths.rs:338`). BBR modülü bütün olarak port edilmemeli [kod].
6. Analog hücrede örnek aralığı ≈ 4 birim; max-filtre kayıpsız aralıkları sık görüyor, BtlBw ≈ gönderim hızı. Bağlayan terim Steady kazancı 1: tel ≈ C, darboğaz ≈ 0,8·C dolu (netem kaybı kuyruk girişinde). K1 pace medyanı 12 262 B/s ile tutarlı [türetilmiş + ölçüldü].
7. 12,5 kB/s'de 1080 B'lık birimin serileşmesi ≈ 86 ms, yani RTprop'un ≈ %27'si. 250‰'lik qd eşikleri (E1 freni, DrainRtprop kapısı) boş kuyrukta bile eşiğe yakın ölçebilir [türetilmiş; ölçülmedi].
8. A-3 sınırı (yukarıda): DATA-önce sırası ön birimden büyük kurtarma ya da pariteyi DATA kuyruğu boşalana kadar bekletiyor [ölçüldü].

## Açık

- Harness kolu (K1+A3+A4) yok, netem ölçümü yok. Etki yalnız deterministik düzenekte gösterildi.
- A-4 (b), TUR-3c D-2/4 sözleşmesinin periyot saatini bayrak arkasında değiştiriyor. Spec yaması gerekiyorsa sahip kararı.
