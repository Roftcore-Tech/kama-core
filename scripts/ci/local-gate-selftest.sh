#!/usr/bin/env bash
# local-gate.sh ve cargo-lock.sh öz-testi: kapının yanlış-yeşil yollarının gerçekten kapandığını sınar.
# cargo derlemez; geçici bir git deposunda betiklerin kopyasını sahte adım setleriyle (KAMA_GATE_STEPS)
# koşturur ve çıkış kodu + özet satırını denetler. Çıkış: 0 = hepsi geçti, 1 = en az bir vaka kaldı.
#
#   scripts/ci/local-gate-selftest.sh
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/kama-gate-selftest.XXXXXX") || exit 2
trap 'rm -rf "$TMP"' EXIT
fail=0; pass=0

# ── Geçici depo: kapının kopyası + sahte CI tanımları (özetleri kopyaya yazılır) ─────────────────────
REPO="$TMP/repo"; mkdir -p "$REPO/scripts/ci" "$REPO/.github/workflows"
cp "$HERE/local-gate.sh" "$HERE/cargo-lock.sh" "$REPO/scripts/ci/" || exit 2
echo "ci: sahte" > "$REPO/.github/workflows/ci.yml"
echo "swc: sahte" > "$REPO/.github/workflows/spec-word-coverage.yml"
ci_sha=$(sha256sum "$REPO/.github/workflows/ci.yml" | cut -d' ' -f1)
swc_sha=$(sha256sum "$REPO/.github/workflows/spec-word-coverage.yml" | cut -d' ' -f1)
sed -i "s/^CI_SHA256=[0-9a-f]*/CI_SHA256=$ci_sha/; s/^SWC_SHA256=[0-9a-f]*/SWC_SHA256=$swc_sha/" "$REPO/scripts/ci/local-gate.sh"
echo "README" > "$REPO/README"
git -C "$REPO" init -q
git -C "$REPO" -c user.email=t@t -c user.name=t add -A
git -C "$REPO" -c user.email=t@t -c user.name=t commit -q -m init
git -C "$REPO" update-ref refs/remotes/origin/main HEAD

FAKE_FLOCK="$TMP/fake-flock"; mkdir -p "$FAKE_FLOCK"; printf '#!/bin/sh\nexit 1\n' > "$FAKE_FLOCK/flock"; chmod +x "$FAKE_FLOCK/flock"
FAKE_CARGO="$TMP/fake-cargo"; mkdir -p "$FAKE_CARGO"; printf '#!/bin/sh\nexit 7\n' > "$FAKE_CARGO/cargo"; chmod +x "$FAKE_CARGO/cargo"

GIT_ID=(-c user.email=t@t -c user.name=t)
reset_repo() {  # her vakadan önce depoyu temiz init commit'ine döndür
  git -C "$REPO" reset -q --hard "$(git -C "$REPO" rev-list --max-parents=0 HEAD)"
  git -C "$REPO" clean -qfdx
  git -C "$REPO" update-ref refs/remotes/origin/main HEAD
}
# gate <ad> <beklenen-çıkış> <beklenen-özet-deseni> <adım seti> [ek argümanlar…]   (ortam: EXTRA_ENV dizisi)
gate() {
  local name=$1 want=$2 pat=$3 steps=$4; shift 4
  local out="$TMP/out-$name"; rm -rf "$out"
  ( cd "$REPO" && env "${EXTRA_ENV[@]}" KAMA_GATE_SELFTEST=1 KAMA_GATE_STEPS="$steps" \
      KAMA_CARGO_LOCK="${LOCK_OVERRIDE:-$TMP/lock}" KAMA_GATE_WT_ROOT="$TMP/wt" KAMA_GATE_TARGET="$TMP/target" \
      bash scripts/ci/local-gate.sh --out "$out" "$@" ) > "$TMP/$name.stdout" 2>&1
  local rc=$?
  if [ "$rc" = "$want" ] && grep -Eq "$pat" "$out/summary.txt" "$TMP/$name.stdout" 2>/dev/null; then
    pass=$((pass+1)); echo "  PASS  $name (exit $rc)"
  else
    fail=$((fail+1)); echo "  FAIL  $name: beklenen exit $want + /$pat/, gelen exit $rc"; sed 's/^/        /' "$TMP/$name.stdout" | head -12
  fi
}
EXTRA_ENV=(); LOCK_OVERRIDE=""

