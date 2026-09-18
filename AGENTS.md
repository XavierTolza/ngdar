# Agent Instructions — Projet ngdar

## Workflow Git

1. **Toujours se synchroniser avant de commencer une PR :**
   ```bash
   git fetch origin
   git checkout develop
   git pull origin develop
   ```

2. **Créer une branche de fonctionnalité** à partir de `develop` :
   ```bash
   git checkout -b ma-branche-de-feature
   ```

3. **Ouvrir une Pull Request** pointant vers `develop` (la branche d'intégration). `master` est géré automatiquement par la CI à partir de `develop` : ne jamais committer, pousser ou ouvrir de PR directement vers `master`.

4. **Suivre le modèle Git Flow** — chaque PR correspond à une feature, un fix ou une amélioration. Pas de commits directs sur `develop` ni `master`.

## Pré-commit

Si un fichier `.pre-commit-config.yaml` existe à la racine du projet, toujours exécuter :

```bash
pre-commit install
```

Cela garantit que les hooks de pré-commit sont actifs et appliqués avant chaque commit.

## Bonnes pratiques

- Faire des PRs **courtes et ciblées**, une seule responsabilité par PR.
- Utiliser des messages de commit clairs et conventionnels.
- Toujours rebaser sa branche sur `develop` avant d'ouvrir / mettre à jour une PR.
- Utiliser `Co-authored-by: openhands <openhands@all-hands.dev>` dans les messages de commit lorsque l'agent participe.
- **Décomposer le travail en tâches** quand c'est possible : découper une feature en sous-tâches claires et les traiter une par une.
- **Créer automatiquement les PR sur GitHub** : une fois une feature terminée, ouvrir automatiquement une Pull Request — une PR par feature, pointant vers `develop`.
