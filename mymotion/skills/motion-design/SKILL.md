---
name: motion-design
description: Créer, animer, critiquer et exporter des motion designs dans EffectCraft (fork mymotion) piloté par MCP — typographie animée, vidéos verticales réseaux sociaux, logos animés, explainers, data-viz. À utiliser dès que l'utilisateur demande une animation, un titre animé, une vidéo verticale/Reels/TikTok/Shorts, un logo animé, un graphique animé, ou de modifier un projet .ecproj, ou parle de keyframes, easing, timing, export MP4/WebM/GIF.
---

# Motion design avec EffectCraft (mymotion)

Tu pilotes **EffectCraft**, un compositeur de type After Effects, via le serveur MCP `mymotion`
(proxy de `effectcraft-cli mcp` qui ajoute des outils de motion design). En mode live, l'utilisateur
voit l'app s'animer pendant que tu travailles : agis par petites étapes visibles.

## Démarrage de session

1. `focus_context` : comp active, temps courant, calques/propriétés/clés **sélectionnés par
   l'utilisateur**, image du panneau Composition. « ce calque », « ça », « ici » = la sélection.
   Si aucune comp n'est ouverte, demande ou crée-la.
2. `brand_kit` : couleurs, polices, échelle typographique, safe areas, timing, easing. **Respecte-la**
   sauf demande contraire.
3. Avant toute modification importante : `checkpoint {"label": "avant <demande>"}`.

## Boucle de travail (toujours)

1. **Construire** par commandes (`command_run`, `command_batch`, ou `batch` atomique avec `$N.layer`).
2. **Regarder le mouvement**, pas une seule image :
   - `render_filmstrip {"frames": 8–12, "from", "to"}` sur la portion modifiée ;
   - `plot_curves {"layer", "path"}` pour toute animation de position/échelle/rotation/opacité
     importante : vérifie `overshootPct`, `peakSpeed` et la forme de la courbe de vitesse.
3. **Critiquer** honnêtement (lisibilité, rythme, safe areas, hiérarchie, cohérence avec la charte),
   corriger, re-vérifier. Montre la planche à l'utilisateur quand c'est utile.
4. Si l'utilisateur n'aime pas : `restore_checkpoint {"label"}` puis autre approche.
5. Sauvegarder : `save_project {"path": "D:/MotionProjects/projets/<nom>.ecproj"}`.

## Conventions EffectCraft (pièges connus)

- **Arguments stricts** : toute clé inconnue est refusée (outil ou commande). En cas de doute :
  `describe_command {"command": "layer.newText"}`.
- **Calques** : par id, `"#n"` (index depuis le haut) ou **nom** → nomme toujours à la création
  (`name`). **Propriétés** par chemin : `transform/position`, `transform/scale` (`[x,y]` en %),
  `transform/rotation`, `transform/opacity` (0–100), `transform/anchorPoint`, `effects/#1/<param>`.
  `get_layer {"layer": "Titre", "flat": true}` donne tous les chemins.
- **Temps** en secondes. `add_keyframe` / `set_property` avec `time` = **temps de calque** ;
  passe `"timeBase": "comp"` si le calque est décalé. `get_property` et `render_frame` = temps comp.
- **Gras** : utilise un vrai style (`"font": "Inter", "style": "Bold"`), pas `fauxBold`.
  Polices disponibles : `list_fonts {"query": "..."}` (Inter : Regular, Medium, SemiBold, Bold, Italic).
- Position = coordonnées comp en pixels, origine en haut à gauche ; le centre d'une 1080×1920 est `[540, 960]`.
- `add_keyframe ... "interpolation": "easyEase"` adoucit toutes les clés passées ; pour une sortie
  accélérée, mets des clés séparées ou ajuste avec `keys.*` (voir `list_commands {"filter": "keys."}`).
- Un `run_script` qui échoue garde ce qu'il a déjà fait : entoure-le de
  `app.beginUndoGroup()` / `app.endUndoGroup()`.
- **Prévisualiser dans l'app** : `command_run {"id": "playback.toggle"}` (lecture/pause).

## Principes d'animation à appliquer

- **Entrées** 0.4–0.7 s avec léger overshoot (5–12 %), **sorties** plus courtes (0.25–0.4 s) en accélération.
- Jamais de mouvement de position linéaire ; pas plus de 2 rebonds.
- **Décalage** (stagger) : ~0.03 s par lettre, ~0.08 s par mot, ~0.1–0.15 s entre éléments.
- Tenir un texte lisible au moins 1.5 s (règle : ~0.3 s par mot + 1 s).
- Un seul point d'attention à la fois ; hiérarchie claire (taille, couleur, ordre d'arrivée).
- Vertical réseaux : **hook visuel dans les 2 premières secondes**, textes dans les safe areas de la
  charte (marges hautes/basses larges, droite dégagée pour les boutons), tailles ≥ minimum de la charte.

## Recettes de base

**Typographie animée** — `layer.newText {text, name, font, style, size, fill, position, justify: "center"}` puis :
- preset intégré : `command_run {"id": "layer.applyTextPreset", "params": {"layer": "Titre", "preset": "bounceInWords"}}`
  (`typewriter`, `fadeUpCharacters`, `bounceInWords`, `trackingIn`, `scramble`, `blurIn`, `jitter`, `dropInLines`) ;
- sur mesure : `layer.addTextAnimator {layer, properties: ["position", "opacity"]}` puis clés sur le range selector.

**Titre pop** — échelle `[0,0]` → `[112,112]` (0.5 s) → `[96,96]` (0.7 s) → `[100,100]` (0.83 s), opacité 0 → 100 en 0.3 s, easyEase. Vérifier avec `plot_curves`.

**Formes / logos** — `layer.newShape {kind: rect|rounded|ellipse|star|polygon, size, fill, stroke, strokeWidth, position, name}` ;
`layer.addShapeItem {layer, kind: "trim"}` renvoie `{path: "contents/trim"}` : clés sur `contents/trim/end` de 0 à 100
pour un tracé qui se dessine (`contents/trim/start`, `offset` aussi). Logo SVG/AI : import fichier puis animation (masque, échelle, trim).

**Data-viz** — une barre = un shape `rect` avec point d'ancrage en bas, animer `transform/scale` Y de 0 à la valeur ;
compteur = texte dont la source est pilotée par une expression (chemin donné par `get_layer {flat: true}`) ; couleurs `colors.chart` de la charte, une couleur par série.

**Explainer** — une comp par scène (nommée), assemblées dans une comp maître ; `layer.timing {layers, start}` pour placer ;
transitions 0.3–0.5 s ; marqueurs sur l'audio de la voix off pour caler les entrées.

**Presets d'animation** — `anim.builtinPresets {category?}` puis `anim.applyBuiltinPreset {preset, layers}` (transitions, fonds animés).

## Exports (formats de l'utilisateur)

Sauvegarde d'abord le projet, puis rends depuis la CLI headless (n'occupe pas l'app) :
- MP4 réseaux : `effectcraft-cli render --project P.ecproj --comp "Nom" --format h264 --out D:/MotionProjects/out/X.mp4` (rapide) ;
- WebM transparent : `... --format webm --webm-codec vp9 --channels rgba --out X.webm` (lent : ~27 s par seconde en 1080×1920, garder pour la version finale) ;
- GIF : `... --format gif --resolution half --fps 15 --out X.gif`.
GPU : ajouter `--gpu` seulement si la comp est chargée en effets/3D (plus lent sur du texte simple).
