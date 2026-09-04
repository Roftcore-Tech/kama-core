# FEC-MARGINAL-R-V1 — ÖN-KAYIT (dondurulmuş)

**Bu belge koşudan ÖNCE yazıldı ve kendi commit'inde durur.** Squash-merge altında gövde-içi hash
tek kalıcı kanıttır; freeze kaydı aşağıda.

---

## 0. Neden bu kampanya var

Bu repodaki her FEC sonucu ya resmen geri çekildi ya hüküm taşımadı. Sebep her seferinde koşudan
önce mekanik olarak yakalanabilecek bir şeydi. Bu oturumda yapılan adli inceleme (74 ajan, birincil
kaynak doğrulaması) 25 tasarım saldırısı üretti; ikisi kill-shot'tı ve **koşmadan önce** düzeltildi:

0. **(Kapı G6, koşu öncesi yakalandı)** `--fec-mode` eklendikten SONRA bile r=0 kolu koşmuyordu:
   `validate_fec` `manual_r == 0`'ı reddediyordu ve kolun HER rep'i başarısız oluyordu — üstelik
   analizcinin ilk sürümü kolu çıktıdan SESSİZCE düşürüyordu. "Bayrak temsil edilebilirliği ≠ kol
   çalışırlığı." Düzeltildi; G6 pilotu artık kolun koştuğunu VE farklılaştığını doğruluyor
   (r=0 → `fec_repairs_tx = 0`, r=2 → `120`). Analizci artık keşfedilen her kolu, verisi sıfır
   olsa bile raporluyor.
1. **Statik r=0 kontrol kolu TEMSİL EDİLEMİYORDU.** `--fec-r 0` adaptif demekti; `--fec-profile 0`
   ise fragmenter'ı kapatıyor (`unit = payload`), yani kontrol kolu 256 B birim, FEC kolları 53 B
   chunk kullanıyordu → **birim granülü ikinci eksen olarak kayıyordu**. Düzeltildi: `--fec-mode`
   seçicisi `r`'den ortogonal; `StaticR { r: 0 }` fragmentasyonu KORUR.
2. **%50 kayıp kodun MDS fizibilite sınırının ALTINDA.** K=8'de blok ancak ≤r silme ile çözülür:
   %50'de r=2 blokların **%5.5**'ini, r=4 **%19.4**'ünü çözer. Bu repodaki tüm %50-kayıp FEC
   ölçümleri kodun çözemeyeceği bölgedeydi. Bu kampanya **%20**'de koşar (r=2 → %67.8, r=4 → %92.7).

Ayrıca bu oturumda düzeltilen ve kampanyayı ilk kez anlamlı kılan kablolama borçları: bench kolu
artık ÜRÜN runtime'ını sürüyor (eskiden kripto-suz stub FEC basamıyordu), fragmenter var (eskiden
53 B tavan), alıcı FEC sayaçları yazılıyor (eskiden `s.err` her rep 0 byte), kuruluş görünür
(eskiden çöküş ile FEC sonucu ayırt edilemiyordu).

---

## 1. Soru (tek cümle, dar)

**Ürünün mühürlü, fragmentli wire yolunda statik proaktif redundancy `r`'nin MARJİNAL faydası
nedir, ve loss YAPISINA göre değişiyor mu?**

ARQ **her kolda AÇIKTIR**. "ARQ vs FEC" çerçevesi bu projede yasaktır; tek geçerli eksen
marjinal-r'dir. Bu bir **codec/wire** sonucudur, **politika** sonucu DEĞİLDİR: kollar
`close_fec_block_manual` ile sabit r basar, yapı-gate'i ve fuzzy kontrolcüyü BYPASS eder.
Sevk edilen politika hakkında bu kampanyadan hüküm çıkarılamaz.

---

## 2. Kollar (tek eksen: `r`)

| kol | `--fec-mode` | `--fec-r` | profil |
|---|---|---|---|
| **r0** (kontrol) | static | 0 | 0x03 |
| **r2** | static | 2 | 0x03 |
| **r4** | static | 4 | 0x03 |

Üç kol da profil 0x03 kurulu → `max_unit_bytes = 53`, `max_message_bytes = 256`, wire çerçevesi
`0x05 DATA-FRAG` + `SourcePrefix`. **DATA çerçeveleme byte-özdeş**; tek wire farkı parity
çerçeveleridir. Diğer tüm config alanları özdeş (ön-uçuş G1 bunu doğrular).

