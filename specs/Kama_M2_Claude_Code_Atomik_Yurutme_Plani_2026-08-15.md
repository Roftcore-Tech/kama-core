# Kama Transport M2 — Claude Code için atomik yürütme ve denetim planı

**Sürüm:** 2026-08-15 rev.2  
**Saat dilimi:** Europe/Istanbul  
**Dal:** `transport-baseline-closure`  
**Sert kesim:** 2026-08-16 20:00  
**Belgenin amacı:** Kalan M2 işini tek ve şişkin bir istem olarak değil; küçük, bağımsız, ölçülebilir ve kullanıcı tarafından denetlenebilir görevler halinde yürütmek.

---

## 0. Bu belge nasıl kullanılacak?

Bu dosya **tek seferde icra edilecek bir mega-prompt değildir**. Claude Code bütün belgeyi bağlam olarak okuyacak, fakat her oturumda yalnızca bir `TASK-*` kartı çalıştıracaktır.

Her yeni görev için aşağıdaki kısa başlatma istemi kullanılmalıdır:

```text
@Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md dosyasını oku.
Yalnızca TASK-<ID> kartını uygula. Sonraki karta geçme.

Önce kartın başlangıç koşullarını doğrula. Bir koşul sağlanmıyorsa hiçbir dosyayı
değiştirmeden BLOCKED raporu ver. Kart bir yazma görevi ise sadece kartta izin verilen
dosyalara dokun. Kartın PASS ölçütlerini kendi yorumunla gevşetme.

İş sonunda belgedeki TASK_RESULT şemasını eksiksiz doldur. Test çıktısı, exit code,
süre, başlangıç/bitiş HEAD'i ve değişen dosyaları kanıt olarak göster. PASS diyebilmek
için bütün PASS maddeleri ölçülmüş olmalı. FAIL/BLOCKED halinde otomatik düzeltme turu
başlatma ve sonraki karta geçme.
```

### 0.1 Claude Code workflow kuralı

Claude Code workflow oluşturacaksa:

- **Bir workflow = bir görev kartı.** Bütün plan tek workflow'a çevrilmez.
- Bir workflow en fazla `Ground → Act → Verify` aşamalarını içerir.
- `Act` ve bağımsız `Verify` aynı model bağlamında yapılmaz.
- Ağır `cargo` komutları paralel ajanlara dağıtılmaz. En fazla **bir ağır Cargo süreci** çalışır.
- Salt-okunur kaynak taramaları, birbirinden gerçekten bağımsızsa en fazla iki alt ajana ayrılabilir.
- Alt ajanlar başka kartları başlatamaz, commit atamaz ve izinli dosya kapsamını genişletemez.
- Her workflow PASS, FAIL, BLOCKED veya DEFERRED ile biter; “kısmen tamamlandı, devam ediyorum” son durum değildir.

### 0.2 Kullanıcı kontrol noktaları

Aşağıdaki kartlardan sonra Claude otomatik ilerlemez; kullanıcı bir sonraki kartı ayrıca başlatır:

- `TASK-A1` — M5 WIP parkı
- `TASK-D2` — semantik deferral commit denetimi
- `TASK-E2` — format-only commit denetimi
- `TASK-V5` — final doğrulama ve HEAD freeze
- `TASK-S2` — 512 OFF-only seçim sonucu ve seed bandı
- `TASK-P4` — nihai efficacy hükmü

---

## 1. Mevcut bilimsel ve teknik durum

### 1.1 Bankalanmış kanıtlar

Şunlar yeniden keşfedilmeyecek; yalnız açık bir regresyon ölçülürse sorgulanacaktır:

- Structural oracle kapalıdır: commit'ler `8d0c2fee`, `74a6736c`, `4f11545c`.
- Gate'e canlı format kontrolü `7e1af66b` ile eklenmiştir.
- Channel-fate yorum ön-kaydı `d510070d` ile dondurulmuştur.
- Channel-event enstrümantasyonu `08fe39a0` üzerinde bulunmaktadır.
- Neutrality kanıtı bankadadır: baseline `d510070d` ile instrumented `08fe39a0` karşılaştırmasında:
  - 32 kayıt,
  - kayıt başına 18 donmuş alan,
  - 0 mismatch,
  - 0 eksik,
  - 0 fazla.
- `dr_cs3_v2_channel`: 45 PASS / 0 FAIL; ölçülen süre yaklaşık 723 saniyedir.
- Channel-fate mutation muhasebesi: 10/12 PASS, 2/12 DEFERRED.
- Structural oracle hücre 7'de parity yalnız girişte pending'dir; hedef turda actuate olmamıştır. Hücre 7, Probe-vs-DATA çekişme kanıtıdır; parity-vs-DATA kanıtı değildir.
- Cell 5 ürün invariant'ı: bootstrap probe kapısı `PreConverge && srtt.is_none()` iken, loss ilan eden ACK ilk RTT örneğini de oluşturur; recovery ile bootstrap probing birlikte oluşamaz.

### 1.2 Henüz kanıtlanmamış veya ertelenmiş olanlar

- **Channel-fate attribution kapalı değildir.** Doğru statü:

  ```text
  CHANNEL-FATE ATTRIBUTION: DEFERRED, NOT CLOSED
  ```

- Fate/cap metrikleri promotion threshold, Gate-5 hükmü veya cold-start efficacy ölçütü değildir.
- Silence dalı, her inflight DATA için `srtt` aralığında `DupRec` üretebildiğinden tick-bağımsız sonlu ürün bound'u yoktur. Doğru verdict:

  ```text
  UNBOUNDED-BY-PRODUCT — HARNESS-SPECIFIC BOUND REQUIRED
  ```

- Önceden tanımlı açık bir harness ceiling yoksa sonradan “yeterince büyük” cap icat edilmeyecektir.
- `EVENT-LOG-OVERFLOW` hiçbir durumda kendiliğinden `PRODUCT FINDING`, Gate-5 failure veya cold-start failure sayılmayacaktır.
- Cold-start problemi henüz “çözüldü” denebilecek efficacy verisine sahip değildir. Kalan amaç, seçilmiş stall fenotipinde bootstrap mekanizmasının kurtarma yapıp yapmadığını ölçmektir.

### 1.3 Authoritative efficacy yolunun dondurulmuş kapsamı

- Authoritative build: Cargo default feature set.
- `experimental-cold-start`: compile-time kapalı.
- Runtime treatment:
  - seçimde OFF,
  - paired panelde OFF ve ON.
- Aynı frozen binary iki arm için de kullanılır.
- Legacy cold-start ladder ve bootstrap probe OFF armında sıfır delta vermelidir.
- Authoritative 512 selection ve paired efficacy arm'larında channel logger **OFF** olacaktır.
- Efficacy; terminal completion, latency, byte/probe sayıları ve mevcut invariant'larla değerlendirilir. Fate/cap tüketilmez.

### 1.4 Ayrı ertelenmiş ürün kalemi

`tail-v2` bu teslimin parçası değildir. Doğru dış kayıt:

```text
Tail probe netem'de %2,2 → %91,4 olarak doğrulandı; ancak default hâlâ OFF ve
adaptive cap uygulanmadı. Bu çalışma shipped/default ürün davranışı değildir.
```

---

## 2. Claude Code verim protokolü

Bu protokol Anthropic'in resmî önerilerinin bu repoya uyarlanmış halidir.

### 2.1 Tek görev, temiz bağlam

- Her `TASK-*` yeni bir Claude Code oturumunda veya `/clear` sonrasında başlar.
- Önceki uzun sohbet yapıştırılmaz; bu dosya, görev ID'si, başlangıç commit'i ve gereken hata çıktısı referans verilir.
- Aynı konuda iki başarısız düzeltmeden sonra üçüncü kez aynı kirlenmiş bağlamda denenmez. Yeni oturum ve düzeltilmiş görev kartı gerekir.
- `CLAUDE.md` içine bu uzun plan taşınmaz. `CLAUDE.md` yalnız tüm oturumlara gerçekten uygulanacak kısa repo kurallarını taşımalıdır.

**Gerekçe:** Claude Code bağlamı doldukça performansın düştüğünü; alakasız işler arasında `/clear` kullanılmasını ve iki başarısız düzeltmeden sonra temiz bağlamla yeniden başlanmasını önerir.

### 2.2 Explore → Plan → Act → Verify ayrımı

- Kaynak belirsizse önce salt-okunur `Ground` kartı çalışır.
- Grounding çıktısı olmadan implementation başlamaz.
- Implementer yalnız onaylanmış küçük diff'i üretir.
- Verifier fresh context'te yalnız diff ve ölçütleri görür; implementation gerekçesini doğru kabul etmez.
- Verifier hata bulursa dosya düzenlemez; FAIL raporu verir. Onarım ayrı karttır.

### 2.3 Claude'ya çalıştırabileceği bir doğrulama ver

Her kart en az bir deterministik doğrulama sinyali taşır:

