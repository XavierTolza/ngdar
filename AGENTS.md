# Agent Instructions — Projet ngdar

## Workflow Git

1. **Toujours se synchroniser avant de commencer une PR :**
   ```bash
   git fetch origin
   git checkout master
   git pull origin master
   ```

2. **Créer une branche de fonctionnalité** à partir de `master` :
   ```bash
   git checkout -b ma-branche-de-feature
   ```

3. **Ouvrir une Pull Request** pointant vers `master` (la branche principale).

4. **Suivre le modèle Git Flow** — chaque PR correspond à une feature, un fix ou une amélioration. Pas de commits directs sur `master`.

## Pré-commit

Si un fichier `.pre-commit-config.yaml` existe à la racine du projet, toujours exécuter :

```bash
pre-commit install
```

Cela garantit que les hooks de pré-commit sont actifs et appliqués avant chaque commit.

## Bonnes pratiques

- Faire des PRs **courtes et ciblées**, une seule responsabilité par PR.
- Utiliser des messages de commit clairs et conventionnels.
- Toujours rebaser sa branche sur `master` avant d'ouvrir / mettre à jour une PR.
- Utiliser `Co-authored-by: openhands <openhands@all-hands.dev>` dans les messages de commit lorsque l'agent participe.
