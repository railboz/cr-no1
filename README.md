# 🗓️ Créno — votre planning, simplement

Alternative légère à Skello, en HTML/CSS/JS pur : vos shifts et absences restent dans le `localStorage` de votre navigateur, sans compte et sans base de données. Aucun serveur à installer pour l'utiliser : ouvrez `index.html` et c'est parti. Deux fonctions optionnelles sortent de l'appareil, et seulement celles-là : la dictée vocale (service de reconnaissance du navigateur) et la synchro Skello (l'URL de votre calendrier, récupérée en direct ou via un relais que vous choisissez).

## ✨ Fonctionnalités

- **Planning hebdomadaire** : grille lun → dim avec vos établissements, blocs de shift colorés (horaires, durée, poste), colonne du jour mise en avant.
- **Établissements multiples** : ajout, renommage, couleur personnalisée, suppression (avec ses shifts).
- **Shifts** : création en cliquant sur une case, édition en cliquant sur un bloc, notes, labels suggérés (DRIVE, CAISSE…).
- **Absences** : congés, RTT, maladie, formation… affichées en pointillés dans le planning et listées dans l'onglet dédié.
- **Statistiques** : heures par semaine (8 semaines), répartition par établissement et par poste, total du mois.
- **Ajouts originaux** :
  - 💶 **Rémunération estimée** (taux horaire réglable dans le profil) et barre d'**objectif hebdomadaire** ;
  - ⚠️ **Disponibilités habituelles** : alerte si vous planifiez un shift un jour habituellement indisponible ;
  - ⌨️ **Raccourcis clavier** : `←` / `→` changer de semaine, `N` nouveau shift ;
  - 📥 **Glisser-déposer** d'un fichier JSON n'importe où dans la fenêtre pour l'importer ;
  - 🎙️ **Dictaphone** : dictez vos notes au micro, le texte s'écrit au curseur (voir [Dictaphone](#-dictaphone-optionnel)) — le son est traité par le service vocal du navigateur, Créno n'enregistre rien.
- **🔒 Relais privé (optionnel)** : un petit service Rust à héberger vous-même pour que l'URL de votre calendrier Skello ne transite par aucun relais public (voir [Relais privé](#-relais-privé-optionnel)).
- **🦊 Extension Firefox dans le dépôt** : sources versionnées dans `extension/`, paquet reconstruit avec `bash tools/build-extension.sh` → `dist/creno-extension.zip`.

## 🎙️ Dictaphone (optionnel)

Dictez vos notes au lieu de les taper : un bouton **micro** en bas à droite de l'écran démarre la dictée, une pastille affiche l'état (écoute, transcription, erreur) et la durée. Quand une fenêtre est ouverte, le même bouton et la même pastille y sont placés, au-dessus de son contenu : vous les voyez toujours et un appui suffit pour arrêter.

- **Insertion au curseur** : le texte reconnu s'écrit dans le champ de saisie sélectionné (notes de shift, noms, profil), sans jamais être envoyé ailleurs. Dans les formulaires de shift et d'absence, vous relisez puis validez avec leur bouton **Enregistrer** ; les champs du profil, eux, sont enregistrés automatiquement dès qu'ils sont remplis — relisez-les avant d'y dicter.
- **Raccourci** : `Ctrl+Maj+M` (`Cmd+Maj+M` sur macOS) pour démarrer et arrêter, y compris dans les fenêtres.
- **5 minutes maximum** par dictée : la reconnaissance peut s'interrompre avant la fin (relancez la dictée), le texte reste modifiable.
- Les champs numériques, les dates, les heures et les listes déroulantes ne sont pas des cibles de dictée.
- **Lecteurs d'écran** : l'état (démarrage, arrêt, erreur) est annoncé une fois ; le minuteur et la transcription en cours ne sont pas lus.

