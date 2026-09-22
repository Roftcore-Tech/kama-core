#!/usr/bin/env bash
# Kama conformance kapısı (T-00.3) — vektör ↔ uygulama.
#
#   scripts/ci/conformance.sh
#
# AMAÇ: vektör ya da ilgili kod değiştiğinde kimsenin "koşmayı unutması" mümkün olmasın.
#
# TASARIM KARARI — JOB HER ZAMAN KOŞAR, ATLAMA YOK. Görev metni bir tetik-yolu listesi
# veriyordu. O liste burada YALNIZ TEŞHİS olarak kullanılır, atlama koşulu olarak DEĞİL.
# Üç gerekçe, üçü de ölçüldü:
#
#   1. GitHub Actions'ta `paths` yalnız workflow düzeyinde (`on:`) tanımlanır; iş düzeyinde
#      yoktur. ci.yml'de zaten workflow düzeyi `paths-ignore` var, oraya `paths` eklemek
#      TÜM job'ları etkilerdi.
#   2. `paths` ile atlanan bir required check sonsuza kadar "pending" kalır ve PR'ı kilitler.
#      Daha sinsisi: bu repodaki `ci-verdict` sınıflandırıcısı `failure`/`cancelled` arıyor,
#      `skipped`'ı görmüyordu — atlanan bir conformance "tüm kapılar yeşil" diye okunurdu.
#      (Bu PR'da ayrıca düzeltildi.)
#   3. Görev metnindeki liste zaten EKSİKTİ: ccr-codec (ADR-05'e göre wire encoding'in tek
#      kaynağı), kama-fec-codec ve kama-transport-schema yoktu. Liste atlama koşulu olsaydı
#      bu crate'ler değişince kapı sessizce koşmazdı.
#
# Maliyet: job yalnız kama-vectors'ü ve keşfedilen conformance hedeflerini derler
# (tüm workspace'i DEĞİL) ve rust-cache'i test-fast ile paylaşır.
#
# Çıkış kodu: 0 = conformance yeşil · 1 = kapı kırmızı · 2 = ortam/kullanım hatası.

set -uo pipefail

# CONF_ROOT: YALNIZ oz-test icindir (scripts/ci/conformance-selftest.sh betigin bir
# kopyasini gecici dizinde kosturur). CI bu degiskeni VERMEZ.
ROOT="${CONF_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT" || exit 2

# ── TABAN: beklenen conformance hedefi sayısı ────────────────────────────────────────────────
# "Boş test setiyle yeşil" istenen davranıştır (henüz conformance testi yazılmadı), AMA boşluk
# BEYAN EDİLİR, sessizce geçilmez. Keşfedilen hedef sayısı bu sayıdan farklıysa kapı kırmızı:
#   * yeni conf_* testi eklendi ve bu satır güncellenmedi  -> kırmızı (kasıtlı eklemeyi pinler)
#   * mevcut conf_* testi silindi                          -> kırmızı (sessiz kapsam kaybı yok)
# Yeni conformance testi eklerken bu sayıyı AYNI commit'te artırın.
CONFORMANCE_TARGETS_EXPECTED=0

# ── Görev metnindeki tetik yolları + eksik bulunan üçü (teşhis amaçlı) ───────────────────────
TRIGGER_PATHS=(
  'vectors/'
  'ref/'                              # henüz yok — plandaki gelecek dizin
  'crates/kama-app/'                  # henüz yok — plandaki gelecek crate
  'crates/kama-noise-wire/'
  'crates/client-core/src/seam.rs'
  'crates/kama-vectors/'
  'crates/ccr-codec/'                 # [EKLENDİ] ADR-05: wire encoding tek kaynağı
  'crates/kama-fec-codec/'            # [EKLENDİ] vektör tüketicisi
  'crates/kama-transport-schema/'     # [EKLENDİ] vektör tüketicisi
)

fail=0
step() { printf '\n── %s ──\n' "$1"; }
bad()  { printf '::error title=CONFORMANCE::%s\n' "$1"; printf 'KIRMIZI: %s\n' "$1"; fail=1; }

