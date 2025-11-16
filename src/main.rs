use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use git2::{BranchType, Repository};
use serde::Serialize;

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
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Pancake in the current repository
    Init(InitArgs),
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
