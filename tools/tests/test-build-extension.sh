#!/usr/bin/env bash
# =============================================================================
# Tests de tools/build-extension.sh (cas nominal, reproductibilité, erreurs).
#   bash tools/tests/test-build-extension.sh
#
# Chaque cas travaille sur une copie jetable du dépôt et exécute la copie du
# script ($COPIE/tools/build-extension.sh) : le script résout la racine depuis
# son propre chemin, donc c'est la seule façon de tester une copie. Le dépôt
# réel n'est jamais modifié.
#
# Convention d'assertion : le code retour est TOUJOURS capturé dans une
# variable juste après la commande ($? relu plus tard est écrasé par le
# moindre echo/printf). check = succès attendu, refute = échec attendu.
# =============================================================================

set -uo pipefail
export LC_ALL=C TZ=UTC

TESTS_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
ROOT=$(cd -- "$TESTS_DIR/../.." && pwd -P)
PASS=0
FAIL=0
TMPDIRS=()
COPY_DIR=

cleanup() {
  local d
  for d in "${TMPDIRS[@]}"; do rm -rf -- "$d"; done
}
trap cleanup EXIT

ok()     { PASS=$((PASS + 1)); printf '  ok   %s\n' "$1"; }
ko()     { FAIL=$((FAIL + 1)); printf '  FAIL %s\n' "$1"; }
# check <libellé> <code> : ok si le code vaut 0.
check()  { if [ "$2" -eq 0 ]; then ok "$1"; else ko "$1"; fi; }
# refute <libellé> <code> : ok si le code est non nul (échec attendu).
refute() { if [ "$2" -ne 0 ]; then ok "$1"; else ko "$1"; fi; }

# Copie jetable du dépôt, sans .git ni artefacts générés.
# Appelée SANS substitution de commande : elle publie son résultat dans les
# globales $COPY_DIR et $TMPDIRS (une substitution la ferait tourner dans un
# sous-shell, où l'enregistrement du dossier temporaire serait perdu).
copy_repo() {
  local d
  d=$(mktemp -d "${TMPDIR:-/tmp}/creno-ext-test.XXXXXX")
  TMPDIRS+=("$d")
  tar -C "$ROOT" \
    --exclude=.git --exclude=dist --exclude='extension/index.html' \
    --exclude='extension/app.js' --exclude='extension/styles.css' \
    --exclude='extension/icons' -cf - . | tar -xf - -C "$d"
  COPY_DIR=$d
}

# Lance le script de la copie $1 avec les arguments suivants.
build_in() {
  local repo=$1
  shift
  bash "$repo/tools/build-extension.sh" "$@"
}

echo "== 1. Build nominal + charge utile =="
copy_repo
C1=$COPY_DIR
OUT1=$(build_in "$C1" 2>&1)
rc=$?; check "build exit 0" "$rc"
printf '%s\n' "$OUT1" | grep -qE '^sha256 : [0-9a-f]{64}$'
rc=$?; check "résumé sha256 imprimé" "$rc"
Z1="$C1/dist/creno-extension.zip"
[ -f "$Z1" ]
rc=$?; check "zip créé" "$rc"
LIST1=$(unzip -Z1 "$Z1" 2>/dev/null | sort)
rc=$?
EXPECTED=$'app.js\nbackground.js\nicons/icon-192.png\nicons/icon-512.png\nindex.html\nmanifest.json\nstyles.css'
[ "$rc" -eq 0 ] && [ "$LIST1" = "$EXPECTED" ]
rc=$?; check "liste des entrées exacte (7 fichiers)" "$rc"
printf '%s\n' "$LIST1" | grep -qxE '(sw\.js|manifest\.webmanifest|README\.md)'
rc=$?; refute "aucun fichier interdit" "$rc"
grep -q 'manifest.webmanifest' "$C1/extension/index.html"
rc=$?; refute "index.html généré sans manifest.webmanifest" "$rc"
grep -qE 'serviceWorker|sw\.js' "$C1/extension/app.js"
rc=$?; refute "app.js généré sans service worker" "$rc"
unzip -p "$Z1" manifest.json | python3 -c 'import json,sys; m=json.load(sys.stdin); assert m["version"]=="1.1.0" and m["manifest_version"]==2'
rc=$?; check "manifeste embarqué v1.1.0 / MV2" "$rc"

