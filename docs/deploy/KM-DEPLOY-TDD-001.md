# KM-DEPLOY-TDD-001 — Hub'ı sunucuya kurmak, güncellemek, telefonları hazırlamak (v0.5)

Sahip: Abkarada · Uygulayan: Yaaz · Sürüm: v0.5 (2026-10-14; v0.4 + ikinci inceleme bulgusu 6: geçerli systemd birimi, çıkış kodu aktarımı) · Bağımlı: `KM-HUB-TDD-001` v0.6.1 (hub ikilisi ve `probe` alt komutu; v0.5 → v0.6 bu belgede davranış değişikliği gerektirmedi, sürüm v0.5 kaldı), `kama-provision` **v1** (hub anahtarı + `hub.json` + `probe` cihazı; Abkarada A2)

**Bu belge tek başına yeter.** Hub'ın ne olduğu için `KM-SEAM-TH-001` §1 (on cümle) yeter. Her komutun yanında nedeni var.

---

## 1. Ne kuruyoruz, neden böyle

Tek Linux sunucu (VPS), üstünde tek program (hub), tek açık UDP portu. Hub'ın içinde veri yok (tablolar bellekte): yedek, veritabanı, disk büyümesi derdi yok. Kurulumun bütün zorluğu üç şeyi **doğru** yapmakta: (1) sunucuya yalnız hub'ın kendi anahtarını koymak, telefonların dosyalarını **asla** koymamak; (2) çalışan programın CI'ın `main`'den derlediği imaj olduğundan emin olmak (elle derlenmiş şey sunucuya girmez); (3) bir komutla güncelleyip bir komutla geri alabilmek.

Yapmadıklarımız (bilerek): Kubernetes, birden fazla sunucu, metrik paneli, alarm, otomatik deploy, TLS/ters proxy (hub UDP konuşur), yedekleme (durum yok).

## 2. Kontrol listesi — bittiğinde hepsi "evet"

| # | Soru | Nasıl bakılır |
|---|---|---|
| C1 | Sunucuda telefonlara ait dosya/anahtar yok; `/opt/kama-hub` içinde yalnız `compose.yml`, `.env`, `.env.prev`, `Makefile`, `hub.json`, `logs/`; tek gizli alan `hub.json` → `hub_ed25519.priv` | `find /opt/kama-hub -type f` + `hub.json` şeması |
| C2 | Hub root değil; konteyner dosya sistemi salt-okunur; yalnız `hub.json` okunuyor; `hub.json` konteyner kullanıcısına (65532) ait ve 0400 — ve bu izinlerle **gerçekten başlıyor** (`probe` yeşil) | `docker inspect` + `stat` + T-DEP-6 |
| C7 | Hub'ın yaşam döngüsünün **tek sahibi** systemd; elle `docker compose up -d`/`down` yok | `systemctl status kama-hub`; Makefile hedefleri `systemctl` çağırır |
| C3 | Dışarıya yalnız 47000/udp + 22/tcp | `ufw status` + dışarıdan TCP taraması; UDP için `probe` (bkz. T-DEP-5) |
| C4 | Çalışan imajın etiketi `main`'deki bir commit'in kısa SHA'sı; CI üretti | `docker image ls` ↔ Actions koşusu |
| C5 | Geri alma tek komut, < 1 dk | T-DEP-4 |
| C6 | Log döndürülüyor, disk dolmuyor | compose log ayarı |

## 3. Sunucuyu hazırlama (bir kez)

| Adım | Ne | Neden |
|---|---|---|
| 3.1 | VPS: İstanbul, Ubuntu 24.04 LTS, 1 vCPU / 1 GB / 20 GB, sabit IPv4 | İki telefon için fazlası gereksiz |
| 3.2 | `deploy` kullanıcısı, SSH anahtarıyla giriş; parola girişi ve root SSH **kapalı**; sudoers yalnız `systemctl (start\|stop\|restart\|status) kama-hub`, `install … /opt/kama-hub/hub.json` ve `docker compose … pull` için (NOPASSWD) | Standart sertleştirme; `deploy` hub'ı yönetir ama sunucuyu yönetmez |
| 3.3 | `apt install docker.io docker-compose-plugin ufw unattended-upgrades` | Çalıştırma, güvenlik duvarı, yamalar |
| 3.4 | `ufw default deny incoming && ufw allow 22/tcp && ufw allow 47000/udp && ufw enable` | Yalnız SSH ve hub portu (port S4 kararı) |
| 3.5 | `timedatectl set-ntp true` | Yalnız log zaman damgaları için; hub'ın cookie'si kendi monoton saatini kullanır, duvar saatine bağımlı değildir |
| 3.6 | `mkdir -p /opt/kama-hub/logs` (sahibi `deploy`) | Tek dizin |

