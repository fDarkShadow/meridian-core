# 10 — Licence & gouvernance des droits

> **Pas un conseil juridique.** La rédaction exacte (Additional Use Grant, CLA, frontière
> œuvre dérivée vs usage via SDK) relève d'un avocat spécialisé OSS. Ce fichier fige
> l'*intention* et ce qui impacte le code (en-têtes, provenance).

## BSL (Business Source License), jour-zéro
Pas d'EE/CE. **Une seule édition, tout ouvert** (audit, SSO via Keycloak, multi-tenant
compris). On monétise la **plateforme** (cloud managé) et l'**usage commercial de revente**
(licence), jamais le déverrouillage de features.

BSL adoptée **dès le départ** (pas de reconversion → pas de bad buzz). Trois paramètres :
- **Additional Use Grant** : « usage interne, dev, embarquement **libres** ; seul l'usage qui
  *offre le produit comme service à des tiers* requiert une licence commerciale. » ← le scalpel.
- **Change Date** : 4 ans après chaque release, **glissant par version** (le récent est BSL,
  l'ancien déjà libre).
- **Change License** : **Apache 2.0** (compat CNCF d'avance sur le code basculé).

Vitrine : « **source-available, zéro feature fermée, open source à date fixe** ». **Jamais**
« open source » nu (openwashing). Argument : plus ouvert que n8n (fair-code, sans bascule) et
Windmill (core amputé).

## CLA obligatoire (pas DCO)
Dès le **premier contributeur externe** (rétrofit = cauchemar). Le CLA donne le **droit de
re-licencier** (scénario fondation → GPL/MIT ; le DCO ne le donne pas). Doit inclure une
**clause IA** : le contributeur (a) atteste avoir le droit de contribuer, (b) divulgue si la
contribution est significativement générée par IA, (c) confirme l'absence de dépendance de
licence problématique.

## Protégeabilité & provenance (impacte le workflow, pas le code)
Une sortie purement machine peut n'être protégée par personne → toute la stratégie BSL/CLA
repose sur des droits d'auteur *valides*. Trois mécanismes qui se renforcent :
- **Review humain de chaque PR** (substantiel, tracé) → bascule « sortie machine » vers
  « œuvre dirigée par un humain ». La règle « chaque PR par ma main » est non-négociable.
- **Scan de licence (SCA) en CI** → provenance : un agent peut recracher du copyleft
  d'entraînement. Une contamination empoisonnerait le droit de re-licencier. **Ne pas
  introduire de dépendance copyleft incompatible.**
- **CLA** → droit de céder / re-licencier.

Côté Anthropic : les CGU cèdent les droits sur les outputs à l'utilisateur ; pas d'ayant droit
caché. Le point de vigilance est « le code est-il protégé *tout court* », couvert ci-dessus.

## En-têtes de fichier
Chaque fichier source porte l'en-tête BSL standard (licence, Change Date, Change License).
Templatiser dans le scaffolding du repo.