echo "== 2. Reproductibilité (deux builds) =="
build_in "$C1" >/dev/null 2>&1
rc1=$?
SHA_A=$(sha256sum "$Z1" 2>/dev/null | cut -d' ' -f1)
build_in "$C1" >/dev/null 2>&1
rc2=$?
SHA_B=$(sha256sum "$Z1" 2>/dev/null | cut -d' ' -f1)
[ "$rc1" -eq 0 ] && [ "$rc2" -eq 0 ] && [ -n "$SHA_A" ] && [ "$SHA_A" = "$SHA_B" ]
rc=$?; check "sha256 identique sur deux builds ($SHA_A)" "$rc"

echo "== 3. --check nominal =="
build_in "$C1" --check >/dev/null 2>&1
rc=$?; check "--check exit 0 sur un zip valide" "$rc"

echo "== 4. --check sans zip =="
copy_repo
C4=$COPY_DIR
build_in "$C4" --check >/dev/null 2>&1
rc=$?; refute "--check échoue sans zip" "$rc"

echo "== 5. --check avec zip altéré =="
copy_repo
C5=$COPY_DIR
build_in "$C5" >/dev/null 2>&1
printf 'x' > "$C5/extension/README.md"
( cd "$C5/extension" && zip -q "$C5/dist/creno-extension.zip" README.md ) >/dev/null 2>&1
build_in "$C5" --check >/dev/null 2>&1
rc=$?; refute "--check échoue si le zip contient un fichier en trop" "$rc"

echo "== 6. Marqueur HTML absent =="
copy_repo
C6=$COPY_DIR
sed -i '/<link rel="manifest" href="manifest.webmanifest">/d' "$C6/index.html"
ERR=$(build_in "$C6" 2>&1)
rc=$?; refute "build échoue si le lien manifest PWA est absent" "$rc"
printf '%s\n' "$ERR" | grep -q 'marqueur'
rc=$?; check "message d'erreur explicite (marqueur)" "$rc"

echo "== 7. Marqueur service worker absent =="
copy_repo
C7=$COPY_DIR
sed -i '/\/\* PWA : enregistrement du service worker/d' "$C7/app.js"
build_in "$C7" >/dev/null 2>&1
rc=$?; refute "build échoue si le bloc service worker est absent" "$rc"

echo "== 8. Icône déclarée absente =="
copy_repo
C8=$COPY_DIR
rm -f "$C8/icons/icon-512.png"
ERR=$(build_in "$C8" 2>&1)
rc=$?; refute "build échoue si une icône déclarée manque" "$rc"
printf '%s\n' "$ERR" | grep -q 'icône déclarée absente'
rc=$?; check "message d'erreur explicite (icône)" "$rc"

echo "== 9. Budget de taille (limite) =="
copy_repo
C9=$COPY_DIR
# Données incompressibles : du texte répétitif passerait sous le budget une
# fois déflaté, et le cas ne testerait plus rien.
head -c 400000 /dev/urandom | base64 >> "$C9/styles.css"
ERR=$(build_in "$C9" 2>&1)
rc=$?; refute "build échoue au-delà de 250 Kio" "$rc"
printf '%s\n' "$ERR" | grep -q 'budget dépassé'
rc=$?; check "message d'erreur explicite (budget)" "$rc"
[ ! -f "$C9/dist/creno-extension.zip" ]
rc=$?; check "zip non conservé quand le budget est dépassé" "$rc"

echo "== 10. --clean =="
copy_repo
C10=$COPY_DIR
build_in "$C10" >/dev/null 2>&1
build_in "$C10" --clean >/dev/null 2>&1
rc=$?; check "--clean exit 0" "$rc"
[ ! -d "$C10/dist" ] && [ ! -f "$C10/extension/index.html" ] && [ ! -d "$C10/extension/icons" ]
rc=$?; check "--clean supprime dist/ et les copies générées" "$rc"
[ -f "$C10/extension/manifest.json" ] && [ -f "$C10/extension/background.js" ]
rc=$?; check "--clean préserve les sources committées" "$rc"

echo
printf 'tests: %d réussis, %d échoués\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
