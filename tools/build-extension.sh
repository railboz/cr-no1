#!/usr/bin/env bash
# =============================================================================
# Créno — construction reproductible de l'extension Firefox (Manifest V2)
#
#   bash tools/build-extension.sh            # assemble extension/ + zip
#   bash tools/build-extension.sh --check    # vérifie le zip déjà produit
#   bash tools/build-extension.sh --clean    # supprime les artefacts générés
#
# Source unique de vérité : la racine du dépôt (index.html, app.js, styles.css,
# icons/). Rien n'est figé en cache : les copies sont refaites à chaque appel.
# Les fichiers générés dans extension/ sont ignorés par git (.gitignore).
#
# Dépendances : bash, python3, zip, unzip, sha256sum (tous usuels sur Linux).
# Sortie : dist/creno-extension.zip (déterministe : -X, ordre trié, mtime figé).
# =============================================================================

set -euo pipefail
export LC_ALL=C
export TZ=UTC

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd -P)

EXT_DIR="$ROOT/extension"
DIST_DIR="$ROOT/dist"
ZIP_PATH="$DIST_DIR/creno-extension.zip"
ZIP_NAME="creno-extension.zip"
STAMP="198001010000.00"          # mtime figé de toutes les entrées du zip
BUDGET_BYTES=256000              # 250 Kio — budget dur du zip
MODE=${1:-build}

die()  { printf 'build-extension: erreur: %s\n' "$*" >&2; exit 1; }
note() { printf '%s\n' "$*"; }

usage() {
  cat <<'EOF'
Usage: bash tools/build-extension.sh [build|--check|--clean|--help]

  build (défaut)  Assemble extension/ depuis la racine du dépôt puis écrit
                  dist/creno-extension.zip (déterministe).
  --check         Vérifie le zip existant : manifeste, fichiers référencés,
                  liste exacte des entrées, fichiers interdits, budget de taille.
  --clean         Supprime extension/{index.html,app.js,styles.css,icons/} et dist/.
  --help          Affiche cette aide.
EOF
}

case "$MODE" in
  build|--check|check|-c|--clean|clean|--help|-h|help) ;;
  *) usage >&2; die "argument inconnu : $MODE" ;;
esac
if [ "$MODE" = "--help" ] || [ "$MODE" = "-h" ] || [ "$MODE" = "help" ]; then
  usage; exit 0
fi

for bin in python3 zip unzip sha256sum; do
  command -v "$bin" >/dev/null 2>&1 || die "$bin est requis"
done

# --- Transformations (documentées, vérifiées) --------------------------------
# 1. index.html : suppression de la seule ligne
#      <link rel="manifest" href="manifest.webmanifest">
#    (le manifeste PWA est inutile et nuisible sous moz-extension://).
#    Échec si le marqueur n'apparaît pas exactement une fois, ou s'il reste
#    une référence à manifest.webmanifest / sw.js.
# 2. app.js : suppression du bloc d'enregistrement du service worker
#      /* PWA : enregistrement du service worker … */ … }
#    Échec si le marqueur est absent, si le bloc ne contient pas
#    navigator.serviceWorker.register, ou s'il reste serviceWorker/sw.js.
transform() {
  python3 - "$ROOT" "$1" <<'PY'
import pathlib, sys

root = pathlib.Path(sys.argv[1])
stage = pathlib.Path(sys.argv[2])

MANIFEST_LINK = '<link rel="manifest" href="manifest.webmanifest">'
SW_COMMENT = '/* PWA : enregistrement du service worker'

def read(src):
    return (root / src).read_text(encoding='utf-8')

def write(name, text):
    (stage / name).write_text(text, encoding='utf-8')

# --- index.html ---
html = read('index.html')
lines = html.splitlines(keepends=True)
hits = [i for i, l in enumerate(lines) if l.strip() == MANIFEST_LINK]
if len(hits) != 1:
    sys.exit("index.html : marqueur <link rel=manifest> attendu 1 fois, trouvé %d" % len(hits))
del lines[hits[0]]
html_out = ''.join(lines)
for forbidden in ('manifest.webmanifest', 'sw.js'):
    if forbidden in html_out:
        sys.exit("index.html : référence %r encore présente après transformation" % forbidden)
write('index.html', html_out)

# --- app.js ---
app = read('app.js')
lines = app.splitlines(keepends=True)
starts = [i for i, l in enumerate(lines) if l.startswith(SW_COMMENT)]
if len(starts) != 1:
    sys.exit("app.js : marqueur service worker attendu 1 fois, trouvé %d" % len(starts))
start = starts[0]
end = None
for i in range(start, len(lines)):
    if lines[i].rstrip('\n') == '}':
        end = i
        break
if end is None:
    sys.exit("app.js : fin du bloc service worker introuvable")
block = ''.join(lines[start:end + 1])
if 'navigator.serviceWorker.register' not in block:
    sys.exit("app.js : le bloc supprimé ne contient pas serviceWorker.register")
del lines[start:end + 1]
app_out = ''.join(lines)
for forbidden in ('serviceWorker', 'sw.js'):
    if forbidden in app_out:
        sys.exit("app.js : référence %r encore présente après transformation" % forbidden)
write('app.js', app_out)
PY
}