- exact test adı ve sonuç satırı,
- komut exit code'u,
- diff/path kümesi,
- hash eşitliği,
- kayıt/mismatch sayısı,
- ya da açık statik kaynak-soyağacı tablosu.

“Kod doğru görünüyor”, “muhtemelen”, “testler genel olarak yeşil” PASS değildir.

### 2.4 Writer / Reviewer ayrımı

- Yazarı aynı oturumda kendi işinin nihai denetçisi yapma.
- Reviewer yalnız stated requirements ve correctness kusurlarını blocker sayar; sırf “daha güzel olabilir” diye scope büyütmez.
- Reviewer'ın sonucu somut dosya/satır/komut kanıtı içermelidir.

### 2.5 Worktree ve kirli çalışma ağacı güvenliği

- Salt-okunur denetimler ve format reproducer'ı clean git worktree'de çalışır.
- Worktree izolasyonu **git paylaşımını ortadan kaldırmaz**; branch ve commit etkileri hâlâ gerçektir.
- Repo-wide `cargo fmt` kirli ana checkout'ta çalıştırılmaz.
- `git stash`, `git reset --hard`, broad `git add .`, untracked dosya silme ve otomatik worktree cleanup yasaktır.
- Worktree çıkarılmadan önce `git status --short`, commit listesi ve untracked dosyalar raporlanır.

### 2.6 Ağır komutları model muhakemesinden ayır

`dr_cs3_v2_channel` tek koşuda yaklaşık 12 dakika sürmektedir. Bu nedenle:

- Bir heavy-run kartı yalnız komutu başlatır, 5–10 dakikada bir process/status heartbeat'i verir ve çıktıyı dosyaya kaydeder.
- Test çalışırken yeni kod yazılmaz.
- Aynı `CARGO_TARGET_DIR` üzerinde iki Cargo süreci paralel koşmaz.
- Warm-cache koşu kabul ediliyorsa bu açıkça yazılır; fresh compile gereken kartta yeni `CARGO_TARGET_DIR` ve gerçek `Compiling ...` kanıtı aranır.
- Terminal/tool timeout, test failure değildir. Önce process'in hâlâ çalışıp çalışmadığı kontrol edilir.
- Runtime altyapı hatasında otomatik retry yoktur. Değişikliklerin ve process durumunun kurtarılabilirliği incelenir; aynı girdilerle en fazla bir kontrollü retry yapılır.
- İkinci altyapı failure'ı `BLOCKED-RUNTIME` olur.

### 2.7 Paralellik sınırı

Paralel yapılabilecekler:

- iki salt-okunur kaynak taraması,
- bağımsız diff review,
- log/hash analizi.

Paralel yapılmayacaklar:

- ağır Cargo testleri,
- aynı dosyayı değiştiren implementer'lar,
- format ile semantik değişiklik,
- selection ile ON panel,
- aynı kanıtı farklı mutation'larla birden çok out-of-tree crate'te yeniden derlemek.

### 2.8 Hooks ve izinler

Bu plan için yeni hook kurmak zorunlu değildir; deadline öncesinde yeni altyapı eklemek yeni risk yaratır. Kullanılıyorsa:

- Salt-okunur agent: `Read, Grep, Glob, Bash` ve write araçları yasak.
- Implementer: yalnız kartın izinli dosyalarına yazabilir.
- Stop/TaskCompleted hook'u ancak şu deterministik kontroller için kullanılabilir:
  - izinli dosya kümesi,
  - `TASK_RESULT` varlığı,
  - çalışma ağacı durumu.
- Hook hiçbir ağır test paketini her turn sonunda tekrar koşturmaz.
- `bypassPermissions` kullanılmaz.

### 2.9 Önerilen agent sınırları

Repo ayarlarına şimdi eklenmesi şart olmayan, oturum bazında uygulanacak sınırlar:

| Rol | Araçlar | `maxTurns` | Yazma | İşlev |
|---|---|---:|---|---|
| Explorer | Read/Grep/Glob/Bash | 12 | Hayır | Dar kaynak-soyağacı ve komut keşfi |
| Implementer | Kartın gerektirdiği araçlar | 20 | İzinli path'ler | Tek küçük diff ve targeted test |
| Verifier | Read/Grep/Glob/Bash | 12 | Hayır | Fresh-context adversarial denetim |
| Heavy runner | Bash | 8 | Yalnız log/artifact | Tek uzun komut, heartbeat, exit code |

### 2.10 Yapılandırılmış sonuç zorunluluğu

Her kart sonunda şu blok doldurulur:

```yaml
TASK_RESULT:
  task_id: TASK-X
  status: PASS | FAIL | BLOCKED | DEFERRED
  started_head: <sha>
  ended_head: <sha>
  worktree: <absolute path>
  allowed_paths: []
  changed_paths: []
  forbidden_path_changes: []
  commands:
    - command: <exact command>
      exit_code: <integer>
      duration_seconds: <integer>
      evidence: <exact result line or artifact path>
  assertions:
    - criterion: <PASS criterion>
      met: true | false
      evidence: <measured fact>
  artifacts:
    - path: <path>
      sha256: <hash>
  unresolved:
    - <known defect or none>
  next_unlocked_task: <TASK-ID or NONE>
```

Bir iddia, yukarıdaki blokta ölçülmüş kanıt taşımıyorsa bankaya girmez.

---

## 3. Durum makinesi ve global stop kuralları

### 3.1 Görev durumları

```text
PENDING → READY → RUNNING → PASS
                         ↘ FAIL
                         ↘ BLOCKED
                         ↘ DEFERRED
```

- Yalnız `PASS`, dependency zincirinde bir sonraki kartı açar.
- `DEFERRED`, görev kapsamının hüküm taşımadığını açıkça kaydeder; PASS yerine geçmez.
- `FAIL`, aynı oturumda scope genişletme yetkisi vermez.
- `BLOCKED`, kullanıcı kararı olmadan ilerlemez.

### 3.2 Global yasaklar

Her kartta geçerlidir:

- Seed search öncesinde selection sonucu üretme.
- Selection tamamlanıp listeler hash'lenmeden ON arm çalıştırma.
- Qualifying seed sayısını gördükten sonra karar bandını değiştirme.
- Seed atlama, değiştirme veya “daha iyi” seed arama.
- Event cap'i observed max + headroom ile seçme.
- Logger'ı authoritative OFF/ON arm'larında asimetrik kullanma.
- Fate metriclerini efficacy/promotion hükmüne sokma.
- Stale Hosted CI run'ını bu branch'in kanıtı sayma.
- Kirli worktree üzerinde repo-wide formatter çalıştırma.
- Kullanıcının M5 WIP'ini düzeltme, silme, formatlama veya M2 commit'ine karıştırma.
- Bir test failure'ını assertion gevşeterek veya hücre/seed adını değiştirerek yeşile çevirme.

### 3.3 Ön-beyanlı zaman-baskısı kırpması

**Ara kontrol:** 2026-08-16 13:00 Europe/Istanbul. Bu kontrol yalnız atomik görev sınırında değerlendirilir; çalışmakta olan bir shell komutu ortadan kesilmez.

Bu saate gelindiğinde `TASK-C0` henüz açılmamışsa veya başlamamışsa:

1. `TASK-V4` henüz koşmadıysa P4 sonrasına ertelenir.
2. `TASK-V5`, `TASK-V1` authoritative logger-OFF ekseni + `TASK-V2` + `TASK-V3` ve clean/frozen HEAD üzerinden binary freeze yapabilir.
3. Bu yolun exact statüsü:

   ```text
   FULL LOCAL 13-CHECK GATE: NOT RUN — PREREGISTERED TIME CUT
   ```

4. `PASS` veya “CI-green” denmez.
5. Gerekçe: V1–V3 + frozen HEAD/binary deney eşlemesinin geçerliliğini taşır; full fmt/clippy/config gate entegrasyon-hazırlığı sinyalidir.
6. Efficacy endpoint tamamlandıktan sonra — `n≥3` için P4, `n<3` için S2 — aynı frozen HEAD'de `TASK-V4` çalıştırılır. V4 FAIL olursa efficacy ölçümü geriye dönük olarak yok sayılmaz; fakat branch integration readiness `FAIL` olur ve ürün/PR hazır denmez.
7. Bu endpoint sonrasında V4'e zaman kalmazsa R0'da `NOT RUN` kalır.

Bu kırpma **sonuçtan bağımsızdır**: qualifying seed sayısına veya ON sonucuna bakılarak açılıp kapatılamaz.

### 3.4 Global deadline kuralı

2026-08-16 20:00 Europe/Istanbul geldiğinde:

1. Yeni görev başlatılmaz.
2. Çalışan atomik shell komutu güvenle bitirilebiliyorsa bitirilir; yeni repair loop açılmaz.
3. Son PASS commit/HEAD dondurulur.
4. Açık durumlar `DEFERRED` veya `BLOCKED` olarak raporlanır.
5. 2026-08-17 Pazartesi yazıma geçilir.

---

