# 🗓️ Créno — votre planning, simplement

Alternative légère et 100 % locale à Skello, en HTML/CSS/JS pur. Aucun serveur, aucune dépendance : ouvrez `index.html` dans votre navigateur et c'est parti.

## ✨ Fonctionnalités

- **Planning hebdomadaire** : grille lun → dim avec vos établissements, blocs de shift colorés (horaires, durée, poste), colonne du jour mise en avant.
- **Établissements multiples** : ajout, renommage, couleur personnalisée, suppression (avec ses shifts).
- **Shifts** : création en cliquant sur une case, édition en cliquant sur un bloc, notes, labels suggérés (DRIVE, CAISSE…).
- **Absences** : congés, RTT, maladie, formation… affichées en pointillés dans le planning et listées dans l'onglet dédié.
- **Statistiques** : heures par semaine (8 semaines), répartition par établissement et par poste, total du mois.
- **Ajouts originaux** :
  - 💶 **Rémunération estimée** (taux horaire réglable dans le profil) et barre d'**objectif hebdomadaire** ;
  - ⧉ **Duplication de la semaine précédente** en un clic ;
  - ⚠️ **Disponibilités habituelles** : alerte si vous planifiez un shift un jour habituellement indisponible ;
  - ⌨️ **Raccourcis clavier** : `←` / `→` changer de semaine, `N` nouveau shift ;
  - 📥 **Glisser-déposer** d'un fichier JSON n'importe où dans la fenêtre pour l'importer.

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

> 🔒 **Vie privée** : si le navigateur bloque la demande directe (CORS), Créno peut passer par un relais public — désactivable dans la fenêtre d'import. Dans ce cas, votre URL (qui contient un jeton privé) transite par un serveur tiers. La méthode 100 % privée reste le fichier `.ics` téléchargé manuellement.

### Méthodes manuelles

- Ouvrez l'URL dans le navigateur : elle télécharge un fichier `.ics` → glissez-déposez-le dans la fenêtre (ou bouton « Choisir un fichier .ics… »).
- Ou collez directement le contenu du calendrier dans la fenêtre d'import.

Créno convertit chaque créneau horaire en **shift** (l'établissement est reconnu ou créé automatiquement) et chaque journée entière en **absence** (congés, RTT, maladie… selon le libellé). Les shifts déjà présents ne sont jamais importés deux fois.

## 🚀 Lancer (desktop)

Ouvrez simplement `index.html` (double-clic). Aucune installation nécessaire.

> ⚠️ Les données restent liées au navigateur et à l'emplacement du fichier. Pensez à exporter un JSON de sauvegarde avant de changer de machine ou de navigateur.

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

> 💡 Les données restent **100 % locales** sur le téléphone (localStorage). Pensez à **⬇️ Exporter (JSON)** de temps en temps comme sauvegarde — l'export est compatible entre la version web, la PWA et l'extension Firefox.

### Variantes d'installation

- **Sans héberger** : servez le dossier depuis votre PC (`serve.ps1`) — mais l'app ne sera accessible que si le PC est allumé, et l'installation PWA complète exige HTTPS.
- **Extension Firefox Android** : voir la section suivante (installation par fichier `.xpi` signé).

## 🦊 Extension Firefox (navigateur)

Une version **extension** existe aussi dans `../creno-extension/` (Manifest V2) : icône dans la barre d'outils, synchro Skello **directe et privée** (permissions host accordées, sans relais). Voir `../creno-extension/README.md` ou la section ci-dessous pour la reconstruction :

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File build-extension.ps1
```



## 📱 Extension Firefox pour téléphone (Android)

Créno existe aussi en **extension Firefox**, pensée d'abord pour le mobile :

- **Navigation en bas de l'écran** (Planning · Absences · Statistiques · Profil) ;
- **Vue jour-par-jour** : pilules de jours en haut, swipe gauche/droite pour changer de jour ;
- **Modales en bottom-sheet** qui montent du bas de l'écran, zones tactiles larges ;
- **Synchro Skello 100 % privée** : grâce aux permissions de l'extension, la récupération directe de votre calendrier ICS est autorisée — **aucun relais public, aucune donnée qui transite par un tiers**.

### Installer sur votre téléphone

L'installation se fait depuis un fichier `.xpi` **signé par Mozilla** (exigence de Firefox Android, ça prend 5 minutes) :

1. **Créez un compte gratuit** sur [addons.mozilla.org](https://addons.mozilla.org/fr/developers/).
2. Allez dans **« Outils pour développeurs » → « Soumettre un nouveau module »**.
3. Choisissez **« Sur votre propre serveur »** (distribution personnelle, non listée) et envoyez le fichier **`creno-extension.zip`**.
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

Le paquet est régénéré avec :

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File build-extension.ps1
```

Le script copie les fichiers à jour de `creno/` dans `creno-extension/` puis produit `creno-extension.zip` (format `.xpi`).

### Détails techniques

- **Manifest V2** (Firefox le supporte « pour le futur prévisible », voir [la position de Mozilla](https://blog.mozilla.org/addons/2024/03/13/manifest-v3-manifest-v2-march-2024-update/)) ;
- L'icône de l'extension ouvre l'app dans un onglet (réutilise l'onglet existant) via `background.js` ;
- Permissions : `tabs` + les domaines Skello/relais, pour autoriser la synchro ICS sans CORS ;
- `browser_specific_settings.gecko.id` : `creno@loganbozier.fr` ;
- Compatible Firefox 115+.
