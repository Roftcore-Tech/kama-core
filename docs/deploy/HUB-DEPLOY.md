# HUB-DEPLOY — hub kurulum ve işletme runbook'u

Bu belge `KM-DEPLOY-TDD-001` v0.5 §6'nın **birebir kopyasıdır** (§6 son satırı: "`docs/deploy/HUB-DEPLOY.md` bu
tablonun kopyasıdır"). Belge değişirse önce TDD değişir, sonra bu kopya. Dosyalar: `deploy/{Dockerfile, compose.yml,
Makefile, kama-hub.service}`; imajı `.github/workflows/hub-image.yml` üretir (`ghcr.io/roftcore-tech/kama-hub:<kısa-sha>`).

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

