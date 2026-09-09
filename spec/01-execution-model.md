# 01 — Modèle d'exécution

## Sémantique : Amazon States Language (ASL)
Le core est une **machine à états** au sens ASL / Step Functions. On vole la *sémantique*,
pas forcément la syntaxe JSON exacte. Raison : loops, retry, parallel, error-handling
deviennent des citoyens de première classe **orthogonaux** — pas des nodes bricolés.

**State-machine dans le core ; projection dataflow dans l'éditeur.** Ne jamais laisser la
sémantique fuiter dans la GUI.

## Types d'états (primitives fermées)
| État | Rôle |
|---|---|
| `Task` | invoque un node-feuille (plugin WASM) |
| `Choice` | branchement conditionnel |
| `Wait` | attente (durée, ou signal → voir HITL dans 06) |
| `Parallel` | branches concurrentes, fan-in |
| `Map` | itération sur une collection, fan-out/fan-in (voir 06) |
| `Pass` | transformation sans effet |
| `Succeed` / `Fail` | terminaux |

## Modificateurs (attributs, PAS des nodes)
- `Retry` (RetryPolicy) — backoff, max attempts
- `Catch` — routage d'erreur vers une branche
- `Timeout` (TimeoutSeconds) — borne de durée du node

Ne JAMAIS créer un « node Retry », « node Loop », « node Wait » séparé. Ce sont des
attributs d'état ou des états composites.

## Field-shaping au boundary, PAS d'edges par champ
Les edges restent **node→node** (le contrôle). Le shaping des champs se fait à
**l'entrée/sortie de chaque état** (la donnée), via les équivalents ASL :
`InputPath` / `Parameters` / `ResultSelector` / `ResultPath` / `OutputPath`.

Ne pas modéliser des edges par champ : spaghetti + casse sur donnée dynamique.

## Schéma de sortie explicite par node
Chaque node déclare un **schéma de sortie** (contrat, pas blob). Permet :
- autocomplétion et validation **statiques** dans l'éditeur (vs expressions stringly-typed
  de n8n qui pètent au runtime) — c'est le vrai « mieux que n8n » ;
- annotations `sensitive: true` par champ → redaction dirigée par le schéma (voir 04/05).

## Expressions
**JSONata** (ou CEL) — sandboxé, non-Turing-complet, field-access natif. **Ne jamais**
écrire un moteur de template maison (langage de programmation accidentel + couplé au modèle
de données). AWS Step Functions a adopté JSONata fin 2024 : c'est le design validé.

## Format & compilation
YAML/JSON en authoring (GitOps) → **parse → validation contre schéma → compilation en IR
normalisé (macros expansées) → stocké content-addressed dans Postgres**.
**Le moteur exécute l'IR compilé, jamais le texte.** C'est ce qui réconcilie « Postgres
source de vérité » et « tout est YAML en interne », et rend gérable le versioning des
exécutions in-flight.

Footguns YAML à valider dur (Norway problem `no`/`off`→bool, coercion de types, anchors).
Forme canonique = AST parsé, pas les octets.

## Modèle d'items
Le modèle de données (items qui traversent, paired-item pour la lignée à travers merges et
branches) est **dans le core**, couplé à la sémantique d'expressions. Il fait partie du
contrat de core (voir CLAUDE.md), pas de la GUI.
