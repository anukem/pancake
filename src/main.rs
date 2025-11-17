use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use git2::{BranchType, Repository};
use serde::{Deserialize, Serialize};

fn main() {
    if let Err(err) = Cli::parse().run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

#[derive(Parser)]
#[command(name = "pk", version, about = "Pancake CLI (early preview)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    fn run(self) -> Result<()> {
        match self.command {
            Commands::Init(args) => handle_init(args),
            Commands::Branch(args) => handle_branch(args),
            Commands::Bc(args) => handle_branch_create(args),
            Commands::Bd(args) => handle_branch_delete(args),
            Commands::Log(args) => handle_log(args),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Pancake in the current repository
    Init(InitArgs),
    /// Branch management commands
    Branch(BranchArgs),
    /// Create a new branch in the stack (alias for 'branch create')
    #[command(name = "bc")]
    Bc(BranchCreateArgs),
    /// Delete a branch from the stack (alias for 'branch delete')
    #[command(name = "bd")]
    Bd(BranchDeleteArgs),
    /// Show the tracked stacks in ASCII form
    #[command(name = "log", alias = "l")]
    Log(LogArgs),
}

#[derive(Args)]
struct BranchArgs {
    #[command(subcommand)]
    command: BranchCommands,
}

#[derive(Subcommand)]
enum BranchCommands {
    /// Create a new branch in the stack
    #[command(alias = "c")]
    Create(BranchCreateArgs),
    /// Delete a branch from the stack
    #[command(alias = "d")]
    Delete(BranchDeleteArgs),
}

#[derive(Args)]
struct BranchCreateArgs {
    /// Name of the new branch
    branch_name: String,
    /// Specify a different base branch (defaults to current branch)
    #[arg(long)]
    base: Option<String>,
}

#[derive(Args)]
struct BranchDeleteArgs {
    /// Name of the branch to delete
    branch_name: String,
    /// Force delete even with unmerged changes
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct LogArgs {
    /// Show all stacks (currently the default behavior)
    #[arg(long = "all")]
    _all: bool,
    /// Print a condensed representation
    #[arg(long)]
    short: bool,
}

#[derive(Args)]
struct InitArgs {
    /// Overwrite existing configuration
    #[arg(long)]
    force: bool,
    /// Explicitly set the main branch
    #[arg(long)]
    main_branch: Option<String>,
    /// Explicitly set the Git remote to use
    #[arg(long)]
    remote: Option<String>,
}

fn handle_init(args: InitArgs) -> Result<()> {
    let repo =
        Repository::discover(".").context("`pk init` must be run inside a Git repository")?;
    let workdir = repo
        .workdir()
        .context("bare repositories are not supported by Pancake")?;
    let repo_root = workdir.to_path_buf();

    let main_branch = match args.main_branch {
        Some(name) => name,
        None => detect_main_branch(&repo)?,
    };

    let remote = args
        .remote
        .or_else(|| detect_remote(&repo))
        .unwrap_or_else(|| "origin".to_string());

    let config_dir = repo_root.join(".pancake");
    fs::create_dir_all(&config_dir).context("failed to create `.pancake/` directory")?;
    let config_path = config_dir.join("config");

    if config_path.exists() && !args.force {
        bail!(
            "Pancake is already initialized at {}\nUse `pk init --force` to overwrite the existing configuration.",
            display_path(&config_path)
        );
    }

    let config = PancakeConfig::new(&main_branch, &remote);
    let serialized =
        toml::to_string_pretty(&config).context("failed to serialize Pancake config")?;
    fs::write(&config_path, serialized)
        .with_context(|| format!("failed to write {}", display_path(&config_path)))?;

    println!(
        "Pancake initialized.\n- repo: {}\n- main branch: {}\n- remote: {}",
        display_path(&repo_root),
        main_branch,
        remote
    );

    Ok(())
}

fn handle_branch(args: BranchArgs) -> Result<()> {
    match args.command {
        BranchCommands::Create(create_args) => handle_branch_create(create_args),
        BranchCommands::Delete(delete_args) => handle_branch_delete(delete_args),
    }
}

fn handle_log(args: LogArgs) -> Result<()> {
    let repo = Repository::discover(".").context("`pk log` must be run inside a Git repository")?;
    let workdir = repo
        .workdir()
        .context("bare repositories are not supported by Pancake")?;
    let repo_root = workdir.to_path_buf();

    // Ensure Pancake is initialized
    let config_path = repo_root.join(".pancake/config");
    if !config_path.exists() {
        bail!("Pancake is not initialized. Run `pk init` first.");
    }

    let metadata = StackMetadata::load(&repo_root)?;
    if metadata.branches.is_empty() {
        println!("No tracked stacks yet. Create one with `pk branch create <name>`.");
        return Ok(());
    }

    let forest = build_stack_forest(&metadata);
    if args.short {
        render_short_view(&forest);
    } else {
        render_full_view(&forest);
    }

    Ok(())
}

fn handle_branch_delete(args: BranchDeleteArgs) -> Result<()> {
    let repo =
        Repository::discover(".").context("`pk branch delete` must be run inside a Git repository")?;
    let workdir = repo
        .workdir()
        .context("bare repositories are not supported by Pancake")?;
    let repo_root = workdir.to_path_buf();

    // Ensure Pancake is initialized
    let config_path = repo_root.join(".pancake/config");
    if !config_path.exists() {
        bail!("Pancake is not initialized. Run `pk init` first.");
    }

    // Check if the branch exists
    if !branch_exists(&repo, &args.branch_name) {
        bail!("Branch '{}' does not exist", args.branch_name);
    }

    // Prevent deleting the current branch
    let head = repo.head().context("unable to resolve current HEAD")?;
    let current_branch = if head.is_branch() {
        head.shorthand().map(|s| s.to_string())
    } else {
        None
    };

    if current_branch.as_deref() == Some(&args.branch_name) {
        bail!("Cannot delete the currently checked out branch '{}'", args.branch_name);
    }

    // Load stack metadata
    let mut metadata = StackMetadata::load(&repo_root)?;

    // Get the parent of the branch being deleted
    let parent = metadata
        .branches
        .get(&args.branch_name)
        .and_then(|m| m.parent.clone());

    // Get all children of the branch being deleted
    let children = metadata.get_children(&args.branch_name);

    // Restack children onto the deleted branch's parent
    for child in &children {
        metadata.update_parent(child, parent.clone());
        println!("Restacked '{}' onto '{}'", child, parent.as_deref().unwrap_or("main"));
    }

    // Delete the Git branch
    let mut branch = repo
        .find_branch(&args.branch_name, BranchType::Local)
        .with_context(|| format!("unable to find branch '{}'", args.branch_name))?;

    // Check if the branch is fully merged (unless --force is used)
    if !args.force {
        // Try to delete with the unmerged check
        match branch.delete() {
            Ok(_) => {},
            Err(e) => {
                bail!(
                    "Branch '{}' has unmerged changes. Use `--force` to delete anyway.\nError: {}",
                    args.branch_name,
                    e
                );
            }
        }
    } else {
        // Force delete
        branch.delete()
            .with_context(|| format!("failed to delete branch '{}'", args.branch_name))?;
    }

    // Remove from stack metadata
    metadata.remove_branch(&args.branch_name);
    metadata.save(&repo_root)?;

    if children.is_empty() {
        println!("Deleted branch '{}'", args.branch_name);
    } else {
        println!(
            "Deleted branch '{}' and restacked {} child branch(es)",
            args.branch_name,
            children.len()
        );
    }

    Ok(())
}

fn handle_branch_create(args: BranchCreateArgs) -> Result<()> {
    let repo =
        Repository::discover(".").context("`pk branch create` must be run inside a Git repository")?;
    let workdir = repo
        .workdir()
        .context("bare repositories are not supported by Pancake")?;
    let repo_root = workdir.to_path_buf();

    // Ensure Pancake is initialized
    let config_path = repo_root.join(".pancake/config");
    if !config_path.exists() {
        bail!("Pancake is not initialized. Run `pk init` first.");
    }

    // Determine the base branch
    let base_branch = match args.base {
        Some(base) => {
            // Verify the base branch exists
            if !branch_exists(&repo, &base) {
                bail!("Base branch '{}' does not exist", base);
            }
            base
        }
        None => {
            // Use current branch as base
            let head = repo.head().context("unable to resolve current HEAD")?;
            if !head.is_branch() {
                bail!("HEAD is not currently on a branch. Cannot determine base branch.");
            }
            head.shorthand()
                .ok_or_else(|| anyhow!("unable to get current branch name"))?
                .to_string()
        }
    };

    // Check if the new branch already exists
    if branch_exists(&repo, &args.branch_name) {
        bail!("Branch '{}' already exists", args.branch_name);
    }

    // Create the new branch
    let base_commit = repo
        .find_branch(&base_branch, BranchType::Local)
        .with_context(|| format!("unable to find branch '{}'", base_branch))?
        .get()
        .peel_to_commit()
        .with_context(|| format!("unable to get commit for branch '{}'", base_branch))?;

    repo.branch(&args.branch_name, &base_commit, false)
        .with_context(|| format!("failed to create branch '{}'", args.branch_name))?;

    // Checkout the new branch
    repo.set_head(&format!("refs/heads/{}", args.branch_name))
        .context("failed to set HEAD to new branch")?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .context("failed to checkout new branch")?;

    // Update stack metadata
    let mut metadata = StackMetadata::load(&repo_root)?;
    metadata.add_branch(args.branch_name.clone(), Some(base_branch.clone()));
    metadata.save(&repo_root)?;

    println!(
        "Created branch '{}' based on '{}' and switched to it",
        args.branch_name, base_branch
    );

    Ok(())
}

fn detect_main_branch(repo: &Repository) -> Result<String> {
    for candidate in ["main", "master", "develop"] {
        if branch_exists(repo, candidate) {
            return Ok(candidate.to_string());
        }
    }

    let head = repo
        .head()
        .with_context(|| "unable to resolve current HEAD branch")?;
    head.shorthand()
        .map(|name| name.to_string())
        .ok_or_else(|| {
            anyhow!("unable to detect the main branch; use `pk init --main-branch <name>`")
        })
}

fn detect_remote(repo: &Repository) -> Option<String> {
    let remotes = repo.remotes().ok()?;
    let has_origin = remotes.iter().flatten().any(|name| name == "origin");
    if has_origin {
        return Some("origin".to_string());
    }

    remotes.iter().flatten().next().map(|name| name.to_string())
}

fn branch_exists(repo: &Repository, name: &str) -> bool {
    repo.find_branch(name, BranchType::Local).is_ok()
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[derive(Serialize)]
struct PancakeConfig<'a> {
    repository: RepositoryConfig<'a>,
    pr: PrConfig<'a>,
    stack: StackConfig<'a>,
    github: GithubConfig,
}

impl<'a> PancakeConfig<'a> {
    fn new(main_branch: &'a str, remote: &'a str) -> Self {
        Self {
            repository: RepositoryConfig {
                main_branch,
                remote,
            },
            pr: PrConfig {
                auto_submit: false,
                draft_by_default: false,
                template: ".github/pull_request_template.md",
            },
            stack: StackConfig {
                max_depth: 10,
                prefix: "",
            },
            github: GithubConfig { api_token: "" },
        }
    }
}

#[derive(Serialize)]
struct RepositoryConfig<'a> {
    main_branch: &'a str,
    remote: &'a str,
}

#[derive(Serialize)]
struct PrConfig<'a> {
    auto_submit: bool,
    draft_by_default: bool,
    template: &'a str,
}

