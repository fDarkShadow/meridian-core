# 04 — Secrets & taint

## Le moteur ne manipule que des références
`secretRef`, jamais des valeurs. L'IR compilé, les logs, l'état d'exécution en DB : rien ne
contient de matière. Elle n'existe qu'en mémoire host, au moment de l'appel I/O, et disparaît
après.

## Interface `SecretProvider`
```
resolve(ref) -> material   // matériel n'existe qu'en mémoire host, éphémère
```
Le stockage est un détail remplaçable derrière cette interface. Backends :
- **Postgres + envelope encryption** (défaut jour 1). PAS « une colonne AES avec clé dans
  l'env ». Design : **KEK** (key-encryption-key) dans un KMS/HSM ; **DEK** par tenant
  chiffrées par la KEK ; AEAD (AES-GCM ou XChaCha20-Poly1305) avec métadonnées (tenant,
  version de clé) en **AAD**. → rotation de KEK sans re-chiffrement + isolation crypto
  par tenant.
- **Vault / OpenBao** (optionnel, derrière l'interface). Secrets dynamiques TTL court, leasing,
  audit. OpenBao = fork OSI, aligné licence. Dépendance ops lourde → pas un prérequis.
- **ESO / CSI** : secrets DE LA PLATEFORME (creds DB, clé KMS) → K8s. **PAS** les credentials
  des tenants. Plan distinct — ne pas confondre `SecretProvider` (secrets tenants runtime)
  et ESO (secrets plateforme).

## Type `Secret<T>` taint-tracké (invariant 7)
1. `Display` / `Debug` / `toString` rendent `[REDACTED]` → **structurellement inlogguable**.
2. Déballable **uniquement** par la capability I/O du host, au dernier moment.
3. Taint **contagieux** (se propage aux valeurs dérivées).

« Les secrets ne *peuvent pas* être loggés » (garantie), pas « on essaie de ne pas les
logger » (espoir). La regex de sortie reste, mais comme **défense-en-profondeur**, pas
mécanisme primaire.

## Champs métier sensibles (PII)
On ne peut pas tout tainter auto. Deux leviers :
- schéma de sortie du node porte `sensitive: true` par champ → **redaction dirigée par le
  schéma**, pas devinée.
- la capture des payloads complets (input/output de chaque node) est **opt-in explicite et
  par-tenant**, jamais le défaut (n8n log tout par défaut = cauchemar RGPD).

## Egress-scope (voir aussi 03)
`connections.egress_scope` : le credential déclare son domaine autorisé ; le host refuse
d'attacher le secret hors scope. **Par-connexion, pas global.** Champ présent dès v1 (rétrofit
douloureux). Donne aussi la défense SSRF.

## Redaction & OAuth
- Redaction des logs/outputs : on logge les refs ; pré-filtre qui scanne les valeurs
  sortantes (filet).
- OAuth : le host possède le refresh flow et met à jour le token stocké ; le node ne
  participe jamais au cycle de vie du credential.
