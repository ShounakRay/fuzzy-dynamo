#!/bin/sh
# One-time setup for dynamo-fuzzing contributors
# Run after cloning: ./setup.sh

set -e

# Upstream (ai-dynamo/dynamo) — for syncing latest code
if ! git remote get-url upstream >/dev/null 2>&1; then
    git remote add upstream https://github.com/ai-dynamo/dynamo.git
    echo "Added remote: upstream (ai-dynamo/dynamo)"
else
    echo "Remote 'upstream' already exists"
fi

# Public fork — for filing PRs to ai-dynamo/dynamo
if ! git remote get-url public-fork >/dev/null 2>&1; then
    git remote add public-fork https://github.com/ShounakRay/fuzzy-dynamo.git
    echo "Added remote: public-fork (ShounakRay/fuzzy-dynamo)"
else
    echo "Remote 'public-fork' already exists"
fi

echo ""
echo "Remotes configured:"
git remote -v
echo ""
echo "=========================================="
echo "  WORKFLOW REFERENCE"
echo "=========================================="
echo ""
echo "--- Daily work ---"
echo "  git push origin sray/fuzzy-dynamo"
echo "    push  = upload commits to a remote"
echo "    origin = ShounakRay/dynamo-fuzzing (this private repo)"
echo "    sray/fuzzy-dynamo = the branch to push"
echo ""
echo "--- Sync latest upstream code ---"
echo "  git fetch upstream"
echo "    fetch    = download commits from a remote (no files change locally)"
echo "    upstream = ai-dynamo/dynamo (the real source repo)"
echo ""
echo "  git merge upstream/main"
echo "    merge         = integrate downloaded commits into your current branch"
echo "    upstream/main = the local copy of upstream's main branch"
echo ""
echo "--- Sync public fork with upstream ---"
echo "  git fetch upstream"
echo "    fetch    = download latest from ai-dynamo/dynamo"
echo "    upstream = ai-dynamo/dynamo"
echo ""
echo "  git fetch public-fork"
echo "    fetch       = download latest from the public fork"
echo "    public-fork = ShounakRay/fuzzy-dynamo"
echo ""
echo "  git push public-fork upstream/main:main"
echo "    push             = upload commits to a remote"
echo "    public-fork      = destination: ShounakRay/fuzzy-dynamo"
echo "    upstream/main    = source: your local copy of ai-dynamo/dynamo's main"
echo "    :main            = destination branch name on public-fork"
echo "    (reads as: push what I locally call 'upstream/main' to public-fork's 'main')"
echo ""
echo "--- File PRs ---"
echo "  git push public-fork sray/fuzzy-dynamo:sray/fuzzy-dynamo"
echo "    push                          = upload commits to a remote"
echo "    public-fork                   = destination: ShounakRay/fuzzy-dynamo"
echo "    sray/fuzzy-dynamo             = source: your local branch"
echo "    :sray/fuzzy-dynamo            = destination branch name on public-fork"
echo "    (reads as: push my local 'sray/fuzzy-dynamo' to public-fork's 'sray/fuzzy-dynamo')"
echo ""
echo "  gh pr create --repo ai-dynamo/dynamo --head ShounakRay:sray/fuzzy-dynamo"
echo "    opens a PR from the public fork's branch to upstream"
echo ""
echo "=========================================="
