# Créno relay

Service Rust auto-hébergé (optionnel) qui récupère un calendrier Skello à la
place du navigateur, quand l'app Créno est ouverte en `file://` ou via
l'extension et que CORS bloque l'appel direct. L'URL du calendrier contient un
jeton privé : elle ne transite alors que par votre serveur, jamais par un relais
public. Contrat d'API complet : [`CONTRACT.md`](CONTRACT.md).

**Fait** : `GET /healthz` → `{"status":"ok"}` ; `GET /ics?url=…` → le `.ics`
amont verbatim avec CORS, `ETag` (revalidation `304`) et
`Cache-Control: private, max-age=300`.
**Ne fait pas** : aucune base, aucun cache disque, aucun compte, aucune
écriture. L'URL cible et le jeton ne sont **jamais** journalisés (seuls méthode,
route, statut, latence, IP). Un déploiement = un utilisateur ; la limite de
débit est par IP, pas un contrôle d'accès.

## Démarrage rapide — Docker

```sh
docker build -t creno-relay:0.1.0 backend                       # ~6 Mo, uid 10001
docker run -d --name relay -p 127.0.0.1:8080:8080 \
  -e CRENO_BIND=0.0.0.0:8080 creno-relay:0.1.0
curl -sS http://127.0.0.1:8080/healthz                          # {"status":"ok"}
docker logs -f relay                                            # journaux sur stdout
```

Avec Compose (lecture seule, capabilities retirées, journaux plafonnés) :

```sh
cd deploy
cp ../backend/.env.example .env      # facultatif : les défauts suffisent
docker compose up -d --build
docker compose ps                    # Status: Up (healthy)
```

## Démarrage rapide — sans Docker

```sh
cd backend
cargo run --release    # écoute sur 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/healthz
```

## Configuration (environnement uniquement)

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `CRENO_BIND` | `127.0.0.1:8080` | adresse d'écoute (`0.0.0.0:8080` en conteneur) |
| `CRENO_ALLOWED_HOSTS` | `api.skello.io` | suffixes d'hôtes amont autorisés, séparés par des virgules |
| `CRENO_MAX_BYTES` | `2097152` | taille max du corps amont (1 KiB à 64 MiB) |
| `CRENO_TIMEOUT_SECS` | `12` | délai amont maximal (1 à 120) |
| `CRENO_RATE_LIMIT` | `30` | requêtes/min par IP (rafale 10) |
| `CRENO_ALLOW_EXTENSION` | `0` | `1` = accepter `moz-extension://` (extension Firefox) |
| `CRENO_ALLOWED_ORIGINS` | *(vide)* | origines exactes supplémentaires, séparées par des virgules |
| `RUST_LOG` | `info` | verbosité des journaux (`debug` pour diagnostiquer) |

Exemple prêt à copier : [`.env.example`](.env.example). Une valeur invalide
arrête le service au démarrage (code 2) avec un message explicite.

## Brancher l'app dessus

Modale de synchronisation Skello, champ **« Relais privé »** : saisir l'URL de
base du relais, sans chemin ni `/ics`, par exemple `https://relais.example.fr`.
L'app appelle alors `<base>/ics?url=…` en priorité, avant les relais publics.

**HTTPS obligatoire** : une page servie en HTTPS ne peut pas appeler un relais
en HTTP (contenu mixte, bloqué par le navigateur). En local (`file://`,
`http://localhost`), HTTP accepté ; sinon, terminer le TLS devant le relais.

## Reverse proxy TLS (Caddy)

```caddyfile
relais.example.fr {
	reverse_proxy 127.0.0.1:8080
}
```

Garder `ports: - "127.0.0.1:8080:8080"` : le relais n'est joignable que depuis
l'hôte, le proxy gère le certificat et le HTTPS.

## Sécurité

Allowlist stricte des hôtes (`api.skello.io` par défaut), refus des cibles
privées/loopback (anti-SSRF), plafond de 2 Mio, délai de 12 s, 30 req/min par
IP. Image Alpine statique (musl), uid 10001 sans shell, sans capability, sans
écriture disque, CA système pour valider le TLS amont.

## Dépannage

| Symptôme | Cause probable |
| --- | --- |
| `403 forbidden_host` | hôte amont absent de `CRENO_ALLOWED_HOSTS`, ou résolution vers une IP privée |
| `429 rate_limited` | plus de `CRENO_RATE_LIMIT` requêtes/min depuis cette IP (`Retry-After`) |
| `502 upstream_error` | Skello a échoué ou refusé : URL expirée, jeton révoqué, panne amont |
| `504 timeout` | amont plus lent que `CRENO_TIMEOUT_SECS` |
| requête bloquée côté navigateur (CORS) | origine non autorisée (`CRENO_ALLOWED_ORIGINS`) ou `CRENO_ALLOW_EXTENSION=0` avec l'extension |
| conteneur `unhealthy` | `CRENO_BIND` resté sur `127.0.0.1` : la sonde interne ne joint plus le port |