## 4. İmaj (CI üretir; elle üretilmez)

- `deploy/Dockerfile`, iki aşama: (1) `rust:<repo toolchain sürümü>` + `rustup target add x86_64-unknown-linux-musl` → `cargo build --release -p hub-server --target x86_64-unknown-linux-musl` (statik ikili; hub'ın bağımlılıkları saf Rust — C kütüphanesi yok; CI'da `ldd` çıktısı "not a dynamic executable" denetlenir); (2) `scratch` tabanlı imaj: yalnız ikili; `USER 65532`; `ENTRYPOINT ["/hub-server","--config","/config/hub.json"]`.
- Etiket `ghcr.io/<org>/kama-hub:<kısa-sha>`; `latest` **kullanılmaz**.
- Hedef boyut < 20 MB.
- `hub-server probe --device <device.json> --hub <host:port>`: iki turlu bind + 3 heartbeat yapar, `bind ok rtt=… ms · heartbeat 3/3` yazar; çıkış kodu 0/1. **Probe kendi cihazıyla bağlanır** (`device-probe.json`, kama-provision v1 `--probe`; eş listelerinde görünmez) — Alice/Bob dosyasıyla probe yapılmaz: aynı kimlikle bind, telefonun dönüş adresini laptopa taşır ve telefonu düşürür.

## 5. Compose ve servis

`/opt/kama-hub/compose.yml`:
```yaml
services:
  hub:
    image: ghcr.io/<org>/kama-hub:${HUB_TAG}
    network_mode: host          # UDP için araya Docker NAT'ı girmesin; port doğrudan 47000
    read_only: true
    user: "65532:65532"
    cap_drop: [ALL]
    volumes:
      - ./hub.json:/config/hub.json:ro   # bind mount: dosyanın SUNUCUDAKİ sahibi/izni geçerli → 65532 okuyabilmeli (§6.2)
    restart: "no"               # yeniden başlatmayı Docker değil systemd yapar (tek sahip)
    logging: { driver: json-file, options: { max-size: "50m", max-file: "5" } }   # ≤ 250 MB
```
`HUB_TAG` `/opt/kama-hub/.env`'de.

**Yaşam döngüsünün tek sahibi systemd'dir.** `/etc/systemd/system/kama-hub.service` (dosya **birebir** budur; repoda `deploy/kama-hub.service`):
```ini
[Unit]
Description=Kama hub (docker compose, foreground)
After=docker.service network-online.target
Requires=docker.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/kama-hub
ExecStart=/usr/bin/docker compose -f /opt/kama-hub/compose.yml up --exit-code-from hub --remove-orphans
ExecStop=/usr/bin/docker compose -f /opt/kama-hub/compose.yml down
Restart=always
RestartSec=2
TimeoutStopSec=20

[Install]
WantedBy=multi-user.target
```
`--exit-code-from hub`: compose `up`, hub konteyneri çıkınca **kendisi de çıkar** ve konteynerin çıkış kodunu döndürür (Docker belgesi: bu bayrak `--abort-on-container-exit` davranışını içerir) → systemd `Restart=always` ile 2 s'de yeniden başlatır; sunucu yeniden başlarsa birim açılışta kalkar. `docker compose up -d` / `down` **elle çalıştırılmaz**: iki sahip (Docker'ın `restart: always`'ı + systemd) aynı konteyneri çekiştirir, `make rollback` hangi sürümün ayakta olduğunu bilemez. Birim dosyası CI'da `systemd-analyze verify` ile denetlenir (T-DEP-0).

`/opt/kama-hub/Makefile` (tek satırlık hedefler, gizli bilgi yok; hepsi systemd üzerinden):
- `make deploy TAG=<sha>` → `cp .env .env.prev`; `echo HUB_TAG=<sha> > .env`; `sudo docker compose pull`; `sudo systemctl restart kama-hub`
- `make rollback` → `cp .env.prev .env`; `sudo systemctl restart kama-hub`
- `make status` → `systemctl status kama-hub --no-pager` + `docker ps` + son 20 `hub stats` satırı (`journalctl -u kama-hub`)

## 6. Kurulum ve işletme (runbook — birebir)