Cold-start: sevk konfigürasyonu (ilk-uçuş penceresi default AÇIK, bootstrap KAPALI, warmup 0).
Üç kolda da AYNI.

---

## 3. Hücreler (yapı ekseni MARJİNAL ORAN EŞLENEREK)

| hücre | fwd qdisc | marjinal kayıp | yapı |
|---|---|---|---|
| **iid20** | `delay 100ms loss 20% rate 1mbit` | %20 | bağımsız |
| **cor20** | `delay 100ms loss 20% 25% rate 1mbit` | %20 | korelasyonlu |

netem'in `loss p% c%` biçimi **korelasyonlu Bernoulli**'dir (Gilbert-Elliott DEĞİL) ve marjinal
oranı `p`'de tutar → yapı ekseni oran sabitken değişir. Bu, R2A'nın oran/yapı confound'unu
yapısal olarak engeller.

Reverse: `delay 100ms loss 2% rate 1mbit`. RTT = 2×100 = **200 ms** (ölçülecek, varsayılmayacak).
**Bant darboğazı ZORUNLU** (`rate 1mbit`): darboğaz yoksa parity byte harcar ama ZAMAN harcamaz,
bu da büyük `r`'ye yapısal kayırmadır.

---

## 4. Yük ve zamanlama

- `msgs = 96`, `payload = 256 B` → 5 chunk/mesaj → **480 birim = 60 × K(8), artık 0**.
  (Kısa blok FLUSH EDİLMİYOR; artık birim olsaydı son mesaj parity ALMAZDI.)
- `submit_interval = 20 ms` → sunum aralığı 1.92 s.
- `d_run_grace = 20 s` → koşu penceresi ≈ 22 s/rep.

---

## 5. Deadline: ANALİZ ekseni, koşu ekseni DEĞİL

Mesaj-başı gecikmeler `out.jsonl`'de ham durur; completion-vs-deadline eğrisi **tek koşudan
post-hoc** çıkar. Üç ad ayrıdır ve çıplak "deadline" kelimesi YASAKTIR:

- `d_msg_ms` — skorlanan mesaj-başı deadline (**hükmün referans aldığı TEK şey**)
- `d_run_grace_ms = 20 000` — koşu truncation payı
- `t_run_window_ms = 20×95 + 20 000 = 21 900`

Skorlama ızgarası (RTT katı, kanal-göreli): `d_msg ∈ {1, 2, 4, 8, 16} × ölçülen RTT`.
**BİRİNCİL `d_msg` = 4 × RTT ≈ 800 ms.** Diğerleri tanımlayıcı.
Zorunlu assertion: `max(d_msg) ≤ d_run_grace`.

---

## 6. Birincil uç nokta

**ALICI-taraflı teslim** (`s.err` → `ServerStats.delivered`), `d_msg` eşiğinde.
İstemci ACK sayısı İKİNCİLDİR: fragmentasyon altında istemci, beş chunk'ın ACK'ini görünce mesajı
teslim sayar; alıcı ise 64-mesaj reassembly cap'ini aşınca en eskiyi düşürür — sapma kola bağlıdır.

İkincil uç noktalar: `fec_recovered_units`, `fec_repairs_rx`, gerçekleşen `udp_tx_bytes`,
teslim-başına byte, çöküş oranı.

---

## 7. Rep geçerliliği (fail-closed; geçersiz rep VERİ NOKTASI DEĞİLDİR)

Bir rep ancak hepsi sağlanırsa sayılır:

1. `rep_status == COMPLETED` (timeout/sunucu-ölümü değil)
2. `left_preconverge == true` — **oturum kuruldu**. Kurulamamış rep bir FEC sonucu değildir.
3. `msg_asm_evicted == 0` ve `frag_chunk_dropped == 0`
4. `stale_decisions` kaydedilir; sıfırdan farklıysa raporlanır
5. `r > 0` kollarında ek olarak: `fec_repairs_tx > 0` **ve** `fec_repair_missing == 0`
   **ve** `fec_repairs_rx > 0` — *(`tx.fec > 0` VAKUM sayaçtır: repair-store miss'te boş symbol
   gövdeli çerçeve basılır, byte harcar, hiçbir şey kurtarmaz)*
6. `r == 0` kolunda: `fec_repairs_tx == 0` (kol farklılaşması)

Geçersiz rep sayısı ve sebebi kol başına RAPORLANIR, sessizce düşürülmez.