## 4. Bağımlılık zinciri

```text
TASK-A0
  → TASK-A1 → TASK-A2
  → TASK-D0 → TASK-D1 → TASK-D2
  → TASK-E1 → TASK-E2
  → TASK-V0 → TASK-V1 → TASK-V2 → TASK-V3
  → [normal: TASK-V4 → TASK-V5 → TASK-C0]
     [13:00 time-cut: TASK-V5 → TASK-C0; TASK-V4 P4 sonrasına]
  → TASK-C1
  → TASK-S0 → TASK-S1 → TASK-S2
  → [n<3: normalde TASK-R0; time-cut yolunda TASK-V4 mümkünse → TASK-R0]
     [n≥3: TASK-P0 → TASK-P1 → TASK-P2 → TASK-P3 → TASK-P4
            → time-cut yolunda TASK-V4 mümkünse → TASK-R0]
```

`TASK-D1` ve `TASK-E1` stacked commit'tir; fakat ağır validation yalnız final HEAD'de bir kez yapılır.

---

## 5. Atomik görev kartları

## TASK-A0 — Salt-okunur repo ve kanıt envanteri

**Amaç:** Hiçbir dosyaya dokunmadan gerçek başlangıç durumunu ölçmek ve sonraki kartların kullandığı komut/path manifestini oluşturmak.

**Tahmin:** 10–20 dakika.  
**Mod:** Plan/read-only.  
**Yazma:** Yalnız `/tmp/kama-m2-a0/` altındaki loglar.

### Girdiler

- Beklenen dal: `transport-baseline-closure`
- Beklenen commit'ler: `8d0c2fee`, `74a6736c`, `4f11545c`, `7e1af66b`, `d510070d`, `08fe39a0`
- Bilinen M5 WIP adayları:
  - `crates/adaptive-transport/src/fate_estimator.rs`
  - `crates/kama-integration/tests/common/fs_cal2_spine.rs`
  - `crates/kama-integration/tests/common/fs_cal2_schema.rs`
  - `crates/kama-integration/tests/fs_cal2_pins.rs`

### Yapılacaklar

1. `git branch --show-current`, `git rev-parse HEAD`, `git status --short --untracked-files=all` kaydet.
2. Beklenen commit'lerin HEAD ancestry'sinde olup olmadığını doğrula.
3. Tracked ve untracked diff/path kümelerini ayrı raporla.
4. Her M5 WIP dosyasının SHA-256 hash'ini ve tracked/untracked durumunu kaydet.
5. Çalışan `cargo`, `rustc`, test runner veya eski Claude workflow process'lerini salt-okunur olarak listele.
6. Repoda şu authoritative entry point'leri bul ve exact komutları bir manifestte dondur:
   - 32×18 neutrality,
   - structural oracle,
   - `dr_cs3_v2_channel`,
   - 13-check `gate.sh`,
   - §12.6 calibration,
   - 512 OFF-only selection,
   - paired OFF/ON panel.
7. Paired panel runner'ın tek invocation içinde bütün OFF kontrollerini ON başlamadan önce doğrulayabildiğini kontrol et. Editsiz yapılamıyorsa bunu `D0` için blocker adayı olarak yaz.
8. Selection addendum ve channel-fate interpretation addendum hash'lerini tekrar ölç.

### PASS

- Branch ve ancestry beklendiği gibi.
- Tüm dirty/untracked dosyalar isim ve hash ile açıklanmış.
- Eski ağır process yok veya varsa kullanıcıya blocker olarak raporlanmış.
- Altı entry point exact path/komut olarak çözülmüş.
- Hiçbir repo dosyası değişmemiş.

### FAIL/BLOCKED

- Beklenmeyen tracked değişiklik → `BLOCKED-UNKNOWN-TRACKED-DIFF`.
- Beklenen commit yok/ancestry farklı → `BLOCKED-HISTORY-MISMATCH`.
- Aynı target dir üzerinde hâlâ ağır Cargo çalışıyor → `BLOCKED-ACTIVE-CARGO`.
- Seçim/panel komutu kaynakta bulunamıyor → `BLOCKED-HARNESS-ENTRYPOINT`.

### Çıktı

`/tmp/kama-m2-a0/manifest.md` ve SHA-256.

---

## TASK-A1 — M5 WIP'i güvenli ve geri getirilebilir biçimde park et

**Amaç:** Repo-wide format ve clean gate öncesinde kullanıcıya ait M5 WIP'i M2 ağacından çıkarmak; içeriği kaybetmemek.

**Önkoşul:** `TASK-A0 PASS`; M2 dalında beklenmeyen tracked diff yok.  
**Tahmin:** 10–20 dakika.  
**Yazma:** Git branch/commit; yalnız A0'da hash'lenen exact WIP path'leri.

### Yapılacaklar

1. A0 manifestindeki path ve hash'leri tekrar doğrula.
2. `wip/m5-deferred-2026-08-15` dal adı mevcutsa üzerine yazma; `BLOCKED-WIP-BRANCH-EXISTS` ver.
3. M2 dalından yeni WIP dalı oluştur.
4. Yalnız exact WIP path'lerini `git add -- <paths>` ile ekle; broad add kullanma.
5. Tek commit at: `wip(m5): park deferred fate-estimator files before M2 validation`.
6. Commit tree'sinden her dosyanın içeriğini okuyup A0 SHA-256 ile karşılaştır.
7. `transport-baseline-closure` dalına geri dön.
8. M2 dalında bu WIP path'lerinin artık dirty/untracked olmadığını; WIP commit'inden geri getirilebilir olduğunu göster.

### PASS

