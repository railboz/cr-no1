# Créno relay

Service Rust auto-hébergé (optionnel) qui récupère un calendrier Skello à la
place du navigateur, quand l'app Créno ne peut pas appeler Skello directement
(CORS). L'URL du calendrier contient un jeton privé : dès que le relais est
renseigné dans l'app, elle ne transite que par votre serveur, jamais par un
relais public. Contrat d'API complet : [`CONTRACT.md`](CONTRACT.md).

**Fait** : `GET /healthz` → `{"status":"ok"}` ; `GET /ics?url=…` → le `.ics`
amont verbatim avec CORS, `ETag` (revalidation `304`) et
`Cache-Control: private, max-age=300`.
**Ne fait pas** : aucune base, aucun cache disque, aucun compte, aucune
écriture. L'URL cible et le jeton ne sont **jamais** journalisés (seuls méthode,
chemin, statut, durée et taille de la réponse : jamais l'IP du client). Un
déploiement = un utilisateur ; la limite de débit est par IP, pas un contrôle
d'accès.

## Démarrage rapide — Docker

```sh
docker build -t creno-relay:0.1.0 backend    # de l'ordre de 7 Mo, binaire statique inclus, uid 10001
docker run -d --name relay -p 127.0.0.1:8080:8080 creno-relay:0.1.0
curl -sS http://127.0.0.1:8080/healthz       # {"status":"ok"}
docker logs -f relay                         # journaux sur stdout
```

Taille exacte de l'image finale (non compressée) :
`docker image inspect -f '{{.Size}}' creno-relay:0.1.0`.

L'image fixe déjà `CRENO_BIND=0.0.0.0:8080` : ne le surchargez pas en
`127.0.0.1`, le conteneur resterait « healthy » mais injoignable depuis l'hôte.
Pour un usage réel, préférez Compose (lecture seule, capabilities retirées,
journaux plafonnés) :

```sh
cp deploy/.env.example deploy/.env    # facultatif : les défauts suffisent
docker compose -f deploy/compose.yml up -d --build
docker compose -f deploy/compose.yml ps   # Status: Up (healthy)
```

## Démarrage rapide — sans Docker

```sh
cd backend
cargo run --release    # écoute sur 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/healthz
```

## Configuration (environnement uniquement)

Les deux variables que l'on modifie en pratique sont **`CRENO_ALLOWED_HOSTS`**
(domaine de l'ICS amont) et **`CRENO_ALLOWED_ORIGINS`** (domaine qui sert la
page Créno) ; tout le reste tient sur des défauts raisonnables.

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `CRENO_BIND` | `127.0.0.1:8080` | adresse d'écoute (en conteneur : `0.0.0.0:8080`, fixé par l'image et `deploy/compose.yml`) |
| `CRENO_ALLOWED_HOSTS` | `api.skello.io` | suffixes d'hôtes amont autorisés, séparés par des virgules (un label seul comme `io` est refusé) |
| `CRENO_ALLOWED_ORIGINS` | *(vide)* | origines exactes supplémentaires, séparées par des virgules |
| `CRENO_MAX_BYTES` | `2097152` | taille max du corps amont (1 KiB à 64 MiB) |
| `CRENO_TIMEOUT_SECS` | `12` | délai amont maximal (1 à 120) |
| `CRENO_RATE_LIMIT` | `30` | requêtes/min par IP (rafale 10) |
| `CRENO_ALLOW_EXTENSION` | `0` | `1` = accepter `moz-extension://` (extension Firefox) |
| `CRENO_ALLOW_FILE_ORIGIN` | `0` | `1` = accepter l'origine `null` d'une page ouverte en `file://` (refusée par défaut) |
| `RUST_LOG` | `info` | verbosité des journaux (`debug` pour diagnostiquer) |

Une seule source de vérité pour les fichiers d'environnement :
[`deploy/.env.example`](../deploy/.env.example), copié en `deploy/.env`
(non versionné, chargé par Compose). [`backend/.env.example`](.env.example) ne
sert qu'au lancement natif (`cargo run`, variables à exporter : le binaire ne
lit aucun fichier `.env`) et n'a rien à faire dans `deploy/.env`. Aucun secret
n'y est attendu : le relais ne stocke ni compte ni jeton. Une valeur invalide
arrête le service au démarrage (code 2) avec un message explicite.

Une page ouverte en `file://` envoie `Origin: null` : refusée par défaut, elle
n'est acceptée qu'avec `CRENO_ALLOW_FILE_ORIGIN=1`. Un serveur local
(`http://localhost:<port>`, `http://127.0.0.1:<port>`) reste accepté d'office.

## Brancher l'app dessus

