# 09 — MCP & autorisation

## MCP = consommateur de l'API (invariant 9)
Le serveur MCP pilote Meridian **au même titre que la GUI/CLI** : un adaptateur de protocole
mince par-dessus l'API de contrôle existante. **Aucun passe-droit.** Il traverse
mécaniquement RBAC + scope tenant + RLS + audit. Rien à réécrire côté sécurité — il hérite de
tout. (Bon test de la propreté de l'API : s'il a besoin d'un raccourci, l'API a un trou.)

Portée : **contrôler les workflows** (lister, inspecter, débugger, lancer, mettre en pause,
tuer). L'essentiel de la valeur est **read-only/diagnostic** (faible risque) ; les mutations
sont rares.

## Par-tenant, dans le namespace
Le serveur MCP vit dans le **namespace du tenant hard** → hérite de l'isolation topologique
(NetworkPolicies deny-all, RLS, ne voit que ses ressources). Isolation stricte gratuite.

## Découpage des outils
- **read-only** (lister, inspecter, diagnostiquer) : accès large.
- **mutating** (lancer, éditer, tuer, supprimer) : RBAC restreint **+ confirmation humaine**
  (le MCP retourne « je vais tuer X, confirme » plutôt que d'exécuter direct). Un LLM peut
  décider seul d'appeler un outil destructeur sur une hallucination → friction volontaire.
- `execution:kill` via MCP exige la **même permission** que partout.

## Auth : OIDC / OAuth 2.1, core IdP-agnostique
- **Keycloak** = Authorization Server (OAuth 2.1, dynamic client registration, scopes). Côté
  nous. Le **core reste agnostique** : il consomme des tokens OIDC/OAuth standard ; n'importe
  quel IdP client convient s'il expose les endpoints de découverte standard.
- Le serveur MCP par-tenant = **Resource Server** : valide le token, extrait `tenant_id` +
  scopes, applique RBAC + RLS.
- **Profil d'autorisation MCP** (OAuth 2.1, RS/AS) : domaine **encore mouvant** — garder la
  couche auth découplée pour suivre l'état de l'art. Le *principe* est stable (déléguer à un
  AS, scoper au tenant, moindre privilège, tout auditer) ; le protocole suit.

## Identité de l'agent ≠ identité de l'humain
- **Moindre privilège de l'agent** : le token de session MCP porte un **sous-ensemble** des
  permissions de l'humain. Scopes dédiés (`mcp:read`, `mcp:execute`, `mcp:kill`) délégués
  explicitement. (Pas de passe-droit règle l'autorité ; le moindre privilège règle la
  *granularité de délégation* — décision produit, activable plus tard sans refonte.)
- **Double identité à l'audit** : `agent:<id>@user:<id>`. Distinguer une action humaine
  délibérée d'une action d'agent sur hallucination.
- **Confused deputy** : le serveur MCP n'a **aucune autorité ambiante** — chaque appel agit
  avec le token scopé de la session, rien de plus. Pas de service account MCP tout-puissant.

## Le mapper rôle-IdP → permission-API (dans le CORE)
Anti-corruption layer. Le core raisonne toujours en **permissions internes**
(`execution:kill`, `workflow:read`…) ; le mapper traduit les rôles IdP entrants. Après cette
frontière, **l'IdP disparaît** — RBAC, RLS, audit, MCP, GUI ne voient que des permissions
internes.
- **Deny par défaut** : un rôle IdP non mappé n'accorde **aucune** permission (allowlist de
  traductions, pas une passoire).
- **Par-tenant** : la table rôle→permission vit au niveau tenant (le rôle `admin` du tenant A
  ≠ celui du tenant B, jamais confondus).
- La **sémantique des permissions appartient au core** ; l'IdP ne fournit que l'identité et
  l'appartenance (claims/rôles). Ne pas laisser la logique de permission fuir dans la config
  Keycloak (sinon couplage à Keycloak).

## BYO model (le modèle du client pilote Meridian)
Le modèle est **externe et non-fiable** ; le serveur MCP est la **frontière de confiance** qui
médie chaque demande.
- Injection de prompt → injection d'action : le RBAC est appliqué **côté serveur, indépendamment
  de ce que le modèle demande**. Le modèle propose, le MCP valide. Le modèle n'est jamais une
  autorité.
- Exfiltration via le contexte : les données renvoyées au modèle **ne contiennent jamais de
  secret** (taint, 04) ; PII en **opt-in explicite** (RGPD). Le BYO model est un choix documenté
  du tenant (le contexte transite vers son modèle).
- Endpoint modèle self-hosted = une **destination egress** comme une autre → passe par
  l'egress-scope (04/03).
