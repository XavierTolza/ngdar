<div align="center">

# 📀 NGDAR — New Generation Disk Archiving

**Git-like incremental archiving for massive binary files on optical media**

[![CI](https://github.com/XavierTolza/ngdar/actions/workflows/ci.yml/badge.svg)](https://github.com/XavierTolza/ngdar/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/badge/crates.io-1.0.0-green)](https://crates.io)
[![Docs](https://img.shields.io/badge/docs-README-brightgreen)](https://github.com/XavierTolza/ngdar#readme)

</div>

---

NGDAR vous permet de gérer l'archivage de vos fichiers volumineux sur disques optiques (DVD, Blu-ray) ou bandes LTO **comme vous géreriez du code avec Git**.

Chaque archive `.tar` produite contient **l'historique complet** de vos données ainsi que **seulement les fichiers modifiés** depuis la dernière session. Plus besoin de tout re-graver à chaque fois : un simple coup d'œil dans les métadonnées en texte clair vous indique quel volume contient quel fichier.

> **Cas d'usage typique** : vous avez 50 Go de photos, vidéos ou documents d'archive à sauvegarder sur des DVD de 4,7 Go. NGDAR découpe l'archivage en sessions, et chaque DVD contient les nouveaux fichiers + l'index complet de toutes vos données.

---

## ✨ Fonctionnalités

- **Archivage incrémental** — Seuls les fichiers nouveaux ou modifiés sont ajoutés à chaque archive
- **Métadonnées en texte clair** — Pas de base de données propriétaire. Un simple `cat` suffit pour lire l'historique
- **Content-Addressable Storage** — Chaque fichier est identifié par son hash BLAKE3 (déduplication automatique)
- **Auto-numérotation des volumes** — Identifiant de volume libre (DVD-001, LOT-2026-08, etc.)
- **Cache système** — Les hashs sont mis en cache dans `~/.cache/ngdar/` pour des exécutions ultra-rapides
- **`.ngdarignore`** — Excluez fichiers temporaires, logs, et autres artefacts
- **Standard `.tar`** — Les archives sont lisibles avec n'importe quel décompresseur
- **Export CSV** — Exportez toute votre base de métadonnées pour inventaire

---

## 🚀 Installation

### Prérequis

- [Rust](https://www.rust-lang.org) 1.70 ou plus récent
- Un système Linux, macOS (ou Windows avec un environnement Unix)

### Depuis les sources

```bash
# Clonez le dépôt
git clone https://github.com/XavierTolza/ngdar.git
cd ngdar

# Compilez et installez
cargo build --release
cp target/release/ngdar ~/.local/bin/    # ou n'importe où dans votre PATH

# Vérifiez
ngdar --version
```

### Avec Cargo (si publié sur crates.io)

```bash
cargo install ngdar
```

---

## 📖 Utilisation pas à pas

### 1. Initialiser un dépôt

```bash
cd /chemin/vers/mes/donnees/
ngdar init
```

Un répertoire caché `.ngdar/` est créé. Il contiendra toutes les métadonnées (jamais les fichiers binaires).

### 2. Ajouter des fichiers

```bash
ngdar add photos/ videos/ documents/
ngdar add rapport-final.pdf
```

NGDAR calcule le hash BLAKE3 de chaque fichier et l'ajoute à la zone de staging (l'index). Si un fichier est déjà archivé sans modification, il est signalé et ignoré.

### 3. Vérifier l'état

```bash
ngdar status
```

Trois catégories s'affichent :

| État | Description |
|---|---|
| ✅ **Staged** | Prêt à être packagé |
| 🔶 **Unstaged** | Modifié sur le disque mais pas re-stagé |
| ❓ **Untracked** | Nouveau fichier non encore suivi |

### 4. Créer une archive

```bash
ngdar pack --vol-id "DVD-001" --out archive_001.tar -m "Première session d'archivage"
```

Cette commande :
1. Crée les objets **Meta** (taille, date, permissions, hash, volume)
2. Construit les objets **Tree** et **Commit**
3. Génère une archive `.tar` contenant :
   - Le dossier `.ngdar/` complet (historique complet !)
   - Les fichiers de cette session, dans leur arborescence d'origine
4. Vide l'index (prêt pour la session suivante)

### 5. Deuxième session

```bash
ngdar add nouvelles-photos/
ngdar pack --vol-id "DVD-002" --out archive_002.tar -m "Nouvelles photos"
```

Le DVD-002 contient seulement les nouvelles photos, **mais** `.ngdar/` dans l'archive référence aussi tous les fichiers du DVD-001. Vous pouvez donc savoir depuis n'importe quelle archive où trouver chaque fichier.

### 6. Consulter l'historique

```bash
# Liste des commits
ngdar log

# Contenu d'un commit spécifique
ngdar log a1b2c3d4e5f6...
```

---

## 🏗️ Architecture

### Principes fondamentaux

1. **Content-Addressable Storage** — Chaque fichier est identifié par son hash BLAKE3. Le hash est calculé une fois et mis en cache dans `~/.cache/ngdar/<repo-id>/cache.tsv`.

2. **Métadonnées en texte clair** — Tout l'historique (commits, arborescences, métadonnées) est stocké dans des fichiers texte nommés par leur propre hash BLAKE3. Pas de base de données, pas de format propriétaire.

3. **Archives auto-suffisantes** — Chaque archive `.tar` contient **l'intégralité** de l'historique des métadonnées + **uniquement** les fichiers binaires changés pendant cette session. Une seule archive suffit pour connaître l'état complet du projet.

### Structure du dépôt

```
.ngdar/                    # Métadonnées locales (pas de duplication des binaires)
├── repository_id          # UUID v4 liant le projet au cache OS
├── HEAD                   # Pointeur vers le dernier hash de commit
├── index                  # Zone de staging (fichiers prêts pour le prochain pack)
├── committed              # Liste des fichiers déjà archivés (hash + chemin)
└── objects/               # Stockage content-addressed
    ├── ab/                # 2 premiers caractères du hash = dossier
    │   └── cd...          # Objet Meta, Tree ou Commit
    └── ...
```

### Format des archives `.tar`

```
archive_001.tar
├── .ngdar/                # Métadonnées complètes (toute l'historique)
│   ├── repository_id
│   ├── HEAD
│   ├── index
│   ├── committed
│   └── objects/
│       └── ...
├── photos/                # Fichiers binaires (incrémentaux)
│   └── plage.jpg
└── videos/
    └── anniversaire.mp4    # Seulement les fichiers de cette session
```

### Modèle d'objets (tout en texte clair)

**Meta** (un fichier)
```
type meta
size 4718592
mtime 1788118000
permissions 644
binary_hash a1b2c3d4e5f6...
volume_id DVD-001
path photos/vacances.jpg
```

**Tree** (un dossier)
```
meta 81a44c7...  document.pdf
tree e3b0c44...  sous-dossier
```

**Commit** (un instantané)
```
tree a1b2c3d...
parent f6e5d4c...
author utilisateur <user@host> 1788118091
os Linux 6.6.0-x86_64
tool_version ngdar-1.0.0

Ajout des photos de vacances
```

---

## 📋 Référence des commandes

| Commande | Description |
|---|---|
| `ngdar init` | Initialise un dépôt dans le répertoire courant |
| `ngdar add <chemins>` | Ajoute des fichiers au staging |
| `ngdar status` | Affiche l'état (staged, unstaged, untracked) |
| `ngdar pack --vol-id <ID> --out <archive> -m <msg>` | Crée une archive TAR |
| `ngdar log [hash]` | Liste les commits ou le contenu d'un commit |
| `ngdar hash <fichier>` | Calcule le BLAKE3 hash d'un fichier |
| `ngdar export <hash> --out <archive>` | Reconstruit une archive depuis les métadonnées |
| `ngdar db-export <fichier.csv>` | Exporte toutes les métadonnées en CSV |

---

## ⚙️ Configuration

### `.ngdarignore`

Placez un fichier `.ngdarignore` à la racine du dépôt, avec les mêmes règles que `.gitignore` :

```
*.log
tmp/
cache/
```

### Cache système

Le cache se trouve dans `~/.cache/ngdar/<repo-id>/cache.tsv` (format TSV : `taille\ttimestamp\thash\tchemin`). Si le cache est purgé par le système, NGDAR recalcule les hashs automatiquement.

---

## 🛠️ Développement

### Compiler et tester

```bash
# Compilation optimisée
cargo build --release

# Exécuter tous les tests
cargo test

# Formatage et lint (comme en CI)
cargo fmt --check
cargo clippy -- -D warnings

# Générer la documentation
RUSTDOCFLAGS="-D warnings" cargo doc --document-private-items
```

### Hooks pré-commit

```bash
pre-commit install
```

Les hooks vérifient le formatage, interdisent certains patterns, et exécutent la CI complète avant de pousser.

---

## 🤝 Contribuer

Les contributions sont les bienvenues ! Voici la marche à suivre :

1. **Fork** le projet
2. Crée une branche : `git checkout -b ma-feature`
3. **Commit** tes changements
4. **Push** : `git push origin ma-feature`
5. Ouvre une **Pull Request** vers `master`

Merci de suivre ces bonnes pratiques :
- PR courtes et ciblées (une seule responsabilité)
- Messages de commit clairs et conventionnels
- Tests inclus pour chaque nouvelle fonctionnalité

---

## 📄 Licence

Distribué sous licence **MIT**. Voir le fichier `LICENSE` pour plus d'informations.

---

<div align="center">

**NGDAR** — *Parce que vos archives méritent mieux qu'un tas de fichiers éparpillés sur des DVD sans index.*

</div>
