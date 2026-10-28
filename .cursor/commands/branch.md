# /branch

Create a new local branch directly from a fetched remote base, without checking out the base locally. The new branch initially tracks the base; an optional push changes its upstream to the new remote branch.

## Branch strategy

Keep each branch focused on one concern to ease review and CI. Include the PR's Linear issue ID in the name so Linear links the branch to its issue: `<area>/eco-<id>-<verb-noun>`, for example `backend/eco-123-fix-db-timeout`.

The default base is `main`. For hotfixes or release work, pass the specific base, such as `release/2025.10`.

## Arguments

- `name` (string, required): new branch name, including the Linear issue ID.
- `base` (string, optional): remote base branch (default: `main`).
- `remote` (string, optional): remote name (default: `origin`).
- `push` (boolean, optional): push and set upstream to `<remote>/<name>` (default: `true`).

## Behavior

Abort if `name` already exists locally. Otherwise, fetch the selected base ref and create the local branch directly from `<remote>/<base>`, initially tracking that base. With `push=false`, the upstream remains `<remote>/<base>`. With `push=true`, a successful push sets the upstream to `<remote>/<name>`.

Example:

```text
/branch "frontend/eco-123-feature-global-analysis-tabs" main origin true
```

## Implementation outline

Map the arguments to `NAME`, `BASE`, `REMOTE`, and `PUSH`, applying the defaults above before running this Bash outline. Stop if a Git operation fails.

```bash
set -e

# Abort before fetching or switching if the local name already exists.
if git show-ref --verify --quiet "refs/heads/${NAME}"; then
  echo "Local branch already exists: ${NAME}" >&2
  exit 1
fi

# Fetch the selected base without checking it out locally.
git fetch --prune "${REMOTE}" "refs/heads/${BASE}:refs/remotes/${REMOTE}/${BASE}"

# Prefer switch; use checkout on older Git versions. Both set tracking explicitly.
if git switch -c "${NAME}" --track "${REMOTE}/${BASE}"; then
  :
else
  git checkout -b "${NAME}" --track "${REMOTE}/${BASE}"
fi

# A successful push replaces the base upstream with the new remote branch.
if [ "${PUSH}" = "true" ]; then
  git push -u "${REMOTE}" "${NAME}"
fi
```

Use [`/pr`](pr.md) or [`/pr-ready`](pr-ready.md) after creating a branch when you are ready to open a PR.