echo "local-gate öz-testi"
reset_repo; gate yesil            0 'SONUÇ: YEŞİL'                 'a|true'
reset_repo; gate ozet_isareti     0 'ÖZ-TEST ADIM SETİ'            'a|true'
reset_repo; gate kirmizi_adim     1 'SONUÇ: KIRMIZI'               $'a|true\nb|false'
reset_repo; echo "ci: değişti" > "$REPO/.github/workflows/ci.yml"; git -C "$REPO" "${GIT_ID[@]}" commit -qam drift
            gate ci_drift         1 'CI-DRIFT \(KIRMIZI\)'         'a|true'
reset_repo; echo "x" >> "$REPO/README"
            gate kirli_izlenen    2 'REDDEDİLDİ — çalışma ağacı kirli' 'a|true'
reset_repo; echo "x" > "$REPO/yeni-dosya"
            gate kirli_izlenmeyen 2 'REDDEDİLDİ — çalışma ağacı kirli' 'a|true'
reset_repo; gate head_degisti     3 'SONUÇ: GEÇERSİZ'              'a|git -c user.email=t@t -c user.name=t commit -q --allow-empty -m yarış'
reset_repo; gate agac_degisti     3 'SONUÇ: GEÇERSİZ'              'a|echo yarış >> README'
reset_repo; gate dosya_eklendi    3 'SONUÇ: GEÇERSİZ'              'a|echo yarış > yarış-dosyası'
reset_repo; LOCK_OVERRIDE="$TMP/yok/yok/lock"; mkdir -p "$TMP/yok"; chmod 0500 "$TMP/yok"
            gate kilit_acilamadi  2 'kilit dosyası açılamadı'      'a|true'
            chmod 0700 "$TMP/yok"; LOCK_OVERRIDE=""
reset_repo; EXTRA_ENV=(PATH="$FAKE_FLOCK:$PATH")
            gate flock_basarisiz  2 'kilit alınamadı'              'a|true'
            EXTRA_ENV=()
reset_repo; echo "x" > "$REPO/yeni-dosya"   # ana ağaç kirli; --ref ayrı worktree'de sınar
            gate ref_worktree     0 'ayrı worktree'                'a|test ! -e yeni-dosya' --ref HEAD
            if [ -z "$(ls -A "$TMP/wt" 2>/dev/null)" ] && [ "$(git -C "$REPO" worktree list | wc -l)" = 1 ]; then
              pass=$((pass+1)); echo "  PASS  ref_worktree_temizlendi"
            else fail=$((fail+1)); echo "  FAIL  ref_worktree_temizlendi: worktree silinmedi"; fi
reset_repo; gate ref_cozulemedi   2 'REDDEDİLDİ: --ref çözülemedi' 'a|true' --ref yok-boyle-ref

echo "cargo-lock öz-testi"
cl() {  # cl <ad> <beklenen-çıkış> <desen> [ortam…]
  local name=$1 want=$2 pat=$3; shift 3
  ( cd "$REPO" && env "$@" KAMA_AGENT=TEST KAMA_CARGO_LEDGER="$TMP/ledger.tsv" bash scripts/ci/cargo-lock.sh build ) > "$TMP/cl-$name.out" 2>&1
  local rc=$?
  if [ "$rc" = "$want" ] && grep -Eq "$pat" "$TMP/cl-$name.out" "$TMP/ledger.tsv" 2>/dev/null; then
    pass=$((pass+1)); echo "  PASS  cl_$name (exit $rc)"
  else
    fail=$((fail+1)); echo "  FAIL  cl_$name: beklenen exit $want + /$pat/, gelen exit $rc"; sed 's/^/        /' "$TMP/cl-$name.out" | head -6
  fi
}
reset_repo; rm -f "$TMP/ledger.tsv"
cl flock_basarisiz 2 'kilit alınamadı'      PATH="$FAKE_FLOCK:$FAKE_CARGO:$PATH" KAMA_CARGO_LOCK="$TMP/lock"
if [ -s "$TMP/ledger.tsv" ]; then fail=$((fail+1)); echo "  FAIL  cl_flock_defter: kilitsiz çağrı deftere yazılmamalı"; else pass=$((pass+1)); echo "  PASS  cl_flock_defter_bos"; fi
mkdir -p "$TMP/yok2"; chmod 0500 "$TMP/yok2"
cl kilit_acilamadi 2 'kilit dosyası açılamadı' PATH="$FAKE_CARGO:$PATH" KAMA_CARGO_LOCK="$TMP/yok2/x/lock"
chmod 0700 "$TMP/yok2"
cl cikis_kodu      7 'TEST'                 PATH="$FAKE_CARGO:$PATH" KAMA_CARGO_LOCK="$TMP/lock"

echo "local-gate öz-testi: $pass geçti, $fail kaldı"
[ "$fail" = 0 ]
