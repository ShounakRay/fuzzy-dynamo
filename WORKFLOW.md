# dynamo-fuzzing — Contributor Workflow

Private repo for fuzzing [ai-dynamo/dynamo](https://github.com/ai-dynamo/dynamo). Detached from the public fork to keep work private until disclosure.

## Setup

After cloning, run the one-time setup script to configure remotes:

```bash
./setup.sh
```

This does three things:
1. Adds remote **upstream** → `ai-dynamo/dynamo` (the real source repo)
2. Adds remote **public-fork** → `ShounakRay/fuzzy-dynamo` (public fork for filing PRs)
3. Sets `core.hooksPath` to `hooks/` — activates the shared git hooks (see [Hooks](#hooks) below)

## Remotes

| Remote | URL | Purpose |
|--------|-----|---------|
| `origin` | `ShounakRay/dynamo-fuzzing` | This private repo (daily work) |
| `upstream` | `ai-dynamo/dynamo` | Real upstream source |
| `public-fork` | `ShounakRay/fuzzy-dynamo` | Public fork (for PRs to upstream) |

## Workflow Reference

### Daily work

```bash
git push origin sray/private-bugs
```

- `push` = upload commits to a remote
- `origin` = ShounakRay/dynamo-fuzzing (this private repo)
- `sray/private-bugs` = the branch to push

### Sync latest upstream code

```bash
git fetch upstream
```

- `fetch` = download commits from a remote (no files change locally)
- `upstream` = ai-dynamo/dynamo (the real source repo)

```bash
git merge upstream/main
```

- `merge` = integrate downloaded commits into your current branch
- `upstream/main` = the local copy of upstream's main branch

### Sync public fork with upstream

When you want the public fork's `main` to match upstream's latest:

```bash
git fetch upstream
git fetch public-fork
git push public-fork upstream/main:main
```

Breaking down `git push public-fork upstream/main:main`:
- `push` = upload commits to a remote
- `public-fork` = destination: ShounakRay/fuzzy-dynamo
- `upstream/main` = source: your local copy of ai-dynamo/dynamo's main
- `:main` = destination branch name on public-fork
- Reads as: "push what I locally call `upstream/main` to public-fork's `main`"

### File PRs

Push your working branch to the public fork, then open a PR from there to upstream:

```bash
git push public-fork sray/private-bugs:sray/public-bugs
```

Breaking down `git push public-fork sray/private-bugs:sray/public-bugs`:
- `push` = upload commits to a remote
- `public-fork` = destination: ShounakRay/fuzzy-dynamo
- `sray/private-bugs` = source: your local branch
- `:sray/public-bugs` = destination branch name on public-fork
- Reads as: "push my local `sray/private-bugs` to public-fork's `sray/public-bugs`"

Then create the PR:

```bash
gh pr create --repo ai-dynamo/dynamo --head ShounakRay:sray/public-bugs
```

This opens a PR from the public fork's branch to upstream.

## Hooks

Git hooks are stored in the tracked `hooks/` directory (not `.git/hooks/`). Running `./setup.sh` sets `core.hooksPath` to `hooks/`, so all contributors share the same hooks automatically.

| Hook | What it does |
|------|-------------|
| `hooks/pre-push` | Blocks any push to `main`. All work must go to branches. |
| `hooks/post-merge` | Auto-removes upstream CI workflow files after merges (they require NVIDIA infrastructure and will fail here). |

**If you need to push to `main`** (e.g., syncing upstream → origin main), bypass the hook for that one push:

```bash
git -c core.hooksPath=/dev/null push origin main
```

## Notes

- **All commands run from this repo's local clone** (`dynamo-fuzzing/`). Git remotes are just named URL aliases — your single local repo talks to all three remotes.
- **Collaborators** can use this same workflow if they have write access to both repos. They clone `dynamo-fuzzing`, run `./setup.sh`, and all the same commands work.
- **Always be explicit with push commands.** Use `git push origin sray/private-bugs`, not bare `git push`. With multiple remotes, bare `git push` may target the wrong remote depending on your tracking config.
