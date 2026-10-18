# mymotion — EffectCraft piloté avec Claude

Ajouts du fork `konty226/effectcraft` (branche `perso`) pour faire du motion design avec Claude.
Tout est dans des fichiers nouveaux, pour que les merges avec `storytold/effectcraft` restent simples.

| Élément | Où | Rôle |
|---|---|---|
| Serveur MCP `mymotion-mcp` | `apps/mymotion-mcp/` | Proxy de `effectcraft-cli mcp` : tous les outils EffectCraft + `render_filmstrip`, `plot_curves`, `focus_context`, `checkpoint` / `checkpoints` / `restore_checkpoint`, `brand_kit` (et la ressource `mymotion://brand`). |
| Charte | `mymotion/brand_kit.example.json` | Couleurs, polices, échelle typo, safe areas verticales, timing, easing, commandes d'export. Copier et personnaliser. |
| Skill Claude | `mymotion/skills/motion-design/SKILL.md` | Méthode de travail, conventions EffectCraft, principes d'animation, recettes vérifiées. Copier dans `<projet>/.claude/skills/motion-design/`. |

## Installation (Windows)

```bash
cargo build --release -p effectcraft -p effectcraft-cli -p mymotion-mcp
```

`.mcp.json` du dossier de travail (ex. `D:\MotionProjects`) :

```json
{
  "mcpServers": {
    "mymotion": {
      "command": "D:\\Fork\\effectcraft\\target\\release\\mymotion-mcp.exe",
      "args": ["--brand", "D:\\MotionProjects\\brand_kit.json", "--bridge", "9877"]
    }
  }
}
```

Puis lancer l'app avec le canal de contrôle : `effectcraft.exe --control 9877`. Sans `--bridge`,
le serveur travaille sans fenêtre (les arguments après `--brand` sont transmis à `effectcraft-cli mcp`,
par exemple `--autosave`, `--project F.ecproj`, `--automation-read-root DIR`).

## Outils ajoutés

- `render_filmstrip {comp?, from?, to?, frames?, columns?, max_side?, transparent?}` : planche contact légendée (secondes, numéro d'image).
- `plot_curves {layer, path, comp?, from?, to?, samples?, width?, height?}` : graphes valeur + vitesse avec les clés ; réponse texte avec dépassement (`overshootPct`) et vitesse maximale.
- `focus_context {max_side?, image?}` : comp active, sélection de l'utilisateur, temps courant, image rendue et capture du panneau Composition.
- `checkpoint {label}`, `checkpoints {}`, `restore_checkpoint {label}` : points de retour dans l'historique (sans perdre les branches).
- `brand_kit {client?}`, `brand_list {}`, `brand_use {client|null}`, `brand_new {client, from?}` : chartes par client et par projet (ci-dessous).
- `project_new {client, project, brief?, script?}`, `project_brief {client?, project?, images?}`, `brief_update {section, content, append?}`, `project_list {client?}` : dossiers projet avec brief (ci-dessous).

## Chartes par client et par projet

`--brand` désigne la charte **studio** ; son dossier est la racine des clients. Chaque niveau ne contient
que ce qui change, et ils se cumulent (objets fusionnés clé par clé, valeurs simples et listes remplacées) :

```text
D:\MotionProjects\brand_kit.json                               studio (défauts)
D:\MotionProjects\clients\<Client>\brand_kit.json              client
D:\MotionProjects\clients\<Client>\projets\<…>\brand_kit.json  dossier de projets (campagne…)
<projet>.brand.json, à côté du .ecproj                         un seul projet
```

Le client est détecté d'après le dossier du projet ouvert, ou forcé pour la session avec `brand_use`.
`brand_new` crée `clients/<Client>/` avec `projets/`, `medias/`, `out/` et une charte vide (ou copiée d'un autre client).

## Projets et briefs

Chaque travail a son dossier, créé par `project_new` et rempli par la conversation ou à la main :

```text
clients\<Client>\projets\<Projet>\
  BRIEF.md     Infos, Objectif et message clé, Public cible, Script, Assets fournis,
               Références et exemples, Mood board, Style, Contraintes et mentions obligatoires,
               Livrables, Notes de suivi
  SCRIPT.md    tableau scène / temps / texte à l'écran / voix off / visuel
  assets\      fichiers fournis (logos, photos, vidéos, musique, voix off, polices)
  references\  exemples (images, vidéos, liens.txt)
  moodboard\   images d'ambiance
  out\         rendus
  <Projet>.ecproj
```

`project_brief` renvoie le brief section par section, le script, ce qui manque encore (un dossier
rempli compte : déposer des images dans `moodboard/` complète la section Mood board), l'inventaire des
fichiers et une planche visuelle du mood board et des références. Le `.ecproj` enregistré dans le dossier
fait du projet le projet « ouvert » : les outils le retrouvent sans arguments, et la charte du client
s'applique (un `brand_kit.json` posé dans le dossier du projet y ajoute ses exceptions).
