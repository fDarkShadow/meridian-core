# 07 — Infrastructure

## Provider & portabilité
Provider de départ : **OVHcloud** (français, hors CLOUD Act, égress gratuit, le plus de
régions EU). Rien de propriétaire : archi **portable**, reproductible en GitOps.

**Portabilité par reproductibilité, PAS par cluster étiré.** Pour changer de provider : monter
un cluster neuf ailleurs (GitOps) + répliquer les données (réplication logique CNPG + sync
RustFS) + basculer le DNS. Ne JAMAIS étaler le control plane / etcd entre providers (etcd =
consensus Raft synchrone, déteste la latence WAN). Seuls des **workers stateless** peuvent
s'exiler. Multi-cluster (par région/juridiction pour les tenants dédiés) via **Cilium Cluster
Mesh**, pas un cluster multi-provider. Règle : **donnée et calcul co-localisés.**

## Control plane
**Self-managed** (pour la portabilité — pas de control plane managé). HA = **3 nodes etcd**
(quorum impair). Jamais de charge applicative lourde sur les nodes etcd en prod.

## Data plane (source de vérité)
- **PostgreSQL via CloudNativePG (CNPG)** : **3 replicas synchrones**, `podAntiAffinity` **dur**
  (jamais 2 replicas sur le même hyperviseur physique). WAL archivé **vers S3 externe
  hors-cluster/hors-provider**. **PITR** (Point-In-Time Recovery).
- **RustFS** (S3-compatible, self-hosted) : blobs WASM, payloads binaires, archives WAL.
- **NATS JetStream** on-cluster.
- Instances data idéalement **NVMe local** (POP2-like) OU block storage réseau (survit au
  remplacement de node, plus simple, ~5k IOPS). Démarrage : block storage (un souci de moins).

## Topologie de démarrage (budget serré)
- **Nodes principaux mutualisés** : K8s CP + etcd + control plane app + NATS + CNPG + RustFS.
  Minimum **3** (quorum etcd + anti-affinité CNPG). ~16 Go/node (marge de survie, pas confort).
- **Node(s) WASM** isolé(s) par **taint** (`workload=wasm:NoSchedule`) **+ nodeAffinity**
  (`workload=wasm`) : le taint empêche les autres d'entrer, l'affinity cloue les pods WASM
  dessus. Sans les deux, l'isolation fuit.
- **Croissance** : grossir le pool WASM (élastique) ; détacher les pools système/data quand la
  marge vient (join de nodes dédiés + taints + drainage). Additif, jamais une refonte.

### Protection de la source de vérité sur cluster mutualisé (4 couches)
| Couche | Agit | Garantit |
|---|---|---|
| anti-affinity | scheduling | perte d'un node = ≤ 1 replica CNPG perdu |
| requests/limits + QoS | allocation | **PG/etcd = Guaranteed** ; **workers = Burstable** |
| PriorityClass / OOM | rupture | le worker WASM meurt avant PG/etcd/CNI |
| `--system-reserved` / `--kube-reserved` | fondation node | toujours de quoi faire tourner kubelet+CNI |

QoS `Guaranteed` (requests=limits) donne l'`oom_score_adj` le plus protecteur → PG/etcd
survivent. Workers en `Burstable` → tués d'abord par l'OOM-killer. CNI en
`system-node-critical`. **Non négociable même en phase radine :** anti-affinité replicas,
WAL hors-cluster, limits strictes sur les workers. Le node WASM reste **stateless** (pas de
bail/état durable dessus ; sa perte est anodine car réconciliateur + idempotence).

## Réseau (voir 08 pour la détection)
- **Cilium** (CNI + mesh de sécurité, eBPF) : NetworkPolicies, policy L7, mTLS/identité,
  observabilité **Hubble**. **Pas d'Istio** sauf besoin de traffic-management L7 avancé
  (canary/circuit-breaking fin) — si un jour requis, Istio **ambient** par-dessus Cilium.
- **WireGuard** (via Cilium) **activé partout** : chiffrement transport node-to-node. Un seul
  mTLS à la fois (Cilium OU Istio, jamais les deux → conflit datapath).
- **Gateway API** partout (pas d'Ingress legacy, pas de CRD mesh propriétaires). « Plus
  d'Ingress » ≠ « plus d'Envoy » : Gateway API est une *spec de config* ; le dataplane reste
  Cilium/eBPF en L7 basique, Envoy réapparaît (embarqué) seulement en L7 avancé.
- **Entrée** : **un seul LoadBalancer managé** (provisionné auto par le cloud controller,
  health-check des backends → node mort retiré en secondes) → Gateway API/Cilium fait tout le
  L7 derrière. Un LB, pas dix. Pas d'ARP/BGP à gérer sur cloud public. (Bare-metal un jour →
  Cilium **L2/ARP**, pas BGP, pour rester « sans routeur ».)
- **Zéro-trust** : NetworkPolicies **deny-all par défaut**, on ouvre **flux par flux**.
  Exceptions minimales : DNS (CoreDNS) + egress contrôlé. Ne pas oublier le DNS (sinon on
  casse le namespace).

## Tenant hard = namespace isolé (GitOps)
« Silo dans un cluster partagé » (ce que fait tout SaaS ; un cluster par client ne scale pas).
Le manifeste GitOps de tenant provisionne un **stack complet** :
namespace + **NetworkPolicies deny-all cross-namespace** + **CNPG dédié** + **stream NATS
dédié** + **RustFS/secrets/DEK isolés** + quotas (+ éventuel nodepool taggé pour le premium).
Onboarding = un `git commit`.

Le namespace isole l'API/scheduling, **pas le kernel** — mais le WASM sandbox donne une
isolation d'exécution supérieure au conteneur multi-tenant. Isolation kernel dure
(nodepool par tenant, gVisor/Kata) = option premium via le même mécanisme de taint.

**Job de quiesce + migration** (réutilise le kill hiérarchique pour geler proprement un
tenant) : sert d'un coup l'upsell soft→dédié, la portabilité région, et la purge RGPD.
Démarrer par migration **à froid (quiesce)** ; à chaud (réplication logique) quand un gros
client l'exige.

## Backups & continuité
**WAL streaming continu** (pas un dump nocturne) → **RPO de quelques minutes**. Restore =
dernier snapshot + rejeu du WAL. Distinguer **RPO** (perte) et **RTO** (temps de
rétablissement) — documenter les deux. **Cohérence Postgres ↔ Object Store** au restore
(sinon `input_ref`/`output_ref` orphelins). WAL/backups **hors-cluster, hors-provider**
(leçon SBG2 : la résilience appartient au client).

## SMTP
Mails transactionnels (presigned URLs HITL, alertes) via un service tiers par API
(Postmark/Resend/TEM…), **pas** un serveur mail sur le cluster (port 25 souvent bloqué).