# ── 0. Tetik teşhisi (bilgi; kapının davranışını ETKİLEMEZ) ──────────────────────────────────
step "tetik teşhisi"
base="${CONF_DIFF_BASE:-}"
if [[ -n "$base" ]] && git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
  changed="$(git diff --name-only "$base"...HEAD 2>/dev/null)"
  hits=()
  for p in "${TRIGGER_PATHS[@]}"; do
    grep -q "^${p}" <<<"$changed" && hits+=("$p")
  done
  grep -qE '(^|/)tests/conf_' <<<"$changed" && hits+=('**/tests/conf_*')
  if ((${#hits[@]})); then
    printf '  dokunulan tetik yolu: %s\n' "${hits[*]}"
  else
    printf '  tetik yolu değişmedi — kapı YİNE DE koşuyor (atlama yok)\n'
  fi
else
  printf '  diff tabanı yok (CONF_DIFF_BASE) — teşhis atlandı, kapı koşuyor\n'
fi

# ── 1. Vektör korpusu YÜKLENEBİLİYOR mu ──────────────────────────────────────────────────────
# Bozuk JSON'lu bir vektör dosyası burada kırmızı olur: kama-vectors'ün parite testi
# vectors/ ağacının tamamını yükler. Bu, format kapısından (validate.py) AYRI bir merci —
# gerçek tüketici kodun kendisi.
step "vektör korpusu tüketici tarafından yükleniyor mu"
log="$(mktemp -t kama-conf-XXXXXX.log)"
if cargo test -p kama-vectors --all-features >"$log" 2>&1; then
  # Repo idyomu: 0 test koşan bir kapı YEŞİL SAYILMAZ (BUL-02/BUL-03).
  if grep -Eq "test result: ok\. [1-9][0-9]* passed" "$log"; then
    printf '  %s\n' "$(grep -c 'test result: ok' "$log") test ikilisi yeşil"
  else
    bad "kama-vectors 0 test koştu — kapı vakumda"
  fi
else
  bad "vektör korpusu yüklenemedi (bozuk vektör dosyası ya da yükleyici regresyonu)"
  tail -40 "$log"
fi
rm -f "$log"

# ── 2. Conformance hedeflerini KEŞFET ────────────────────────────────────────────────────────
step "conformance hedefleri"
mapfile -t conf_files < <(find crates -path '*/tests/conf_*' -type f 2>/dev/null | sort)
mapfile -t conf_rs < <(printf '%s\n' "${conf_files[@]}" | grep '\.rs$' || true)
found=0
((${#conf_rs[@]})) && [[ -n "${conf_rs[0]}" ]] && found=${#conf_rs[@]}

printf '  keşfedilen: %d (beklenen taban: %d)\n' "$found" "$CONFORMANCE_TARGETS_EXPECTED"
for f in "${conf_rs[@]}"; do [[ -n "$f" ]] && printf '    %s\n' "$f"; done

# `.rs` OLMAYAN conf_* dosyaları cargo test hedefi değildir (fixture/veri olabilirler).
# Sessizce yok saymak vakumdur: `tests/conf_vectors.json` koyan biri onun koşulduğunu sanır.
# Görünür kılınır; hiç `.rs` hedefi yokken veri dosyası varsa bu "kimsenin okumadığı veri"dir.
mapfile -t conf_other < <(printf '%s\n' "${conf_files[@]}" | grep -v '\.rs$' | grep . || true)
if ((${#conf_other[@]})); then
  printf '  cargo hedefi OLMAYAN conf_* dosyaları (fixture/veri):\n'
  for f in "${conf_other[@]}"; do printf '    %s\n' "$f"; done
  if [[ "$found" -eq 0 ]]; then
    bad "conf_* veri dosyası var ama hiç conf_*.rs hedefi yok — bu veriyi kimse okumuyor"
  fi
fi

if [[ "$found" -ne "$CONFORMANCE_TARGETS_EXPECTED" ]]; then
  bad "conformance hedef sayısı tabandan farklı (bulunan $found, taban $CONFORMANCE_TARGETS_EXPECTED). \
Yeni test eklediyseniz scripts/ci/conformance.sh içindeki CONFORMANCE_TARGETS_EXPECTED değerini AYNI commit'te güncelleyin; \
test silindiyse kapsam kaybı bilinçli mi?"
fi

# ── 3. Her hedefi koştur; hiçbiri 0 test koşmasın ────────────────────────────────────────────
for f in "${conf_rs[@]}"; do
  [[ -z "$f" ]] && continue
  crate_dir="${f%%/tests/*}"
  target="$(basename "$f" .rs)"
  pkg="$(sed -n 's/^name = "\(.*\)"/\1/p' "$crate_dir/Cargo.toml" | head -1)"
  [[ -z "$pkg" ]] && { bad "$crate_dir/Cargo.toml içinde paket adı okunamadı"; continue; }

  step "conformance: $pkg :: $target"
  log="$(mktemp -t kama-conf-XXXXXX.log)"
  if cargo test -p "$pkg" --all-features --test "$target" >"$log" 2>&1; then
    if grep -Eq "test result: ok\. [1-9][0-9]* passed" "$log"; then
      printf '  %s\n' "$(grep -E 'test result: ok' "$log" | head -1)"
    else
      bad "$pkg::$target 0 test koştu — vakum (dosya var, test yok)"
    fi
  else
    bad "$pkg::$target kırmızı"
    tail -40 "$log"
  fi
  rm -f "$log"
done

printf '\n'
if [[ $fail -eq 0 ]]; then
  printf 'CONFORMANCE: YEŞİL (%d hedef)\n' "$found"
else
  printf 'CONFORMANCE: KIRMIZI\n'
fi
exit $fail
