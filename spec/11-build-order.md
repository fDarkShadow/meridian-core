# 11 — Ordre d'implémentation & sujets ouverts

Ordre suggéré. Le principe directeur : **la correction du moteur avant tout le reste**.
Rien de ce qui touche l'état ne doit être optimisé avant d'être correct sous crash/redelivery.

## Phase 0 — fondations
1. Schéma Postgres v1 **complet** (toutes les colonnes réservées 🔒, `spec/02`), RLS, enum des
   états terminaux, type `Secret<T>`.
2. IR + content-addressing (parse YAML/JSON → validation → IR normalisé → store par hash).
3. Squelette gRPC/WIT du protocole de node.

## Phase 1 — le moteur correct (le cœur, non négociable d'abord)
4. Machine à états ASL : primitives fermées + modificateurs Retry/Catch/Timeout.
5. Boucle d'exécution : durabilité aux frontières de node, idempotence interne (upsert par
   node-run-id), garde de réception, resume sans replay.
6. Baux + réconciliateur (lit Postgres) + ack-wait ; heartbeat optionnel.
7. Map durable (iteration_index, sémaphore, fan-in CAS push+pull, politique collect).
8. Annulation / kill hiérarchique + états `interrupted`/`killed` + TTL d'exécution.
9. Audit hash-chaîné + outbox + relay. (À faire tôt : c'est transactionnel avec l'état.)

## Phase 2 — exécution de node
10. Runtime WASM (Wasmtime/Extism), interface WIT, une instance par invocation, epoch-int.
11. Host-mediated I/O + capabilities + cleanup des ressources sur tous les chemins de sortie.
12. SecretProvider (Postgres + envelope encryption) + egress-scope + taint de bout en bout.

## Phase 3 — surfaces & ingestion
13. API de contrôle (le contrat) + mapper rôle-IdP→permission (deny par défaut, par-tenant).
14. Triggers : scheduler découplé + timers ; harness webhook (durable-avant-ack, dédup).
15. HITL : `Wait-on-signal` + presigned HMAC (recalculable, CAS, wait_deadline, key_id).
16. Observabilité : exporters OTel pluggables + live-tail via NATS→websocket.
17. Rejeu (lignage `replay_of`, 3 granularités).

## Phase 4 — plateforme
18. Infra GitOps : cluster (pools mutualisés + WASM tainté), CNPG (3 replicas anti-affinité,
    WAL→S3 externe, PITR), RustFS, NATS, Cilium+WireGuard+Gateway API, deny-all.
19. Tenant hard : manifeste GitOps de stack complet + job quiesce/migration.
20. Détection : Tetragon, CrowdSec (parser Hubble), Vector→Wazuh, anomalie egress par tenant.
21. MCP par-tenant (consommateur d'API, scopes agent, BYO model médié).

## Sujets OUVERTS (outillage/infra — ne touchent PAS le modèle ; ne pas bloquer dessus)
- **Test / replay déterministe du moteur** : rendre le moteur rejouable en test depuis un état
  persisté arbitraire. À concevoir dans l'archi (pas après) — le prévoir en Phase 1.
- **Read-model GUI dédié** : projections dénormalisées, seulement quand les requêtes SQL
  directes pincent.
- **Map distribué > N items** : borne v1 explicite + pagination applicative ; batch réservé.
- **Registry des modules WASM** : packaging/distribution (OCI ?).
- **Backpressure / fairness par tenant** : queues NATS partitionnées / quota-aware scheduling
  (le free tier le rend nécessaire tôt → l'anticiper au dispatch).
- **mTLS / identité** (zéro-trust complet) : à l'échelle conformité.
- **Migration à chaud** (réplication logique) : quand un gros client l'exige.

## Rappels transverses (à re-vérifier à chaque PR)
- Aucune colonne réservée 🔒 omise.
- Aucun secret dans log / output / contexte modèle / IR.
- Réconciliateur lit Postgres, jamais NATS.
- Audit dans la même transaction que l'action (jamais via subscriber).
- Pas de nouvelle primitive de contrôle (plugins = Tasks + macros).
- MCP / GUI / CLI = consommateurs de l'API, zéro passe-droit.
- Scan de licence CI vert (pas de copyleft incompatible).
