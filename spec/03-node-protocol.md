# 03 — Protocole de node (WASM in-process)

## Un node = un module WASM in-process
Runtime : **Wasmtime** (ou framework **Extism**). Interface standard via **WIT /
Component Model**. Un « worker » est un process hôte qui **charge et exécute plusieurs
modules WASM in-process** — un node n'est PAS un container. (On fuit le modèle pod-per-step
type Argo.)

Pourquoi WASM : satisfait simultanément in-process (pas d'enfer ops), sandboxé (isolation
mémoire dure), polyglotte (Rust, Go/TinyGo, JS/QuickJS, Python/componentize-py).
Perf : ~1.5–2× overhead sur du calcul lourd, mais **95 % des nodes sont I/O-bound** (appels
API distants) → non-sujet. Dimensionner **RAM-first** (mémoire linéaire par instance).

## Host-mediated I/O (CRITIQUE — invariant 10)
Le node **ne fait pas** l'appel réseau. Il le **décrit**. Le host expose une capability
(ex. `http`) ; le node dit « POST vers ce provider, ce body » ; **le host injecte le
credential au moment de l'appel, hors sandbox**. Le module WASM ne voit JAMAIS le secret.

Egress **lié au credential** : le credential déclare son `egress_scope` ; le host **refuse**
d'attacher le secret si la cible sort du scope. Ferme l'exfiltration (un node malveillant qui
met `url=attacker.com` ne reçoit pas le token) ET le SSRF.

Le node devient : logique pure + déclaration d'intention d'I/O. Zéro autorité ambiante
(capabilities WASI granulaires — on grante exactement quelles fonctions host il peut appeler).

## Cycle de vie & blast radius
- **Epoch-interruption** (Wasmtime) : timeout dur sur un module en vrille. Sert aussi au kill
  (voir 06) — même marteau, déclencheur différent (temps vs signal).
- **Limites mémoire** par instance. **Une instance par invocation** (ou pool à état non
  partagé) pour l'isolation multi-tenant. Un module qui panic ne tue pas le worker.
- **Cleanup des ressources host** (sockets, fichiers, handles ouverts via capability) lié au
  cycle de vie de l'instance, sur **TOUS les chemins de sortie** (fin normale, trap
  epoch-interruption, panic). Ordre impératif : **fermer les ressources host D'ABORD, puis
  persister le snapshot d'état.** (Fermer le monde extérieur, puis graver la vérité.)

## Pas de zombie
Un module WASM n'est pas un process OS (pas de PID, pas de `wait()`). On drop l'instance, la
mémoire linéaire est récupérée. Le seul « zombie » réel est **logique** : un node-run
`running` d'un worker crashé → réglé par bail + réconciliateur (voir 06), pas par l'OS.

## Distinction : node WASM vs credential OAuth
Le host possède le cycle de vie du credential (refresh OAuth inclus). Le node ne connaît
qu'un **handle opaque**, jamais le token ni le refresh flow.

## Caveats calendrier (à sonder par un spike avant engagement)
Component Model / WASI Preview 2 se stabilise (2025-2026) ; `wasi-http` est le maillon le
moins mûr. JS/Python en WASM sont lourds (moteur embarqué). TinyGo a des limites (pas 100 %
du langage). Rien de bloquant, mais valider tôt.