| Adım | Nerede | Komut / eylem | Beklenen |
|---|---|---|---|
| 6.1 Provisioning | Laptop | `kama-provision init --out ./prov --devices alice,bob --probe --hub <VPS_IP>:47000` (v1) | `prov/root.json`, `prov/hub.json`, `prov/device-alice.json`, `prov/device-bob.json`, `prov/device-probe.json` |
| 6.2 Hub config | Laptop → VPS | `scp prov/hub.json deploy@<VPS>:/tmp/hub.json && ssh deploy@<VPS> 'sudo install -o 65532 -g 65532 -m 0400 /tmp/hub.json /opt/kama-hub/hub.json && rm /tmp/hub.json'` | **Yalnız** `hub.json` gider; `root.json` ve `device-*.json` sunucuya **asla** (C1). **Sahip 65532:65532, mod 0400** — konteyner bu kullanıcıyla koşar ve bind mount sunucudaki izni aynen görür; `deploy`'a ait 0600 dosyayı hub **okuyamaz ve başlamaz** (T-DEP-6b). `/opt/kama-hub` dizini 0755 kalır (geçiş izni) |
| 6.3 İlk başlatma | VPS | `cd /opt/kama-hub && echo HUB_TAG=<sha> > .env && sudo systemctl enable --now kama-hub` | `systemctl status kama-hub` → active; `docker ps` → Up; `journalctl -u kama-hub -f` → 10 s'de bir `hub stats` |
| 6.4 Doğrulama | Laptop | `hub-server probe --device prov/device-probe.json --hub <VPS_IP>:47000` | `bind ok … heartbeat 3/3` |
| 6.5 Güncelleme | VPS | `make deploy TAG=<yeni-sha>` | Kesinti hedefi ≤ 5 s (ölçülür); telefonlar kendiliğinden toparlar (§6 süre kuralı) |
| 6.6 Geri alma | VPS | `make rollback` | < 1 dk (C5) |
| 6.7 Durum | VPS | `make status` | |
| 6.8 Yeni cihaz / anahtar yenileme | Laptop | 6.1'i yeniden çalıştır (herkes yeni kimlik alır — PoC kuralı), 6.2–6.4 tekrar | |

**Süre kuralı (bütün belgelerde aynı; sayı uydurulmaz):** pasif sessizlik tek başına hiçbir şeyi düşürmez; telefon "koptu"yu yalnız **cevapsız aktif yoklama** sonrasında ilan eder — **trafik varken** ARQ yedeklemesi + yoklama patlaması içinde (≈ RTO + 3 s), **sessizlikte** heartbeat aralığı + yoklama patlaması içinde (8 s ya da 25 s + 4 s). "Koptu" = **yeniden bind**; oturumlar düşmez, devam eder. Hub yeniden başladığında tabloları boş olduğu için eski handle ilk veride `ROUTE_MISS` alır ve o oturum bir el sıkışmayla yeniden kurulur. 60 saniyelik bir kesinti de aynı yoldan toparlar (INT-03 (4)). T-DEP-3 ve INT-03 (5) bunu ölçer ve raporlar.

`docs/deploy/HUB-DEPLOY.md` bu tablonun kopyasıdır.

## 7. CI (kararlar Abkarada'nın)

- Mevcut PR kontrolleri (`ci.yml`) `hub-server`'ı zaten kapsar (`cargo test --workspace`, clippy, deny, layer_check); eklenen tek adım: S4 hub conformance (`scripts/ci/hub-conformance.sh --target real`, A2'nin harness'ı) ve `vectors/hub/` python `--check`.
- Yeni `hub-image.yml`: yalnız `main`'e merge'de; Dockerfile'ı derler, `ghcr.io/<org>/kama-hub:<kısa-sha>` push eder; cache ile hedef ≤ 6 dk; `cosign` yok (PoC).
- **Otomatik deploy yok**: sunucuya `make deploy` ile insan koyar.
- Her koşu Actions dakikasını rapora yazar.

## 8. Telefonları hazırlama (APK/AAR)

- **AAR**: `kama-mobile` `.so` (arm64-v8a, 16 KB sayfa hizalı; Abkarada'nın K4G-15 release'i) + üretilmiş Kotlin → `kama-mobile-<sha>.aar`; Hasan'ın projesi `libs/` altından alır.
- **APK**: Hasan'ın projesinden; yan yükleme imzası (`debug` **değil**; imza anahtarı `apk-signing/` dizininde, repoya girmez); `versionName = <tarih>-<sha>`.
- **Telefona kurulum** (her telefon ~7 dk): `adb install app-<sha>.apk` → `adb push prov/device-alice.json /sdcard/Download/` → uygulamada "İçe aktar" → dosyayı seç (uygulama kendi özel dizinine kopyalar, `/sdcard` kopyasını siler) → overlay "bağlı" gösterir.
- `docs/deploy/PROVISION-DEMO.md`: iki telefonu sıfırdan kurma, komutlarıyla, 15 dk.

## 9. Asla yapılmayacaklar

`root.json` ya da `device-*.json`'u sunucuya koymak · Alice/Bob dosyasıyla probe · `latest` etiketi · elle derlenmiş imaj · root ya da yazılabilir dosya sistemi · 47000/udp ve 22/tcp dışında port · otomatik deploy · imza anahtarını repoya koymak · systemd dışında `docker compose up -d`/`down` · `hub.json`'u `deploy` kullanıcısının sahipliğinde bırakmak (hub başlamaz).

