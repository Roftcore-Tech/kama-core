# E1-TESHIS-G2 — G2 teşhis manifesti (ön kayıt)

| alan | değer |
|---|---|
| yazan | ajan-M2 (ISEMRI-YOL3-M2 KK-M2-09) |
| donma | Sal 10 Kas 2026 20:00'den önce, `yol3/m2-e1-bayrak` dalında tek docs commit'iyle; sonra **değişmez** (outcome-blind). İkili sha256'sı ayrı "koşu kaydı" satırıdır, bu dosyaya girmez. |
| dayanak | CC §4.2 (CC-K7 imzalı; "6 Kas" = G2 Çar 11 Kas), sahip kararı v4 I.3 ("CC §4.2 teşhisi … **aynen**"), CC §6.4 1–6 |
| koşan | Ö (D-18: orkestratör başlatır); okuma: M2 + orkestratör; ölçüm süresince ajan ve cargo yok |
| etiket | gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta. |

§2 ve §5 CC §4.2'den **aynen** alıntıdır (iş emri §2.4); §1, §3, §4, §6 bu manifestin kendi satırlarıdır.

---

## 1. Kollar → çalışma zamanı bayrakları

Tek release ikilisi; kol yalnız `RuntimeConfig.e1_flags` (`adaptive_transport::rate::E1Flags`) ile seçilir. Yapılandırma yeri KK-M2-02 (G1'de onaylandı): alan `crates/client-core/src/runtime.rs` `live_diag`'ın ardı; varsayılan `E1Flags::default()` (beşi `false`); oturum kurulumunda `set_initial_window` bloğunun ardında `session.sender_mut().set_e1_flags(cfg.e1_flags)`.

| kol | içerik (CC §4.2) | `e1_startup_cwnd` (c) | `e1_applimited_plateau` (d) | `e1_drainrt_gate` (DrainRtprop) | `e1_probe_keep` (e-dar) | `retx_first` |
|---|---|---|---|---|---|---|
| K0 | kapalı | 0 | 0 | 0 | 0 | 0 |
| K0r | K0 + retx_first | 0 | 0 | 0 | 0 | 1 |
| K1 | K2 − retx_first | 1 | 1 | 1 | 1 | 0 |
| K2 | tam aday: (c) + (d) + DrainRtprop + (e-dar) + retx_first | 1 | 1 | 1 | 1 | 1 |
| K2−c | K2 − (c) | 0 | 1 | 1 | 1 | 1 |
| K2−e | K2 − (e-dar) | 1 | 1 | 1 | 0 | 1 |

- **(d)'nin qd freni: GİRER** (§4; KK-M2-08). Ayrı bayrak değildir: `e1_applimited_plateau` açıkken sabit parçadır (işaretli turda tur işaretindeki qd ≥ 250‰ ise tur yine değerlendirilir).
- Ön testler geçti (§3) → KK-M2-11 devreye girmez; (e-dar) kollarda kalır; toplam iş 160.
- **Kaynak ağaç (KK-M2-10):** `yol2/o1` = `main` (Ö-a düzenek dahil) + `yol3/m2-e1-bayrak` + Ö'nün kol seçimi; orkestratör kurar, release'i orkestratör derler (D-34), origin'e push. K0 aynı ikilide bayraklar kapalı. M2 dal SHA'sını G1 raporunda verir. Bayrakların üretim kodu `c4c37a5f`'te tamamdır; `4d568f08` yalnız clippy düzeltmesidir (`update_state`'te (e-dar) koşulunun De Morgan yazımı; davranış aynı); diğer sonraki commit'ler testler (`#[cfg(test)]` modülleri, `tests/`) ve docs'tur. Teşhis ikilisinin kaynak SHA'sı ve sha256'sı koşu kaydına yazılır.
- **Profil etiketi (INV-009; KK-M2-25):** K0 dışındaki kollar `kama-custom` + sapma listesi taşımalıdır. Bench `profile_delta` (`crates/bench/src/kama_product.rs:256-`) `RuntimeConfig`'i alan alan kıyaslar; `e1_flags` oraya eklenmezse K0r/K1/K2/K2−c/K2−e koşuları `kama-v1-full` etiketi alır (INV-009 ihlali). Ekleme Ö'nündür.
- **Ö'nün komut satırları:** Ö verir. Bu dosya donduğunda kol ↔ bayrak eşlemesi yukarıdaki tablodur ve bağlayıcıdır; komut satırları ve ikili sha256'sı koşu kaydına yazılır (donmuş içeriğe girmez).
- **Model ↔ kod adları:** model (`~/kama-wt/yol3-m2-model/model/kama_model.py`) `c` = (c), `d` = (d), `drq` = DrainRtprop, `e_dar` = (e-dar), `retx_first`; `dqb` = qd freni (yalnız model kopyasında, KK-M2-08).