# Vérifie que chaque icône déclarée existe et que ses dimensions réelles
# correspondent à la taille annoncée dans le manifeste.
icon_check() {
  local iconfile=$1
  python3 - "$ROOT/$iconfile" <<'PY'
import struct, sys, pathlib
p = pathlib.Path(sys.argv[1])
if not p.is_file():
    sys.exit("icône absente : %s" % p)
b = p.read_bytes()
if b[:8] != b'\x89PNG\r\n\x1a\n':
    sys.exit("%s n'est pas un PNG" % p)
w, h = struct.unpack('>II', b[16:24])
print(f"{w}x{h}")
PY
}

# Assemble la charge utile complète dans le dossier $1 (créé/vidé par l'appelant).
assemble() {
  local stage=$1
  mkdir -p "$stage"

  # Fichiers de l'app, copiés depuis la racine À CHAQUE build (aucun cache).
  for f in index.html app.js styles.css; do
    [ -f "$ROOT/$f" ] || die "fichier source absent : $f"
  done
  transform "$stage"
  cp "$ROOT/styles.css" "$stage/styles.css"

  # Sources committées de l'extension (déjà en place quand $stage == extension/).
  for f in manifest.json background.js; do
    [ -f "$EXT_DIR/$f" ] || die "source d'extension absente : extension/$f"
    if [ "$stage" != "$EXT_DIR" ]; then cp "$EXT_DIR/$f" "$stage/$f"; fi
  done

  # Icônes : exactement celles référencées par le manifeste.
  local icons
  icons=$(python3 - "$EXT_DIR/manifest.json" <<'PY'
import json, sys
m = json.load(open(sys.argv[1], encoding='utf-8'))
paths = []
paths += list(m.get('icons', {}).values())
bi = m.get('browser_action', {}).get('default_icon')
if isinstance(bi, str):
    paths.append(bi)
elif isinstance(bi, dict):
    paths += list(bi.values())
seen = []
for p in paths:
    if p not in seen:
        seen.append(p)
print('\n'.join(seen))
PY
)
  [ -n "$icons" ] || die "manifeste : aucune icône déclarée"
  local icon
  while IFS= read -r icon; do
    [ -f "$ROOT/$icon" ] || die "icône déclarée absente du dépôt : $icon"
    mkdir -p "$stage/$(dirname "$icon")"
    cp "$ROOT/$icon" "$stage/$icon"
  done <<< "$icons"

  # Modes stables (indépendants de l'umask) pour un zip reproductible.
  find "$stage" -type d -exec chmod 755 {} +
  find "$stage" -type f -exec chmod 644 {} +
}

# Charge utile exacte du zip (liste blanche triée) : manifeste, background,
# app transformée et icônes référencées. Tout le reste d'extension/ (README.md,
# fichiers parasites) est exclu par construction.
payload_list() {
  python3 - "$1" <<'PY'
import json, pathlib, sys

stage = pathlib.Path(sys.argv[1])
m = json.loads((stage / 'manifest.json').read_text(encoding='utf-8'))
files = ['manifest.json', 'background.js', 'index.html', 'app.js', 'styles.css']
files += list(m.get('icons', {}).values())
bi = m.get('browser_action', {}).get('default_icon')
if isinstance(bi, str):
    files.append(bi)
elif isinstance(bi, dict):
    files += list(bi.values())
files += list(m.get('background', {}).get('scripts', []))
files = sorted(set(files))
missing = [f for f in files if not (stage / f).is_file()]
if missing:
    sys.exit("fichiers de charge utile absents : %s" % ', '.join(missing))
print('\n'.join(files))
PY
}

