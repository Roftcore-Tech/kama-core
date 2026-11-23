# Ham ürün izleri (2026-11-23)

Yalnız ölçüm çıktısı; hiçbir dala birleştirilmez. Ürün testi `crates/kama-integration/tests/asym_product_outcome.rs`, release, anahtarlı kayıp, ACK kipi bitmap (pilot yolu; V2 ürün yolu bu izlerde YOK).

| Dizin | Ürün kodu SHA | Test dosyası | Hücre | PTT kipi | Tohum |
|---|---|---|---|---|---|
| `alti-hucre/` | `86c5602caa18ff592c201e23bd0946d17e43a779` | aynı SHA | altı kapı hücresi (dosya adı) | yalın, 200 ms / 80 B | 1–3 |
| `ana-iz/237-ile/` | `86c5602caa18ff592c201e23bd0946d17e43a779` | `e6ff8b59b4aae16977cb75414b1181cd9da882ba` | `measure_rate_drop_during_file_transfer` | yalın ve kopyalı (`KAMA_ASYM_PTT_LEAN=off`) | 1–3 |
| `ana-iz/237-siz/` | `cd61a9756c4a2767ea539a2256024fd6d467b3d4` (#242 başı) | aynı test dosyası (ağaç `faff3e65310294874791bb8df95291159f148176`) | aynı | yalın ve kopyalı | 1–3 |

Komut: `KAMA_ASYM_SEED=<t> [KAMA_ASYM_PTT_LEAN=off] <ikili> <hücre> --exact --nocapture`.
Satır türleri: `ÜRÜN-SONUÇ`, `ÖLÇÜT`, `İZ-HAT`, `İZ-ARALIK`, `İZ-PTT`, `İZ-ACK`.