## 2. Hücreler, okuma kuralı, ön testler (CC §4.2, aynen)

Kollar:

| kol | içerik |
|---|---|
| K0 | kapalı (`a858f3fc` + Ö-a düzenek, bayraklar kapalı) |
| K0r | K0 + retx_first |
| K1 | K2 − retx_first |
| K2 | tam aday: (c) + (d) + DrainRtprop + (e-dar) + retx_first |
| K2−c | K2 − (c) |
| K2−e | K2 − (e-dar) |

Hücreler. Hepsi N = 10.

Hücreler. Hepsi N = 10.

| sıra | hücre | kollar | iş | amaç |
|---|---|---|---|---|
| 1 | gösterim analoğu: `a_r12k_d150_l20_foto150k` (Y10) hazırsa, yoksa `a_r12k_d300_l30_dosyali` | hepsi (6) | 60 | seçim: PTT p95, dosya bitişi |
| 1 | `a_r12k_d300_l30_dosyasiz` | K0, K2, K2−c | 30 | PTT-yalnız oturumda yeni veri emisyonu |
| 2 | manşet `b_r12k_d1000_l30_p1000` | K0, K2, K2−c | 30 | "E1 var/yok" (hız) |
| 3 | `b_r12k_d1000_l0_p1000` | K0, K2 | 20 | koruma: T0 |
| 3 | `b_r25_d300_l20_p65` | K0, K2 | 20 | koruma: K-3 |

- "**Toplam 160 iş.** Süre [türetilmiş; ölçek 90 iş ≈ 25 dk paralel, ≈ 76 dk seri]: ≈ 45 dk paralel, ≈ 2,3 sa seri. Önce 1. sıra (90 iş; ≈ 25 dk / ≈ 76 dk)."
- "Ölçüm sırasında ajan ve cargo yok (v2 §11.5); bu süre bütün yerel ajanları durdurur."
- "K0r dosyasız hücrede koşulmaz: retx_first pacer hızını artırmaz; K0'ın emisyonunu en fazla azaltır [çıkarım]."
- "K1 ve K2−e dosyasız hücrede koşulmaz; K2'nin sonucu kullanılır [çıkarım: (e-dar) bu hücrede devreye girmez; retx_first emisyonu yalnız azaltabilir]."
- "Koruma hücreleri yalnız K2'de koşulur; seçilen set K2'nin alt kümesi olduğu için okuma ona da uygulanır [çıkarım]."

