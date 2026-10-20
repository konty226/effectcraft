---
name: motion-design
description: Créer, animer, critiquer et exporter des motion designs dans EffectCraft (fork mymotion) piloté par MCP — typographie animée, vidéos verticales réseaux sociaux, logos animés, explainers, data-viz. À utiliser dès que l'utilisateur demande une animation, un titre animé, une vidéo verticale/Reels/TikTok/Shorts, un logo animé, un graphique animé, ou de modifier un projet .ecproj, ou parle de keyframes, easing, timing, export MP4/WebM/GIF.
---

# Motion design avec EffectCraft (mymotion)

Tu pilotes **EffectCraft**, un compositeur de type After Effects, via le serveur MCP `mymotion`
(proxy de `effectcraft-cli mcp` qui ajoute des outils de motion design). En mode live, l'utilisateur
voit l'app s'animer pendant que tu travailles : agis par petites étapes visibles.

## Tout commence par un brief

Chaque travail est un **projet** : `clients/<Client>/projets/<Projet>/` avec `BRIEF.md`, `SCRIPT.md`,
`assets/`, `references/`, `moodboard/`, `out/` et le `.ecproj`.

- **Nouveau travail** : `project_list` (existe-t-il déjà ?), sinon `project_new {client, project,
  brief, script}` en remplissant `brief` avec **tout ce que l'utilisateur a déjà dit** (sections :
  Infos, Objectif et message clé, Public cible, Script, Assets fournis, Références et exemples,
  Mood board, Style, Contraintes et mentions obligatoires, Livrables). Puis pose **en une seule fois**
  les questions sur ce qui manque d'essentiel (`missing`) : objectif, durée et format, plateformes,
  textes/script, assets imposés, style, livrables, échéance. Dis-lui de déposer ses fichiers dans
  `assets/`, `references/` et `moodboard/` (chemins dans la réponse).
- **Avant de créer** : `project_brief` (le projet ouvert par défaut). Lis le brief, le script et les
  notes de références ; **regarde la planche** du mood board et des références et déduis-en la
  direction (palette, lumière, densité, typo, rythme) en l'expliquant en 2–3 phrases à l'utilisateur.
  Utilise les fichiers de `assets/` en priorité sur toute image trouvée ailleurs.
- **Respecte le script** : textes exacts, ordre et minutage de `SCRIPT.md`. Si tu proposes un
  changement, demande-le d'abord.
- **Retour en arrière** : `checkpoint` ne vit que le temps de la session du serveur MCP et l'historique d'annulation
  de l'app ne garde que 32 étapes : avant une grosse modification, sauvegarde aussi une copie du projet
  (`save_project {path: "<Projet> avant <étape>.ecproj"}` puis rouvre l'original).
- **Garde la trace** : `brief_update {section: "notes", content, append: true}` pour chaque
  décision, retour ou version (date, fichier). Les réponses de l'utilisateur vont dans leur section.
- Sauvegarde le projet sous `saveProjectAs` (dans le dossier du projet) et les rendus dans son `out/`.
- Images trouvées sur le web : seulement libres de droits, avec accord de l'utilisateur avant
  téléchargement ; note auteur, page et licence dans `assets/SOURCES.md`. Écarte les images avec
  logos de marques ou enseignes lisibles.

## 3D et VFX : Blender + EffectCraft

La **vraie 3D** (modélisation, matériaux, simulations, suivi de caméra) se fait dans **Blender**
(MCP Blender, `execute_blender_code`) ; **EffectCraft** compose, fait les VFX sur vidéo et la finition.
Dossiers du projet : `3d/` (.blend), `renders/` (séquences), `footage/` (plans tournés).

1. **Logo en 3D** : `vectorize_logo {input: <PNG>, color}` (si pas de SVG fourni), puis dans Blender :
   `exec(open(r"D:/Fork/effectcraft/mymotion/blender/mymotion_blender.py", encoding="utf-8").read())`
   et `logo_from_svg(svg)`, `studio_lights()`, `camera_push()`, `setup_render(<projet>/renders/<plan>)`.
   Sauvegarde le .blend dans `3d/`.
2. **Aperçu avant le vrai rendu** : une image à 40 % et 24 échantillons (`bpy.ops.render.render(write_still=True)`
   en PNG), regarde-la, corrige matériaux/cadrage. Compte ~8–9 s par image 1080×1920 à 96 échantillons
   (Cycles, RTX 2060) : préviens l'utilisateur de la durée.
3. **Rendu** dans un Blender séparé en arrière-plan (commande système) :
   `blender.exe -b <projet>/3d/<fichier>.blend -a`, pour ne pas bloquer l'interface ; suis la progression
   au nombre d'images dans `renders/`.
4. **Compositing** : `import_render {path: <dossier ou name_[0001-0090].exr>}` crée la comp, multiplie la
   passe d'ombre (Shadow Catcher), pose la beauty par-dessus et liste les passes (Depth, Mist, AO,
   Cryptomatte) ; ajoute fond, titres, lueurs, grain comme pour le reste. Les autres passes s'extraient
   avec l'effet EXtractoR (`red/green/blue` = `ViewLayer.Mist.Z`…).
