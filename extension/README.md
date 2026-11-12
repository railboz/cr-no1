# Créno — extension Firefox

**Manifest V2** (Firefox 115+) : ouvrir l'app Créno dans un onglet, avec un
périmètre réseau minimal — l'app elle-même, plus `https://api.skello.io/*` pour
la sync ICS directe.

## Permissions demandées

`extension/manifest.json` ne déclare que deux entrées :

| Permission | Usage | Avertissement à l'installation |
| --- | --- | --- |
| `storage` | mémoriser l'identifiant de l'onglet Créno (zone `session`, repli `local`) afin de le réactiver au clic suivant | aucun (permission d'API, pas d'accès aux données) |
| `https://api.skello.io/*` | lire l'ICS Skello en direct | accès aux données de ce seul hôte |

Pas de `tabs`, pas de `<all_urls>`, donc **pas** d'avertissement « Lire votre
historique de navigation » : réutiliser l'onglet existant ne passe plus par
`browser.tabs.query({ url })` (qui exige `tabs`) mais par un identifiant
mémorisé, relu avec `browser.tabs.get` puis réactivé avec `browser.tabs.update`.
Si l'onglet a été fermé, un nouvel onglet est créé et son identifiant remplace
l'ancien.

## Confidentialité

La synchronisation part **directement** vers `https://api.skello.io` : aucune
donnée de planning n'est envoyée ailleurs par l'extension.

Nuance sur les relais, à connaître avant d'activer l'option : l'app essaie les
routes dans l'ordre direct → relais privé (si une URL est configurée) → relais
publics (allorigins, corsproxy, codetabs) **si `allowProxy` est activé**.

- `allowProxy` désactivé : jamais de relais public, quel que soit le réglage du
  relais privé. Seuls le direct et, s'il est configuré, le relais privé sont
  contactés.
- `allowProxy` activé et relais privé qui répond correctement : l'ICS arrive du
  relais privé, les relais publics ne sont jamais contactés.
- `allowProxy` activé sans relais privé, ou avec un relais privé qui échoue
  (erreur HTTP, réponse non-ICS) : l'app retombe sur les relais publics, et
  l'URL Skello + le contenu de l'ICS transitent alors par un tiers. Cette option
  n'existe que pour les réseaux qui bloquent l'accès direct ; la laisser
  désactivée garantit qu'aucune donnée ne sort vers un relais public.

L'extension ne déclare aucune permission d'hôte vers ces relais publics : leur
usage dépend de leurs en-têtes CORS, pas d'un accès privilégié accordé à
l'extension.

## Limites dans l'extension

La **dictée vocale est indisponible** : elle repose sur l'API Web Speech
(`SpeechRecognition`), absente de Firefox. L'app détecte le contexte extension
et masque le bouton micro (aucune erreur, raccourci Ctrl+Shift+M laissé au
navigateur). Le reste de l'app est identique à la version web.

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
`--check` vérifie le zip **existant** : le lancer après un build (sur un
checkout neuf, `dist/` n'existe pas).

**Sortie** : `dist/creno-extension.zip`. Ignorés par git (générés) :
`extension/{index.html,app.js,styles.css,icons/}`, `dist/`. Committés :
`extension/{manifest.json,background.js}`, ce README et `tools/` (build + 26
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
