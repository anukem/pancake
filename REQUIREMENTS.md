# Pancake - Stacked PR CLI Tool Requirements

## Overview
Pancake is a Rust-based command-line tool designed to help developers create, manage, and submit stacked pull requests (PRs). The CLI is invoked using the `pk` command. Stacked PRs allow developers to break large features into smaller, reviewable chunks that build on top of each other, improving code review quality and iteration speed.

## Core Concepts

### Stack Structure
- A **stack** is a series of branches where each branch builds on top of the previous one
- Each branch in the stack corresponds to one pull request
- Changes flow upward: rebasing the bottom of the stack automatically updates branches above it
- The bottom of the stack is based on the main branch (e.g., `main` or `master`)

### Branch Tracking
- Track parent-child relationships between branches
- Maintain metadata about stack structure locally
- Support visualization of the current stack hierarchy

## Core Features

### 1. Stack Creation & Management

#### `pk init`
- Initialize Pancake in the current repository
- Create configuration file (`.pancake/config`)
- Set up default branch tracking
- Detect main branch (main/master/develop)

#### `pk branch create <branch-name>` (alias: `pk bc`)
- Create a new branch in the stack
- Automatically set the current branch as the parent
- Track the relationship in local metadata
- Options:
  - `--base <branch>`: Specify a different base branch
  - `--insert-before <branch>`: Insert new branch before specified branch in stack
  - `--insert-after <branch>`: Insert new branch after specified branch in stack

#### `pk branch rename <new-name>` (alias: `pk br`)
- Rename current branch
- Update all metadata and tracking
- Update remote if branch was already pushed

#### `pk branch delete <branch-name>` (alias: `pk bd`)
- Delete a branch from the stack
- Restack children branches onto the deleted branch's parent
- Options:
  - `--force`: Force delete even with unmerged changes

#### `pk branch checkout <branch-name>` (alias: `pk co`)
- Checkout a branch in the current stack
- Support fuzzy finding/partial name matching
- Show stack context after checkout

### 2. Stack Navigation

#### `pk up` (alias: `pk u`)
- Navigate to the parent branch in the stack
- Options:
  - `pk up <n>`: Move up n branches

#### `pk down` (alias: `pk d`)
- Navigate to the child branch in the stack
- If multiple children exist, show selector
- Options:
  - `pk down <n>`: Move down n branches

#### `pk top`
- Navigate to the topmost branch in the current stack

#### `pk bottom`
- Navigate to the bottom of the current stack (just above main)

#### `pk log` (alias: `pk l`)
- Display the current stack structure
- Show branch names, commit counts, PR status
- Visual tree representation
- Options:
  - `--all`: Show all stacks, not just current
  - `--short`: Compact view

### 3. Stack Synchronization

#### `pk sync` (alias: `pk s`)
- Rebase current branch onto its parent
- Automatically rebase all children recursively
- Handle conflicts interactively
- Options:
  - `--all`: Sync all branches in the stack
  - `--from-main`: Sync the entire stack from the main branch
  - `--continue`: Continue after resolving conflicts
  - `--abort`: Abort the sync operation

#### `pk restack`
- Rebase the entire stack from bottom to top
- Update all branches to reflect changes
- Preserve individual branch commits

### 4. Commit Management

#### `pk commit` (alias: `pk c`)
- Create a commit in the current branch
- Options:
  - `-m <message>`: Commit message
  - `--amend`: Amend the last commit
  - `--all`: Stage all changes

#### `pk amend`
- Amend the last commit and propagate changes up the stack
- Automatically rebase children if needed

#### `pk move` (alias: `pk mv`)
- Move commits between branches in the stack
- Interactive commit selector
- Options:
  - `--to <branch>`: Target branch for commits
  - `--from <branch>`: Source branch for commits

### 5. Pull Request Management

#### `pk submit` (alias: `pk pr`)
- Create/update pull requests for branches in the stack
- Automatically set base branch to parent
- Add stack context to PR description
- Options:
  - `--all`: Submit all branches in the stack
  - `--from <branch>`: Submit from specified branch upward
  - `--draft`: Create as draft PRs
  - `--no-edit`: Skip PR description editing

#### `pk pr status`
- Show status of all PRs in the current stack
- Display review status, CI status, approval state
- Highlight which PRs are ready to merge

