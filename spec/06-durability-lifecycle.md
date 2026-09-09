# 06 — Durabilité, cycle de vie, Map, annulation, HITL

## Durabilité de base
Persister transactionnellement l'état après chaque node (outputs + frontière + statut),
*puis* ack NATS. Crash → recharge de l'état, ré-enqueue de la frontière.
**Le resume recharge les outputs persistés ; il ne re-run JAMAIS** — les side-effects déjà
commis ne se rejouent pas.

## `running` = un bail
Chaque node-run dispatché porte `leased_until` + `owned_by_worker`. Sans le bail, impossible
de distinguer « tourne légitimement » de « orphelin d'un worker mort ».

## Réconciliateur (lit Postgres, JAMAIS NATS)
Balayeur périodique (déclenché par le scheduler découplé) :
```sql
SELECT * FROM node_runs WHERE status='running' AND leased_until < now();
-- réclamation par compare-and-swap :
UPDATE node_runs SET status='pending', attempt=attempt+1, leased_until=NULL
 WHERE id=? AND leased_until=<valeur lue>;   -- CAS : le perdant voit 0 ligne, passe
-- ENSUITE seulement : re-dispatch via NATS. La DB décide, NATS transporte.
```
Trois usages du même balayeur : orphelins (baux expirés) + fan-in ratés (complétude) +
TTL dépassés (deadlines). Un seul daemon, trois requêtes.

Filets empilés : **ack-wait JetStream = filet rapide/local au message** ; **réconciliateur DB
= filet lent/global à l'état**. Complémentaires, pas l'un à la place de l'autre.

## Heartbeat : optionnel-mais-recommandé
- **Sans** : `leased_until` fixe = ack-wait → **la durée max d'un node est bornée par
  l'ack-wait** (modèle simple, viable jour 1).
- **Avec** : le bail glisse tant que le worker bat → **nodes longs sûrs**.
`leased_until` existe dans le schéma dans les deux cas → activer le heartbeat plus tard est
un ajout de comportement, jamais une migration.

## Trois bornes temporelles (invariant 8)
| Borne | Protège de | Déclencheur |
|---|---|---|
| ack-wait / bail | worker mort | silence du worker |
| timeout de node | node en vrille | durée du node dépasse sa limite |
| TTL d'exécution | workflow bloqué | durée totale dépasse `executions.deadline` |
TTL dépassé = un kill (hérite de toute la sémantique kill ci-dessous, motif `ttl_exceeded`).

## Map — fan-out durable
- **Une node-run DURABLE par item** (façon GitLab CI), avec `iteration_index` dans la clé.
  → retry par-item (item N rejoué seul via `attempt`), resume au grain de l'item.
- Concurrence bornée par **sémaphore** (`MaxConcurrency`, enforced **au dispatch**) + timeout
  par item.
- **Pagination de la source déléguée au dev.** Borne v1 explicite (« Map durable jusqu'à N
  items ; au-delà = sous-pipeline paginé »). Map distribué en batches = réservé, pas construit.
- **Baux/lignes INDÉPENDANTS par item** (pas de compteur partagé → pas de contention sur une
  ligne unique).

### Fan-in (Map & Parallel)
Personne ne « sait » qu'il est le dernier → CAS idempotent sur `map_state.fan_in_status`
(`pending → aggregating`) : le gagnant agrège, les autres s'abstiennent.
- **push** : le dernier item, après avoir marqué `completed`, tente le CAS (latence).
- **pull** : le réconciliateur rattrape les Map dont tous les items sont terminaux mais le
  fan-in pas fait (filet — même duo push-rapide / pull-de-sécurité que les orphelins).
Le seul point de synchro résiduel = **un CAS unique au fan-in**. Irréductible, au bon endroit.

### Politique d'échec = collect (défaut)
On **attend la fin de toutes les branches**, puis on lève une erreur qui **nomme les branches
fautives** (pour rejeu ciblé). Fail-fast = champ optionnel par-Parallel. Défaut collect
(cohérent avec le durable : fail-fast annulerait des branches en vol → side-effects à moitié
faits).

