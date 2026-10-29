# K4G-05b rev6: yerel kapı ve netem 7 profil kanıt paketi

Dayanak: CEVAP PAKETİ 16 v2 §1.3 (2026-10-26). Bu dizin #140'ın netem ve yerel kapı kanıtıdır. Sonuçlar orkestratörün makinesinde üretildi.

Etiketler:
- [ölçüldü]: koşu çıktısı.
- [kod]: kaynak okuması.
- [betimsel, sonradan]: ön-kayıt dışı; sonuçlar görüldükten sonra eklenen analiz.
- [çıkarım]: ölçülmemiş akıl yürütme.

## 0. Hüküm

**Son aday `49864510` (oturum 2) için ön-kayıtlı hüküm DUR'dur** [ölçüldü].

- p2_loss80'de KAYMA-M1 var, yönü artış.
- Diğer altı profilde ön-kayıtlı kuralla kayma saptanmadı.
- Ön-kayıt §2: kayma iki yönlü sayılır → DUR, sahibe rapor, merge yok. Sessiz yeniden koşu yok.
- "yedi profilde ön-kayıtlı kayma saptanmadı" cümlesi **bu aday için yazılamaz**.

Oturum 1 (`a8c4e8c6`, DUR-45 (C) öncesi aday) yedi profilde kayma vermemişti. O sonuç son aday için geçerli değildir. §4'te yalnız bağlam olarak duruyor.

## 1. Kimlik

| | taban | oturum 1 adayı | oturum 2 adayı (son aday) |
|---|---|---|---|
| kaynak | `origin/main` `1b1425d2` | `k4g/05b-retx-fix` `a8c4e8c6` | `k4g/05b-retx-fix` `49864510` |
| ikili sha256 | `14b5f7d3b8d6f62193ea0b8d22f9a00737a33bbf0a45db8d38a12910845e0dfc` (iki oturumda aynı) | `b642a9ec…` (ikili üzerine yazıldı; yalnız kayıt) | `fd3089b14dc2fe54b8c7ba27a3cc783e71bdf088b8f164a56989c57cbe79dd30` |
| koşu | — | 2026-10-26 01:49:23Z – 02:56:06Z | 2026-10-28 23:15:46Z – 2026-10-29 00:22:29Z |
| çekirdek | — | 7.2.8-8-omarchy-bore | 7.2.8-9-omarchy (arada yeniden başlatma) |

Ortak derleme ayarları:
- Derleme: `cargo build --release --locked -p bench --bin bench`. Her kol kendi worktree'sinde ve ayrı target'ta derlendi.
- Toolchain: 1.96.0 (`rust-toolchain.toml`).
- Ürün kolu: `--transport kama-product`.
- Manifest kimliği: taban `kama-v1-full`, aday `kama-v1.1-full`. 05b'nin bilinçli kimlik değişikliği; `profile_delta = []` 280/280 koşuda.

Kod farkı [kod]:
- `a8c4e8c6..49864510` arasında bench'in 8 yerel crate'inde test dışı tek davranış farkı DUR-45 (C)'dir. Bu, `adopt_due_ns`'ten `|| self.probe_scope_active()` kapısının kaldırılması.
- Bunun dışında yalnız salt gözlem tanı alanları değişti.

## 2. Ön-kayıt

- `ONKAYIT.md` tam metindir ve değiştirilmedi; sha256 `6d667853281129875dcd5e1c46c2d632d36696a8074cd491d79e754e2672ef24`.
- Dondurma 2026-10-25T20:01:12Z'de yapıldı (`ONKAYIT.sha256.kayit`).
- İki oturumda da koşu anında alınan hash'ler (`*/onkayit-hash-kosu-ani.txt`) dondurma kaydıyla birebir aynı.
- Sürücüler `arac/netem7-kos.sh` (oturum 1) ve `arac/son-aday-49864510.sh` (oturum 2). İkisinin netem bölümü SHA dışında aynı: shard sırası, ≤ 3 shard, timeout ve yollar.
- Oturum 2 sürücüsünde iki ek var:
  - netem'den önce, sırayla koşan tam yerel kapı;
  - ANALIZ'den sonra çağrılan betimsel `netem7_ci.py`. Bu betik ön-kayıt hash kümesinde yok ve hükmü değiştirmez.

