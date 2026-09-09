# 05 — Observabilité & audit

## Deux systèmes distincts, exigences opposées — NE PAS mélanger

### Log d'audit = DONNÉE (pas du log)
Event store **append-only, hash-chaîné**, dans **Postgres** (source de vérité, requêtable).
Propriétés non-négociables :
- append-only révoqué au **niveau rôle DB** (pas par convention) ;
- horodaté, attribué (`actor` + `tenant` + trace-id) ;
- **hash-chaîné** : chaque entrée inclut le hash de la précédente (tamper-evident) ;
- écrit **transactionnellement, dans la même transaction que l'action auditée**.

Le **hash-chaînage se calcule à l'écriture DB**, jamais dans un subscriber (l'ordre canonique
appartient à la table ; l'ordre de livraison NATS corromprait la chaîne). Insertion
sérialisée par tenant (chaque entrée dépend du hash de la précédente) — point de
sérialisation assumé.

### Log applicatif = diagnostic éphémère
Structuré (JSON), niveau, corrélé par trace-id. Modèle **OpenTelemetry**. Volumineux,
samplable, rétention courte. Best-effort — perdre une ligne n'est pas un incident.

## Pattern OUTBOX (interdiction du dual-write) — CRITIQUE
**NE JAMAIS** faire naître l'audit d'un subscriber NATS (« worker publie event d'audit → sub
l'écrit en DB »). Ça rouvre le dual-write : crash après commit mais avant publish → **action
réelle, zéro trace d'audit**. Un audit avec des trous est pire qu'inutile.

Ordre correct (DB d'abord, NATS dérive) :
1. Worker, **une seule transaction** : changement d'état (node-run) + entrée `audit_log`
   (append, hash-chaînée) + ligne `outbox`. À cet instant l'audit EST durable et complet.
2. **Relay séparé** (CDC / réplication logique / poller sur `outbox`) publie sur NATS.
3. Subscribers NATS font le fan-out **best-effort** : websocket, mail, syslog, otlp, SIEM…

NATS down → aucune entrée perdue (elles sont en base, le relay rattrape). Le bus porte la
*notification* d'un fait déjà durable, jamais le fait.

Frontière : **avant le commit = la garantie (fermé, core)** ; **après le commit = les
subscribers du fan-out (ouvert, extensible)**. Un nouveau canal = un nouveau subscriber, sans
toucher au chemin critique.

## Exporters (sinks) pluggables
Événement produit **une fois**, schéma canonique (OTel). Exporters interchangeables :
`file`, `s3`, `syslog`, `otlp` (Loki/Elastic/Datadog…), `websocket`. Même pattern que
`SecretProvider`.

Le **websocket** n'est PAS un sink de rétention : c'est le live-tail vers l'éditeur, via le
**pub/sub NATS bridgé** (le même système nerveux). Ne pas construire un second chemin
temps-réel.

## Visualisation façon Airflow = gratuite
DAG coloré par statut, erreurs par node, drill-down (input/output/attempts/timing), timeline
= **requêtes SQL** sur `node_runs` / `executions` / `audit_log`. Le live vient du pub/sub
NATS. Un read-model dédié (projections dénormalisées) seulement le jour où les requêtes
directes pincent — pas d'avance.

## Rétention
`retention_class` par flux (voir 02) : audit (années, immuable) / applicatif (jours) /
payloads (très court, purgeable pour droit à l'effacement RGPD — savoir où sont les données,
donc ne pas les éparpiller dans des fichiers).