**Okuma kuralı.** "Ön kayıtlı; G1'de sha256'lı, teşhisten önce donar."
- **S, seçim (bağlayıcı):**
  - Bir kol uygundur ⇔ şunların hepsi:
    - (i) gösterim analoğunda dosya bitiş medyanı ≤ K0 × 1,10;
    - (ii) PTT alıcı p95 medyanı ≤ K0 × 1,10;
    - (iii) E1 bileşeni içeriyorsa dosyasız hücrede yeni-veri emisyonu/sunulan medyanı ≥ 0,95;
    - (iv) R3 geçer.
  - Uygun kollar arasında PTT p95 medyanı en düşük olan seçilir. En düşüğün %10'u içindekiler arasında en az bayraklı olan; bayrak sayısı eşitse dosya bitişi en kısa olan.
  - Hiçbir E1 kolu uygun değilse: K0r, PTT p95 ≤ K0 × 0,90 ve (i) sağlanıyorsa; değilse K0. İkisinde de PTT yalnız kısa basışla (SAHİP §III.5 geri dönüşü); (b) + (e) Aralık'a.
- **R1, "E1 var/yok" (raporlanan satır; seçimi bağlamaz):**
  - Manşette tuzaksız koşularda (PreConverge çıkışı ≤ 10 s) gönderim/gereken medyanı ≥ 0,90. Tuzaksız koşu 7'den azsa bütün koşular kullanılır. Bütün koşuların medyanı ayrıca yazılır.
  - Yöntem ACIL'inki, pencere [3, 28) s.
  - Tuzaksız okuma bağlayıcı, çünkü PC tuzağını gösterim ağacında E2a m6/m8 kartları kapatıyor.
- **R3, koruma (bağlayıcı):**
  - d1000 l0'da p95 medyanı ≤ K0 × 1,10.
  - r25'te teslim@60 ≥ K0 − 1 ve gönderim ≤ K0 × 1,10.
  - Uyarı [ölçüldü]: F0'da r25 koşularının hepsi 60 s içinde Startup ya da PreConverge'de bitiyor; bu hücrede plato ve (d) hiç devreye girmiyor. R3-r25 boş geçebilir. Uzun r25 ölçümü §4.4'te.
- **R4, raporlanan (kapı değil):**
  - manşet sw95 medyanı ve 9. sıra;
  - her kolda pace medyanı ve ondan türeyen EAI ve kayıp eşiği;
  - Startup dönemi kaybı ([0, 20) s ayrı);
  - koşu sonu RTprop ↔ K0 (DrainRtprop cırcırı);
  - Ö G1'de kablolayabilirse `diag_probe` sayaçları (M4 doğrudan ölçülür).