## 3. Karar kuralı (tek kural; ön-kayıt §2'den aynen)

- KAYMA(M1) ⇔ |medyan(aday) − medyan(taban)| ≥ 0,05 **ve** p < α. M1 = koşu başına D = 20 s içinde teslim oranı.
- KAYMA(M2) ⇔ medyan oranı ∉ [0,8; 1,25] **ve** p < α. M2 = koşu başına, teslim edilenlerin gecikme medyanı.
- Test kesin, iki yanlı ve bağ-düzeltmeli Mann–Whitney; α = 0,05/7.
- Yön iki yönlü sayılır.
- Herhangi bir KAYMA, EKSİK ya da ALTYAPI-DUR → DUR.

Kural "M1 veya M2"dir.

## 4. Sonuç [ölçüldü]

### 4.1 Oturum 2, son aday `49864510`

Altyapı arızası 0. Her kolda N = 20 ve 280/280 koşuda `rc = 0`. Kaynaklar: `oturum-2-49864510/ANALIZ.txt` ve `kosular.csv`.

| profil | taban M1 medyan | aday M1 medyan | p (M1) | M2 oranı | p (M2) | hüküm |
|---|---|---|---|---|---|---|
| p1_loss50 | 1,000 | 1,000 | 0,0083 | 1,373 | 0,48 | kayma yok |
| **p2_loss80** | **0,510** | **0,915** | **0,005282** | 0,580 | 0,51 | **KAYMA-M1 (artış)** |
| p3_loss95 | 0,000 | 0,000 | 0,106 | 0,988 | 0,86 | kayma yok |
| p4_jitter | 1,000 | 1,000 | 1 | 0,994 | 0,62 | kayma yok |
| p5_revpath | 1,000 | 1,000 | 0,49 | 1,576 | 0,049 | kayma yok |
| p6_blackout | 0,890 | 0,900 | 0,0012 | 0,965 | 2,9·10⁻¹¹ | kayma yok (p < α ama etki eşiği sağlanmıyor) |
| p7_bbr | 1,000 | 1,000 | 1 | 0,989 | 0,76 | kayma yok |

p2'nin p-değeri kesin olarak 121339723/22974421470 ≈ 0,005282 < α ≈ 0,007143.

Bu sayı üç bağımsız uygulamada birebir aynı çıktı:
- ön-kayıtlı betik;
- `sorgu/betikler/yeniden-hesap__recompute.py`;
- `sorgu/betikler/elestirmen__teyit.py`.

### 4.2 Oturum 1, `a8c4e8c6` (bağlam; son aday için geçerli değil)

Yedi profilde kayma yok. p2: taban 0,150, aday 0,875, p = 0,3492. Kaynak: `oturum-1-a8c4e8c6/ANALIZ.txt`.

## 5. Sorgu: enstrüman → türetim → kod [betimsel, sonradan]

Ön-kayıt §3 sorgu sırasını önceden yazmıştı: önce enstrüman, sonra türetim, sonra kod. Sorgu salt-okunur yürütüldü:
- yeni koşu yok;
- mevcut ham veri ve kod okuması;
- dört bağımsız şerit ve bir eleştirmen.

Ham çıktı `sorgu/sorgu-wf_e5bb56d8-sonuc.json`'da, betikler `sorgu/betikler/`'de. Bu bölüm **hükmü değiştirmez**.

### 5.1 Enstrüman

Denetlenen kalemlerde arıza bulunmadı [ölçüldü]:
- ikili ve araç hash'leri;
- realized qdisc (yalnız çekirdek tohumu farklı);
- kol sırası dönüşümü;
- shard eşzamanlılığı;
- `c.err` teslim sayısı ile `out.jsonl` arasındaki eşleşme (280/280);
- gate ile netem'in çakışmaması.

Dışlanamayanlar:
- Dosyaya yazmayan CPU yükü: masaüstü uygulamaları açıktı.
- **M1'in ölçüm noktası iki kolda kod düzeyinde aynı değil** [kod]. Bench'in `delivered_ns` değeri adayda `Sender::take_confirmed_units` üzerinden, tabanda runtime'ın seq→birim haritasından geliyor. Bunun M1'e katkısı ölçülmedi.
- Kısmi karşı denetim: istemci teyit yolundan geçmeyen ve D ile sınırlanmamış sunucu teslim sayısı (`s.err`) da aynı yönde. Taban medyanı 57, aday 94,5 (MWU p = 0,0032).

