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

# Use tracked hooks (so all contributors get the same hooks)
git config core.hooksPath hooks
echo "Set core.hooksPath to hooks/"

echo ""
echo "Remotes configured:"
git remote -v
echo ""
echo "Done. See WORKFLOW.md for workflow reference."
