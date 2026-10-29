# 05b netem-7 arşivi: KTP-2 / F0

Arşivleme dayanağı: sahip kararı "#140 / netem KAYMA-M1", 2026-10-29 (ISEMRI-KTP2-F0 başı).

## Sahip kararı
- Ön-kayıtlı sonuç kabul edildi: `p2_loss80`'de KAYMA-M1 var. Teslim medyanı 0,510 → 0,915; p = 0,005282, α = 0,00714.
- İyileşme yönündeki kayma da ön-kayıtlı kurala tabidir. Bu yüzden "kayma yok" denmez.
- Diğer altı profilde kayma yok.
- #140 merge edilmez. Taslak kalır.
- DUR-46'nın (a), (b) ve (c) seçeneklerinin hiçbiri için yeni çalışma başlatılmaz.

## Ayırt edici deney yapılmadı
Önerilen ablasyon (taban / taban + yalnız madde 7 / 49864510) koşulmadı. `README.md` §5.5'teki mekanizma adayları [çıkarım] olarak kalır.

## Aday SHA
`49864510f0b01f138119a9f3f388c0e18a369124`, origin'de `k4g/05b-retx-fix` dalında.
- Taban ikilisi `1b1425d2`: sha256 `14b5f7d3…`.
- Aday ikilisi: sha256 `fd3089b1…`.

## İçerik
| dosya / dizin | içerik |
|---|---|
| `README.md` | kanıt paketi, 2026-10-29 sürümü; hüküm DUR |
| `ONKAYIT.md` | ön-kayıt; sha256 `6d667853…` |
| `oturum-1-a8c4e8c6/`, `oturum-2-49864510/` | ham koşu arşivleri (`*.tar.xz`), analiz, aralıklar, kapı özetleri |
| `sorgu/` | salt-okunur sorgunun yapılandırılmış çıktısı ve betikleri. Bütünü [betimsel, sonradan]; ön-kayıtlı hükmü değiştirmez. |
| `SAHIP-RAPORU-2026-10-29.md` | sahibe giden DUR raporu |

## F0'a aktarılan
- `p2_loss80` profilinde eklenmiş gecikme yok (`scripts/netem/kapi_manifest.py`). Bu yüzden adayın son srtt'si olan ≈ 19 ms fiziksel olarak makuldür. Tabanın 0,5–10 s srtt'si, 05b'nin RTT kabul kuralıyla düzelttiği bilinen srtt şişmesi kusurudur.
- Bu veriler nedeni ya da gerçek bant verimini tek başına kanıtlamaz.
- F0'da her hücre için oturumlar arası dağılım raporlanır. N < 30 olan hücrelerden hüküm çıkarılmaz.
