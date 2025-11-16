use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn branch_create_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["branch", "create", "feature/test"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn branch_create_from_current_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "create", "feature/new-branch"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/new-branch' based on 'main'"));

    // Verify branch exists
    assert!(branch_exists(repo.path(), "feature/new-branch"));

    // Verify metadata was created
    let metadata = read_metadata(&repo);
    let branch_meta = metadata["branches"]["feature/new-branch"]
        .as_object()
        .expect("branch metadata should exist");
    assert_eq!(
        branch_meta["parent"].as_str(),
        Some("main"),
        "parent should be main"
    );
    assert!(
        branch_meta.contains_key("created_at"),
        "should have created_at timestamp"
    );

    // Verify we're on the new branch
    assert_eq!(current_branch(repo.path()), "feature/new-branch");
}

#[test]
fn branch_create_with_base_option() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a branch first
    pk_cmd()
        .args(["branch", "create", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Create another branch from main explicitly
    pk_cmd()
        .args(["branch", "create", "feature/second", "--base", "main"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/second' based on 'main'"));

    // Verify metadata
    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/second"]["parent"].as_str(),
        Some("main")
    );
}

#[test]
fn branch_create_stacked_branches() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create first branch
    pk_cmd()
        .args(["branch", "create", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Create second branch on top of first
    pk_cmd()
        .args(["branch", "create", "feature/second"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/second' based on 'feature/first'"));

    // Create third branch on top of second
    pk_cmd()
        .args(["branch", "create", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/third' based on 'feature/second'"));

    // Verify metadata shows correct parent chain
    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/first"]["parent"].as_str(),
        Some("main")
    );
    assert_eq!(
        metadata["branches"]["feature/second"]["parent"].as_str(),
        Some("feature/first")
    );
    assert_eq!(
        metadata["branches"]["feature/third"]["parent"].as_str(),
        Some("feature/second")
    );
}

#[test]
fn branch_create_rejects_existing_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "create", "feature/test"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["branch", "create", "feature/test"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Branch 'feature/test' already exists"));
}

#[test]
fn branch_create_rejects_nonexistent_base() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "create", "feature/test", "--base", "nonexistent"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Base branch 'nonexistent' does not exist"));
}

#[test]
fn branch_create_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Test using alias 'c'
    pk_cmd()
        .args(["branch", "c", "feature/alias-test"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/alias-test'"));

    assert!(branch_exists(repo.path(), "feature/alias-test"));
}

#[test]
fn bc_top_level_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Test using top-level alias 'bc'
    pk_cmd()
        .args(["bc", "feature/bc-test"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created branch 'feature/bc-test'"));

    assert!(branch_exists(repo.path(), "feature/bc-test"));

    // Verify metadata was created
    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/bc-test"]["parent"].as_str(),
        Some("main")
    );
}

struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    fn new(default_branch: &str) -> Self {
        let dir = TempDir::new().expect("temp dir");
        run_git(dir.path(), &["init"]);
        fs::write(dir.path().join("README.md"), "# Test repo").expect("write readme");
        run_git(dir.path(), &["add", "README.md"]);
        run_git(dir.path(), &["commit", "-m", "init"]);

        checkout_branch(dir.path(), default_branch);

        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

fn init_pk(repo: &TestRepo) {
    pk_cmd()
        .arg("init")
        .current_dir(repo.path())
        .assert()
        .success();
}

fn read_metadata(repo: &TestRepo) -> serde_json::Value {
    let metadata_path = repo.path().join(".pancake/stacks.json");
    let raw = fs::read_to_string(metadata_path).expect("metadata should exist");
    serde_json::from_str(&raw).expect("metadata should be valid json")
}

fn branch_exists(dir: &Path, branch: &str) -> bool {
    StdCommand::new("git")
        .args(["rev-parse", "--verify", branch])
        .current_dir(dir)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn checkout_branch(dir: &Path, branch: &str) {
    if current_branch(dir) == branch {
        return;
    }
    run_git(dir, &["checkout", "-b", branch]);
}

fn current_branch(dir: &Path) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-parse");
    assert!(output.status.success(), "failed to query current branch");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Pancake")
        .env("GIT_AUTHOR_EMAIL", "pancake@example.com")
        .env("GIT_COMMITTER_NAME", "Pancake")
        .env("GIT_COMMITTER_EMAIL", "pancake@example.com")
        .status()
        .unwrap_or_else(|err| panic!("failed to run git {:?}: {err}", args));

    assert!(status.success(), "git {:?} failed", args);
}

fn pk_cmd() -> assert_cmd::Command {
    #[allow(deprecated)]
    {
        assert_cmd::Command::cargo_bin("pk").expect("pk binary")
    }
}
