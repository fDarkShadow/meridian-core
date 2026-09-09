# Meridian — contexte projet (à lire en premier)

> **Nom de code : Meridian.** Remplacer partout par le nom définitif quand il sera choisi.

Meridian est un **moteur d'orchestration de workflows durable, source-available**.
Principe fondateur : **le workflow est une donnée (un graphe déclaratif interprété), pas du code.**
Le moteur *interprète* un graphe ; il ne rejoue jamais du code impératif.

Ce fichier est le contrat de travail. Les détails vivent dans `spec/`. **Avant d'écrire du
code sur un domaine, lis le fichier `spec/` correspondant.** Ne ré-ouvre pas une décision
déjà figée : si une contrainte te gêne, signale-le, ne la contourne pas silencieusement.

---

## Les invariants non-négociables (voir `spec/00-invariants.md`)

Ces règles ne se violent jamais. Si une implémentation semble l'exiger, c'est
l'implémentation qui est fausse — pas l'invariant.

1. **Frontière durable / best-effort.** Tout mécanisme est soit durable (transactionnel,
   source de vérité), soit best-effort (transitoire, rattrapable). Jamais entre les deux.
2. **Postgres est la SEULE source de vérité.** NATS ne transporte que la *notification*
   d'un fait déjà rendu durable — jamais le fait lui-même. Rien ne lit NATS pour connaître
   la vérité.
3. **Durabilité aux frontières de node.** L'état est cohérent *entre* les nodes, jamais au
   milieu de l'un. La frontière de node est le seul point d'interruption/reprise sûr.
4. **Idempotence interne obligatoire** par `(execution_id, node_id, iteration_index, attempt)`.
   Upsert conditionné dessus. Non déléguable à l'auteur du pipeline.
5. **At-least-once** sur l'exécution (side-effects possiblement rejoués). Une **garde
   d'idempotence à la réception** jette le travail déjà terminal venu du bus.
6. **Primitives de contrôle = ensemble FERMÉ possédé par le core.** Les Tasks (nodes-feuilles)
   sont ouvertes (plugins WASM). Les macros se *compilent* en primitives. Un plugin n'ajoute
   JAMAIS une primitive de contrôle.
7. **Secret = type taint-tracké.** La matière secrète et les champs `sensitive` ne circulent
   jamais en clair loggable. La regex de sortie n'est qu'un filet.
8. **Terminaison garantie.** Toute exécution atteint un état terminal en temps borné
   (3 bornes : ack-wait, timeout node, TTL exécution). Aucun chemin vers le blocage éternel.
9. **Pas de passe-droit.** Toute interface (GUI, CLI, MCP) est un *consommateur de l'API*.
   Aucune ne dispose d'un pouvoir que l'API n'expose pas, ni d'un chemin qui échappe au
   RBAC / scope tenant / RLS / audit.
10. **Egress médié.** Un node ne fait jamais d'I/O directe : il *décrit* l'appel, le host
    injecte le credential hors sandbox, l'egress est allowlisté *par credential*.

---

## Stack (décidée)

| Couche | Choix | Notes |
|---|---|---|
| Source de vérité | **PostgreSQL** (self-managed, CloudNativePG) | 3 replicas synchrones, anti-affinité |
| Transport / nerf | **NATS JetStream** | dispatch, triggers, progress, Object Store |
| Stockage objets | **RustFS** (S3-compatible, self-hosted) | blobs WASM, payloads binaires, archives WAL |
| Exécution de node | **WASM in-process** (Wasmtime / Extism) | Component Model / WIT pour l'interface |
| Protocole core↔node | **gRPC / WIT** | pas d'API REST « lisible » requise |
| Format de définition | **JSON + sémantique ASL** (Amazon States Language) | YAML accepté en authoring → compilé en IR |
| Expressions | **JSONata** (ou CEL) — sandboxé, non-Turing-complet | field-access sans moteur de template maison |
| Provider (start) | **OVHcloud** | portable ; voir `spec/07-infra.md` |
| Réseau | **Cilium** (CNI + mesh, eBPF) + WireGuard partout | Gateway API ; pas d'Istio sauf besoin L7 avancé |
| IdP | **Keycloak** côté nous ; core **IdP-agnostique** (OIDC/OAuth 2.1) | mapper rôle→permission dans le core |
| Détection | Tetragon (runtime) + CrowdSec (nord-sud) + Wazuh (SIEM) + Vector | |
| Licence | **BSL** jour-zéro → Apache 2.0 (bascule 4 ans glissante) | CLA obligatoire ; voir `spec/10-licensing.md` |

