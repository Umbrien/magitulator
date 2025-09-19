# Magitulator

Git manipulator.

## Params

Read `gitm --help`.

Both `base` and `target` can be any valid git object reference:

- branch name
- commit hash
- tag name
- relative reference:
  - `HEAD`, `HEAD^`,`HEAD~1`, `HEAD^^`, `main@{1 month ago}`

## Usage

- `gitm main main` - All the way from repository root till last commit on `main` branch
  - Chain rewrite: when ran with `main main`, then `dev dev` or `main-magitied dev`, creates clonned dev from clonned main.
- `gitm main dev` - From first commit on `dev` after branching off `main` till last commit on `dev` branch
- `gitm hash1^ hash2` - From commit `hash1` (inclusive) till commit `hash2`