> ⚠️ **Vie privée** : le son quitte l'appareil. Le navigateur envoie l'audio à son propre service de reconnaissance vocale (Google, Microsoft ou Apple selon le navigateur). Créno n'enregistre rien, ne transmet rien et ne stocke aucun fichier audio : seul le texte reconnu est écrit dans votre champ, puis conservé dans le `localStorage` comme le reste de vos données.

### Compatibilité

| Navigateur | Dictée |
| --- | --- |
| Chromium, Edge, Safari récents | ✅ disponible |
| Firefox (bureau ou Android) | ❌ non disponible : pas d'API `SpeechRecognition` |
| Version extension Firefox | ❌ non disponible |

Une connexion internet est nécessaire. Hors ligne (ou sans réseau), la dictée démarre puis échoue avec le message « Connexion requise pour la transcription » ; le reste de l'application continue de fonctionner normalement. Si le navigateur ne prend pas la dictée en charge, le bouton est masqué.

### Si la dictée ne marche pas

- **Permission micro refusée** : réautorisez le microphone dans les réglages du site du navigateur, puis rechargez la page.
- **Message « Rien entendu »** : rapprochez-vous du micro et parlez après le démarrage de la dictée.

## 💾 Sauvegarde & transfert

- Les données sont **sauvegardées automatiquement** dans le `localStorage` du navigateur (indicateur « Sauvegarde locale » en haut à droite).
- **Export JSON** : menu avatar → « Exporter ». Génère `creno-export-AAAA-MM-JJ.json`.
- **Import JSON** : menu avatar → « Importer », ou glissez-déposez le fichier sur la page. Les données actuelles sont remplacées (confirmation demandée).

## 📅 Synchroniser avec Skello

Skello publie votre planning au format calendrier (ICS) — plus besoin de rien retaper.

### Synchronisation automatique ⚡

1. Dans Skello : **Mon profil → Synchroniser le calendrier**, copiez l'URL affichée.
2. Dans Créno : menu avatar → **« 📅 Importer depuis Skello (ICS) »** → collez l'URL → **Enregistrer**.
3. Un bouton **« ⟳ Sync Skello »** apparaît dans la barre du planning : un clic récupère le planning, ajoute les nouveaux shifts et ignore les doublons.
4. **Synchro automatique au démarrage** : l'app se met à jour toute seule (1 fois toutes les 12 h) tant que l'URL est enregistrée.