**Convention de langue :** prose et commentaires en français ; identifiants de code, noms de
tables/colonnes, états et scopes en anglais (`node_runs`, `leased_until`, `execution:kill`…).

---

## Le contrat de core (la ligne qui fait tout tenir)

Le core possède quatre choses ; la GUI/les éditeurs n'en possèdent AUCUNE :
(a) le schéma de graphe, (b) le modèle de données/items, (c) la sémantique
d'expressions/référencement, (d) le protocole d'exécution de node.
Toute autre surface (GUI, CLI, MCP, éditeurs tiers) est un *client* de ce contrat.
Le core exprime l'autorisation en **permissions internes** ; l'IdP est traduit une seule
fois à l'entrée par le mapper (`spec/09-mcp-auth.md`).

**State-machine dans le core, projection dataflow dans l'éditeur.** Le core est rigoureux
(ASL) ; l'ergonomie « flow » est une projection de la GUI, pas une sémantique du core.

---

## Comment travailler sur ce repo

- **Lis le `spec/` du domaine avant de coder.** Plusieurs peuvent s'appliquer.
- **Respecte les colonnes réservées v1** (`spec/02-data-model.md`) : elles sont dans la clé,
  le type ou l'enum. Les ajouter après coup = migration douloureuse ou réinterprétation
  d'historique. Ne les omets jamais « pour aller vite ».
- **Idempotence et durabilité d'abord.** Tout code qui touche l'état d'exécution doit être
  correct sous crash/redelivery avant d'être optimisé.
- **Ne mets jamais de secret dans un log, un output, un contexte de modèle, ou l'IR.**
  Utilise le type `Secret<T>` taint-tracké.
- **Quand tu hésites entre « simple mais viole un invariant » et « plus de travail mais
  correct », choisis correct** et explique le surcoût.
- Chaque PR est relue par un humain (protégeabilité + provenance, voir `spec/10-licensing.md`).
  Un scan de licence (SCA) tourne en CI : n'introduis pas de dépendance copyleft incompatible.

---

## Index des specs

| Fichier | Domaine |
|---|---|
| `spec/00-invariants.md` | Les règles non-négociables (détaillées) |
| `spec/01-execution-model.md` | Sémantique ASL, primitives fermées, macros, expressions |
| `spec/02-data-model.md` | Schéma v1, colonnes réservées, states, content-addressing |
| `spec/03-node-protocol.md` | WASM in-process, WIT, host-mediated I/O, cycle de vie |
| `spec/04-secrets-security.md` | SecretProvider, envelope encryption, taint, egress-scope |
| `spec/05-observability.md` | Audit hash-chaîné, outbox, logs OTel, rétention |
| `spec/06-durability-lifecycle.md` | Baux, réconciliateur, Map, annulation, HITL, webhooks |
| `spec/07-infra.md` | Cluster, CNPG, réseau Cilium, tenant hard GitOps, backups |
| `spec/08-security-detection.md` | Tetragon, CrowdSec, Wazuh, Vector, angles morts |
| `spec/09-mcp-auth.md` | MCP consommateur d'API, OIDC, mapper rôle→permission |
| `spec/10-licensing.md` | BSL, CLA, en-têtes de fichier, provenance IA |
| `spec/11-build-order.md` | Ordre d'implémentation suggéré + sujets ouverts |