Modale de synchronisation Skello, champ **« Relais privé »** : saisir l'URL de
base du relais, sans chemin ni `/ics`, par exemple `https://relais.example.fr`.
L'app appelle alors `<base>/ics?url=…` et la chaîne s'arrête là :
`direct → relais privé`, point final. Les relais publics ne servent **que** si
aucun relais n'est configuré et que l'utilisateur les autorise. Si le relais
échoue (service arrêté, origine non autorisée), l'app affiche une erreur
explicite et ne bascule sur **aucun** relais public.

**HTTPS obligatoire** : une page servie en HTTPS ne peut pas appeler un relais
en HTTP (contenu mixte, bloqué par le navigateur). Seule exception, la boucle
locale : `http://localhost`, `http://127.0.0.1` et `http://[::1]` sont acceptés
par l'app ; tout autre `http://` est refusé. Pour un relais distant, terminer le
TLS devant le relais.

## Reverse proxy TLS (Caddy)

```caddyfile
relais.example.fr {
	reverse_proxy 127.0.0.1:8080
}
```

Garder `ports: - "127.0.0.1:8080:8080"` : le relais n'est joignable que depuis
l'hôte, le proxy gère le certificat et le HTTPS.

## Journal d'accès du reverse proxy

L'URL cible arrive en paramètre de requête : `GET /ics?url=…token=…` porte le
jeton en clair. Le relais ne le journalise jamais (méthode, chemin, statut,
durée, taille) ; le proxy doit être réglé de même, sinon le jeton atterrit dans
son journal d'accès et dans les archives de rotation.

- **nginx** : un `log_format` sans `$request` ni `$args`, par exemple
  `log_format sans_url '$status $request_method $uri';` utilisé par
  `access_log /var/log/nginx/relais.log sans_url;` — ou `access_log off;` dans
  le bloc `server` si ce journal ne sert à rien. `$request` et `$args`
  contiennent la query, donc le jeton ; `$uri` seul ne la contient pas.
- **Caddy** : ne pas activer `log` dans le bloc du site, ou filtrer la requête
  pour en retirer la query (le champ `request.uri` du format JSON par défaut
  inclut `?url=…`).

Un journal qui contient `?url=` est un journal de secrets : purgez-le et
régénérez l'URL du calendrier côté Skello.

## Sécurité

Allowlist stricte des hôtes (`api.skello.io` par défaut), refus des cibles
privées/loopback (anti-SSRF), plafond de 2 Mio, délai de 12 s, 30 req/min par
IP. Les prévols CORS (`OPTIONS`) ne sont pas comptés dans cette limite : seule
la requête `GET` l'est. Image Alpine statique (musl), uid 10001 sans shell, sans
capability, sans écriture disque, CA système pour valider le TLS amont ; aucun
outil de compilation n'y est copié (le binaire arrive seul depuis l'étage de
build) et aucun secret n'y est stocké.

## Dépannage

| Symptôme | Cause probable |
| --- | --- |
| `403 forbidden_host` | hôte amont absent de `CRENO_ALLOWED_HOSTS`, ou résolution vers une IP privée |
| `429 rate_limited` | plus de `CRENO_RATE_LIMIT` requêtes/min depuis cette IP (`Retry-After`) |
| `502 upstream_error` | Skello a échoué ou refusé : URL expirée, jeton révoqué, panne amont |
| `504 timeout` | amont plus lent que `CRENO_TIMEOUT_SECS` ; la connexion TCP/TLS initiale est plafonnée à 5 s, donc un hôte mort (TLS injoignable) échoue en ~5 s même avec `CRENO_TIMEOUT_SECS=120` |
| requête bloquée côté navigateur (CORS) | origine non autorisée (`CRENO_ALLOWED_ORIGINS`) ou `CRENO_ALLOW_EXTENSION=0` avec l'extension |
| requête bloquée (CORS) pour une page ouverte en `file://` | l'origine envoyée est `null`, refusée par défaut : démarrer le relais avec `CRENO_ALLOW_FILE_ORIGIN=1` |
| mon jeton apparaît dans les logs du proxy | le journal d'accès du proxy enregistre l'URL complète : voir [Journal d'accès du reverse proxy](#journal-daccès-du-reverse-proxy) |
| conteneur `healthy` mais `curl` depuis l'hôte refusé | `CRENO_BIND=127.0.0.1` a été forcé dans l'environnement du conteneur : la sonde interne passe (boucle locale), la publication `ports:` non. Retirer la variable, l'image et Compose fixent `0.0.0.0:8080` |
| conteneur `unhealthy` | processus arrêté au démarrage (variable invalide, code 2) : voir `docker logs` |
