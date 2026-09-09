# 00 — Invariants non-négociables

Ces règles priment sur toute considération de simplicité ou de rapidité. Si le code semble
exiger d'en violer une, le code est faux.

## 1. Frontière durable / best-effort
Chaque mécanisme se range d'un côté :
- **Durable** : transactionnel, dans Postgres, source de vérité. (état d'exécution, audit,
  baux, timers, versions)
- **Best-effort** : transitoire, rattrapable. (dispatch NATS, logs applicatifs, notifications,
  progression temps réel)

Un fait naît durable, *puis* est dérivé vers le best-effort. Jamais l'inverse.

## 2. Postgres = seule source de vérité
NATS ne porte que la notification d'un fait déjà durable. **Aucun composant ne lit NATS pour
savoir ce qui est vrai.** Le réconciliateur lit Postgres, jamais le stream.

## 3. Durabilité aux frontières de node
L'état est cohérent entre les nodes. Après chaque node : persister transactionnellement
(outputs + nouvelle frontière + statut), *puis* ack NATS. Un crash → recharge de l'état,
ré-enqueue de la frontière. Jamais d'état cohérent « au milieu » d'un node.

## 4. Idempotence interne obligatoire
Identité stable : `(execution_id, node_id, iteration_index, attempt)`. La reprise fait un
upsert conditionné dessus. Ce n'est PAS de la coordination distribuée — c'est un upsert.
Non déléguable à l'auteur du pipeline.

## 5. At-least-once + garde de réception
Un node peut être rejoué (side-effects externes possiblement en double). À la réception d'un
node-run, le worker vérifie d'abord si `(exec, node, iter, attempt)` est déjà terminal → si
oui, il ack et jette sans ré-exécuter. C'est CETTE garde (pas le réconciliateur) qui rend le
at-least-once inoffensif.

Clé d'idempotence externe : le moteur propage une clé stable dérivée du node-run-id que les
nodes passent au provider *quand il la supporte*. Best-effort côté effet externe.

## 6. Primitives de contrôle = ensemble FERMÉ
- **Fermé (core)** : Task, Choice, Wait, Parallel, Map, Pass, Succeed, Fail + modificateurs
  Retry, Catch, Timeout. ~10, gelés. Le moteur doit comprendre la sémantique d'exécution de
  *tout* type de node pour garantir durabilité et vérifiabilité.
- **Ouvert (plugins)** : les Tasks (nodes-feuilles) = intégrations. Infini.
- **Macros** : sous-graphes nommés réutilisables qui se *compilent* en primitives. Le moteur
  n'exécute jamais que des primitives.

Règle : plugins = Tasks + macros. **Jamais** de nouvelle primitive de contrôle.

## 7. Secret = type taint-tracké
Voir `04-secrets-security.md`. `Secret<T>` : `[REDACTED]` sur Display/Debug/toString ;
déballable uniquement par la capability I/O du host ; taint contagieux. Les secrets sont
*structurellement inlogguables*. La regex de sortie est un filet de défense-en-profondeur,
pas le mécanisme primaire.

## 8. Terminaison garantie
Trois bornes indépendantes, chacune rattrape ce que les autres ne voient pas :
- **ack-wait / bail** → worker mort (silence).
- **timeout de node** → node en vrille (worker vivant, node qui ne finit pas).
- **TTL d'exécution** → workflow entier bloqué (coordination). Défaut généreux (ex. 30 j),
  surchargeable par pipeline. C'est le disjoncteur de dernier recours.

## 9. Pas de passe-droit
GUI, CLI, MCP, éditeurs tiers = consommateurs de l'API. Aucune capacité au-delà de l'API,
aucun chemin qui échappe à RBAC / scope tenant / RLS / audit. Durcir l'API durcit tous les
clients gratuitement.

## 10. Egress médié (host-mediated)
Un node ne fait jamais d'I/O directe. Il *décrit* l'appel ; le host injecte le credential
hors sandbox ; l'egress est allowlisté **par credential** (`connections.egress_scope`).
Le module WASM ne voit jamais le secret. Ferme l'exfiltration ET le SSRF.

---

## Corollaire : ne jamais confondre annulation et compensation
« Annuler » arrête ; ça ne *défaut* pas ce qui a eu lieu. Rembourser un effet déjà émis est
de la **logique métier** (saga, handler de Catch) composée par l'auteur — jamais une magie du
moteur. Le moteur arrête proprement et *expose ce qu'il sait* de l'état (états `interrupted`
quand le side-effect est incertain).
