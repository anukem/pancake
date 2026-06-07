# `pk land` — Design Plan

## Problem

After a stacked branch is merged into main, Pancake has no automatic way to:
- Detect that the branch has landed
- Reparent its children to `main`
- Restack those children on the new base

Today the user must manually run `pk bd <merged-branch>` for each merged branch (in
bottom-up order), then `pk restack`. `pk sync` is unaware of the remote entirely.

---

## Command

```
pk land [--dry-run] [--no-delete] [--no-restack]
```

| Flag | Effect |
|---|---|
| `--dry-run` | Print planned actions, touch nothing |
| `--no-delete` | Reparent and restack but keep merged branches locally |
| `--no-restack` | Reparent and update metadata only, skip rebasing |

---

## Steps

### 1. Fetch
```
git fetch <remote>
```
Bring `origin/main` up to date before any detection. Remote name comes from
`.pancake/config` (same field already used by `pk submit`).

### 2. Detect merged branches
For each branch tracked in `stacks.json`, check whether its tip commit is an
ancestor of `origin/main`:
```
git merge-base --is-ancestor <tip> origin/main
```
This is local, fast, and correct regardless of how the branch was merged (squash,
rebase, merge commit). No `gh` call required.

Process branches in bottom-up order (root → leaves) so a chain where A, B, and C
are all merged resolves correctly in a single pass.

### 3. Reparent children
For each landed branch, extract the reparent logic already present in
`handle_branch_delete` into a shared helper, then call it here:
- Find all children whose `parent` == landed branch name
- Set their `parent` to the landed branch's `parent` (or `main` if it had none)
- Write updated `stacks.json`

### 4. Remove landed branches from metadata
Delete their entries from `stacks.json`. Optionally delete the local git branch
(skipped with `--no-delete`).

### 5. Restack remaining branches
Run the existing restack logic across all branches still tracked. On conflict, pause
and print `--continue` / `--abort` instructions, identical to `pk restack` today.
Skipped with `--no-restack`.

---

## Example output

```
Fetching origin...
Landed:    submit/data-model, submit/gh-helpers
Reparented: submit/command → main
Deleted local: submit/data-model, submit/gh-helpers
Restacked: submit/command
```

Nothing to do case:
```
Fetching origin...
Nothing to land.
```

---

## Edge cases

| Case | Behavior |
|---|---|
| No tracked branches merged | Print "Nothing to land." and exit 0 |
| Entire stack merged | Remove all entries, nothing to restack |
| Restack conflict mid-run | Pause, persist state, print `--continue`/`--abort` |
| Branch merged but no local copy | Skip local delete, still reparent and update metadata |
| Dirty working tree | Bail before fetching (same guard as `pk restack`) |
| `--no-delete` + `--no-restack` | Metadata-only update, useful for scripting |

---

## Files to touch

- `src/main.rs`
  - New `Commands::Land`, `LandArgs`, `handle_land`
  - Extract reparent logic from `handle_branch_delete` into a shared
    `reparent_children(metadata, branch, new_parent)` helper so both `pk bd` and
    `pk land` use the same code path
