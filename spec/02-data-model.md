# 02 — Modèle de données (schéma v1)

> **⚠ Colonnes RÉSERVÉES v1 marquées 🔒.** Elles vivent dans la clé, le type ou l'enum.
> Les ajouter après coup = migration douloureuse ou réinterprétation d'historique.
> Ne jamais les omettre.

Multi-tenant : **`tenant_id` dans chaque FK composite** + **RLS Postgres** (session
`SET LOCAL app.tenant_id`, policies deny-par-défaut). FK composite protège l'intégrité ;
RLS protège l'accès (un SELECT sans filtre ne fuit pas).

## `node_runs`
```
PK: (execution_id, node_id, iteration_index, attempt)   🔒 iteration_index, attempt
    tenant_id            🔒  -- FK composite + RLS
    status               🔒  -- enum terminal gelé v1 (voir ci-dessous)
    leased_until         🔒  -- LE BAIL. distingue "en cours" de "orphelin".
    owned_by_worker      🔒  -- qui exécute (réclamation)
    input_ref                -- ref vers Object Store (RustFS), jamais un blob
    output_ref               -- idem
    created_at, updated_at
```
`iteration_index` rend le **Map durable** possible (une node-run durable par item).
`leased_until` : rempli fixe (= ack-wait) si pas de heartbeat, ou glissant si heartbeat.
`running` n'est PAS un statut, c'est un bail.

### Enum `status` (états terminaux gelés v1) 🔒
| état | retryable ? | sens |
|---|---|---|
| `pending` | — | prêt à dispatcher |
| `running` | — | leased (voir leased_until) |
| `done` | terminal | succès, side-effect commis |
| `failed` | **terminal-retryable** | échec, peut retry |
| `cancelled` | terminal | annulé propre (non démarré) |
| `interrupted` | terminal | tué en vol — **side-effect INCERTAIN** |
| `killed` | **terminal-définitif, hors-retry** | kill opérateur / TTL |

`killed` ≠ `failed`. Un kill ne relance jamais via retry. Reprendre un `killed` = acte
manuel explicite.

## `executions`
```
PK: (id)
    tenant_id            🔒
    status
    deadline / ttl       🔒  -- disjoncteur de terminaison. défaut généreux, surchargeable.
    version_pin          🔒  -- hash de l'IR épinglé (reprise correcte à J+30)
    replay_of            🔒  -- lignage du rejeu (nouveau run lié, jamais mutation)
    created_at, updated_at
```

## `map_state`
```
PK: (execution_id, node_id)
    tenant_id            🔒
    fan_in_status        🔒  -- CAS: pending -> aggregating. l'unicité du fan-in vit ici.
    total_items, remaining_terminal  -- pour la détection de complétude
```

## `connections`  (credentials applicatifs par tenant)
```
    tenant_id            🔒
    secret_ref               -- référence, JAMAIS la valeur
    egress_scope         🔒  -- domaine(s) autorisé(s), LIÉ au credential.
                             -- ferme exfiltration host-mediated + SSRF.
```

## `audit_log`  (append-only, hash-chaîné)
```
    tenant_id
    prev_hash, hash          -- chaîne tamper-evident, calculée À L'ÉCRITURE DB (pas subscriber)
    actor                    -- ex. user:<id> | agent:<id>@user:<id> | ttl_exceeded
    motif                    -- obligatoire pour les actions à haute conséquence (kill)
    resource, action, result, ts
```
Append-only révoqué au niveau **rôle DB** (pas par convention). Écrit **dans la même
transaction** que l'action auditée (voir 05, outbox).

## `outbox`  (pont DB → NATS)
```
    payload, created_at, published_at
```
Buffer durable. Poller au début ; réplication logique / CDC quand le volume le justifie.

## `signals` (HITL — Wait-on-signal, voir 06)
```
    execution_id, node_id, iteration_index
    tenant_id
    wait_deadline        🔒  -- péremption via réconciliateur -> branche Catch
    key_id                   -- sélecteur de clé HMAC (rotation)
    expected_payload_shape   -- pour le field-shaping de l'injection
    -- NE stocke PAS le token : la signature est recalculable depuis l'état + la clé
```

## Rétention (transverse) 🔒
`retention_class` sur chaque flux : audit (années, immuable) / applicatif (jours) /
payloads (très court, purgeable RGPD). Champ dès v1.

## Type transverse : `Secret<T>` 🔒
Type distinct taint-tracké, propagé dans tout le moteur (voir 04). `[REDACTED]` par défaut.
Incompatible avec un ajout tardif — vit dans le type de base par lequel toute donnée circule.

## Content-addressing
IR compilé **et** modules WASM adressés **par hash de contenu**. La définition référence les
nodes par `(identité logique + version → hash)`. Rollback = repointer sur les hashes d'avant.
Blobs WASM dans l'Object Store (RustFS), métadonnées en Postgres. Jamais de `bytea` de blob.