- WIP commit'i ayrı dalda var.
- Dört dosya (ve yalnız A0'da belirlenen exact WIP dosyaları) hash-identical.
- M2 dalı clean.
- `git show <wip-commit>:<path>` her dosyayı geri üretiyor.
- `git stash`, reset, silme, format yok.

### FAIL/BLOCKED

- Hash değişikliği, beklenmeyen dosya veya tracked M2 diff → hemen dur.
- Branch switch conflict → otomatik çözme yok.
- Herhangi bir içeriğin kaybolma ihtimali → `BLOCKED-WIP-SAFETY`.

---

## TASK-A2 — WIP parkının bağımsız denetimi

**Amaç:** A1'i yapan oturumdan bağımsız olarak WIP'in kaybolmadığını ve M2 ağacının formatter için güvenli olduğunu doğrulamak.

**Önkoşul:** `TASK-A1` tamamlandı.  
**Mod:** Read-only fresh session.  
**Tahmin:** 5–10 dakika.

### PASS

- WIP branch ve commit erişilebilir.
- Manifestteki her SHA-256 eşit.
- M2 dalında worktree clean.
- WIP dosyaları M2 diff'inde yok.
- Forbidden reset/stash izine rastlanmıyor.

Herhangi biri sağlanmıyorsa `FAIL`; A1 oturumu düzeltmeye çağrılmaz, ayrı recovery kartı hazırlanır.

---

## TASK-D0 — Channel-fate deferral ve authoritative harness grounding

**Amaç:** Semantik commit D'de yapılacak exact küçük değişiklikleri, veri üretmeden önce salt-okunur şekilde tanımlamak.

**Önkoşul:** `TASK-A2 PASS`.  
**Mod:** Plan/read-only.  
**Tahmin:** 20–30 dakika.

### Kaynak soruları

1. Silence/DupRec dalının neden tick-bağımsız finite product bound vermediğini source lineage ile göster.
2. Harness'ta önceden tanımlı, sonuçtan bağımsız explicit ceiling var mı? Varsa kaynağını göster; yoksa `NO-PREREGISTERED-HARNESS-CEILING` de.
3. A–E kusurlarını exact guard/test/satır düzeyinde adlandır:
   - A: cap guard,
   - B: `control_13` / logger producer,
   - C–E: event-cap derivasyonu ve config/producer kapsamı.
4. Hangilerinin product-bound premise correction ile çözüldüğünü, hangilerinin deferred kaldığını ayır.
5. Efficacy karar mantığının fate/cap alanlarını tüketmediğini **tam call-site taramasıyla** kanıtla. “Logger yok” deme; doğru iddia “decision/outcome logic fate/cap tüketmiyor”dur.
6. Authoritative selection ve paired panelde logger'ın runtime olarak OFF olmasını sağlayan mevcut call path'i göster.
7. Paired runner şu global adaleti sağlayabiliyor mu?
   - bütün selected seed'ler için OFF önce,
   - herhangi bir OFF selection fenotipini yeniden üretmezse bütün kampanya stop,
   - ancak bütün OFF kontrolleri geçerse ON başlar,
   - aynı invocation/mode/binary/config.
8. Bu davranış mevcutsa exact testini göster. Mevcut değilse minimal test-harness değişikliğini Commit D kapsamına öner; shipping değişikliği önerme.
9. 512 selection ve paired panelin terminal completion/latency/bytes/probe/invariant metriklerinin exact isim ve kaynaklarını dondur. Yeni acceptance threshold icat etme.
10. Commit D'nin locked-panel davranışına etkisi için aşağıdaki iki beyandan **tam birini** seç ve D1 başlamadan grounding artifact'iyle hash'le:

    ```text
    LOCKED-PANEL IMPACT: NONE
    Commit D tamamen additive/reporting/control-plane'dir; frozen locked-panel
    davranış alanlarını değiştiremez. V1 authoritative logger-OFF ekseninde
    sıfır mismatch beklenir.
    ```

    veya:

    ```text
    LOCKED-PANEL IMPACT: PREREGISTERED EXACT DELTA
    Değişebilecek exact kayıt/alanlar: <sabit liste>
    Her alan için: <before> → <after>, kaynak call path'i, neden kasıtlı olduğu,
    efficacy/qualification girdisi olup olmadığı.
    Liste dışındaki her fark V1 failure'dır.
    ```

    “Ne farklı çıkarsa izinli say” biçiminde dynamic exclusion yasaktır. Exact delta bir efficacy/qualification girdisini değiştiriyorsa V1 bunu `INSTRUMENT INVALID` diye yanlış etiketlemez; fakat `LOCKED-PANEL-EFFICACY-SEMANTICS-CHANGED — USER DECISION REQUIRED` diyerek selection'ı durdurur.

### PASS

- On sorunun her biri source/satır kanıtlı.
- No-cap/deferred hükmünün kapsamı açık.
- Efficacy non-consumption proof tam.
- Commit D'nin izinli dosya listesi ve exact diff planı çıkmış.
- Locked-panel etki beyanı iki formdan biriyle exact yazılmış ve D1 öncesi hash'lenmiş.
- Hiçbir sonuç/seed/ON arm çalıştırılmamış.

### BLOCKED

- Global OFF-before-ON stop davranışı test-only kapsamda sağlanamıyor → `SHIPPING/COMMON CHANGE REQUIRED — USER DECISION REQUIRED`.
- Efficacy logic fate/cap tüketiyor → `BLOCKED-EFFICACY-COUPLING`.
- Önceden tanımlı ceiling bulundu ama hangi verdict'in geçerli olduğu belirsiz → kullanıcı kararı.

### Çıktı

`/tmp/kama-m2-d0/grounding.md` ve önerilen exact path listesi.

---

## TASK-D1 — Semantik channel-fate deferral commit'i

**Amaç:** D0'ın onayladığı küçük semantik değişikliği ve dürüst muhasebeyi tek commit'e almak. Format cleanup yapılmaz.

**Önkoşul:** `TASK-D0 PASS`.  
**Tahmin:** 30–45 dakika.  
**İzinli path'ler:** Yalnız D0 manifestinde onaylanan `dr_cs3_panel.rs`, ilgili M2 runner/test dosyası gerekiyorsa o exact dosya ve bir yeni/var olan report addendum'u.  
**Forbidden:** `crates/adaptive-transport/src/**`, M5 dosyaları, `scripts/gate.sh`, repo-wide fmt.

### Gerekli içerik

1. Exact verdict:

   ```text
   UNBOUNDED-BY-PRODUCT — HARNESS-SPECIFIC BOUND REQUIRED
   ```

2. Explicit, sonuçtan önce tanımlanmış harness ceiling yoksa cap enforcement authoritative yoldan çıkarılır; yeni sayı seçilmez.
3. Fate/cap metrikleri:
   - authoritative efficacy raporunda yok,
   - promotion gate değil,
   - Gate-5 hükmü değil,
   - product-finding kaynağı değil.
4. Authoritative logger politikası:
   - 512 OFF selection: OFF,
   - paired OFF: OFF,
   - paired ON: OFF.
5. Optional diagnostic replay logger ON olabilir; iki arm'da simetrik olmak ve `NON-GATING DIAGNOSTIC` etiketi taşımak zorundadır.
6. Diagnostic replay'de ölçülen event maksimumu yalnız:

   ```text
   EMPIRICAL OBSERVATION — NOT A BOUND
   ```

7. A–E kusurları isimleriyle ledger'a yazılır. Muhasebe:

   ```text
   10/12 mutation proofs PASS
   2/12 DEFERRED (known-defective/non-load-bearing guards)
   CHANNEL-FATE ATTRIBUTION: DEFERRED, NOT CLOSED
   M2 EFFICACY PATH: UNBLOCKED (subject to final validation)
   ```

8. Pre-treatment prefix assertion diagnostic logger-on testlerinde korunur; authoritative efficacy'nin nedensel metriği olarak sunulmaz.
9. Global `PAIRING-HARNESS-MISMATCH — STOP` davranışı gerekiyorsa D0'ın minimal test-only tasarımı uygulanır:
   - bütün OFF'lar önce,
   - herhangi biri uyuşmazsa ON hiç başlamaz,
   - seed atlama/replacement yok.
10. Selection kaydının rolü yalnız seed qualification kanıtıdır; efficacy kontrol kolu paired panelde yeniden koşulan OFF'tur.
11. D0'ın hash'li `LOCKED-PANEL IMPACT` beyanına uyulur. D1 bu beyanın exact alan listesini genişletemez veya V1 sonucuna göre değiştiremez.

### Targeted doğrulama

- Önce formatlama yapmadan ilgili exact küçük unit/control testleri çalıştır.
- `dr_cs3_v2_channel` bütün 723 saniyelik paketi bu kartta tekrar koşma; final validation'a bırak.
- `git diff --check` çalıştır.
- Forbidden path diff'in sıfır olduğunu ölç.

### PASS

- D0 exact planından sapma yok.
- Yeni/post-hoc cap yok.
- Logger authoritative iki arm'da da OFF.
- Global OFF mismatch ON'u engelliyor; targeted test bunu pozitif ve negatif kontrolle gösteriyor.
- Fate/cap call-site non-consumption statik assert/test ile korunuyor veya D0'ın onayladığı açık kanıt report'a yazılmış.
- Targeted testler yeşil.
- Tek amaçlı commit atılmış: `test(m2): defer non-gating channel-fate attribution`.

### FAIL

- Gerçek ölçüm beklenenden farklıysa assertion gevşetme yok; FAIL.
- Yeni shipping/common kapsamı çıkarsa commit atma; BLOCKED.
- Logger OFF yapıldığında efficacy çıktısı değişirse `INSTRUMENT-DEPENDENCY-FOUND — STOP`.

---

## TASK-D2 — Commit D bağımsız denetimi

**Amaç:** D1 implementer'ından bağımsız, fresh-context review.

**Önkoşul:** `TASK-D1` commit'i var.  
**Mod:** Read-only, tercihen clean worktree.  
**Tahmin:** 20–30 dakika.

### Denetim

1. Yalnız D0 izinli path'leri değişmiş mi?
2. D1'in 10 gerekli içeriğinin her birini satır referansıyla doğrula.
3. `rg` ile fate/cap'in efficacy karar mantığında kullanılmadığını bağımsız tarat.
4. `rg` ile yasak iddiaları ara:
   - `channel-fate PASS`,
   - observed maximum'u bound yapan dil,
   - overflow'u product failure sayan dil,
   - logger OFF/ON asimetrisi.
5. Targeted testleri tekrar çalıştır; D1 loguna güvenme.
6. Commit mesajı ve diff'in semantik/format karışımı taşımadığını doğrula.
7. D0'ın hash'li locked-panel etki beyanıyla D1 diff'ini karşılaştır; beyan dışı davranış etkisi blocker'dır.

### PASS

Bütün kontroller açık kanıtlı. Reviewer dosya düzenlemez.

### FAIL

Tek blocker bile varsa `TASK-D2 FAIL`; `TASK-E1` açılmaz. Fix, kullanıcı onayından sonra ayrı ve yeni bir repair kartında tanımlanır.

---

## TASK-E1 — Pinned rustfmt tarafından üretilen format-only commit

**Amaç:** Branch'in kendi post-CI format drift'ini tek amaçlı commit ile temizlemek; D commit'iyle karıştırmamak.

**Önkoşul:** `TASK-D2 PASS`, M2 worktree clean, M5 WIP park edilmiş.  
**Tahmin:** 15–30 dakika.  
**Mod:** Clean M2 worktree.

### Yapılacaklar

1. `rust-toolchain.toml`, `rustc --version`, `cargo --version`, `rustfmt --version` kaydet.
2. Başlangıçta `cargo fmt --all -- --check` çalıştır; beklenen RED'i ve `Diff in` path listesini kaydet.
3. Repo clean değilse dur; formatter çalıştırma.
4. `cargo fmt --all` çalıştır.
5. `cargo fmt --all -- --check` → exit 0.
6. Değişen bütün path'leri kaydet; yalnız rustfmt'ın ürettiği tracked `.rs` değişiklikleri olmalı.
7. M5 WIP, reports, scripts, Cargo manifestleri ve başka untracked dosyalarda değişiklik olmadığını doğrula.
8. `git diff --check` çalıştır.
9. `git diff --ignore-all-space` çıktısını yalnız diagnostic olarak raporla; bunu tek başına “semantik değişiklik yok” kanıtı sayma.
10. Commit: `style: apply pinned rustfmt to branch drift`.

### PASS

- Formatter öncesi worktree clean.
- Formatter exact pinned toolchain.
- Formatter sonrası check green.
- Diff yalnız formatter çıktısı.
- M5 hash'leri/park commit'i değişmemiş.
- Format commit'i D'den ayrı.

### FAIL/BLOCKED

- Formatter beklenmeyen non-Rust path'e dokunursa restore/reset yapma; diff'i bırakıp kullanıcıya raporla.
- M5 path'i ortaya çıkarsa `BLOCKED-WIP-LEAK`.
- Manual semantic edit gerekirse bu kartın kapsamı değildir.

---

## TASK-E2 — Format-only reproducibility denetimi

**Amaç:** E1 commit'inin gerçekten pinned `cargo fmt --all` çıktısı olduğunu bağımsız üretimle kanıtlamak.

**Önkoşul:** `TASK-E1` commit'i var.  
**Mod:** Read-only ana repo; ayrı clean reproducer worktree.  
**Tahmin:** 10–20 dakika.

### Denetim

1. E1'in parent'ından clean worktree oluştur.
2. Aynı pinned `cargo fmt --all` komutunu çalıştır.
3. Reproducer diff path listesi ve normalized patch hash'ini E1 ile karşılaştır.
4. E1 commit'inde rustfmt'ın üretmediği ek edit olmadığını doğrula.
5. E1 HEAD'de `cargo fmt --all -- --check` exit 0.
6. Reproducer worktree'yi silmeden önce status/commit/untracked durumunu raporla; sonra güvenli cleanup.

### PASS

E1 patch'i reproducer formatter patch'iyle byte-identical veya git'in metadata farkları çıkarıldıktan sonra içerik-identical.

### FAIL

Fark varsa format commit'i semantik saflık kazanmaz; final validation başlamaz.

---

## TASK-V0 — Final validation manifesti ve tek-koşu disiplini

**Amaç:** Uzun testlere başlamadan exact HEAD, komut, target dir ve artifact path'lerini dondurmak.

**Önkoşul:** `TASK-E2 PASS`.  
**Tahmin:** 10 dakika.  
**Yazma:** Yalnız `/tmp/kama-m2-final-validation-<HEAD>/`.

### Yapılacaklar

- Worktree clean ve branch doğru.
- Final validation HEAD kaydet.
- D0/A0 exact komut manifestini yeniden doğrula.
- Her ağır komut için ayrı log path'i, timeout politikası ve beklenen sonuç satırı yaz.
- Bir command failure'ında otomatik kod değişikliği yapılmayacağını kaydet.
- Aynı target dir üzerinde yalnız bir Cargo süreci kuralını doğrula.

### PASS

Manifest hash'lenmiş, HEAD clean ve tüm komutlar çözümlü.

---

## TASK-V1 — Final HEAD 32×18 neutrality: authoritative ve diagnostic eksenler

**Amaç:** Shared panel path'e son semantik/format dokunuşlarından sonra bankalanmış davranış kanıtını final HEAD'de yenilemek.

**Önkoşul:** `TASK-V0 PASS`.  
**Tahmin:** 45–75 dakika.  
**Rol:** Heavy runner + ayrı read-only verifier.

### Eksen A — authoritative logger OFF (efficacy-gating)

- Baseline: `d510070d`, enstrümansız 839 satırlık panel.
- Candidate: final validation HEAD, channel logger **OFF**.
- Her iki reverse-direction mode.
- 8 repetition × 2 tail config = 16 kayıt/koşu; toplam 32.
- Donmuş exact 18 alan; dynamic “ne farklıysa dışla” yok.
- D0 `LOCKED-PANEL IMPACT: NONE` dediyse exact sıfır mismatch zorunlu.
- D0 `PREREGISTERED EXACT DELTA` dediyse yalnız hash'li exact kayıt/alan/before→after farkları ayrı sınıfta sayılabilir. Liste çalışma sırasında genişletilemez.

### Eksen B — candidate logger OFF ↔ logger ON (diagnostic, non-gating)

- Aynı final HEAD, binary, feature fingerprint, seed/repetition ve harness modu.
- Tek runtime farkı diagnostic channel logger OFF/ON.
- Aynı frozen 32 kayıt × 18 alan karşılaştırılır.
- Logger'ın kendi yeni alanları yalnız önceden donmuş sabit exclusion isim listesiyle dışlanabilir.
- Bu eksen channel logger'ın gözlem nötrlüğünü sınar; authoritative efficacy arm'ları logger OFF olduğundan efficacy kapısı değildir.

### PASS ve ayrı eksen muhasebesi

Eksen A'nın zorunlu sonucu:

```text
records_compared = 32
fields_per_record = 18
unapproved_mismatches = 0
missing = 0
extra = 0
```

D0 `NONE` ise `approved_preregistered_deltas = 0`; exact-delta yolu varsa sayı ve alanlar D0 hash'iyle birebir eşleşmelidir.

Eksen B'nin ideal sonucu:

```text
records_compared = 32
fields_per_record = 18
diagnostic_logger_mismatches = 0
missing = 0
extra = 0
```

`TASK_RESULT` tek genel “mismatch” sayısı vermez; şu iki sonucu ayrı taşır:

```yaml
authoritative_logger_off_neutrality: PASS | FAIL | BLOCKED
diagnostic_logger_on_neutrality: PASS | DEFERRED_INVALID
```

### Sonuç kuralları

- **Eksen A'da beyan dışı fark:**

  ```text
  AUTHORITATIVE-NEUTRALITY-FAILURE — EFFICACY PATH BLOCKED
  ```

  Kodun yalnız çalışması değil, logger OFF iken varlığı davranışı değiştirmektedir. V2 ve selection açılmaz; otomatik fix yok.

- **Eksen A'da D0-onaylı fakat efficacy/qualification alanını değiştiren exact fark:**

  ```text
  LOCKED-PANEL-EFFICACY-SEMANTICS-CHANGED — USER DECISION REQUIRED
  ```

  Bu `INSTRUMENT INVALID` diye etiketlenmez, fakat selection kullanıcı kararı/rebaseline olmadan açılmaz.

- **Eksen B'de fark, Eksen A PASS:**

  ```text
  DIAGNOSTIC-LOGGER-NEUTRALITY-FAILURE
  CHANNEL-FATE DIAGNOSTIC: INVALID / DEFERRED
  M2 EFFICACY PATH: UNAFFECTED (AUTHORITATIVE LOGGER OFF)
  ```

  V2 açılabilir; R0 diagnostic failure'ı saklamaz. Task'ın dependency-gating sonucu Eksen A'ya göre belirlenir.

---

## TASK-V2 — Structural oracle regresyonu

**Amaç:** Final HEAD'in kapalı structural oracle'ı bozmadığını ölçmek.

**Önkoşul:** `TASK-V1` authoritative logger-OFF ekseni PASS ve selection-blocking exact delta yok.  
**Tahmin:** 10–30 dakika.

### Komut

A0 manifestindeki exact komut kullanılacak; en az:

```bash
cargo test -j2 -p adaptive-transport --test dr_cs3_structural_oracle -- --nocapture
```

### PASS

- Test exit 0.
- Gate-5 denominator `{cell_3, cell_7}`.
- Her ikisinde direct displacement 0 ve tüm divergence boyutları 0.
- Hücre 7 label'ı parity pending/not actuated ayrımını koruyor.
- Yanlış `PRODUCT FINDING` yok.

### FAIL

Tek failure → dur; gate'e geçme.

---

## TASK-V3 — Channel target regresyonu

**Amaç:** Channel harness'ın final HEAD'de bilinen yeşil sonucunu ölçmek.

**Önkoşul:** `TASK-V2 PASS`.  
**Tahmin:** 12–20 dakika; 723 saniye normaldir.

### Komut

```bash
cargo test -j2 -p kama-integration --test dr_cs3_v2_channel
```

### PASS

```text
45 passed; 0 failed
```

ve authoritative efficacy path logger OFF contract testleri yeşil.

### FAIL

- 12 dakikayı “hung” sayma; process kontrol et.
- Exit nonzero ise otomatik rerun/repair yok.

---

## TASK-V4 — Full local 13-check gate

**Amaç:** Bu branch için gerçek entegrasyon kapısını final HEAD'de çalıştırmak.

**Önkoşul:** `TASK-V3 PASS`, worktree clean.  
**Tahmin:** 60–120 dakika.  
**Rol:** Tek heavy runner.

### Zamanlama

- Normal yolda V3'ten hemen sonra ve V5'ten önce çalışır.
- §3.3 time-cut aktifse V5/C0/selection'ı bloke etmez; terminal efficacy endpoint'inden sonra aynı frozen HEAD'de çalışır.
- Time-cut yolunda bu kartın geç çalışması veya hiç çalışmaması deney arm'larını yeniden çalıştırma gerekçesi değildir; yalnız integration-readiness statüsünü belirler.

### Komut

A0'ın doğruladığı exact gate entry point; beklenen:

```bash
scripts/gate.sh
```

Gate'in 13 bileşeni:

```text
0 fmt
1 compile closure
2–6 five test configs
7 clippy ×5
8 worktree clean
```

### PASS

- Gate exit 0.
- `cargo fmt --all -- --check` gerçekten step 0'da çalışmış.
- Hiçbir check silinmemiş, allowlist edilmemiş veya daraltılmamış.
- Worktree clean.

### FAIL

- Exact failing step ve log raporlanır.
- Gate failure bu kart içinde tamir edilmez.
- Hosted CI iddiası kurulmaz. Doğru dil: `FULL LOCAL 13-CHECK GATE PASS/FAIL`.

---

## TASK-V5 — Final validation hükmü ve HEAD/binary freeze

**Amaç:** V1–V3 kanıtını ve varsa V4'ü bağımsız olarak konsolide etmek; selection için değişmez HEAD/binary üretmek.

**Önkoşul:** `TASK-V1` authoritative ekseni + `TASK-V2` + `TASK-V3` PASS; ayrıca ya `TASK-V4 PASS` ya da §3.3'teki 13:00 time-cut aktif.  
**Mod:** Read-only verifier.  
**Tahmin:** 15–30 dakika.

### Yapılacaklar

1. V1–V3 loglarını ve exit code'ları; normal yolda ayrıca V4 logunu bağımsız oku.
2. Uygulanabilir bütün kanıtların aynı HEAD'de çalıştığını doğrula.
3. Default-feature selection/panel binary'sini bir kez build et.
4. Binary SHA-256, `rustc/cargo` sürümü, feature fingerprint, HEAD, Cargo.lock hash'i ve logger default'unu manifestte dondur.
5. Selection addendum hash'ini yeniden doğrula.
6. Normal yolda şu statüyü yaz:

   ```text
   M2 STRUCTURAL ORACLE: CLOSED
   M2 EFFICACY PATH: VALIDATED AND READY FOR CALIBRATION
   CHANNEL-FATE ATTRIBUTION: DEFERRED, NOT CLOSED
   FULL LOCAL 13-CHECK GATE: PASS
   HOSTED CI ON THIS BRANCH: NOT RUN / NOT CLAIMED
   ```

7. §3.3 time-cut yolunda statü şu olur:

   ```text
   M2 STRUCTURAL ORACLE: CLOSED
   M2 EFFICACY PATH: VALIDATED AND READY FOR CALIBRATION
   CHANNEL-FATE ATTRIBUTION: DEFERRED, NOT CLOSED
   FULL LOCAL 13-CHECK GATE: NOT RUN — PREREGISTERED TIME CUT
   HOSTED CI ON THIS BRANCH: NOT RUN / NOT CLAIMED
   ```

### PASS

Tek HEAD, clean tree, binary hash ve o yürütme yolunda zorunlu bütün validation kanıtları tutarlı. Time-cut yolunda V4'ün yokluğu PASS diye yeniden etiketlenmemiş.

### FAIL

Farklı HEAD/binary/config altında bir kanıt varsa freeze yapılamaz.

---

## TASK-C0 — §12.6 calibration tasarımını dondur

**Amaç:** 512 selection başlamadan worker/CPU/RSS ölçüm komutlarını ve karar kuralını veri görülmeden yazmak.

**Önkoşul:** `TASK-V5 PASS`.  
**Mod:** Plan/read-only.  
**Tahmin:** 10–20 dakika.

### Dondurulacaklar

- Final frozen binary hash.
- Logger OFF.
- Default features; `experimental-cold-start=false`.
- Rep9 converging ve rep16 stalling kalibrasyon örnekleri veya mevcut prereg'in exact eşdeğerleri.
- Worker adayları: 1 ve 2; daha fazlası yeni karar gerektirir.
- Ölçümler: wall time, process CPU, peak RSS, sim ticks, fwd/rev events.
- Derleme ile deney çalışma süresi ayrılır.
- Sonuç toplama repetition index'e göre; worker completion order kullanılmaz.

### PASS

Karar kuralı ve komutlar sonuçtan önce hash'lenmiş.

---

## TASK-C1 — §12.6 calibration çalıştır

**Amaç:** Final HEAD'de selection worker sayısını ölçmek.

**Önkoşul:** `TASK-C0 PASS`.  
**Tahmin:** 15–30 dakika.

### Beklenen referans, hüküm değil

```text
converge ≈ 5.518 s
stall ≈ 0.454 s
all-converge 512 ≈ 0.78 wall-core h
2-worker ideal ≈ 23.5 min
```

### PASS

- Her worker adayında command, wall/CPU/RSS ölçülmüş.
- OOM, swap thrash, target-dir collision yok.
- Seçilen worker sayısı ölçülen kaynak sınırına dayanıyor; varsayılan 2'yi kör kabul etmiyor.
- Binary/config/logger fingerprint V5 ile aynı.

### FAIL

Kaynak baskısı veya non-determinism → worker 1'e düşürmek yalnız C0 kuralı izin veriyorsa; aksi halde BLOCKED.

---

## TASK-S0 — Selection koşusu için outcome-blind preflight

**Amaç:** İlk seed sonucu görülmeden seçim sözleşmesini son kez ölçüp hash'lemek.

**Önkoşul:** `TASK-C1 PASS`.  
**Tahmin:** 10 dakika.  
**Veri üretimi:** Yok.

### Dondurulacak sözleşme

- Universe: exact 512 repetition index.
- OFF-only; ON build/run/read yok.
- Default features; `experimental-cold-start=false`.
- Frozen V5 binary.
- Logger OFF.
- OFF bootstrap `probe_emitted delta = 0`.
- OFF legacy `cs_probe_emitted delta = 0`.
- Herhangi biri ihlal:

  ```text
  INVALID-BASELINE-CONFIG — ABORT SELECTION
  ```

- Results repetition index order'da.
- Qualifying stall fenotipi ve bütün acceptance alanları mevcut prereg'den exact alınır; yeni threshold yok.
- Forward packet window dağılımı kaydedilir.
- Band kuralı:
  - `n ≥ 6`: ilk 3 challenge, sonraki 3 confirmation; extras reserve-only, replacement değil.
  - `3 ≤ n ≤ 5`: ilk 3 challenge; kalan 0–2 confirmation, `CONFIRMATION-UNDERPOWERED`; default promotion yasak.
  - `n < 3`: default promotion blocked; ON campaign yok.

### PASS

Manifest seçim sonucu görülmeden commit/hash'lenmiş ve addendum hash'i doğru.

---

## TASK-S1 — 512 OFF-only selection

**Amaç:** Preregistered universe'te qualifying OFF-stall seed'lerini bulmak; efficacy çalıştırmamak.

**Önkoşul:** `TASK-S0 PASS`.  
**Tahmin:** 25–60 dakika.  
**Rol:** Tek deterministic runner; C1 worker sayısı.

### Yapılacaklar

1. Exact 512 repetition, OFF-only.
2. Aynı frozen binary/config/logger fingerprint'ini her worker başlangıcında assert et.
3. Her repetition'da iki OFF probe deltasını assert et.
4. Raw per-repetition output'u değişmez artifact'e yaz.
5. Worker completion sırasını at; canonical çıktı repetition index'e göre sort edilir.
6. Qualifying predicate mevcut prereg'den exact uygulanır.
7. ON arm'a ait hiçbir execution/output okunmaz.

### PASS

- 512 unique index; 0 eksik, 0 duplicate.
- Bütün baseline feature/probe assertions geçer.
- Raw artifact ve canonical artifact hash'lenir.
- Qualifying list deterministic ve index-sorted.

### FAIL

- Probe delta/config mismatch → bütün seçim invalid; seed atlama yok.
- Missing/duplicate index → run invalid; otomatik kısmi tamamlama yok.
- Process failure → aynı raw artifact üzerinde recoverability denetlenmeden retry yok.

---

## TASK-S2 — Selection bandı, seed listeleri ve global hüküm

**Amaç:** S1 sonucunu önceden dondurulmuş kuralla sınıflamak ve ON öncesi listeleri hash'lemek.

**Önkoşul:** `TASK-S1 PASS`.  
**Mod:** Read-only analysis.  
**Tahmin:** 10–20 dakika.

### Hükümler

#### `n ≥ 6`

- Challenge = ilk 3 qualifying index.
- Confirmation = sonraki 3.
- Extra seed'ler yalnız reserve record; failure replacement için kullanılamaz.
- İki liste ON çalışmadan önce ayrı ve birlikte hash'lenir.
- `TASK-P0` açılır.

#### `3 ≤ n ≤ 5`

- Challenge = ilk 3.
- Confirmation = kalan 0–2.
- `CONFIRMATION-UNDERPOWERED (n<3)`.
- Partial mechanism evidence mümkün; **default promotion mümkün değil**.
- Listeler hash'lenir; `TASK-P0` açılır.

#### `n < 3`

- `DEFAULT PROMOTION BLOCKED`.
- ON campaign başlatılmaz.
- Forward packet window yaklaşık 20 ise `CHANNEL-MODEL-PROPERTY`.
- Window belirgin biçimde daha uzunsa `SIZING-ASSUMPTION-MISMATCH`.
- Universe sessizce büyütülmez; yeni prereg gerekir.
- Doğrudan `TASK-R0` açılır.

### PASS

Band sonucu mekanik, listeler index-order ve hash'ler ON öncesi timestamp'li.

---

## TASK-P0 — Paired harness ve adalet preflight'i

**Amaç:** ON görmeden, authoritative paired invocation'ın seçimle aynı harness modunda çalışacağını doğrulamak.

**Önkoşul:** `TASK-S2` en az 3 qualifying seed üretti.  
**Tahmin:** 10–20 dakika.  
**Veri:** ON yok.

### Adalet sözleşmesi

- Aynı seed.
- Aynı V5 binary hash.
- Aynı Cargo/default feature fingerprint.
- Aynı channel config/order.
- Aynı panel invocation/mode.
- Logger iki arm'da OFF.
- Yalnız runtime treatment değişir.
- OFF: bootstrap probe delta 0, legacy CS delta 0.
- ON: legacy CS delta 0, treatment enable true; bootstrap actuation mevcut prereg'e göre kaydedilir.
- Paired efficacy OFF kaydı selection'dan devralınmaz; panel invocation içinde yeniden koşulur.
- Runner bütün OFF arm'larını önce çalıştırır ve global eşdeğerlik kontrolü geçmeden hiçbir ON arm başlatmaz.

### PASS

Runner kaynak ve targeted dry-run/control ile bu akışı kanıtlıyor; config fingerprint selection'la exact.

### FAIL

Asimetri veya cross-mode farkı → `PAIRING-HARNESS-MISMATCH — STOP`; seed değişimi yok.

---

## TASK-P1 — Determinizm pini

**Amaç:** Herhangi bir ON arm öncesinde ilk challenge seed'in paired-mode OFF çıktısının deterministik olduğunu göstermek.

**Önkoşul:** `TASK-P0 PASS`.  
**Tahmin:** 10–25 dakika.

### Koşu

İlk challenge seed, OFF, logger OFF, aynı binary/config/mode ile üç kez.

### PASS

- Üç frozen outcome/trace hash'i byte-identical.
- Üçünde qualifying selection stall fenotipi yeniden oluşuyor.
- Probe deltas sıfır.

### FAIL

Herhangi bir fark global campaign stop'tur:

```text
PAIRING-HARNESS-MISMATCH — STOP
```

Seed atlanmaz; kalan seed'lere geçilmez; ON çalıştırılmaz.

---

## TASK-P2 — Tek authoritative invocation: bütün OFF'lar, global gate, sonra bütün ON'lar

**Amaç:** Challenge + confirmation listesinin tamamını **tek panel process/invocation** içinde çalıştırmak: önce bütün OFF arm'ları, sonra global reproduction gate ve yalnız bu gate geçerse bütün ON arm'ları.

**Önkoşul:** `TASK-P1 PASS`.  
**Tahmin:** Frozen binary hazırsa simülasyon yaklaşık 1 dakika; harness başlatma/artifact doğrulamayla 5–20 dakika. Yeniden derleme gerekiyorsa bu süre ayrıca raporlanır.  
**Rol:** Tek deterministic heavy runner; bu kart sırasında kod yazılmaz.

### Tek komutun zorunlu iç akışı

1. Frozen selected seed listesi ve hash'i okunur.
2. Bütün selected seed'lerin OFF arm'ları aynı invocation/mode içinde çalışır.
3. Her OFF, selection'daki exact qualifying stall fenotipiyle ve sıfır probe delta'larıyla karşılaştırılır.
4. **Bütün** OFF'lar geçmeden ON loop'una girilemez.
5. Herhangi bir OFF uyuşmazsa process nonzero çıkar; o invocation'da tek bir ON arm bile çalışmamış olmalıdır.
6. Bütün OFF'lar geçerse aynı process, aynı yüklenmiş binary/config/mode içinde bütün selected ON arm'larını çalıştırır.
7. Challenge/confirmation etiketleri ve seed sırası frozen manifestten gelir.
8. Logger OFF'tur; yalnız runtime treatment OFF→ON değişir.
9. Raw event/outcome kaydı ve execution-order audit kaydı artifact'e yazılır.

### PASS (koşunun geçerliliği)

- Her selected seed için paired OFF exact qualifying stall fenotipini üretir.
- Binary/config/logger/probe invariants exact.
- Bütün OFF ve ON kayıtları **aynı tek invocation** içindedir.
- Execution-order kanıtı `OFF(all) → global gate PASS → ON(all)` gösterir.
- Tüm preregistered ON arm'ları exact birer kez çalışmıştır; 0 eksik, 0 duplicate.
- Raw ve canonical artifact'ler hash'lenmiştir.

**Not:** Bir seed'in ON armında kurtarılmaması `TASK-P2` altyapı FAIL'i değildir; geçerli negatif ürün sonucudur ve `TASK-P4`te hükme girer.

### FAIL — global

Herhangi bir OFF uyuşmazsa:

```text
PAIRING-HARNESS-MISMATCH — STOP
```

- Kampanya bütünü durur.
- Seed atlanmaz/değiştirilmez.
- Daha önceki seed'ler geçse bile hiçbir ON başlamaz.
- Sorun o seed'e değil, selection-mode/panel-mode eşdeğerliğine yazılır.

Başka config/binary/list uyuşmazlığı da run-invalid'dir; otomatik retry veya seed değişimi yapılmaz.

---

## TASK-P3 — Paired campaign artifact ve yürütme sırası denetimi

**Amaç:** P2'yi çalıştıran oturumdan bağımsız olarak tek-invocation adaletini, artifact bütünlüğünü ve run geçerliliğini doğrulamak; efficacy hükmü vermemek.

**Önkoşul:** `TASK-P2` geçerli artifact üretmiş.  
**Mod:** Fresh-context, read-only verifier.  
**Tahmin:** 15–30 dakika.

### Denetim

1. Artifact seed listesi S2 hash'iyle exact mı?
2. Her seed için tam bir OFF ve bir ON var mı; eksik/duplicate var mı?
3. Process/invocation ID bütün arm'larda aynı mı?
4. Execution order bütün OFF kayıtlarının bütün ON kayıtlarından önce olduğunu gösteriyor mu?
5. Global gate PASS kaydı son OFF'tan sonra, ilk ON'dan önce mi?
6. Binary, default-feature fingerprint, channel config, mode ve logger OFF bütün kayıtlarda aynı mı?
7. OFF probe delta'ları sıfır mı; ON legacy CS delta sıfır mı; runtime treatment dışında config farkı var mı?
8. Challenge/confirmation etiketleri ve sıra frozen manifestle aynı mı?
9. Acceptance predicate S0/prereg'den exact mı; sonuçtan sonra threshold değişmiş mi?
10. Fate/cap metriği authoritative artifact veya karar girişinde bulunuyor mu?

### PASS

- On kontrolün tamamı ölçülmüş ve geçmiştir.
- P2 run'ı valid paired campaign olarak `TASK-P4`e açılır.

### FAIL

Bir run-validity kusuru efficacy sonucu üretmez. Reviewer dosya düzenlemez, run'ı `INVALID-PAIRED-CAMPAIGN` olarak işaretler.

---

## TASK-P4 — Efficacy hükmü

**Amaç:** S2 bandı ve P2/P3 verisini dondurulmuş kurallarla yorumlamak.

**Önkoşul:** `TASK-P3 PASS`.  
**Mod:** Read-only independent verifier.  
**Tahmin:** 20–30 dakika.

### `n ≥ 6` hükmü

Binding acceptance:

```text
challenge 3/3 AND confirmation 3/3
```

- 6/6 recovery + bütün safety/budget/invariant koşulları PASS ise izinli iddia:

  > Bootstrap probing recovered all six preregistered, selected V2 cold-start stall cases under the tested configuration.

- Şunlar söylenmez:
  - “Cold-start çözüldü.”
  - “Bütün popülasyonda etkilidir.”
  - “Prevalence budur.”
- Herhangi bir valid ON arm selected stall'ı kurtarmazsa bu “sonuçsuz” değildir:

  ```text
  VALID NEGATIVE PRODUCT FINDING
  Mechanism did not recover the preregistered selected phenotype.
  Bootstrap default remains OFF.
  ```

#### `3 ≤ n ≤ 5` hükmü

- Challenge sonuçları ve varsa 0–2 confirmation sonucu raporlanır.
- Hepsi kurtarılsa bile:

  ```text
  CONFIRMATION-UNDERPOWERED
  PARTIAL MECHANISM EVIDENCE ONLY
  DEFAULT PROMOTION NOT GRANTED
  ```

- Herhangi valid ON failure yine negatif ürün bulgusudur; default OFF.

### PASS

- Hüküm mekanik olarak prereg'e uyuyor.
- Selection OFF yalnız seed qualification kanıtı olarak kullanılmış.
- Paired OFF efficacy kontrol koludur.
- Channel-fate/cap yok.
- Claims scope doğru.

---

## TASK-R0 — Final rapor ve Çarşamba teslim sınırı

**Amaç:** Başarı/negatif/underpowered/blocked sonucunu dürüst kapsamla belgelemek.

**Önkoşul:** `TASK-S2` n<3 sonucu veya `TASK-P4`; §3.3 time-cut yolunda V4 çalıştırılabildiyse onun sonucu da alınmış, çalıştırılamadıysa `NOT RUN` kaydı mevcut.  
**Tahmin:** 30–60 dakika.

### Zorunlu ayrımlar

- Structural invariant ≠ measured efficacy.
- Selected challenge recovery ≠ prevalence.
- Zero displacement ≠ zero cost.
- Channel-fate divergence ≠ emission-schedule divergence.
- Diagnostic event max ≠ bound.
- Local 13-check gate ≠ Hosted CI.
- Channel-fate attribution deferred ≠ PASS.
- Tail-v2 netem sonucu ≠ shipped/default behavior.

### Rapor durum satırları

Koşula göre exact olanlar seçilir:

```text
M2 STRUCTURAL ORACLE: CLOSED
M2 EFFICACY: PASS | VALID NEGATIVE | UNDERPOWERED | BLOCKED
CHANNEL-FATE ATTRIBUTION: DEFERRED, NOT CLOSED
FULL LOCAL 13-CHECK GATE: PASS | FAIL | NOT RUN — PREREGISTERED TIME CUT
HOSTED CI ON transport-baseline-closure: NOT RUN / NOT CLAIMED
TAIL-V2: NETEM-VALIDATED, DEFAULT-OFF, ADAPTIVE-CAP NOT IMPLEMENTED
```

---

## 6. Kart başına önerilen Claude Code istemleri

Bu kısa istemler görev kartındaki ayrıntıları tekrar etmez; bağlam şişmesini önler.

### Salt-okunur kart

```text
@Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md içindeki TASK-<ID>'yi
fresh context'te, plan/read-only modda yürüt. Yalnız belirtilen soruları yanıtla.
Hiçbir repo dosyasını değiştirme. Bulguları source/line veya exact komut çıktısıyla kanıtla.
PASS ölçütlerinden biri ölçülemezse PASS deme; BLOCKED raporla. TASK_RESULT ile bitir.
```

### Implementer kartı

```text
@Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md içindeki TASK-<ID>'yi uygula.
Başlangıç koşullarını ölçmeden edit yapma. Yalnız kartta izinli path'lere dokun.
Başka kartı başlatma, scope genişletme, assertion gevşetme ve otomatik repair loop yapma.
Targeted doğrulamayı çalıştır; exact diff ve exit code kanıtını ver. PASS ise kartın söylediği
tek amaçlı commit'i at. TASK_RESULT ile bitir.
```

### Bağımsız verifier kartı

```text
@Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md içindeki TASK-<ID>'yi,
implementer'ın muhakemesini doğru kabul etmeden fresh context'te denetle. Dosya değiştirme.
Yalnız correctness ve stated-requirement blocker'larını raporla; stil önerileriyle scope büyütme.
Her closure criterion için met/false ve somut kanıt ver. Tek blocker varsa FAIL de ve sonraki
kartı açma. TASK_RESULT ile bitir.
```

### Heavy runner kartı

```text
@Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md içindeki TASK-<ID>'nin tek ağır
komutunu çalıştır. Kod yazma. Aynı target dir'de ikinci Cargo başlatma. Çıktıyı artifact'e kaydet,
5-10 dakikada bir process heartbeat ver. Tool timeout'u test failure saymadan process'i kontrol et.
Exit nonzero veya runtime failure'da otomatik retry/fix yapma. Exact süre, exit code, sonuç satırı,
HEAD ve artifact hash'iyle TASK_RESULT ver.
```

---

## 7. Zaman ve kaynak bütçesi

| Blok | Yaklaşık duvar süresi | Kör bekleme riski | Kontrol noktası |
|---|---:|---|---|
| A0–A2 hygiene | 30–50 dk | Düşük | Kullanıcı A2 sonrası |
| D0–D2 semantic deferral | 70–105 dk | Orta | Kullanıcı D2 sonrası |
| E1–E2 format | 25–50 dk | Düşük | Kullanıcı E2 sonrası |
| V0–V5 normal final validation | 2–4 saat | Yüksek | Her ağır kart ayrı |
| V0–V3 + V5 time-cut validation | 1,5–2,75 saat | Orta/yüksek | V4 P4 sonrasına |
| Deferred V4 integration gate | 60–120 dk | Yüksek | Terminal efficacy endpoint sonrası |
| C0–C1 calibration | 25–50 dk | Düşük/orta | C1 sonrası |
| S0–S2 selection | 45–90 dk | Orta | Kullanıcı S2 sonrası |
| P0–P4 paired efficacy | 60–125 dk | Orta | P4 sonrası; P2 simülasyonu ≈1 dk, kart 5–20 dk |
| R0 rapor | 30–60 dk | Düşük | Final |

Bu tahminler bir üst sınır garantisi değildir. 2026-08-16 13:00 kırpması §3.3'te sonuç görülmeden dondurulmuştur. En önemli fark: hiçbir adım 3,5 saatlik görünmez bir ajan kuyruğu değildir; ağır işler ayrı kart ve ayrı logdur.

---

## 8. Neden bu plan Claude Code'dan daha iyi verim alır?

1. **Doğrulanabilirlik:** Her görevde Claude'nun çalıştırabileceği exact PASS/FAIL sinyali vardır.
2. **Bağlam kontrolü:** Bir oturum tek task ID görür; eski hata ve loglar bütün bağlamı doldurmaz.
3. **Araştırma/uygulama ayrımı:** Grounding bitmeden code edit yoktur.
4. **Bağımsız değerlendirme:** Writer kendi işini bankaya sokamaz.
5. **İzolasyon:** Worktree, paralel session'ların file edit çarpışmasını önler.
6. **Kısıtlı paralellik:** Salt-okunur işler paralel olabilir; 723 saniyelik Cargo hedefleri seri kalır.
7. **Failure containment:** Bir FAIL bütün planı otomatik onarım döngüsüne sokmaz.
8. **Deney körlüğü:** Selection/ON sırası ve seed bandı sonuç görülmeden dondurulur.
9. **Kanıt muhasebesi:** PASS, DEFERRED ve BLOCKED birbirine karıştırılmaz.
10. **Deadline güvenliği:** Pazar 20:00'de yeni tur açılmasını engelleyen açık kesim vardır.

---

## 9. Resmî Claude Code kaynakları

Bu plandaki Claude Code kullanım tercihleri yalnız Anthropic'in resmî belgelerine dayanır:

- [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices): Claude'ya çalıştırabileceği doğrulama vermek; önce explore/plan sonra code; ayrıntılı context; `/clear`; fresh-context writer/reviewer; adversarial review; kitchen-sink session ve sonsuz araştırmadan kaçınma.
- [How Claude Code works](https://code.claude.com/docs/en/how-claude-code-works): agent döngüsünün context toplama → action → verification yapısı ve açık başarı sinyallerinin önemi.
- [Common workflows](https://code.claude.com/docs/en/common-workflows): plan mode, codebase exploration, küçük/test edilebilir refactor adımları ve session resume kalıpları.
- [Run parallel sessions with worktrees](https://code.claude.com/docs/en/worktrees): paralel oturumları ayrı git checkout'larında izole etme, worktree yaşam döngüsü ve cleanup güvenliği.
- [Create custom subagents](https://code.claude.com/docs/en/sub-agents): subagent'larda tool kısıtlama, `disallowedTools`, `maxTurns`, `permissionMode`, background ve `isolation` alanları.
- [Permission modes](https://code.claude.com/docs/en/permission-modes): salt-okunur plan modu ve farklı izin seviyeleri.
- [Hooks reference](https://code.claude.com/docs/en/hooks): Stop, TaskCompleted ve worktree yaşam döngüsü gibi deterministik kontrol noktaları.
- [Claude Code sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing): dosya sistemi/ağ sınırlarıyla güvenli otonomi ve daha az izin kesintisi.

---

## 10. Son bağlayıcı cümle

Claude Code bu dosyayı okuyunca **bütün planı gerçekleştirmeye başlamaz**. Kullanıcının verdiği tek `TASK-ID`yi, o kartın izin/kanıt/durma kurallarıyla yürütür ve `TASK_RESULT` sonrası durur. Bir sonraki kartı yalnız kullanıcı başlatır.