write_zip() {
  local stage=$1 out=$2
  local tmp="$out.tmp.$$" list="$out.list.$$"
  payload_list "$stage" > "$list"
  ( cd "$stage"
    find . -type f -exec touch -t "$STAMP" {} +
    rm -f "$tmp"
    zip -X -q -9 "$tmp" -@ < "$list"
  )
  rm -f "$list"
  mv -f "$tmp" "$out"
}

manifest_asserts() {
  python3 - "$1" <<'PY'
import json, sys, zipfile, struct, io

zp = sys.argv[1]
z = zipfile.ZipFile(zp)
names = z.namelist()

m = json.loads(z.read('manifest.json').decode('utf-8'))

def fail(msg):
    sys.exit("assertion échouée : " + msg)

if m.get('manifest_version') != 2: fail('manifest_version != 2')
if m.get('version') != '1.1.0': fail('version != 1.1.0')
perms = m.get('permissions', [])
if perms != ['tabs', 'https://api.skello.io/*']: fail('permissions inattendues : %r' % perms)
for p in perms:
    if 'allorigins' in p or 'corsproxy' in p or 'codetabs' in p:
        fail('relais public encore autorisé : %r' % p)
if m.get('background', {}).get('persistent') is not False: fail('background.persistent != false')
if m['background'].get('scripts') != ['background.js']: fail('background.scripts inattendu')
csp = m.get('content_security_policy')
if csp != "script-src 'self'; object-src 'none';": fail('CSP absente ou inattendue : %r' % csp)
gecko = m.get('browser_specific_settings', {}).get('gecko', {})
if gecko.get('id') != 'creno@loganbozier.fr': fail('gecko.id inattendu')
if gecko.get('strict_min_version') != '115.0': fail('strict_min_version inattendu')
if gecko.get('data_collection_permissions', {}).get('required') != ['none']:
    fail('data_collection_permissions.required != ["none"]')

# Tous les fichiers référencés par le manifeste existent dans le zip.
refs = []
for size, path in m.get('icons', {}).items():
    refs.append((path, int(size)))
bi = m.get('browser_action', {}).get('default_icon')
if isinstance(bi, str):
    refs.append((bi, None))
elif isinstance(bi, dict):
    refs += [(p, int(s)) for s, p in bi.items()]
refs += [(p, None) for p in m['background']['scripts']]
refs += [('index.html', None), ('app.js', None), ('styles.css', None)]

for path, size in refs:
    if path not in names:
        fail('fichier référencé absent du zip : %s' % path)
    if path.endswith('.png') and size is not None:
        b = z.read(path)
        if b[:8] != b'\x89PNG\r\n\x1a\n': fail('%s non-PNG' % path)
        w, h = struct.unpack('>II', b[16:24])
        if (w, h) != (size, size):
            fail('%s : dimensions %dx%d != %d déclaré' % (path, w, h, size))

# Pas de PWA dans l'extension : ni manifeste web, ni service worker.
html = z.read('index.html').decode('utf-8')
app = z.read('app.js').decode('utf-8')
if 'manifest.webmanifest' in html or 'sw.js' in html:
    fail('index.html généré référence encore la PWA')
if 'serviceWorker' in app or 'sw.js' in app:
    fail('app.js généré enregistre encore le service worker')

print("manifeste OK : MV%s v%s, id=%s, permissions=%s"
      % (m['manifest_version'], m['version'], gecko['id'], ','.join(perms)))
PY
}

do_clean() {
  rm -rf -- "$DIST_DIR"
  rm -f -- "$EXT_DIR/index.html" "$EXT_DIR/app.js" "$EXT_DIR/styles.css"
  rm -rf -- "$EXT_DIR/icons"
  note "Artefacts supprimés : dist/, extension/{index.html,app.js,styles.css,icons/}."
}