5. **Titres 2D dans la scène 3D (caméra partagée)** : dans Blender, `export_camera(<projet>/3d/camera_<plan>.json,
   objects=(<empties ou objets à suivre>), points={"nom": (x, y, z)})` (après `exec` de la recette) ; dans EffectCraft,
   `camera_from_blender {json}` dans la comp du rendu : caméra à deux nœuds clé par image + nulls 3D pour les repères
   (alignement mesuré < 1 px). Puis `layer.newText` + `layer.setSwitch {switch: "threeD", value: true}` + position 3D
   (coordonnées d'un repère) ou `layer.setParent` sur le null. Échelle : 1 m Blender = 100 px à la profondeur 0 ;
   **la perspective agrandit** un calque proche de la caméra (×≈2,8 à 9 m avec un 50 mm en 1080×1920) : réduis la taille
   du texte en conséquence et vérifie qu'il reste dans le cadre en début ET en fin de mouvement.
6. **Contrôle** : `render_filmstrip` sur la comp ; vérifie le **contraste objet/fond** (un logo bleu sur fond
   bleu se perd : faces plus claires ou chromées, fond assombri derrière), le cadrage dans les safe areas et
   le contact au sol (ombre).

Pièges Blender 5.2 (déjà gérés par la recette) : EXR multicouche = `media_type = "MULTI_LAYER_IMAGE"` puis
`file_format = "OPEN_EXR_MULTILAYER"` ; passe d'ombre dans `view_layer.cycles.use_pass_shadow_catcher` ;
EXR multipart (EffectCraft lit la passe Combined comme image). Si le MCP Blender ne répond pas sur 9876,
lance Blender avec `--python-expr` qui exécute `bpy.ops.blmcp.server_start()` après 3 s.

VFX sur vidéo tournée (dans EffectCraft) : Key Light (fond vert) + nettoyage, suivi de points, 3D Camera
Tracker (ou suivi de caméra Blender, plus robuste), Roto Brush / Object Matte, Content-Aware Fill ; toujours
montrer avant/après en planche contact.

## Démarrage de session

1. `focus_context` : comp active, temps courant, calques/propriétés/clés **sélectionnés par
   l'utilisateur**, image du panneau Composition. « ce calque », « ça », « ici » = la sélection.
   Si aucune comp n'est ouverte, demande ou crée-la.
2. `brand_kit` : couleurs, polices, échelle typographique, safe areas, timing, easing. **Respecte-la**
   sauf demande contraire. Vérifie le champ `client` de la réponse : l'utilisateur a **plusieurs
   clients** (voir « Clients et chartes » ci-dessous). Si ce n'est pas le bon client, corrige avant
   de créer quoi que ce soit.
3. Avant toute modification importante : `checkpoint {"label": "avant <demande>"}`.

## Clients et chartes

Les chartes se cumulent, chaque niveau ne contenant que ce qui change :
`D:/MotionProjects/brand_kit.json` (studio) → `clients/<Client>/brand_kit.json` →
`brand_kit.json` d'un sous-dossier de projets → `<projet>.brand.json` à côté du `.ecproj`.

- Le client est **détecté d'après le dossier du projet ouvert** (`clients/<Client>/…`). Range donc
  chaque projet dans `D:/MotionProjects/clients/<Client>/projets/` (`save_project` avec ce chemin),
  médias dans `…/medias/`, rendus dans `…/out/`.
- `brand_list` : clients existants, client actif, client détecté. `brand_use {"client": "X"}` force
  un client pour la session (`null` = revenir à la détection). `brand_kit {"client": "X"}` lit la charte
  d'un client sans changer de client actif.
- Nouveau client : `brand_new {"client": "Nom"}` (ou `"from": "AutreClient"` pour partir d'une copie),
  puis remplis son `brand_kit.json` avec **uniquement** ce qui diffère du studio (couleurs, polices,
  logo…) : demande les valeurs à l'utilisateur, ne les invente pas.
- Exception pour un seul projet (couleur d'accent de campagne, durée…) : écris un
  `<projet>.brand.json` à côté du `.ecproj` plutôt que de modifier la charte du client.
- Les objets se fusionnent clé par clé ; une valeur simple ou une liste (ex. `colors.chart`)
  **remplace** celle du niveau au-dessus.

## Boucle de travail (toujours)

1. **Construire** par commandes (`command_run`, `command_batch`, ou `batch` atomique avec `$N.layer`).
2. **Regarder le mouvement**, pas une seule image :
   - `render_filmstrip {"frames": 8–12, "from", "to"}` sur la portion modifiée ;
   - `plot_curves {"layer", "path"}` pour toute animation de position/échelle/rotation/opacité
     importante : vérifie `overshootPct`, `peakSpeed` et la forme de la courbe de vitesse.
3. **Critiquer** honnêtement (lisibilité, rythme, safe areas, hiérarchie, cohérence avec la charte),
   corriger, re-vérifier. Montre la planche à l'utilisateur quand c'est utile.
4. Si l'utilisateur n'aime pas : `restore_checkpoint {"label"}` puis autre approche.
5. Sauvegarder : `save_project {"path": "D:/MotionProjects/clients/<Client>/projets/<Projet>/<Projet>.ecproj"}`.

## Conventions EffectCraft (pièges connus)

- **Arguments stricts** : toute clé inconnue est refusée (outil ou commande). En cas de doute :
  `describe_command {"command": "layer.newText"}`.
- **Calques** : par id, `"#n"` (index depuis le haut) ou **nom** → nomme toujours à la création
  (`name`). **Propriétés** par chemin : `transform/position`, `transform/scale` (`[x,y]` en %),
  `transform/rotation`, `transform/opacity` (0–100), `transform/anchor` (point d'ancrage), `effects/#1/<param>`.
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

**Data-viz** — une barre = un shape `rect` avec point d'ancrage en bas (`transform/anchor` = `[0, hauteur/2]`), animer `transform/scale` Y de 0 à la valeur ;
compteur = texte dont la source est pilotée par une expression (chemin donné par `get_layer {flat: true}`) ; couleurs `colors.chart` de la charte, une couleur par série.

**Explainer** — une comp par scène (nommée), assemblées dans une comp maître ; `layer.timing {layers, start}` pour placer ;
transitions 0.3–0.5 s ; marqueurs sur l'audio de la voix off pour caler les entrées.

**Presets d'animation** — `anim.builtinPresets {category?}` puis `anim.applyBuiltinPreset {preset, layers}` (transitions, fonds animés).

## Exports (formats de l'utilisateur)

Sauvegarde d'abord le projet, puis rends depuis la CLI headless (n'occupe pas l'app) :
- MP4 réseaux : `effectcraft-cli render --project P.ecproj --comp "Nom" --format h264 --out D:/MotionProjects/clients/<Client>/projets/<Projet>/out/X.mp4` (rapide) ;
- WebM transparent : `... --format webm --webm-codec vp9 --channels rgba --out X.webm` (lent : ~27 s par seconde en 1080×1920, garder pour la version finale) ;
- GIF : `... --format gif --resolution half --fps 15 --out X.gif`.
GPU : ajouter `--gpu` seulement si la comp est chargée en effets/3D (plus lent sur du texte simple).