## Annulation & kill opérateur
« Interruptible » = arrêt **aux frontières de node sûres** par défaut, overridable (un node
peut déclarer une section non-coupable-à-chaud). Deux niveaux :
- **annulation coopérative** : signale, le node finit son I/O / atteint son point sûr.
- **interruption dure (kill opérateur / TTL)** : **epoch-interruption** → module tué net.
  Conséquence assumée : si l'I/O était parti, l'effet distant a **peut-être** eu lieu → état
  terminal `interrupted` (side-effect INCERTAIN), pas `cancelled`. **Exposer l'incertitude,
  pas la masquer.**

Kill opérateur = action à haute conséquence :
- **audité 1ʳᵉ classe** (qui, quand, quelle exéc, **motif obligatoire**), committé avec l'action ;
- **RBAC dédié** `execution:kill` (distinct de lancer/éditer) ;
- **terminal-définitif hors-retry** (`killed` ≠ `failed`) ;
- **hiérarchique** : subject NATS scopé `cancel.{tenant}.{pipeline}.{execution}` → kill d'un
  pipeline entier / disjoncteur-tenant gratuits en publiant sur le préfixe parent.
Signal d'annulation = **DB source de vérité + NATS pour la latence** (comme l'audit).

**PAS de compensation automatique.** Voir invariant / 00 corollaire.

## Human-in-the-loop = `Wait-on-signal` + presigned URL (façon S3)
Pas un sous-système : un `Wait` qui se débloque sur signal externe corrélé, ingéré par le
harness webhook (durable-avant-ack).
- **Autorisation = signature** (capability infalsifiable façon presigned S3) : token = **HMAC
  signé** sur `(execution_id, node_id, iteration_index, nonce, expiry, key_id)`. Le moteur
  **vérifie** la signature avant d'agir. La détenir = le droit de débloquer, non transférable
  à une autre exécution.
- **Idempotent / usage unique** : CAS `waiting → signaled` (le 1ᵉʳ appel gagne, les suivants
  = no-op). Même CAS que le fan-in.
- **Péremption** : `wait_deadline` vérifié par le réconciliateur → branche `Catch` (timeout).
- **Zéro secret stocké** : la signature est **recalculable** depuis l'état persisté
  `(exec, node, iter, deadline)` + la clé (façon S3 côté serveur). `signals` ne stocke que
  `wait_deadline`, `key_id`, `expected_payload_shape` — jamais le token.
- **Clé rotative** via `key_id` (sélecteur non signé ; garder N dernières clés actives).
- **V1 = un signal, un waiter.** Quorum = fan-in de signaux (baux par signal + CAS de seuil),
  réservé via un futur `signal_group`, pas câblé.
- Le payload entrant traverse le **field-shaping** (l'humain injecte de la donnée, pas juste
  un ping).

## Ingestion webhook (harness)
Invariant : **persister durablement AVANT d'acker l'appelant HTTP (200)**, sinon déclencheur
perdu. Ordre : reçois → écris (durable) → réponds 200 → corrèle/déclenche en asynchrone.
Dédup (idempotency key du provider si dispo, sinon hash payload+source), débounce.

## Rejeu (façon GitLab, bien fait)
Un rejeu = **nouveau run lié** (`replay_of`), **jamais** une mutation de l'ancien (historique
immuable, cohérent avec l'audit append-only). Trois granularités :
exécution entière / depuis un node (recharge des outputs amont) / items fautifs seulement.

## Scheduler / heartbeat découplé
Composant à part (ni cron Docker ni CronJob K8s ne portent la logique). Le trigger externe est
**bête et sans état** : émet un event JetStream **déterministe** (`schedule_id + fenêtre`,
jamais un uuid random) puis meurt. Le control plane porte la logique + dédup (`Nats-Msg-Id`).
Élection de leader au début ; claim atomique en DB quand on scale le scheduler.