do_build() {
  local stage=$EXT_DIR
  rm -f -- "$EXT_DIR/index.html" "$EXT_DIR/app.js" "$EXT_DIR/styles.css"
  rm -rf -- "$EXT_DIR/icons"
  assemble "$stage"
  mkdir -p "$DIST_DIR"
  write_zip "$stage" "$ZIP_PATH"

  local count bytes sha
  count=$(unzip -Z1 "$ZIP_PATH" | wc -l | tr -d ' ')
  bytes=$(stat -c %s "$ZIP_PATH")
  if [ "$bytes" -gt "$BUDGET_BYTES" ]; then
    rm -f -- "$ZIP_PATH"
    die "budget dépassé : $bytes > $BUDGET_BYTES octets (zip supprimé)"
  fi
  sha=$(sha256sum "$ZIP_PATH" | cut -d' ' -f1)
  note "Créno extension : $count fichiers, $bytes octets → dist/$ZIP_NAME"
  note "sha256 : $sha"
}

do_check() {
  [ -f "$ZIP_PATH" ] || die "aucun zip à vérifier (lancer d'abord : bash tools/build-extension.sh)"
  [ -f "$EXT_DIR/manifest.json" ] || die "extension/manifest.json absent"
  [ -f "$EXT_DIR/background.js" ] || die "extension/background.js absent"

  # 1) Le manifeste source est un JSON valide et l'extension committée aussi.
  python3 -c "import json,sys; json.load(open(sys.argv[1], encoding='utf-8'))" \
    "$EXT_DIR/manifest.json" || die "extension/manifest.json n'est pas un JSON valide"
  note "manifeste source : JSON valide"

  # 2) Le zip contient exactement les fichiers attendus, rien de plus.
  local tmp expected actual
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/creno-ext-check.XXXXXX")
  # shellcheck disable=SC2064
  trap "rm -rf -- '$tmp'" EXIT
  assemble "$tmp"
  expected=$(payload_list "$tmp")
  actual=$(unzip -Z1 "$ZIP_PATH" | sort)

  if [ "$expected" != "$actual" ]; then
    printf 'attendu :\n%s\nobtenu :\n%s\n' "$expected" "$actual" >&2
    die "la liste des fichiers du zip diffère de l'attendu (zip périmé ? relancer le build)"
  fi
  note "liste du zip conforme ($(printf '%s\n' "$expected" | wc -l | tr -d ' ') fichiers)"

  # 3) Fichiers interdits.
  local f
  for f in sw.js manifest.webmanifest README.md build-extension.sh .gitignore; do
    if printf '%s\n' "$actual" | grep -qxF "$f"; then
      die "fichier interdit dans le zip : $f"
    fi
  done
  note "aucun fichier interdit (sw.js, manifest.webmanifest, README.md)"

  # 4) Timestamps figés (reproductibilité) et absence de métadonnées -X.
  python3 - "$ZIP_PATH" <<'PY' || die "horodatages du zip non figés"
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
bad = [i.filename for i in z.infolist() if i.date_time != (1980, 1, 1, 0, 0, 0)]
if bad:
    sys.exit("mtime non figé : %r" % bad)
PY
  note "horodatages figés (1980-01-01 00:00:00)"

  # 5) Manifeste embarqué + fichiers référencés + PWA absente.
  manifest_asserts "$ZIP_PATH"

  # 6) Budget de taille.
  local bytes
  bytes=$(stat -c %s "$ZIP_PATH")
  [ "$bytes" -le "$BUDGET_BYTES" ] || die "budget dépassé : $bytes > $BUDGET_BYTES octets"
  note "budget de taille respecté : $bytes ≤ $BUDGET_BYTES octets"

  # 7) Les copies générées ne doivent pas apparaître dans git.
  if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
    for f in extension/index.html extension/app.js extension/styles.css extension/icons/icon-192.png dist/creno-extension.zip; do
      git -C "$ROOT" check-ignore -q "$f" || die "$f n'est pas ignoré par .gitignore"
    done
    note "artefacts générés bien ignorés par git"
  fi

  local sha
  sha=$(sha256sum "$ZIP_PATH" | cut -d' ' -f1)
  note "sha256 : $sha"
  note "Vérification OK."
}

case "$MODE" in
  build) do_build ;;
  --clean|clean) do_clean ;;
  --check|check|-c) do_check ;;
esac