### 5.2 Hükmün kırılganlığı

Hüküm α sınırına yakın: p/α ≈ 0,74.
- Taban rep9 ya da rep12'de bir mesaj daha D içinde teslim edilseydi p = 0,00722 > α olurdu ve hüküm dönerdi.
- Hodges–Lehmann kestirimi +0,265. %95 aralığı [0,020; 0,540], 1 − 0,05/7 aralığı [0,000; 0,860]. Aralıklar bağsız dağılımla hesaplandığı için yaklaşıktır.

### 5.3 Dağılımın şekli

M1 dört dağılımda da (2 oturum × 2 kol) iki kipli.

| sınıf (bu oturum) | taban | aday |
|---|---|---|
| koşu sonunda PreConverge'de (rtt_samples = 0) | 7/20 | 3/20 |
| ara bant, 0,1 ≤ M1 < 0,8 | 7/20 | 1/20 |
| M1 ≥ 0,8 | 6/20 | 16/20 |

- PreConverge farkı oturumlar arası oynama içinde. Aynı taban ikilisinde bu sayı 8 → 7 idi.
- Ara bant iki oturumda da neredeyse yalnız tabanda görülüyor.

**Aynı taban ikilisi** iki oturumda p2 M1 medyanı 0,150 ve 0,510 verdi (MWU p = 0,41). Yani p2 medyanı oturumdan oturuma etki büyüklüğü mertebesinde oynuyor.

### 5.4 Ön-kayıt gerekçesi eksikti

Bu, orkestratörün kendi tahminiydi. §3'teki gerekçe şuydu: "05b yalnız token yetmezliği ya da ≥ RTO_MAX bekleyen kayıt varken devreye girer." Bu gerekçe 05b'nin her ACK'te çalışan değişikliklerini hesaba katmadı [kod]:
- madde 7 RTT kabul kuralı;
- madde 2 teyit kümesi;
- madde 3 ARQ tavanı devri;
- madde 4(b) prob bütçesi devri;
- madde 5 kopya drenajı.

"Sapma beklenen yer" olarak gösterilen p6'daki DUR-40, 25 s'lik koşuda tetiklenemez [kod].

Gerekçe eksikti; kural ve hüküm geçerlidir.

### 5.5 Mekanizma adayları

Hepsi [çıkarım]; tek değişkenli ablasyon yapılmadı.

**madde 7, RTT kabul kuralı.**
- Taban RTT örneğini yalnız Data kökenli kayıttan alıyor (`sender.rs:3090` @1b1425d2). Aday çerçevenin en büyük seq'inden alıyor (`:3449-3460` @49864510).
- Koşu sonu anlık görüntüsü (yörünge değil), PreConverge'den çıkan koşular:

  | | taban | aday |
  |---|---|---|
  | srtt_us medyanı (oturum 1 / 2) | 494 / 655 ms | — |
  | srtt_us aralığı | 19 ms – 9,95 s | 18,9–19,6 ms (bir istisna 77,7 ms) |
  | rtt_samples | 2–8 | 9–22 |

- Tabanın bir kısım koşusunda pace tam 4800 B/srtt tavanında.
- 05b tasarım notu bu taban davranışını B.2.0'da "srtt 60,5 s" kusuru olarak zaten kaydetmişti.

**Sahiplik değişiklikleri** (madde 2/3/4/5, PAY). Aynı yönde olabilirler; mevcut sayaçlar bunları madde 7'den ayıramıyor.

**DUR-45 (C)'nin p2'de etkisi beklenmez** [kod]. `adopt_due_ns` PreConverge/Idle'da kapalı ve H ≥ ≈ 14 s.

### 5.6 Maliyet gözlemleri

