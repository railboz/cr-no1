# Créno — extension Firefox

**Manifest V2**, version 1.1.1 (Firefox 121+, `strict_min_version: "121.0"` pour `:has()`) : ouvrir l'app Créno dans un
onglet, avec un périmètre réseau minimal — l'app elle-même, plus
`https://api.skello.io/*` pour la sync ICS directe.

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
donnée de planning n'est envoyée ailleurs par l'extension. Le relais privé est
**optionnel** ; le champ « Relais privé » peut rester vide.

Ordre des tentatives, à connaître avant de configurer un relais :

- **Relais privé configuré** (`relayUrl` renseigné) : `direct` → relais privé, et
  **arrêt**. Aucun relais public n'est contacté, même avec `allowProxy` activé
  (`allowProxy` ne joue alors aucun rôle).
- **Relais configuré mais en échec** (injoignable, erreur HTTP, réponse non-ICS)
  ou **invalide** (ex. `http://` hors loopback) : la chaîne s'arrête là — si
  `direct` échoue aussi, une erreur explicite s'affiche. Jamais de repli public,
  donc aucune fuite vers un tiers.
- **Aucun relais configuré** (`relayUrl` vide) : `direct`, puis relais publics
  (allorigins, corsproxy, codetabs) **uniquement si `allowProxy` est activé**.
  C'est le seul cas où un tiers voit l'URL Skello (et son jeton) et l'ICS ; la
  laisser désactivée garantit qu'aucune donnée ne sort.

L'extension ne déclare aucune permission d'hôte vers ces relais publics : leur
usage dépend de leurs en-têtes CORS, pas d'un accès privilégié accordé à
l'extension. Si le relais est le backend Créno, il faut y autoriser l'origine
`moz-extension://` avec `CRENO_ALLOW_EXTENSION=1`, sinon la requête est bloquée
par le navigateur.

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
`index.html`, `app.js`, les 9 feuilles `styles/*.css` et les icônes depuis la racine, retire
le lien `manifest.webmanifest` et le bloc service worker, puis écrit un zip
déterministe (`-X`, entrées triées, mtime 1980-01-01) + son sha256, cap 250 Kio.
Le paquet contient **15 fichiers** : `manifest.json`, `background.js`, `index.html`,
`app.js`, les 9 CSS sous `styles/` (`tokens`, `base`, `layout`, `components`,
`planning`, `mobile`, `screens`, `dicta`, `utilities`) et `icons/icon-192.png` +
`icons/icon-512.png`. Les feuilles embarquées sont exactement celles liées par
`index.html`, dans le même ordre.
`--check` vérifie le zip **existant** : le lancer après un build (sur un
checkout neuf, `dist/` n'existe pas).

**Sortie** : `dist/creno-extension.zip`. Ignorés par git (générés) :
`extension/{index.html,app.js,styles/*.css,icons/}`, `dist/`. Committés :
`extension/{manifest.json,background.js}`, ce README et `tools/` (build + 22
assertions en 10 sections via `bash tools/tests/test-build-extension.sh`).

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