#[derive(Serialize)]
struct StackConfig<'a> {
    max_depth: u32,
    prefix: &'a str,
}

#[derive(Serialize)]
struct GithubConfig {
    api_token: &'static str,
}

// Stack metadata structures
#[derive(Debug, Serialize, Deserialize)]
struct StackMetadata {
    branches: HashMap<String, BranchMetadata>,
}

impl StackMetadata {
    fn load(repo_root: &Path) -> Result<Self> {
        let stacks_path = repo_root.join(".pancake/stacks.json");
        if !stacks_path.exists() {
            return Ok(Self {
                branches: HashMap::new(),
            });
        }

        let contents = fs::read_to_string(&stacks_path)
            .with_context(|| format!("failed to read {}", display_path(&stacks_path)))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", display_path(&stacks_path)))
    }

    fn save(&self, repo_root: &Path) -> Result<()> {
        let stacks_path = repo_root.join(".pancake/stacks.json");
        let serialized = serde_json::to_string_pretty(self)
            .context("failed to serialize stack metadata")?;
        fs::write(&stacks_path, serialized)
            .with_context(|| format!("failed to write {}", display_path(&stacks_path)))
    }

    fn add_branch(&mut self, branch_name: String, parent: Option<String>) {
        self.branches.insert(
            branch_name.clone(),
            BranchMetadata {
                parent,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
    }

    fn get_children(&self, branch_name: &str) -> Vec<String> {
        self.branches
            .iter()
            .filter_map(|(name, metadata)| {
                if metadata.parent.as_deref() == Some(branch_name) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn remove_branch(&mut self, branch_name: &str) {
        self.branches.remove(branch_name);
    }

    fn update_parent(&mut self, branch_name: &str, new_parent: Option<String>) {
        if let Some(metadata) = self.branches.get_mut(branch_name) {
            metadata.parent = new_parent;
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BranchMetadata {
    parent: Option<String>,
    created_at: String,
}

#[derive(Debug)]
enum StackRoot {
    ExternalParent { name: String, children: Vec<BranchNode> },
    Standalone { node: BranchNode },
}

#[derive(Debug)]
struct BranchNode {
    name: String,
    children: Vec<BranchNode>,
}

fn build_stack_forest(metadata: &StackMetadata) -> Vec<StackRoot> {
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut external_roots: HashMap<String, Vec<String>> = HashMap::new();
    let mut standalone_roots: Vec<String> = Vec::new();

    for (name, branch) in &metadata.branches {
        match &branch.parent {
            Some(parent) => {
                if metadata.branches.contains_key(parent) {
                    children_map
                        .entry(parent.clone())
                        .or_default()
                        .push(name.clone());
                } else {
                    external_roots
                        .entry(parent.clone())
                        .or_default()
                        .push(name.clone());
                }
            }
            None => standalone_roots.push(name.clone()),
        }
    }

    for children in children_map.values_mut() {
        children.sort();
    }
    for children in external_roots.values_mut() {
        children.sort();
    }
    standalone_roots.sort();

    let mut roots: Vec<StackRoot> = Vec::new();

    let mut external_names: Vec<_> = external_roots.keys().cloned().collect();
    external_names.sort();
    for name in external_names {
        let children = external_roots
            .get(&name)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|child| build_branch_node(&child, &children_map))
            .collect();
        roots.push(StackRoot::ExternalParent { name, children });
    }

    for branch_name in standalone_roots {
        roots.push(StackRoot::Standalone {
            node: build_branch_node(&branch_name, &children_map),
        });
    }

    roots
}

fn build_branch_node(name: &str, children_map: &HashMap<String, Vec<String>>) -> BranchNode {
    let child_names = children_map.get(name);
    let mut children = Vec::new();
    if let Some(names) = child_names {
        for child in names {
            children.push(build_branch_node(child, children_map));
        }
    }

    BranchNode {
        name: name.to_string(),
        children,
    }
}

fn render_full_view(roots: &[StackRoot]) {
    for (idx, root) in roots.iter().enumerate() {
        match root {
            StackRoot::ExternalParent { name, children } => {
                println!("{name}");
                render_children(children);
            }
            StackRoot::Standalone { node } => {
                println!("{}", node.name);
                render_children(&node.children);
            }
        }

        if idx + 1 < roots.len() {
            println!();
        }
    }
}

fn render_children(children: &[BranchNode]) {
    for (idx, child) in children.iter().enumerate() {
        let is_last = idx == children.len() - 1;
        render_branch(child, "", is_last);
    }
}

fn render_branch(node: &BranchNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "`--" } else { "|--" };
    println!("{prefix}{connector} {}", node.name);

    let next_prefix = if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}|   ")
    };

    for (idx, child) in node.children.iter().enumerate() {
        let child_is_last = idx == node.children.len() - 1;
        render_branch(child, &next_prefix, child_is_last);
    }
}

fn render_short_view(roots: &[StackRoot]) {
    let mut lines = Vec::new();

    for root in roots {
        match root {
            StackRoot::ExternalParent { name, children } => {
                for child in children {
                    collect_paths(child, vec![name.clone()], &mut lines);
                }
            }
            StackRoot::Standalone { node } => {
                collect_paths(node, Vec::new(), &mut lines);
            }
        }
    }

    for line in lines {
        println!("{}", line.join(" -> "));
    }
}

fn collect_paths(node: &BranchNode, mut current: Vec<String>, output: &mut Vec<Vec<String>>) {
    current.push(node.name.clone());
    if node.children.is_empty() {
        output.push(current);
    } else {
        for child in &node.children {
            collect_paths(child, current.clone(), output);
        }
    }
}