**Gönderim hacmi.** p2'de istemcinin gönderim hacmi (tx_total) taban medyanı 84,3 KB, aday 155,1 KB. Oturum 1'de 47,0 ve 164,0 KB. Aday kopya sayısı bu oturumda yaklaşık 14 kat: dups 251 / 17,5, kopya baytı 71,0 / 5,0 KB. Tel bütçesi açısından ölçülmüş bir maliyet; bu denetimin kuralına girmiyor.

**Gecikme (p1 ve p5).** M2 oranları 1,37 ve 1,58. Bant dışında ama p ≥ α.
- Aynı yön oturum 1'de de var (1,16 ve 1,08).
- Büyüklük, aynı taban ikilisinin oturumlar arası oynaması içinde kalıyor.
- Bu veriyle ne gecikme artışı ne de yokluğu gösterilebilir.

## 6. Tam yerel kapı

**Son aday `49864510`.** Kaynak: `oturum-2-49864510/kapi/gate-49864510-summary.txt`.
- 26 kalem, hepsi `exit=0`.
- Kalemler: `k4g-gate.sh --vectors --integration`; ek `cargo metadata --locked` ve `cargo check --workspace --all-targets --all-features --locked`; `systemd-analyze verify deploy/kama-hub.service`.
- test_fast 1683 / 0 / 18 ignored (main ile aynı 18); integration 386 / 0.
- Cargo.lock değişmedi, ağaç temiz, toolchain 1.96.0.

Önceki kapı `593029eb`: `oturum-1-a8c4e8c6/kapi/`.

Bu paket dala commit edilirse son SHA değişir. "Son aday SHA'da tam kapı" kuralı gereği kapı o SHA'da yeniden koşar.

## 7. Komutlar (yeniden üretim)

```
# ikililer (iki ayrı worktree)
cargo build --release --locked -p bench --bin bench
# koşu (unshare -rnm, rootless; sch_netem modülü yüklü olmalı)
bash arac/son-aday-49864510.sh   # kapı → aday derleme → netem7_cell.sh × 7 profil → analiz
# ön-kayıtlı analiz
python3 -I arac/netem7_analyze.py <koşu-dizini>
# betimsel aralıklar (sonradan)
python3 -I arac/netem7_ci.py <koşu-dizini> kosular.csv
```

Harness `scripts/netem/kapi3_harness.sh` taban worktree'sinden kullanıldı. sha256 ve SHA: `arac/kapi3_harness.sh.sha256`.

## 8. Sınırlar (beyan)

- "netem 7 profil" depoda koşulabilir bir kapı olarak tanımlı değildi. Buradaki tanım orkestratörün yorumudur: ADR-18 p1–p7 (`kapi_manifest.py:40-55` aynen), ürün kolu, taban ↔ aday.
- netem kaybı tohumsuz: çekirdek RNG'si hücre başına rastgele, bench `--seed rep`. Koşular oturumlar arasında eşlenemez.
- p6 geometrisi farklı: 1000 ms aralık (ön-kayıt §1).
- M2, hiç teslim olmayan koşuda tanımsız ve testten düşüyor. Örneklem p2'de 16 / 17, p3'te 2 / 5. Ön-kayıt bunu yazmıyordu; bu koşuda hükmü etkilemiyor.
- N = 20 < 30. Depo sözleşmesi (ADR-hostile-validation-contract) performans iddiasına izin vermez. Rakip kolu yok: bu bir regresyon denetimi, kıyas değil.

## 9. Dosyalar

| dosya | içerik |
|---|---|
| `ONKAYIT.md`, `ONKAYIT.sha256.kayit` | ön-kayıt + dondurma anı hash'leri |
| `arac/` | iki sürücü, hücre koşucusu, ön-kayıtlı analiz, betimsel aralık betiği, harness hash'i |
| `oturum-2-49864510/` | son aday: `ANALIZ.txt`, `kosular.csv`, `betimsel-ci.md`, `ikili-sha.txt`, `driver.log`, `onkayit-hash-kosu-ani.txt`, `build-aday.log`, `run-49864510.tar.xz` (+ `.sha256`), `kapi/` |
| `oturum-1-a8c4e8c6/` | önceki aday (bağlam): aynı yapı + `run-a8c4e8c6.tar.xz` (smoke dahil) |
| `sorgu/` | §5 sorgusunun yapılandırılmış çıktısı ve betikleri [betimsel, sonradan] |