> 🔒 **Vie privée** : avec un relais privé, la garantie est nette — l'URL de votre calendrier (qui contient un jeton privé) ne part que vers votre serveur, jamais vers un tiers. Les relais publics ne servent que si **aucun** relais n'est configuré *et* que vous les autorisez dans la fenêtre d'import ; dans ce cas seulement, votre URL transite par un serveur tiers. Pour l'éviter : le [relais privé](#-relais-privé-optionnel), qui garde la synchro automatique en la faisant passer par votre propre serveur, ou le fichier `.ics` téléchargé manuellement.

### Méthodes manuelles

- Ouvrez l'URL dans le navigateur : elle télécharge un fichier `.ics` → glissez-déposez-le dans la fenêtre (ou bouton « Choisir un fichier .ics… »).
- Ou collez directement le contenu du calendrier dans la fenêtre d'import.

Créno convertit chaque créneau horaire en **shift** (l'établissement est reconnu ou créé automatiquement) et chaque journée entière en **absence** (congés, RTT, maladie… selon le libellé). Les shifts déjà présents ne sont jamais importés deux fois.

## 🔒 Relais privé (optionnel)

Un petit service Rust (fourni dans `backend/`) peut aller chercher votre calendrier Skello à votre place : l'URL qui contient votre jeton privé ne transite alors que par votre propre serveur, jamais par un proxy public.

**C'est facultatif.** Sans relais, l'app essaie Skello en direct, puis les relais publics si vous les avez autorisés. Avec un relais, la chaîne est `direct → votre relais` et elle s'arrête là : les relais publics ne sont **jamais** pris en secours. Si le relais échoue (service arrêté, origine non autorisée), l'app affiche une erreur explicite et ne bascule sur aucun relais public.

### Lancer le relais

**Docker (recommandé)** — depuis la racine du dépôt :

```bash
cp deploy/.env.example deploy/.env          # facultatif : les défauts suffisent
docker compose -f deploy/compose.yml up -d
curl -sS http://127.0.0.1:8080/healthz      # {"status":"ok"}
```

L'image (Alpine, binaire statique, uid 10001) tourne sans privilèges : disque racine en lecture seule, toutes les capabilities retirées, `no-new-privileges`, `/tmp` en mémoire, journaux plafonnés (10 Mio × 3 fichiers). Le port n'est publié que sur la boucle locale (`127.0.0.1:8080`) : pour l'exposer, laissez-le ainsi et terminez le TLS devant (`relais.example.fr { reverse_proxy 127.0.0.1:8080 }` avec Caddy — exemple complet dans `backend/README.md`).

**Sans Docker** (Rust 1.88 ou plus) :

```bash
cd backend
cargo run --release                         # → écoute sur http://127.0.0.1:8080
```

Mêmes variables `CRENO_*` dans les deux cas, avec les défauts du code ci-dessous. En Docker, réglez-les dans `deploy/.env` (chargé automatiquement par Compose) : les deux que l'on change le plus sont `CRENO_ALLOWED_ORIGINS` (origine exacte de la page Créno) et `CRENO_ALLOWED_HOSTS`. N'y remettez pas `CRENO_BIND=127.0.0.1:8080` : dans le conteneur l'écoute doit rester `0.0.0.0:8080`, c'est la publication du port qui la limite à la boucle locale. Par défaut le relais ne récupère que sur `api.skello.io`.

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `CRENO_BIND` | `127.0.0.1:8080` | adresse d'écoute (`0.0.0.0:8080` en conteneur) |
| `CRENO_ALLOWED_HOSTS` | `api.skello.io` | hôtes autorisés (suffixes séparés par des virgules) |
| `CRENO_MAX_BYTES` | `2097152` (2 Mio) | taille maximale du calendrier récupéré |
| `CRENO_TIMEOUT_SECS` | `12` | délai maximal côté Skello |
| `CRENO_RATE_LIMIT` | `30` | requêtes par minute et par IP (rafale 10) |
| `CRENO_ALLOW_EXTENSION` | `0` | autorise l'origine `moz-extension://` (extension Firefox) |
| `CRENO_ALLOW_FILE_ORIGIN` | `0` | `1` = accepter l'origine `null` d'une page ouverte en `file://` (refusée par défaut) |
| `CRENO_ALLOWED_ORIGINS` | *(vide)* | origines CORS exactes autorisées en plus de `localhost` |
| `RUST_LOG` | `info` | verbosité des journaux (`debug` pour diagnostiquer) |

`GET /healthz` répond `{"status":"ok"}` : de quoi vérifier que le service tourne.

> ⚠️ **Origine de la page** : le navigateur n'appelle le relais que si l'origine de Créno lui est déclarée. Un serveur local (`http://localhost:8080`) est accepté d'office ; une page ouverte en `file://`, elle, envoie `Origin: null` et n'est acceptée qu'avec `CRENO_ALLOW_FILE_ORIGIN=1`. Pour une page en ligne, ajoutez son origine exacte — `CRENO_ALLOWED_ORIGINS=https://votre-pseudo.github.io` dans `deploy/.env`, ou en préfixe de `cargo run --release` sans Docker.

Contrat d'API, dépannage et durcissement détaillés : `backend/README.md`.

### Activer le relais dans l'app

Menu avatar → **« 📅 Importer depuis Skello (ICS) »** → champ **« Relais privé (optionnel) »** → `https://votre-relais.exemple.fr` → **Enregistrer**. Champ vide : aucun relais n'est utilisé. Seule la boucle locale est acceptée en `http://` (`http://localhost`, `http://127.0.0.1`, `http://[::1]`) ; toute autre adresse `http://` est refusée par l'app (le jeton partirait en clair).

### Ce qu'il faut savoir

- **HTTPS en pratique** : une page servie en HTTPS ne peut pas appeler un relais en `http://` (contenu mixte bloqué). Derrière un reverse proxy, le TLS se termine sur le proxy.
- **Mono-utilisateur** : pas de compte ni d'authentification ; ne l'exposez pas à des inconnus.
- **Rien n'est conservé** : le calendrier est relayé tel quel, sans stockage côté serveur. Seul le navigateur peut le garder 5 minutes (`Cache-Control: private, max-age=300`).
- **Journal** : méthode, chemin (`/ics`), statut et durée uniquement — jamais l'URL cible ni le jeton. Le reverse proxy qui précède doit être réglé de même : son journal d'accès ne doit pas contenir la query (`access_log off;` ou `log_format` sans `$request`/`$args` en nginx, `log` désactivé en Caddy) — détail dans `backend/README.md`, section « Journal d'accès du reverse proxy ».
- **Reverse proxy** : la limite par IP voit l'adresse du proxy, donc tous les clients partagent le même quota ; `X-Forwarded-For` n'est pas pris en compte.

## 🚀 Lancer (desktop)

Ouvrez simplement `index.html` (double-clic). Aucune installation nécessaire.

> ⚠️ Les données restent liées au navigateur et à l'emplacement du fichier. Pensez à exporter un JSON de sauvegarde avant de changer de machine ou de navigateur.

### Fichiers du front

Le style est découpé en 9 feuilles, chargées dans cet ordre par `index.html` :

| Feuille | Rôle |
| --- | --- |
| `styles/tokens.css` | tokens du thème (couleurs, typo, espacements, rayons, motion) |
| `styles/base.css` | reset, canvas, typographie de base, champs natifs |
| `styles/layout.css` | coquille de page, barre du haut, navigation, rail des établissements |
| `styles/components.css` | boutons, champs, cartes, dialogues, toasts |
| `styles/planning.css` | grille de la semaine, blocs de shift, résumé |
| `styles/mobile.css` | vue jour, navigation basse, bottom-sheets |
| `styles/screens.css` | absences, statistiques, profil, établissements |
| `styles/dicta.css` | dictaphone (bouton, pastille) |
| `styles/utilities.css` | utilitaires, focus global, `prefers-reduced-motion` |

L'ordre est contractuel (la cascade en dépend) et `utilities.css` reste en dernier. Le design system complet (tokens, composants, contrat JS → CSS `--c` / `--v`) est décrit dans `docs/DESIGN.md` : c'est la référence à lire avant de toucher au CSS, au HTML ou au JS du front.

## 📲 Installer comme application sur votre téléphone (PWA)

Créno est une **PWA** (Progressive Web App) : depuis une page web, vous l'installez comme une vraie application — **icône dans le tiroir d'applications, ouverture plein écran sans barre de navigateur, fonctionne hors-ligne**. Pas de store, pas de signature, aucune autorisation.

### 1 · Mettre Créno en ligne (une fois, 5 minutes)

L'app a besoin d'être servie en HTTPS au moins une première fois. Option recommandée :

**GitHub Pages** — aucune publicité, aucun badge, rien en bas de l'écran ✨
1. Créez un compte sur [github.com](https://github.com), puis un dépôt public (ex. `creno`).
2. Téléversez le contenu du dossier `creno` (via « Add file → Upload files »).
3. **Settings → Pages → Source : `main` / racine → Save**. Après 1-2 min : `https://votre-pseudo.github.io/creno/`.

> ⚠️ **Si vous passez de Netlify à GitHub Pages** : les données sont liées au domaine. Faites d'abord **⬇️ Exporter (JSON)** depuis la version installée, puis réinstallez depuis la nouvelle URL et **⬆️ Importer (JSON)**.

Alternative : Netlify Drop ([app.netlify.com/drop](https://app.netlify.com/drop), glissez le dossier `creno`) — simple mais affiche un badge « Powered by Netlify » sur la version gratuite.

### 2 · Installer sur le téléphone

Ouvrez l'URL sur votre téléphone, puis :

- **Chrome (Android)** : bandeau « Ajouter Créno à l'écran d'accueil · Installer », ou menu ⋮ → **Installer l'application**.
- **Firefox (Android)** : menu ⋮ → **Installer** (ou « Ajouter à l'écran d'accueil »).

Créno apparaît avec l'icône calendrier dans vos applications, s'ouvre en plein écran et **fonctionne sans internet** (service worker qui met tout en cache). Les mises à jour s'appliquent automatiquement dès que vous rouvrez l'app en ligne.

> 💡 Les données que vous saisissez restent **sur le téléphone** (localStorage) ; seule la dictée vocale, optionnelle, envoie le son au service de reconnaissance du navigateur, et seule la synchro Skello, optionnelle, envoie l'URL de votre calendrier. Pensez à **⬇️ Exporter (JSON)** de temps en temps comme sauvegarde — l'export est compatible entre la version web, la PWA et l'extension Firefox.

### 🔋 Batterie & arrière-plan (Android)

Créno ne fait **rien tourner en arrière-plan** : aucun minuteur, aucun suivi de position, aucune notification, aucune synchronisation périodique. La synchro Skello (si une URL est enregistrée) ne se lance qu'à l'ouverture, **au premier plan et en ligne** ; toute requête en cours est annulée dès que l'app passe en arrière-plan.

Si Android affiche malgré tout une consommation en arrière-plan :

1. **Paramètres → Applications → Créno → Batterie → « Restreindre »** (aucune fonction de l'app n'en dépend) ;
2. balayez l'app hors des applications récentes après usage ;
3. vérifiez que la consommation ne vient pas de Chrome/Firefox lui-même (mises à jour de PWA) ;
4. désinstallez les anciennes installations de test (les données sont liées à l'URL).

Sur mobile, les effets graphiques coûteux (fond fixe, flou GPU) sont désactivés automatiquement pour préserver la batterie.

### Variantes d'installation

- **Sans héberger** : servez le dossier depuis votre PC — depuis la racine du dépôt, `python3 -m http.server 8080` puis `http://localhost:8080` — mais l'app ne sera accessible que si le PC est allumé, et l'installation PWA complète exige HTTPS.
- **Extension Firefox Android** : voir la section suivante (installation par fichier `.xpi` signé).

## 🦊 Extension Firefox (navigateur)

Une version **extension** existe aussi, directement dans ce dépôt : le dossier `extension/` (Manifest V2) fournit l'icône de la barre d'outils et la synchro Skello **directe et privée** (permission réseau limitée à `https://api.skello.io/*`, plus `storage` pour l'onglet de l'extension, sans relais public).

Les fichiers repris de l'app (`extension/index.html`, `app.js`, les 9 feuilles `styles/*.css`, `icons/`) et `dist/` sont générés à chaque construction et ignorés par git ; `extension/manifest.json`, `extension/background.js`, son `README.md` et `tools/` sont versionnés. Détails et reconstruction : `extension/README.md` et la section [Reconstruire l'extension](#reconstruire-lextension) plus bas.

## 📱 Extension Firefox pour téléphone (Android)

Créno existe aussi en **extension Firefox**, pensée d'abord pour le mobile :

- **Navigation en bas de l'écran** (Planning · Absences · Statistiques · Profil) ;
- **Mobile** : vue jour-par-jour par défaut (swipe pour changer de jour), barre de navigation semaine compacte (‹ › , « Aujourd'hui »), bouton **📅 Semaine** qui affiche la liste verticale des 7 jours (devient « Jour » pour revenir), ⟳ Sync dans la ligne de date si une URL Skello est enregistrée ;
- **Modales en bottom-sheet** qui montent du bas de l'écran, zones tactiles larges ;
- **Synchro Skello 100 % privée** : grâce aux permissions de l'extension, la récupération directe de votre calendrier ICS est autorisée — **aucun relais public, aucune donnée qui transite par un tiers**.

### Installer sur votre téléphone

L'installation se fait depuis un fichier `.xpi` **signé par Mozilla** (exigence de Firefox Android, ça prend 5 minutes) :

1. **Créez un compte gratuit** sur [addons.mozilla.org](https://addons.mozilla.org/fr/developers/).
2. Allez dans **« Outils pour développeurs » → « Soumettre un nouveau module »**.
3. Choisissez **« Sur votre propre serveur »** (distribution personnelle, non listée) et envoyez le fichier **`dist/creno-extension.zip`**.
4. Mozilla le signale et vous laisse télécharger le **`.xpi` signé** (la signature est automatique et instantanée pour la distribution non listée).
5. Transférez ce `.xpi` sur votre téléphone, puis dans Firefox Android :
   - **Paramètres → À propos de Firefox**, touchez **5 fois le logo Firefox** (menu développeur caché) ;
   - revenez dans **Paramètres → Installer une extension depuis un fichier**, choisissez le `.xpi` ;
   - validez : Créno apparaît dans vos extensions, le menu ⋮ → **Extensions** permet de l'ouvrir.

> 💡 Astuce : une fois Créno ouvert dans un onglet, utilisez le menu ⋮ de Firefox → « Ajouter à la barre d'outils » l'icône Créno pour l'ouvrir en un geste.

### Récupérer vos données

L'extension a son propre stockage (indépendant du site web). Une seule fois :

1. Sur la version web : menu avatar → **⬇️ Exporter (JSON)** ;
2. Dans l'extension : menu avatar → **⬆️ Importer (JSON)**.

Ensuite, renseignez votre **URL Skello** dans la fenêtre d'import ICS : la synchro directe (privée) se chargera du reste.

### Reconstruire l'extension

Depuis la racine du dépôt (outils requis : `bash`, `python3`, `zip`, `unzip`, `sha256sum`) :

```bash
bash tools/build-extension.sh            # → dist/creno-extension.zip (déterministe, 15 fichiers)
bash tools/build-extension.sh --check    # vérifie le zip produit
bash tools/build-extension.sh --clean    # supprime les artefacts générés
```

Le script recopie `index.html`, `app.js` et les 9 feuilles `styles/*.css` depuis la racine, retire le manifeste PWA et le service worker, puis écrit un zip reproductible (`dist/creno-extension.zip`, horodatages figés) accompagné de son sha256. Le paquet contient 15 fichiers : `manifest.json`, `background.js`, `index.html`, `app.js`, les 9 CSS sous `styles/` et les 2 icônes. `--check` contrôle la liste exacte des entrées, l'absence de fichiers PWA et le budget de taille (250 Kio) sur un zip déjà construit. Le zip se renomme `.xpi` à l'installation, une fois signé par Mozilla.

### Détails techniques

- **Manifest V2** (Firefox le supporte « pour le futur prévisible », voir [la position de Mozilla](https://blog.mozilla.org/addons/2024/03/13/manifest-v3-manifest-v2-march-2024-update/)) ;
- L'icône de l'extension ouvre l'app dans un onglet (réutilise l'onglet existant) via `background.js` ;
- Permissions : la seule permission réseau est `https://api.skello.io/*`, plus `storage` pour mémoriser l'onglet de l'extension — pas de `tabs`, pas de `<all_urls>`, aucun relais public autorisé, la synchro ICS se fait sans CORS ;
- `background.persistent: false` (pas de processus résident) et CSP explicite `script-src 'self'; object-src 'none';` ;
- `browser_specific_settings.gecko.id` : `creno@loganbozier.fr` ;
- Compatible Firefox 115+.
