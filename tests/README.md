# Créno — QA harness

Vérification headless, hors ligne et déterministe du front, de la synchro ICS (relais
privé, relais publics, accès direct) et du paquet d'extension Firefox.

## Lancer

```bash
bash tools/run-tests.sh                     # point d'entrée unique (suites câblées)
node tests/e2e/relay-sync.mjs               # E2E synchro ICS seule (vrai navigateur, requêtes interceptées)
node tests/e2e/ui-contract.mjs              # E2E contrat UI seul (vrai navigateur, hors ligne)
node tests/extension/check.mjs              # paquet d'extension seul (construit le zip)
bash tools/tests/test-build-extension.sh    # tests du script de build (à lancer à part)
```

`bash tools/run-tests.sh` affiche une ligne par suite (`PASS` / `FAIL` / `SKIP`) puis
`N passed, M failed, K skipped (of T suites)` et sort en 0 seulement si aucune suite n'a
échoué (un `SKIP` n'est ni un échec ni une réussite : il est compté à part).

**État actuel du câblage : 4 suites** (syntaxe, `relay-sync`, `ui-contract`, paquet
d'extension). **Cible : 5 suites**, la cinquième étant `tools/tests/test-build-extension.sh`,
en cours d'ajout à `tools/run-tests.sh` par un autre agent ; en attendant, cette suite se
lance à la main et `run-tests.sh` affiche `4 passed ... (of 4 suites)`.

Variables d'environnement (transmises au harnais E2E) : `CRENO_REPO=/tmp/copie` pointe le
harnais vers une autre copie de l'app (utilisé par l'auto-test), `CRENO_BROWSER` et
`PLAYWRIGHT_MODULE` forcent la détection du navigateur / du module Playwright. Playwright
vient du cache npm de `npx` et pilote le binaire Brave installé : rien n'est téléchargé,
aucune requête ne quitte la machine.

Les suites E2E sont entièrement hors ligne : `page.route` intercepte toute requête
externe (API Skello, relais privé, les trois relais publics) et répond depuis le harnais.
Tout autre appel est enregistré comme requête externe inattendue et fait échouer le cas.
Le navigateur tourne en plus avec `--host-resolver-rules=MAP * ~NOTFOUND` comme seconde
barrière. L'app est servie localement par `python3 -m http.server` sur `127.0.0.1`.

## Les suites

| Suite | Fichier | Contenu |
| --- | --- | --- |
| Syntaxe | (dans `run-tests.sh`) | `node --check` sur `app.js`, `extension/background.js`, les 3 harnais, `bash -n` sur `run-tests.sh` |
| Synchro ICS | `tests/e2e/relay-sync.mjs` | 10 cas (A → I) sur la chaîne de récupération |
| Contrat UI | `tests/e2e/ui-contract.mjs` | 18 assertions sur l'interface |
| Paquet d'extension | `tests/extension/check.mjs` | zip construit, liste d'entrées, `sha256` reproductible |
| Script de build | `tools/tests/test-build-extension.sh` | 10 sections / 22 assertions |

### `tests/e2e/relay-sync.mjs` — 10 cas

- **A** relais configuré + proxy autorisé : `direct` échoue, le relais privé
  `<relayUrl>/ics?url=…` réussit, les relais publics ne sont **pas** appelés, les shifts
  sont importés, `lastIcsSync` avance.
- **B** aucun relais : la chaîne est exactement `direct → allorigins → corsproxy →
  codetabs` ; le dernier importe.
- **C** aucun relais + `allowProxy=false` : seulement `direct`, toast d'erreur visible,
  aucun import, aucune autre requête.
- **D1** relais configuré qui répond 403 (+ proxy autorisé) : liste de requêtes exactement
  `[direct, privé]`, aucun import, erreur visible, **aucun** appel à `api.allorigins.win`,
  `corsproxy.io` ni `api.codetabs.com`.
- **D2** idem D1 avec `allowProxy=false` : même chaîne `[direct, privé]`, même garantie.
- **E** relais qui répond 200 en HTML sans `BEGIN:VCALENDAR` : rejeté, aucun shift importé,
  erreur affichée.
- **F** schéma `localStorage` après import : les clés de base `state` / `settings` sont
  toutes présentes, `relayUrl` est le seul ajout, rien n'est renommé ni supprimé.
- **G** relais configuré mais invalide (`http://relais.local`, hors loopback) : seule la
  tentative directe, toast d'erreur, relais publics **non** appelés, relais invalide jamais
  contacté.
- **H** exception loopback : `http://127.0.0.1:8080` est accepté comme base de relais —
  chaîne `[direct, privé]`, ICS importé, aucun relais public.
- **I** budget de synchro : un relais privé qui ne répond jamais est abandonné au budget de
  l'app (assertion entre 8 s et 25 s, ~20 s attendus), erreur affichée, aucun relais public.
  Les faux timers de Playwright avancent le temps virtuel : le cas coûte quelques
  millisecondes réelles.

Cas dépendants du relais (A, D1, D2, E, I) : si l'app testée n'a aucune route de relais
privé, le harnais signale `relay route not present (agent/app-relay not merged)` au lieu de
planter. Aucun cas n'est ignoré : chacun passe ou échoue avec une ligne nommant le
comportement attendu.

### `tests/e2e/ui-contract.mjs` — 18 assertions

- navigation de semaine : `‹` / `›` (`#prevWeek` / `#nextWeek`), « Aujourd'hui »
  (`#todayBtn`), saut par date (`#jumpDate`) et bascule mobile `#mdWeekBtn` ;
- estimation de salaire (`#summaryBar`, montant, objectif) et cartes de statistiques ;
- panneau Skello : ouverture, états de la pastille de synchro (`idle` / `busy` / `ok` /
  `error`), champ relais privé ;
- persistance : rechargement de la page, les données `state` / `settings` survivent ;
- contrats structurels : `.dialog-body`, `#weekGrid`, variables CSS `--c` et `--v` posées
  en style inline, clé `settings.relayUrl`.

### `tests/extension/check.mjs` et `tools/tests/test-build-extension.sh`

`tests/extension/check.mjs` construit le paquet (`bash tools/build-extension.sh`) puis
vérifie le zip : liste exacte des entrées, absence de fichiers PWA, `sha256` reproductible.
Il sort en 0 pour un `PASS` **et** pour un `SKIP` explicite (`SKIP ` en tête de ligne), ce
`SKIP` signifiant que le script de build est absent — `run-tests.sh` lit cette ligne pour ne
pas le compter comme une réussite.

`tools/tests/test-build-extension.sh` teste le script de build lui-même, sur des copies
jetables du dépôt (le dépôt réel n'est jamais modifié) : 10 sections, 22 assertions — build
nominal et charge utile de 15 fichiers, reproductibilité (deux builds, même sha256),
`--check` nominal / sans zip / zip altéré, marqueurs HTML et service worker absents, icône
déclarée manquante, budget de 250 Kio dépassé, `--clean`.