## 10. Testler ve kabul

| ID | Senaryo | Beklenen |
|---|---|---|
| T-DEP-0 | `systemd-analyze verify deploy/kama-hub.service` (CI adımı, Ubuntu koşucusu) ve VPS'te `systemctl cat kama-hub` = repodaki dosya (sha256) | uyarı/hata yok; dosya bayt-özdeş |
| T-DEP-1 | Temiz VPS'te 6.1–6.4 | ≤ 30 dk; `probe` → `bind ok … heartbeat 3/3` |
| T-DEP-2 | Sunucu yeniden başlatılır | hub ≤ 60 s'de ayakta; `probe` yeşil |
| T-DEP-3 | `make deploy` yeni etiketle, iki test düğümü bağlıyken | kesinti ölçülür (hedef ≤ 5 s); düğümler §6 süre kuralı içinde yeniden bind + oturum; sayılar rapora |
| T-DEP-4 | `make rollback` | < 1 dk; eski etiket çalışıyor |
| T-DEP-5 | Dışarıdan `nmap -sS` tüm TCP portlar → yalnız 22; UDP için `probe` yeşil (hub yabancıya cevap vermediği için `nmap -sU` "open\|filtered" gösterir — kanıt sayılmaz) | |
| T-DEP-6 | `hub.json` sahibi 65532:65532, mod 0400; konteyner kullanıcı 65532; read-only; cap_drop ALL — **ve bu izinlerle gerçek başlangıç:** `sudo systemctl restart kama-hub` → `active`, `journalctl`'de `hub stats`, laptoptan `probe` yeşil. İzin denetimi tek başına kabul değildir | `stat` + `docker inspect` + `probe` |
| T-DEP-6b | Karşı deney: `hub.json` `deploy:deploy` 0600 yapılıp yeniden başlatılır | hub **başlamaz**, log tek satır "config unreadable: /config/hub.json" (yük/anahtar yok); systemd yeniden dener; 6.2'deki `install` sonrası düzelir. Testin gerçekten izni sınadığının kanıtı |
| T-DEP-7 | 48 saat kesintisiz koşu, iki telefon bağlı | log ≤ 250 MB; `hub stats` endpoints/links sabit; bellek sabit |
| T-DEP-8 | İki telefon sıfırdan kurulum | ≤ 15 dk; hub üzerinden TEXT |
| T-DEP-9 | **Gerçek VPS'te:** konteyner elle öldürülür (`docker kill`), ayrıca hub sürecine `SIGKILL` (konteyner içinden çıkış kodu ≠ 0) | her ikisinde `journalctl` compose `up`'ın çıktığını gösterir, systemd ≤ 5 s'de yeniden başlatır; `probe` yeşil; `docker ps`'te tek hub konteyneri (iki sahip yok); `systemctl show -p NRestarts` artmış |

Kabul: T-DEP-0…9 yeşil ve INT-03 (laptop + bu VPS + kötü ağ) bu hub'la geçmiş.

---

## Ek — v0.3 → v0.4 değişiklikleri

1. `hub.json` sahipliği: `chmod 600` (sahip `deploy`) → `install -o 65532 -g 65532 -m 0400` (6.2); C2 ve T-DEP-6 buna göre; T-DEP-6b karşı deneyi eklendi. Gerekçe: bind mount sunucudaki izni aynen taşır; 65532 kullanıcısı `deploy`'un 0600 dosyasını okuyamaz, hub başlamaz; eski test (`stat` 0600) bunu yeşil gösterirdi.
2. Tek yaşam döngüsü sahibi: compose `restart: always` → `"no"`; systemd birimi tam yazıldı (`ExecStart` ön plan `up`, `ExecStop` `down`, `Restart=always`); Makefile `up -d` → `systemctl restart`; C7, T-DEP-9 ve §9 yasağı eklendi.
3. `deploy` sudoers kapsamı (3.2).
4. §6 süre kuralı S4 v1.1 C6/C7 diliyle hizalandı: "pasif sessizlik tek başına düşürmez", "koptu = yeniden bind, oturum devam eder", 60 s kesinti aynı yol.

## Ek — v0.4 → v0.5 değişiklikleri

1. systemd birimi gerçek bir unit dosyası olarak yazıldı (bölüm başlıkları ve direktifler ayrı satırlarda; `·` ayraçları kalktı); `systemd-analyze verify` CI adımı (T-DEP-0). Önceki blok doğrudan kopyalandığında `Invalid section header` veriyordu — incelemeci haklı.
2. Konteyner çıkışının servis sürecine aktarımı açık: `docker compose up --exit-code-from hub` (Docker belgesi: `--abort-on-container-exit`'i içerir); T-DEP-9 gerçek VPS'te, iki ölüm biçimiyle ve `NRestarts` kanıtıyla.