---

## 8. Tahmin edici ve hüküm kuralı (dondurulmuş)

Birim = **REP** (mesajlar rep içinde masif korelasyonlu; çöküş 96'yı birden götürür).
Kollar netem'in kernel-rastgeleliği altında **eşleştirilmemiştir** → eşli test YOK.

İki parçalı, tek havuzlanmış ortalama ASLA:

- **(a) Çöküş oranı**: `completion_at_primary_d_msg < 0.5` → çöküş. Kol başına Bernoulli,
  **Clopper-Pearson** kesin %95 aralık.
- **(b) Koşullu completion**: çökmeyen repler arasında, rep-düzeyi **bootstrap** (B = 10 000,
  seed = `SHA256("FEC-MARGINAL-R-V1" ‖ cell ‖ arm)`), %95 CI. Hücreler arası **Holm** düzeltmesi.

**HÜKÜM KURALI (koşudan önce donmuş):**
- **MARGINAL-R-FAYDALI(hücre)** ⟺ r2 veya r4'ün koşullu completion'ı r0'a göre CI-ayrık ARTIŞ
  gösterir **ve** çöküş oranı CI-ayrık ARTMAZ.
- **MARGINAL-R-ETKİSİZ(hücre)** ⟺ CI'lar örtüşür.
- **MARGINAL-R-ZARARLI(hücre)** ⟺ CI-ayrık AZALIŞ (completion ↓ veya çöküş ↑).
- Fayda ilan edilse bile **teslim-başına byte** birlikte raporlanır; maliyet gizlenmez.

**Güç:** kalibrasyon geçişinden (aşağıda) rep-düzeyi sd ölçülür ve hücre başına
%80-güç MDE ön-kayda BİRİNCİL SATIR olarak yazılır: *"bu hücre ancak ≥X pp etkiyi görebilir."*
MDE, hareket edilebilir etkiden büyükse hücre TANIMLAYICI ilan edilir.

---

## 9. n, sıra ve durdurma

- **Kalibrasyon geçişi**: hücre başına 3 rep, **ATILIR**, yalnız sd ve rep süresi ölçümü için.
- **Kalibrasyon ÖLÇÜMÜ (yapıldı, veri atıldı):** rep süresi ≈ **2.8 s** (istemci tüm mesajlar
  teslim olunca erken çıkar; çöken rep tam ufku kullanır → süre sonuç-bağımlıdır, bu yüzden
  durdurma kuralı SABİT REP SAYISIDIR, duvar-saati değil).
- **n = 40 / (kol × hücre)** → 3 × 2 × 40 = **240 rep**; muhafazakâr 5 s/rep ile ≈ **20 dk**.
- **Sıra**: rep indeksine göre ARALIKLI (rep1: iid20{r0,r2,r4}, cor20{r0,r2,r4}; sonra rep2 …).
  Herhangi bir noktada kesilme DENGELİ tasarım bırakır.
- **Durdurma**: sabit rep sayısı. Program tamamlanamazsa kampanya **HÜKÜM VERMEZ** ve kısmi veri
  `INCOMPLETE` etiketiyle yayımlanır. Duvar-saati bütçesi durdurma kuralı DEĞİLDİR.
- **Dışsal geçersizleme**: yalnız makine/harness arızası (ön-uçuş G5 tekrar koşturulur).
  Sonuç-güdümlü geçersizleme YASAK.

---

## 10. Freeze kaydı

- ön-uçuş çıktısı: `preflight.txt`, `sha256 = 98d36169571365376d3cbc1421887b32f97306c0ab6d945e6ea941484ae34bae`
- koşucu: `scripts/netem/fec_marginal_r.sh`
- analiz: `scripts/netem/fec_marginal_r_analyze.py`
- binary: `target/release/bench`, kaynak commit bu commit'in ebeveyni
- Bu belge, ön-uçuş çıktısı, koşucu ve analiz betiği **TEK commit'te** ve **ilk rep'ten ÖNCE** iner.

## 11. Bu kampanyanın CEVAPLAMADIĞI şeyler

- Sevk edilen FEC POLİTİKASI hakkında hiçbir şey (kollar gate'i bypass eder).
- quinn'e karşı üstünlük (quinn kolu YOK).
- %20 dışındaki kayıp oranları, 200 ms dışındaki RTT, 1 mbit dışındaki bant.
- Blackout/kesinti rejimi.
