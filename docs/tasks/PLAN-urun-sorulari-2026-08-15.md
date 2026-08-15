# PLAN — ürün sorularına dönüş

**Amaç:** ölçüm altyapısını mükemmelleştirmeyi bırakıp, transport hakkındaki gerçek soruları
ölçmek. Kartlar küçük, tek çıktılı ve bağımsız doğrulanabilir olacak.

---

## 0. Neyin elimizde olduğu — doğrulanmış envanter

Aşağıdakiler bu planın temeli. **Yıldızlı olanlar bu turda dosya düzeyinde doğrulandı**; kalanlar
ilk kartta doğrulanacak, varsayım olarak kullanılmayacak.

| varlık | durum |
|---|---|
| `tail_recovery.rs` ürün mekanizması ★ | var (20 325 B) |
| `dr-arq-product-tail-1` sonucu ★ | `result.csv` **2291 satır**, CELL/PAIRWISE/FINAL-REPORT var |
| netem script'leri ★ | `dr_arqtail_{preflight,manifest,run,launch,analyze}` var |
| `dr_arqtail_product` runner ★ | **var ve TRACKED** — `crates/adaptive-transport/examples/`, 20 080 B |
| örnek runner envanteri ★ | 12 tracked; untracked tek örnek `fs_cal2_runner.rs` (kullanıcı M5 WIP'i, doğru) |
| FS-1 v1.3.2 datası ★ | var |
| DR-ARQ-TAIL-1 R1/R2A raporları ★ | var |
| `fusion.rs` + `fusion_policy.rs` ★ | **var — kod yazılmış, ölçülmemiş** |
| DR-CS-3 bootstrap | default-OFF, panel 2/3 FAIL |
| structural oracle | CLOSED |
| 32×18 komparatörü | **kayıp** |

### Bu envanterden çıkan dersler

1. **Kayıp tek: `neut_compare.py`.** İlk taslakta `dr_arqtail_product`'ı da kayıp saymıştım —
   **yanlıştı**, yalnız `kama-integration/examples/` altına bakmıştım, runner
   `adaptive-transport/examples/` altında ve tracked. "İki kayıp, aynı desen" iddiası geri
   çekildi; tek veri noktasından desen çıkarılmıştı.
   Kural yine de geçerli ve ucuz: **bir sonucu üreten her araç, sonuçla aynı commit'te tracked.**
   Envanter bunun zaten sağlandığını gösteriyor — 12 runner'ın 12'si tracked.
2. **`dr-arq-product-tail-1` yeniden üretilebilir.** Runner + 5 netem script + donmuş manifest
   duruyor, yani B fazı yeni bir baseline koşusu gerektirmiyor.
3. **Füzyon kodu var ama ölçüm yok.** En yüksek bilgi getirisi olan soru, en az iş gerektiren
   yerde duruyor olabilir — C1 bunu tespit edecek.

---

## 1. Gerçek ürün soruları

Sıra, **bilgi getirisi ÷ maliyet** oranına göre:

| # | soru | durum | neden bu sırada |
|---|---|---|---|
| S1 | Tail probe default-ON yapılabilir mi? | mekanizma GO, tuning açık | Kanıtın %90'ı hazır; eksik olan tek bir tuning kararı |
| S2 | FEC+ARQ füzyonu ne kazandırıyor? | kod var, ölçüm yok | Hiç ölçülmemiş; kod hazır olduğu için maliyeti düşük |
| S3 | Cold start çözüldü mü? | kısmi | Gerçek Noise handshake Sprint-6'da mühürlü — **bu tur dışında** |

**S3 bu planın kapsamında değil.** Bootstrap RTT'nin neutrality zinciri (N0/N1/N2/D1/G0/V1) askıya
alındı; ancak S3'e dönüldüğünde ve default-ON kararı verilirken gerekli olacak.

---

## 2. Kart tasarım kuralları

Önceki turların başarısızlık biçimlerinden türetildi:

* bir kart = **bir çıktı** + **bir commit**;
* her kart 20–40 dk, aşarsa bölünecek;
* **koşu maliyeti kart yazılırken ölçülecek** — 12 dakikalık bir testi 20 kez koşturan kart yazılmaz;
* sonucu üreten araç, sonuçla **aynı commit'te tracked**;
* BLOCKED bir başarısızlık değil; kapsamı genişletmek başarısızlıktır;
* bağımsız denetçi yalnız **ürün iddiası taşıyan** kartlarda — envanter/hazırlık kartlarında değil.

---

## 3. Kartlar

### FAZ A — İPTAL (envanterle zaten kapandı)

A1 ve A2 gereksiz. Runner tracked, 12 runner'ın 12'si tracked, tek untracked örnek kullanıcının M5
WIP'i. **Doğrudan FAZ B'den başlanacak** — bu, planı ~1 saat kısaltır.

### FAZ B — S1: tail probe default-ON kararı (~3–4 saat)

**B1 — mevcut kanıtı oku, eksiği adlandır.** `FINAL-REPORT.md` + `CELL-RESULTS.csv`'den default-ON'u
bloklayan tam listeyi çıkar. Bilinen iki madde: L10-med 0/30 (cap-3 tükeniyor), %20–40 kayıpta
cold-start kırılganlığı. *Çıktı:* blocker listesi, ölçülmüş. *Denetçi:* hayır.

**B2 — cap/backoff politikasını tasarla ve dondur.** Tail uzunluğuna uyarlanan cap. Veri görülmeden
dondurulacak. *Çıktı:* donmuş politika + beklenen etki. *Denetçi:* evet.

**B3 — politikayı uygula.** Yalnız `tail_recovery.rs`; `sender.rs` girdi-eşleme dışında
değişmeyecek (§2 conformance korunacak). *Çıktı:* tek commit + hedefli testler.

**B4 — aynı grid'i yeniden koş.** Donmuş manifest, aynı hücreler. *Çıktı:* yeni `result.csv`
+ runner aynı commit'te.

**B5 — default-ON kararı.** B1'in blocker listesi kapandı mı? *Çıktı:* GO / NO-GO, gerekçeli.
*Denetçi:* evet.

### FAZ C — S2: füzyon ölçümü (~3–4 saat)

**C1 — füzyon kodunun kapsamını çıkar.** `fusion.rs` + `fusion_policy.rs` ne yapıyor, hangi
kararları veriyor, hangi girdileri okuyor, ürün yolunda çağrılıyor mu? *Çıktı:* kaynak-soyağaçlı
kapsam raporu. *Denetçi:* hayır.

**C2 — ölçülebilir mi, karar ver.** Mevcut netem harness'ı füzyonu tetikleyebiliyor mu, yoksa yeni
bir sürücü mü gerek? *Çıktı:* ölçülebilirlik hükmü + gereken en küçük değişiklik.

**C3 — deney tasarımını dondur.** Kollar (FEC-yok / FEC-tek / ARQ-tek / füzyon), hücreler,
metrikler, karar kuralı. **Veri görülmeden.** *Çıktı:* donmuş prereg. *Denetçi:* evet.

**C4 — koş.** *Çıktı:* dataset + runner aynı commit'te.

**C5 — analiz.** Donmuş karar kuralını uygula. *Çıktı:* füzyon hükmü. *Denetçi:* evet.

---

## 4. Bu planın kapsamadıkları

Açıkça dışarıda, sessizce düşürülmüş değil:

* **DR-CS-3 cold-start neutrality zinciri** (N0/N1/N2/D1/G0/V1) — askıda. `D0A` kararları geçerli
  kalır; bootstrap default-ON gündeme geldiğinde canlanır.
* **`gate.sh` self-dirty düzeltmesi (G0)** — B/C fazlarını bloklamıyor, çünkü o fazlar hedefli test
  koşuyor, tam gate değil. Entegrasyon öncesi gerekli.
* **Repo-wide format temizliği** — aynı gerekçe.
* **`first_probe_tick` onarımı** — diagnostic-deferred, hiçbir efficacy yolu tüketmiyor.
* **Cold start (S3)** — gerçek Noise handshake Sprint-6'da.

---

## 5. Bu planın düşme koşulları

* C1 füzyonun ürün yolunda çağrılmadığını gösterirse: C fazı ölçüm değil **entegrasyon** işine döner
  ve yeniden planlanır.
* B4'ün grid'i, `dr_arqtail_run.sh`'ın `unshare -r -n -m` yolunu kullanıyor. Bu ortamda rootless
  netns/netem'in hâlâ çalıştığı B1'de doğrulanacak — script kendi `setup_ns` doğrulamasını taşıyor
  (10 deneme, fail-closed), yani sessizce yanlış ölçmez, ama çalışmazsa B fazı durur.

### Kendi hata payım hakkında

İlk taslakta runner'ı kayıp saydım çünkü tek bir crate'e baktım. Envanteri **yazmadan önce**
doğrulamalıydım. Bu planın §0'ındaki yıldızlı satırlar tam da bunun için var: her temel iddia
dosya düzeyinde ölçülecek, hafızadan yazılmayacak.
* Herhangi bir kart iki kez BLOCKED dönerse: o soru askıya alınır, diğerine geçilir.
