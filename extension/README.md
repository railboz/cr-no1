# Créno — extension Firefox

**Manifest V2** (Firefox 115+) : ouvrir l'app Créno dans un onglet, avec la
permission `https://api.skello.io/*` → sync ICS Skello directe et privée, sans
relais public, sans collecte de données.

## Construire

```bash
bash tools/build-extension.sh            # assemble extension/ + dist/creno-extension.zip
bash tools/build-extension.sh --check    # vérifie le zip produit
bash tools/build-extension.sh --clean    # supprime les artefacts générés
```

Prérequis : `bash`, `python3`, `zip`, `unzip`, `sha256sum`. Le script recopie
`index.html`, `app.js`, `styles.css` et les icônes depuis la racine, retire le
lien `manifest.webmanifest` et le bloc service worker, puis écrit un zip
déterministe (`-X`, entrées triées, mtime 1980-01-01) + son sha256, cap 250 Kio.

**Sortie** : `dist/creno-extension.zip`. Ignorés par git (générés) :
`extension/{index.html,app.js,styles.css,icons/}`, `dist/`. Committés :
`extension/{manifest.json,background.js}`, ce README et `tools/` (build + 23
assertions via `bash tools/tests/test-build-extension.sh`).

## Tester dans Firefox

`about:debugging#/runtime/this-firefox` → **Charger un module temporaire…** →
`extension/manifest.json` ; l'icône ouvre Créno et le module expire au redémarrage.

## Signer (AMO, self-distribution non listée)

<https://addons.mozilla.org/developers/> : « Soumettre un nouveau module » → **Sur
votre propre serveur** (unlisted) → envoyer `dist/creno-extension.zip`. Le `.xpi`
signé revient quasi instantanément ; garder zip + sha256. Firefox Android : À
propos de Firefox → 5 taps sur le logo → Installer depuis un fichier.

## Ancienne extension

`~/Documents/ia/creno-extension` (hors git) est **retirée** : ne plus la modifier
ni la synchroniser ; ce dépôt reste la seule source de vérité.
