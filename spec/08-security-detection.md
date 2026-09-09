# 08 — Détection & cybersécurité

La prévention (host-mediated, egress-scope, WASM sandbox, taint, deny-all) élimine la voie
d'exfiltration la plus courante *par construction*. La détection est le filet **« si on se
fait poutrer »**.

## Le point d'étranglement egress = notre meilleur capteur
Tout l'egress passe par la médiation I/O du host → **un seul entonnoir** à instrumenter (pas
de sniff diffus). On y pose la détection comportementale **par tenant** : volume anormal,
débit anormal, destination inhabituelle. Déjà dans le chemin de code.

## Pile de détection
| Agent | Couche | Rôle |
|---|---|---|
| **Tetragon** (eBPF, Cilium) | runtime kernel | détecte **ET bloque** : syscalls, connexions, accès fichiers. RCE → socket inattendue tuée. Se greffe sur Cilium déjà présent. |
| **Hubble** (Cilium) | flux L3-L7 | observabilité réseau, baseline. **Parser Hubble → CrowdSec (déjà en place).** |
| **CrowdSec** | nord-sud | anti-abus sur surfaces exposées (webhooks, presigned URLs, auth, free tier). Bannit les IP. |
| **audit hash-chaîné** | applicatif | substrat de détection (accès de masse anormal par tenant). |
| **Vector** | collecte | collecte in-cluster → pousse vers le SIEM. |
| **Wazuh** | SIEM (infra interne) | corrélation. L'exfiltration = rarement un signal isolé. |

Priorité d'implémentation : Tetragon → détection d'anomalie sur la médiation egress →
corrélation Wazuh via Vector → CrowdSec (déjà) → baselines **par tenant** (pas globales).

## CrowdSec — piège d'install
Il lit des logs et doit voir les **vraies IP sources**. Derrière un LB, propager
`X-Forwarded-For` / proxy protocol du LB jusqu'à l'ingress, sinon il bannit le LB ou rien.
Bouncer au niveau ingress.

## Angles morts (à instrumenter explicitement)
1. **Credential légitime abusé** : un token compromis qui pousse lentement des données via sa
   destination *autorisée* (l'API Stripe elle-même) → l'egress-scope ne voit rien. **Détection
   volume/débit** sur la médiation, pas la destination.
2. **Exfiltration au repos** : dump direct de Postgres ou RustFS. → audit Postgres + logs
   d'accès RustFS + détection de requêtes anormales (`SELECT *` massif hors pattern). Ne pas
   tout concentrer sur l'egress applicatif.
3. **Host compromis** (pas juste un workflow tenant) : l'attaquant est *dans* la frontière de
   médiation → Tetragon/Falco au niveau **node** (surveillent l'hôte, dernier rempart).

## Zéro-trust identité (à l'échelle conformité)
Les NetworkPolicies filtrent par position réseau (couche 3/4). Le vrai zéro-trust ajoute
l'**identité cryptographique** (mTLS Cilium). Position + identité. Pas requis jour 1 ; à
ajouter quand un client conformité l'exige.

## Abus du free tier (protège la survie du compte provider aussi)
Limites de **concurrence** (enforced au dispatch, par-tenant) + volume + cardinalité Map
bornée + timeout d'exécution court sur le free. + vérification email/carte à l'inscription.
Sur Hetzner/OVH, un abuseur (cryptomining, scraping sortant) peut faire suspendre *ton*
compte → ces protections ne sont pas du confort.