- **Ön testler (v2 §5.2):** "ProbeUp sırasında retx sürer" ve "(e-dar) yalnız, p = 0: tel baytı bayrak kapalıyla bit-aynı". Geçmezse (e-dar) bütün kollardan çıkar; teşhis kalan kollarla koşar (v2'deki "teşhis sonuçsuz, G4'e kayar" kuralının yerine).
- ~~Kayma kuralı: G2 kayarsa Cmt 7 Kas …~~ **[v4 ile geçersiz; Bölüm IV kayma kuralı geçerli]**
- **Akşam sahibe tek satır:** "Gösterim seti: <kol>. E1 (manşet, tuzaksız): K2 <x>, K2−c <y>."

## 3. Ön testler ve `retx_first` testi — G1 sonucu (koşuldu)

Komut (D-34 sarmalayıcısı, worktree'nin kendi `target/`'ı):
`KAMA_AGENT=M2 scripts/ci/cargo-lock.sh test -p adaptive-transport --lib -- e1_` → `test result: ok. 15 passed; 0 failed` (2026-11-09, ağaç = `yol3/m2-e1-bayrak` @ `005edbfe`).

| test | tanım | sonuç |
|---|---|---|
| ön test 1 | `sender::tests::e1_pretest1_retx_continues_during_probe_up` — toplu aktarım, 12,5 kB/s, RTT 300 ms; ProbeUp'ta gönderilen ilk DATA düşürülür. Kapalı: durum RecoverArq'a geçer, preempt ≥ 1, ProbeUp'ta retx 0 (bugünkü davranış). Açık (`e1_probe_keep` yalnız): durum ProbeUp kalır, retx ProbeUp içinde tele çıkar, preempt 0. | geçti |
| ön test 2 | `sender::tests::e1_pretest2_probe_keep_alone_p0_wire_identical` — kayıpsız kapalı döngü (toplu + 80 ms'de 160 B demet + toplu), 40 s; ProbeUp ≥ 1 kez girildi (boş geçiş değil), retx 0. Bayrak kapalı ↔ yalnız `e1_probe_keep` açık: motorun tele verdiği karar dizisi (zaman, tür, seq, birim, boyut, send_record) ve prob muhasebesi bit-aynı. | geçti |
| `retx_first` | `sender::tests::e1_retx_first_arq_first_on_every_grant` — deterministik %20 DATA kaybı, toplu. Kapalı: DATA beklerken retx yalnız `tick % K_ARQ_SHARE == 0` izinlerinde önce, diğer izinlerde DATA önce ve retx bekler. Açık: retx beklerken hiçbir izin DATA ile başlamaz; K'ıncı olmayan izinlerde de retx önce. | geçti |

Ön test 2'nin "tel baytı" ölçüsü motor düzeyindeki karar dizisidir; tel kodlaması client-core'da bu dizinin deterministik işlevidir (KK-M2-26).

K0 tabanı (bayraklar kapalı = önceki `main`): `tests/e1_goster.rs::e1_flags_off_trace_digest_reference` dört kapalı-döngü senaryosunun (toplu p=0, toplu %20, PTT p=0, karışık %10) karar dizisi özetini basar; aynı senaryo `set_e1_flags` çağrısı çıkarılarak `origin/main` `5de1ce3f` ağacında koşuldu. Dört özet varsayılan ve `--all-features` derlemede bit-aynı (`5903368f0467da80`, `3efa627a0ccb8e95`, `dd81553968598d65`, `4be8d90ae77aedb9`). `cargo test -p adaptive-transport --all-features`: 51 test ikilisi, 491 geçti, 0 kaldı (ağaç `005edbfe` + çalışma kopyası, 2026-11-09).

## 4. (d) qd freni — G1 model kararı (KK-M2-08)

**qd freni: GİRER — S1 Δ 0,000 (K2 0,878 / 0,878; K2−c 0,443 / 0,443), S3 PTT p95 1,373 / 1,373 (K2), 1,297 / 1,297 (K2−c) [tahmin; model kopyası, PC düzeltmesiz, N = 30, medyan; frenli / frensiz].**

- Ölçüt (KK-M2-08, CC §2.1 lafzı): S1'de gönderim/gereken düşüşü ≤ 0,02 **ve** S3'te PTT p95 frenli medyan ≤ frensiz medyan × 1,10, iki kolda (K2, K2−c). İkisi de sağlandı → fren (d)'nin parçası.
- Sayaç (salt gözlem, aynı N = 30): S1'de (d)'nin işaretlediği turlar K2 161 / K2−c 51; bunların qd ≥ 250‰ olanı K2 11 / K2−c 5. S3'te (d) hiç tur işaretlemedi (dosya aktarımı sürerken kuyruk hiç boş değil) → fren S3'te etkisiz [tahmin].
- Komutlar: `python3 -I ~/kama-wt/yol3-m2-model/SON/betik/k2c.py 30 S1,S3` (ARMS'a `K2+fren`, `K2-c+fren` eklenmiş kopya; çıktı `SON/betik/k2c_qdb_n30.txt`) ve `python3 -I ~/kama-wt/yol3-m2-model/SON/betik/qdb_sayac.py 30 S1,S3` (çıktı `SON/betik/qdb_sayac_n30.txt`). Özgün model dizinine yazılmadı.
- Kodda: `e1_applimited_plateau` açıkken işaretli tur, tur işaretinde `qd_ratio_permille_now() ≥ E1_PLATEAU_QD_BRAKE_PERMILLE` (250) ise yine değerlendirilir. Kayıp girdi değildir (INV-RATE-1; "kayıp > p̂ ise plato" alınmadı).

## 5. Beklenti kaydı (CC §4.2, aynen)

**Beklenti kaydı** [tahmin; model, PC düzeltmesiz, N = 30, medyan; yalnız yön]. Gösterim analoğu satırları S3'ten (RTT 0,3 s, %20 iki yön, dosya + PTT).

| ölçü | K0 | K0r | K1 | K2 | K2−c | K2−e |
|---|---|---|---|---|---|---|
| gösterim analoğu PTT p95 (s) | 2,13 | 1,10 | 2,71 | 1,37 | 1,30 | 1,24 |
| dosya bitiş (s) | 49,3 | 52,2 | 43,6 | 43,7 | 43,2 | 47,8 |
| dosya goodput/C | 0,74 | 0,68 | 0,79 | 0,83 | 0,83 | 0,73 |
| kuyruk bekleme p95 (s) | 0,17 | 0,15 | 0,37 | 0,39 | 0,30 | 0,28 |
| manşet gönderim/gereken, bütün / tuzaksız | 0,46 / 0,52 | 0,44 | 0,90 / 0,99 | 0,88 / 0,99 | 0,44 | 0,88 |
| manşet sw95 (s) | 23,4 | 31,3 | 6,8 | 9,6 | 25,9 | 9,6 |
| d300 l30 mesaj gönderim/gereken (dosyasız hücrenin en yakın model karşılığı) | 0,85 | 0,78 | 0,97 | 0,98 | 0,96 | 0,97 |
| d1000 l0 p95 (s) | 4,1 | 4,1 | 3,4 | 3,4 | — | 3,4 |
| r25 | aynı | aynı | aynı | aynı | aynı | aynı |

- "Dosyasız hücre modellenmedi. ACIL #15'e göre K0'da pacer 293–419 B/s, PTT 640 B/s istiyor; oran ≈ 0,46–0,65 [ölçüldü + türetilmiş]. Bu yüzden K0r (iii)'ü sağlamaz [çıkarım]."
- "**Kurala göre model tahmini [tahmin]:** uygun kollar K2, K2−c, K2−e. K1 PTT şartında kalıyor (2,71 > 2,34). En düşük PTT K2−e (1,24). K2−c (1,30) %10 içinde, bayrak sayısı aynı, dosya bitişi daha kısa. **Seçim K2−c: (d) + DrainRtprop + (e-dar) + retx_first, (c) olmadan.**"
- "Manşette R1'i yalnız (c) içeren kollar geçiyor; K2−c manşette K0 düzeyinde (0,44). Yani gösterim seti büyük olasılıkla manşet hız kusurunu çözmeyecek. Gösterim profili manşet değil; hız kusuru Aralık'ta CC-v2 ile kapanır."
- "Model N = 10'da R1'i geçme olasılığı K1 0,46, K2 0,40 (modelin tuzak sıklığı F0'ın ≈ 3 katı); tuzak sıklığı F0 düzeyine çekilince 0,74–0,80 [tahmin, denetim-uygulama]."

## 6. Kod ↔ model farkları (beyan; okumayı değiştirmez)

- (d) mandalı kodda KK-M2-04 lafzıyla kurulur: `schedule_transmit` başında `tx_queue` boş ∧ `pacer.tokens() == pacer.burst_cap()`. Model aynı mandalı yalnız PC/Idle dışında ve bekleyen retx yokken kurar. Kod PC/Idle farkını Startup (yeniden) girişinde mandalı sıfırlayarak kapatır (KK-M2-27); bekleyen-retx farkı kalır (KK-M2-28).
- (c) kodda `PacingPhase::Startup`'a bağlıdır (Startup ve Startup tabanlı RecoverArq/RecoverFwd overlay'i); model RecoverArq overlay'inde aynı; RecoverFwd/emergency modelde yok (`kama_model.py:18` "Modellenmeyenler").
