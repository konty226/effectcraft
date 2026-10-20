# Plan d'évolution — mon éditeur de motion design piloté avec Claude

> Fork de [storytold/effectcraft](https://github.com/storytold/effectcraft) : `konty226/effectcraft`, cloné dans `D:\Fork\effectcraft`.
> Analyse faite le 18 octobre 2026 sur `upstream/main` `f3f24ef1` (v0.12.0 en développement, dernière release v0.11.1).

---

## 1. Ce que j'ai trouvé (synthèse de l'analyse)

### 1.1 En une phrase

EffectCraft est un clone open source d'After Effects en Rust pur, **déjà conçu pour être piloté par des agents** : un serveur MCP complet existe (`effectcraft-cli mcp`), sur le même modèle que PhotoCraft (même famille « Crafting Apps », mêmes outils `command_run` / `command_batch` / `doc_inspect` / `render_preview`). Le travail n'est donc **pas** de construire l'intégration Claude, mais de la **spécialiser** pour ma façon de travailler.

### 1.2 Fiche technique

| Sujet | Constat |
|---|---|
| **Langage / stack** | Rust 2024 (≥ 1.95), workspace de 32 crates + 3 apps + `xtask`. UI **egui/eframe 0.36**, GPU **wgpu 30** (Direct3D 12 / Vulkan / Metal / WebGPU), rendu CPU parallèle (rayon, tiny-skia), texte (skrifa + harfrust), expressions et scripts **JavaScript via boa**. Build web (WASM) aussi. |
| **Taille** | ≈ 389 000 lignes de Rust, ≈ 3 700 tests. Code écrit quasi entièrement par des agents (Opus 5.5), avec des règles strictes. |
| **Architecture** | Couches L0→L6 vérifiées par `cargo xtask layers` : `time`/`geom`/`color` → `raster`/`keyframe`/`path` → `project`/`text`/`effects` → `render`/`gpu`/`export`/`lottie`/`plugin` → `engine` (Session, undo, **registre de commandes**) → `ui-egui` / `automation` (MCP) → apps `effectcraft` (bureau), `effectcraft-cli`, `effectcraft-web`. Le moteur ne dépend jamais de l'UI. |
| **Principe clé** | **Tout est une commande** (id, label, menu, raccourci, params JSON validés, `enabled()`, `run()`). Menus, raccourcis, CLI, canal de contrôle et MCP passent par le même registre → tout ce que je fais à la souris, Claude peut le faire. |
| **Modèle de données** | `Project { items, render_queue }` → `Comp { taille, fps, durée, layers, markers }` → `Layer { source (footage/comp/solid/texte/forme/null/caméra/lumière), in/out/start, stretch, parent, blend, matte, props }`. Tout ce qui s'anime est une `Property` dans un arbre `PropGroup`, adressée par chemin (`transform/position`, `effects/#1/blurriness`, `@uid`). Temps exact en `Tick` entiers (254 016 000 000/s), keyframes en **temps de calque**. |
| **Animation** | Keyframes linéaire / Bézier / hold / auto / continu, roving, Easy Ease, Graph Editor (valeur + vitesse), expressions AE (`wiggle`, `loopOut`, `thisComp.layer(...)`), animateurs de texte, puppet, time remap. |
| **Effets** | 306 effets (les 298 d'AE + extras), 9 styles de calque, CPU + GPU (shaders compute wgpu vérifiés contre le CPU). 54 presets d'animation originaux (texte, transitions, fonds). |
| **Rendu** | `Renderer::comp_frame(comp, t)` : source → masques → effets → styles → transform (motion blur) → matte → blend. 3D classique + « Advanced 3D » (glTF/OBJ/FBX, caméras, lumières, ombres). |
| **Export** | Render Queue type AE : **H.264 MP4**, **ProRes MOV** (jusqu'à 4444 XQ avec alpha), HEVC, AV1, **WebM VP9 + alpha**, séquences PNG/JPEG/TIFF/EXR, **GIF**, WAV/AIFF, **Lottie JSON / .lottie** (import et export). Encodeurs 100 % Rust, **pas de FFmpeg**, pas d'accélération matérielle (NVENC absent). |
| **Format projet** | `.ecproj` = **JSON lisible et versionné** (diffable dans git) ; `.ecprojx` = XML. Pas d'ouverture des `.aep`. |
| **Templates** | **Essential Graphics** : contrôles exposés d'une comp, export en `.ectemplate`, rendu en série en CLI : `effectcraft-cli render --template lt.ectemplate --values values.json --out ...`. Idéal pour du motion design « par données ». |
| **Extensibilité** | (1) Scripts JS au modèle objet d'AE (`app.project`, `comp.layers.addText`…) + panneaux **ScriptUI** dockables ; (2) **plug-ins d'effets WebAssembly** sandboxés (API v1, interpréteur wasmi) ou en Rust ; (3) presets `.ecpreset` ; (4) dossier `extensions/` pour outils livrés en script. |
| **Intégration agents (existante)** | Serveur MCP stdio, **headless** (`mcp`, `--demo`, `--project`, `--gpu`, `--autosave`) ou **bridgé** à l'app ouverte (`mcp --bridge 9877` + `effectcraft --control 9877`). ≈ 36 outils : `command_list/run/batch`, `doc_inspect`, `render_preview`, `get_layer/get_property/set_property`, `add_keyframe`, `add_effect`, `list_effects`, `list_fonts`, `run_script`, `batch` atomique avec références `$N.key`, `undo/redo/history` (arbre d'historique), `render_frame` (PNG, alpha possible) ; en mode bridge : `screenshot`, `ui_inspect`, `ui_click`, `ui_drag`, `ui_key`, `ui_type`, `ui_set`. Ressources `effectcraft://document` et `effectcraft://commands`. Confinement des fichiers par `--automation-read-root` / `--automation-write-root` (comme PhotoCraft). Progression et annulation des rendus longs. `.mcp.json` fourni. |
| **Qualité du code** | Très disciplinée : règle « Never crash » (pas d'`unwrap`/`panic`/`unsafe` hors tests), `unsafe_code = "forbid"`, clippy `-D warnings`, test de régression par bug, couches vérifiées, attribution des assets vérifiée. Faiblesses : **aucune CI sur les PR** (le `cargo xtask ci` ne tourne qu'en local), fidélité vs AE presque pas mesurée, 6 crashs connus ouverts. |
| **Maturité** | **Alpha**, premier commit le **1er octobre 2026** (!). Les mainteneurs estiment « ≈ 91 % des fonctions d'AE existent » mais « ≈ 45 % prêt pour du travail pro ». **macOS est la plateforme la plus testée** ; Linux/Windows ont des bugs d'interaction (dont #613 : compositeur Direct3D 12 qui échoue sur certains GPU Windows). |
| **Maintenance** | Extrêmement active : **2 187 commits en 18 jours** (≈ 800 cette dernière semaine), 7 255 étoiles, 3 855 forks, release tous les 2 à 4 jours, 411 issues et 178 PR ouvertes. |
| **Licence** | **MIT OU Apache-2.0** (au choix) → fork modifié, usage perso et même commercial **autorisés**. Obligations : garder `LICENSE-*`, `NOTICE`, `ATTRIBUTION.md` (polices OFL, etc.). ⚠️ Les **marques ArtCraft** (`docs/brand/`, nom, logos, liens Discord/getartcraft dans l'app) ne sont **pas** sous licence libre : si je **publie ou distribue** une version modifiée, je dois les retirer et ne pas la présenter comme un produit ArtCraft. Je peux écrire « basé sur EffectCraft par l'équipe ArtCraft ». |

### 1.3 Ce que ça change pour moi

- ✅ **L'intégration Claude existe déjà et est solide** : Claude peut créer des comps, animer, appliquer des effets et regarder une image rendue dès aujourd'hui.
- ✅ Le format JSON + git + historique branché = collaboration traçable avec Claude.
- ✅ Les templates Essential Graphics + rendu CLI = production en série.
- ⚠️ **Rust n'est pas installé sur ma machine** : il faut rustup + Visual Studio Build Tools (MSVC) avant tout. Premier build long (≈ 15–30 min en release).
- ⚠️ Je suis sur **Windows 11** : la plateforme la moins rodée. Le GPU (NVIDIA) peut poser problème → rendu CPU en secours.
- ⚠️ L'amont bouge **très** vite : un fork qui modifie le cœur sera vite en conflit. **Stratégie : fork « mince »** (voir §3).

---

## 2. Ce qui manque pour en faire *mon* outil de motion design avec Claude

L'existant permet à Claude d'**agir** ; il est plus faible pour l'aider à **juger une animation** et à **travailler dans mon style**.

| Manque | Pourquoi c'est important | Réponse prévue |
|---|---|---|
| Claude ne voit qu'**une image à la fois** (`render_frame`) | Le motion design se juge dans le temps (rythme, easing, enchaînements). | Outil `render_filmstrip` (planche contact de N images avec timecodes) + `render_clip` (GIF/MP4 court basse résolution) + `plot_curves` (graphe des courbes de valeur/vitesse en image). |
| Pas de **vocabulaire métier** haut niveau | « Fais une entrée de titre élastique » = 15 appels bas niveau. | Bibliothèque de **recettes** (scripts JS / `batch`) exposées en outils MCP et en *prompts* MCP : titre, lower third, logo sting, transitions, compteur, kinetic typo, formats réseaux. |
| Pas de **charte graphique** | Chaque projet doit respecter mes couleurs, polices, durées, marges. | `brand_kit.json` (couleurs, polices, logo, safe areas, easing préférés) lu par une ressource MCP `mymotion://brand` et appliqué par les recettes. |
| Pas de **contexte partagé « ce que je regarde »** | En mode bridge, Claude doit deviner ce que je pointe. | Outil `focus_context` : comp active, CTI, calques/keyframes/propriétés sélectionnés, zone de travail, + capture du viewer — en un seul appel. |
| Pas de **revue des changements** | Je dois voir ce que Claude a modifié avant d'accepter. | `diff_since {checkpoint}` lisible (calques/propriétés/keyframes ajoutés, modifiés, supprimés) en s'appuyant sur l'historique et le JSON `.ecproj` ; marqueurs « Claude » dans l'historique. |
| Démarrage MCP lent / fragile | Le `.mcp.json` fourni lance `cargo run` (compile au démarrage → timeout). | Binaire compilé une fois, `.mcp.json` pointant sur `target\release\effectcraft-cli.exe` avec racines de lecture/écriture vers mon dossier de projets. |
| Pas de **Skill Claude** dédié | Claude redécouvre les conventions (temps de calque vs comp, chemins de propriétés…) à chaque session. | Un skill `motion-design` : conventions de l'outil, pièges (docs/agents.md « Pitfalls »), principes d'animation (anticipation, overshoot, timing 12/24 images…), ma charte, mes recettes. |

---

## 3. Mes choix (réponses du 18 octobre 2026)

| Question | Réponse | Conséquence sur le plan |
|---|---|---|
| Type de motion design | **Typographie animée, contenus verticaux réseaux, logos, explainers, data-viz** | Recettes centrées sur texte, formes, 9:16, graphiques animés à partir de données. |
| 3D et effets spéciaux (choix du 20/10/2026) | **Logos et produits 3D, scènes et décors 3D, VFX sur vidéo tournée, simulations** — pipeline **Blender + EffectCraft** | Blender (déjà piloté par Claude via MCP) fait la vraie 3D et les simulations ; EffectCraft compose, fait les VFX sur vidéo et la finition. Nouvelle phase 2b. L'interop Premiere reste hors périmètre. |
| Formats d'export | **MP4, WebM avec transparence, GIF** | Trois presets d'export prioritaires (H.264, VP9 + alpha, GIF) ; ProRes, Lottie, HEVC/AV1 restent disponibles mais ne sont ni mis en avant ni testés en priorité. |
| Mode de travail avec Claude | **Voir l'app s'animer en direct** | Le **mode bridge** (`effectcraft --control 9877` + `mcp --bridge 9877`) devient le mode principal : stabilité de l'app sous Windows, `focus_context` et retour visuel en direct passent en tête du MVP. |
| Usage | **Distribuer l'outil** | Rebranding **obligatoire** avant toute diffusion, installeur Windows à mon nom, connexion à Claude en un clic pour les utilisateurs, et maintenance à long terme du fork. |

Encore sans réponse, et à trancher plus tard (ça ne bloque pas les phases 0 et 1) : **projets After Effects `.aep` à reprendre ?** (supposé : non) et **production en série** de déclinaisons (supposé : oui pour les formats réseaux, d'où les templates en phase 2).

---

## 4. Stratégie de fork (distribuer sans se noyer dans les conflits)

1. **Ne pas toucher au cœur** (`crates/render`, `effects`, `project`…) sauf correctif de bug, que je propose aussi en PR à l'amont.
2. Mettre mes ajouts dans des **zones à moi** :
   - `extensions/mymotion/` : scripts JS, panneaux ScriptUI, presets `.ecpreset`, recettes ;
   - un **crate séparé** `crates/mymotion-mcp` (surcouche MCP qui délègue à l'automation existante) pour les nouveaux outils ;
   - `mymotion/` (hors workspace Cargo) : charte, templates `.ectemplate`, skill Claude, docs.
3. **Branches** : `main` = miroir de `upstream/main` ; `perso` = mes ajouts, mergée chaque semaine avec l'amont ; `release` = versions que je distribue, taguées (`v0.1.0-mymotion`…), après test de non-régression.
4. **Rebranding concentré** : l'identité de l'app est presque toute dans `crates/engine/src/links.rs` (`APP_NAME`, `APP_SLUG`, liens Discord/site/GitHub, apps sœurs) et dans `packaging/` (installeur WiX `packaging/windows/*.wxs`, identifiants `ai.storyteller.effectcraft`). Je garde les noms de crates `effectcraft-*` pour que les merges restent simples.

---

## 5. Phases (réordonnées)

Estimations en heures de travail avec Claude (agent), pour une personne qui pilote et valide.

### Phase 0 — Mise en route en mode live · ✅ faite le 18 octobre 2026

- [x] Visual Studio 2022 Build Tools (C++) et Rust 1.99.1 (`stable-x86_64-pc-windows-msvc`) installés via winget.
- [x] `cargo build --release -p effectcraft -p effectcraft-cli` : **46 min** au premier build (12 threads, 16 Go). Binaires de ≈ 88 Mo dans `target
elease\`.
- [x] App lancée avec `--control 9877` : GPU **NVIDIA RTX 2060 sur Vulkan** (pas Direct3D 12, donc le bug #613 ne s'applique pas ici).
- [x] `D:\MotionProjects\` créé (`projets\`, `medias\`, `out\`) avec :
  - `.mcp.json` : `effectcraft-app` (bridge, port 9877, mode principal) et `effectcraft` (headless, `--autosave`, confiné à `D:\MotionProjects`) ;
  - `Lancer EffectCraft (live Claude).cmd` : ouvre l'app avec le canal de contrôle.
  - Ouvrir une session Claude Code dans `D:\MotionProjects` pour avoir les outils.
- [x] Test complet via MCP bridge : comp 1080×1920, titre + sous-titre animés (échelle avec overshoot, fondus, glissement) créés **en direct dans la fenêtre**, `render_frame` et `screenshot` OK, projet sauvegardé.
- [x] Exports depuis la CLI headless (4 s, 1080×1920) : **MP4 H.264 en 8 s**, **GIF** 540×960 15 i/s en 2 s, **WebM VP9 + alpha en 108 s** (piste alpha vérifiée).

**Constats pour la suite**
- Le serveur MCP **rejette tout argument inconnu** (strict) : bon pour la fiabilité, à rappeler dans le skill.
- `fauxBold` sur `layer.newText` n'a pas épaissi Inter → utiliser un vrai style (`style: "Bold"`) ; à vérifier dans le skill.
- Le **WebM VP9 est lent** (≈ 27 s par seconde de vidéo 1080×1920) : rendre en WebM seulement pour la version finale, tester `--webm-codec av1` et la résolution moitié pour les essais.
- Rendre depuis la CLI headless sur le `.ecproj` sauvegardé évite de bloquer l'app pendant l'export.

### Phase 1 — MVP « on anime à deux, en direct » · 🟡 première tranche faite le 18 octobre 2026

**Fait** (`apps/mymotion-mcp`, `mymotion/`, voir [mymotion/README.md](mymotion/README.md)) :
- [x] Serveur MCP **`mymotion-mcp`** : proxy de `effectcraft-cli mcp` (tous les outils EffectCraft passent tels quels, progression et annulation des rendus comprises) + outils ajoutés, sans toucher au cœur (seule modif. amont : une ligne dans `xtask` pour la couche du nouveau crate).
- [x] `render_filmstrip` : planche contact légendée pour juger timing et espacement.
- [x] `plot_curves` : graphes valeur + vitesse avec clés, dépassement en % et vitesse maximale (dimensions identiques fusionnées « X=Y »).
- [x] `focus_context` : sélection de l'utilisateur, temps courant, image rendue + capture du panneau Composition en live.
- [x] `checkpoint` / `checkpoints` / `restore_checkpoint` sur l'historique branché.
- [x] Charte `brand_kit.json` (+ ressource `mymotion://brand`) avec safe areas verticales et règles de timing/easing.
- [x] **Chartes par client et par projet**, cumulées studio → client → dossier de projets → `<projet>.brand.json` ; client détecté d'après le dossier du projet (`clients/<Client>/projets/`) ou forcé (`brand_use`) ; outils `brand_list`, `brand_new`.
- [x] **Dossier projet avec brief** : `project_new` crée `BRIEF.md` (infos, objectif, public, script, assets, références, mood board, style, contraintes, livrables, notes), `SCRIPT.md`, `assets/`, `references/`, `moodboard/`, `out/` ; `project_brief` lit tout avec une planche visuelle du mood board ; `brief_update`, `project_list`.
- [x] Premier vrai projet : spot Reel 30 s de la 21e Semaine du numérique (v1 typographique, v2 photos + verre dépoli + reflets).
- [x] Skill Claude **`motion-design`** (méthode, pièges, principes d'animation, recettes vérifiées : presets texte `bounceInWords`…, trim paths `contents/trim/end`, style `Bold`).
- [x] `D:\MotionProjects\.mcp.json` passe par `mymotion` (live) et `mymotion-headless`.
- [x] Testé en headless et en live (bridge) ; 7 tests unitaires, clippy `-D warnings` et `cargo xtask layers` OK.

**Reste** :
- [ ] Étiqueter les actions de Claude dans l'historique (« Claude : … ») — demande un changement côté moteur ; à proposer en PR amont.
- [ ] Presets d'export nommés dans la Render Queue (pour l'instant : commandes CLI dans la charte et le skill).
- [ ] Vérifier qualité/poids des GIF (palette, tramage) et lecture du WebM alpha dans Chrome/Firefox.
- [ ] Choix automatique CPU/GPU à l'export (le mode « auto » existe dans `bench`, à exposer).
- [ ] Session réelle avec l'utilisateur pour ajuster le skill.

### Phase 2 — Recettes pour mes 5 types de contenu (≈ 30–50 h)

Recettes = scripts JS paramétrables dans `extensions/mymotion/recipes/`, exposées en outils MCP (`recipe_list`, `recipe_apply {name, params}`) et dans un panneau ScriptUI pour l'utilisateur.

| Domaine | Recettes |
|---|---|
| **Typographie animée** | apparition par lettre/mot/ligne (fondu, glissement, rebond, machine à écrire), kinetic typography sur rythme, mot mis en valeur, texte qui suit un tracé. |
| **Vertical réseaux** | gabarits 9:16 avec **safe areas** TikTok / Reels / Shorts, **sous-titres animés** mot par mot (à partir d'un SRT ou d'un texte minuté), call-to-action, hook des 2 premières secondes, recadrage 16:9 → 9:16 / 1:1 / 4:5. |
| **Logos** | révélations (tracé du contour puis remplissage, masque, morceaux qui s'assemblent), boucle de logo, version courte « sting » ; import SVG/AI déjà supporté. |
| **Explainers** | enchaînement de scènes, icônes animées, flèches et callouts, numéros d'étapes, transitions entre scènes, calage sur une voix off (marqueurs posés sur l'audio). |
| **Data-viz** | **nouvel import de données** CSV/JSON → barres, courbes, camemberts/donuts, compteurs de chiffres, barres de progression, classements animés (« bar chart race »), avec légendes et axes aux couleurs de la charte. |

Plus : **templates Essential Graphics** (`.ectemplate`) pour les formats récurrents et **rendu par lots** (`template_render_batch` : un CSV → N vidéos), en s'appuyant sur `effectcraft-cli render --template … --values …` qui existe déjà.

**Critère de sortie** : « fais-moi la vidéo verticale des chiffres du trimestre à partir de ce CSV » produit un MP4 conforme à la charte, sans retouche manuelle.

### Phase 2b — 3D et effets spéciaux : pipeline Blender ↔ EffectCraft (≈ 60–110 h)

**Pourquoi deux logiciels** : la 3D d'EffectCraft (Classic / Advanced 3D, modèles glTF/OBJ/STL/FBX
statiques, caméras, lumières, ombres douces, profondeur de champ) et ses outils VFX (Keylight, suivi de
points, 3D Camera Tracker, Warp Stabilizer, Roto Brush IA, Object Matte, Content-Aware Fill, particules,
EXR multicouche, Cryptomatte) existent, mais sont estimés **≈ 35 % prêts pour un usage pro** par leurs
auteurs (pas de reflets entre objets, pas d'animation FBX, comportement non comparé à After Effects).
Advanced 3D rend à ≈ 0,8 s par image HD sur la RTX 2060. **Blender** apporte la vraie 3D (modélisation,
matériaux, Cycles/EEVEE, simulations, suivi de caméra libmv) ; Claude pilote déjà Blender via MCP.

**Répartition**

| Étape | Outil | Exemples |
|---|---|---|
| Modélisation, matériaux, éclairage, simulations | Blender | logo en volume, packshot, décor, fumée/feu/fluides/destruction, tissus |
| Rendu 3D | Blender (Cycles, OptiX sur la RTX) | séquences **EXR multicouches** : beauty, ombres, AO, profondeur, Cryptomatte, alpha |
| Suivi de caméra d'un plan tourné | Blender (libmv) en priorité, 3D Camera Tracker d'EffectCraft en secours | intégrer de la 3D dans une vidéo réelle |
| Compositing, VFX, titres, motion design, étalonnage, export | EffectCraft | fond vert, rotoscopie, incrustation, lueurs, grain, typo animée |

**Fait (20/10/2026)** : premier essai complet (logo SN vectorisé → extrudé dans Blender 5.2 → Cycles 90 images EXR
multicouche en 12 min 45 → composé dans EffectCraft) ; puis industrialisé : dossiers `3d/`, `renders/`, `footage/` et
section « 3D et VFX » du brief, outils `import_render` (passes empilées automatiquement) et `vectorize_logo`, recettes
Blender `mymotion/blender/mymotion_blender.py`, skill « 3D et VFX ». Caméra partagée Blender → EffectCraft faite (`export_camera` + `camera_from_blender`, écart mesuré < 1 px, titres 3D du spot SN). VFX vidéo : remplacement d'écran automatique (`replace_screen` : détection, suivi 4 points, corner pin, Key Light ; testé sur un plan Pexels, spot SN dans le téléphone) et planche `render_before_after`. Reste : sens inverse EffectCraft → Blender (3D Camera Tracker), fond vert de personne et intégration 3D dans un plan tourné, recettes 3D/VFX
(points 5–6), contrôle qualité avant/après (point 7).

**À construire dans `mymotion`**
1. **Dossier projet étendu** : `3d/` (fichiers `.blend`), `renders/` (séquences EXR/PNG), `footage/`
   (plans tournés) ; sections du brief « 3D / VFX » (modèles, plans, simulations, contraintes de rendu).
2. **`import_render {path, comp?}`** : importe une séquence de rendu Blender et monte ses passes
   (beauty, ombres en multiplication, AO, profondeur pour la profondeur de champ / brouillard,
   Cryptomatte pour isoler un objet) dans une comp prête à étalonner.
3. **Caméra partagée** : `camera_from_blender` (animation de caméra Blender → caméra 3D EffectCraft,
   conversion focale ↔ zoom, unités, repère) pour placer titres et éléments 2D dans la scène 3D ;
   `camera_to_blender` (solution du 3D Camera Tracker → caméra Blender) pour intégrer de la 3D dans un plan tourné.
4. **Réglages de rendu Blender standard** (via le MCP Blender) : EXR multicouche 16 bits, passes,
   gestion des couleurs linéaire/AgX alignée avec l'espace de travail d'EffectCraft (32 bpc linéaire).
5. **Recettes 3D** : logo en volume (extrusion, biseau, métal/verre, reflet qui passe), rotation produit
   (turntable), survol de décor, révélation par particules.
6. **Recettes VFX** : fond vert (Keylight + nettoyage + étalonnage d'intégration), suivi de points et
   incrustation, remplacement d'écran, rotoscopie IA, intégration 3D dans plan tourné, simulations
   Blender composées dans la vidéo.
7. **Contrôle qualité** : planches contact et comparaisons avant/après (plan brut, masque, résultat) pour que
   Claude juge les détourages et l'intégration (bords, grain, lumière).

**Matériel** : la RTX 2060 (6 Go) et 16 Go de RAM suffisent pour des logos, packshots et des simulations
légères ; pour des décors lourds ou des simulations fines, prévoir des rendus de nuit, 32 Go de RAM, ou une
ferme de rendu.

**Critère de sortie** : « fais un logo SN en verre qui se révèle dans une salle futuriste, puis incruste-le
sur ce plan tourné » : Claude construit la 3D dans Blender, rend les passes, compose et finit dans
EffectCraft, et livre le MP4.

### Phase 3 — Distribution (≈ 25–45 h) · *obligatoire avant de partager*

1. **Nouveau nom et nouvelle identité** (à choisir) : ni « ArtCraft », ni « Claude » (marque d'Anthropic), ni « After Effects » dans le nom. Modifier `links.rs`, l'About, l'écran d'accueil, le menu Aide, le bouton Discord ; supprimer `docs/brand/` et les mentions ArtCraft du README ; nouvelle icône (l'icône actuelle est sous MIT/Apache, mais en avoir une à moi évite la confusion).
2. **Mentions légales** : conserver `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` (en y ajoutant « basé sur EffectCraft par l'équipe ArtCraft » et mes modifications), `ATTRIBUTION.md` ; ajouter les attributions de mes propres assets (`cargo xtask assets`).
3. **Installeur Windows** : adapter `packaging/windows/*.wxs` (nom, identifiants, dossier d'installation), tester l'installation propre et la mise à jour ; **signature de code** (certificat à acheter) sinon SmartScreen avertit les utilisateurs.
4. **« Connecter à Claude » en un clic** : commande dans l'app qui écrit la config MCP de Claude Desktop (`claude_desktop_config.json`) avec le bon chemin vers `effectcraft-cli.exe` et le port bridge, et qui vérifie la connexion. Livrer le **skill `motion-design`** avec l'outil.
5. **Page de téléchargement** (releases GitHub de mon fork) avec notes de version ; décider si le fork devient public ou si je distribue seulement les binaires.
6. **CI GitHub Actions** à moi (l'amont n'en a pas sur les PR) : `cargo xtask ci` + build MSI à chaque tag.

**Critère de sortie** : un utilisateur installe le MSI, clique « Connecter à Claude », et Claude anime dans son app.

### Phase 4 — Itérations

- Support d'autres utilisateurs : remontée de bugs, mises à jour, merge hebdomadaire de l'amont, tests de non-régression sur les recettes.
- Plug-ins d'effets WebAssembly à moi (looks signature, grain, glitch).
- Contribution en amont des correctifs Windows (moins de code à maintenir dans mon fork).
- macOS / Linux : les scripts de packaging existent ; à activer seulement si des utilisateurs le demandent.

### Hors périmètre (masqué, pas supprimé)

Interop Premiere (EDL/FCPXML/AAF), effets audio, Lottie (disponibles mais non mis en avant). La 3D, les VFX, le Roto Brush et le tracking entrent dans le périmètre (phase 2b) ; ProRes 4444 et les séquences EXR servent d'intermédiaires entre Blender et EffectCraft. Rien n'est supprimé du code, pour garder des merges faciles.

---

## 6. Risques

| Risque | Probabilité | Impact | Parade |
|---|---|---|---|
| Bugs/crash **sous Windows en mode live** (GPU D3D12, interactions) | Élevée | **Élevé** (c'est le mode principal) | Diagnostic en phase 0, rendu CPU en secours, `--autosave`, corrections remontées à l'amont. |
| **Alpha** : bugs d'effets, de texte, de masques | Élevée | Moyen | Mes recettes n'utilisent qu'un sous-ensemble testé ; tests de non-régression par recette. |
| **Support utilisateurs** d'un logiciel alpha | Élevée | Élevé | Annoncer clairement « bêta », canal de bugs, versions taguées et testées. |
| **Dérive de l'amont** (≈ 800 commits/semaine) | Certaine | Moyen | Fork mince, ajouts dans des fichiers nouveaux, merge hebdomadaire, branche `release` figée. |
| **Marques** (ArtCraft, Claude, Adobe) | Moyenne si négligé | Juridique | Rebranding complet en phase 3 avant toute diffusion ; nom neutre. |
| Installeur non signé | Certaine sans certificat | Moyen (confiance) | Certificat de signature de code. |
| Export lent (pas d'accélération matérielle) | Moyenne | Faible-moyen | Formats courts (réseaux) ; rendus de travail en basse résolution. |
| Qualité/poids des **GIF** | Moyenne | Moyen | Vérification en phase 1 ; preset optimisé (palette, fps réduit). |
| **3D / VFX d'EffectCraft encore jeunes** (≈ 35 % prêts) | Élevée | Élevé | La vraie 3D et le suivi de caméra dans Blender ; EffectCraft pour le compositing ; tests sur de vrais plans avant tout engagement client. |
| **Puissance de calcul** (RTX 2060 6 Go, 16 Go RAM) | Élevée pour les décors et simulations lourds | Moyen | Rendus de nuit, résolution de travail réduite, passage à 32 Go de RAM ou ferme de rendu si besoin. |
| **Couleurs entre Blender et EffectCraft** | Moyenne | Moyen | Flux EXR linéaire et réglages standard des deux côtés (phase 2b, point 4). |

## 7. Effort total estimé

| Phase | Effort | Cumul |
|---|---:|---:|
| 0 — Mise en route en mode live | 3–6 h | 3–6 h |
| 1 — MVP « on anime à deux, en direct » | 20–35 h | ≈ 25–40 h |
| 2 — Recettes (typo, vertical, logos, explainers, data-viz) | 30–50 h | ≈ 55–90 h |
| 2b — 3D et effets spéciaux (Blender ↔ EffectCraft) | 60–110 h | ≈ 115–200 h |
| 3 — Distribution | 25–45 h | ≈ 140–245 h |
| 4 — Itérations et support | continu | — |

## 8. Prochaines décisions

1. **Le nom** du logiciel (pour la phase 3, pas bloquant avant).
2. Fork **public** ou distribution des **binaires seuls** ?
3. Projets **After Effects `.aep`** à reprendre ? (supposé : non)
4. **Ordre** : finir le spot SN (musique, déclinaisons) avant la phase 2b, ou attaquer la 3D tout de suite ?
5. **Matériel** : RAM 32 Go / ferme de rendu si les décors et simulations deviennent lourds.