#### `pk pr list`
- List all stacks and their associated PRs
- Show summary information

#### `pk land` (alias: `pk merge`)
- Merge a PR and restack remaining branches
- Automatically update children to new base
- Options:
  - `--all`: Land all merged PRs in the stack
  - `--squash`: Squash merge
  - `--merge`: Merge commit
  - `--rebase`: Rebase merge

### 6. Stack Visualization

#### `pk stack`
- Show detailed stack visualization
- Include:
  - Branch names
  - Commit counts
  - PR numbers and status
  - Behind/ahead counts
  - CI status
- ASCII tree diagram

#### `pk graph` (alias: `pk g`)
- Show commit graph for the current stack
- Visual representation of commits across branches

### 7. Remote Synchronization

#### `pk push`
- Push current branch and update PR
- Force-push with lease for safety
- Options:
  - `--all`: Push all branches in the stack
  - `--no-pr`: Skip PR creation/update

#### `pk pull`
- Pull changes from remote
- Sync with main branch if needed
- Restack if necessary

#### `pk fetch`
- Fetch remote changes without applying them
- Show status of what would change

## Advanced Features

### Interactive Rebase
- `pk rebase -i`: Interactive rebase with stack awareness
- Automatically update children after rebase

### Conflict Resolution
- Smart conflict detection across stack
- Show which branches will be affected
- Guided conflict resolution workflow

### Integration with GitHub/GitLab
- Automatic PR description with stack context
- Link to parent and child PRs
- CI status integration
- Review status tracking

### Undo/Redo
- `pk undo`: Undo last Pancake operation
- `pk redo`: Redo undone operation
- Maintain operation history

## Configuration

### Repository Config (`.pancake/config`)
```toml
[repository]
main_branch = "main"
remote = "origin"

[pr]
auto_submit = false
draft_by_default = false
template = ".github/pull_request_template.md"

[stack]
max_depth = 10
prefix = ""  # Optional prefix for stack branches

[github]
api_token = ""  # Can also use environment variable
```

### Global Config (`~/.config/pancake/config.toml`)
```toml
[defaults]
editor = "vim"
pager = "less"

[aliases]
# Custom command aliases
```

## Data Storage

### Metadata Storage
- Store stack metadata in `.pancake/stacks.json`
- Track:
  - Branch relationships (parent/child)
  - PR associations
  - Creation timestamps
  - Custom metadata

### Git Notes
- Use git notes as backup for branch relationships
- Enable sync across machines
- Fallback if `.pancake/` directory is lost

## Error Handling

### Conflict Detection
- Detect conflicts before they happen
- Show impact analysis
- Allow dry-run of operations

### Validation
- Validate stack integrity before operations
- Prevent orphaned branches
- Detect and fix broken relationships

### Recovery
- Auto-backup before destructive operations
- Recovery commands for common mistakes
- Clear error messages with suggested fixes

## Performance Considerations

- Lazy loading for large repositories
- Caching of git operations
- Parallel PR status checks
- Incremental stack updates

## CLI Design Principles

1. **Fast by Default**: Common operations should be quick
2. **Safe**: Prevent data loss, confirm destructive operations
3. **Informative**: Clear output, helpful error messages
4. **Composable**: Commands work well together
5. **Progressive Disclosure**: Simple commands for common tasks, flags for advanced usage

## Comparison with Existing Tools

### vs. Graphite (gt)
- Similar stack management model
- Pancake advantages:
  - Written in Rust (faster, single binary)
  - Simpler command structure
  - Better offline support
  - More flexible branch relationships

### vs. git-town
- git-town focuses on workflow patterns
- Pancake is specialized for stacked PRs
- More GitHub/GitLab integration
- Better PR lifecycle management

### vs. Stacked Git (stgit)
- stgit uses patch-based model
- Pancake uses branch-based model (more familiar)
- Better integration with PR workflows
- Modern CLI/UX

## Future Enhancements

- TUI (Terminal UI) mode for interactive stack management
- VS Code / IDE extensions
- Web dashboard for stack visualization
- Team collaboration features
- Stack sharing and review
- Analytics and insights
- AI-assisted commit/PR descriptions
- Integration with issue trackers

## Success Metrics

- Time to create and submit a stack
- Conflict resolution success rate
- User retention and daily active usage
- PR review cycle time improvement
- Adoption rate in development teams
