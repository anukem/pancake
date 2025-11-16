use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn branch_delete_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["branch", "delete", "feature/test"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn branch_delete_simple_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create and delete a branch
    pk_cmd()
        .args(["branch", "create", "feature/to-delete"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Switch back to main before deleting
    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .args(["branch", "delete", "feature/to-delete"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Deleted branch 'feature/to-delete'"));

    // Verify branch is gone
    assert!(!branch_exists(repo.path(), "feature/to-delete"));

    // Verify metadata was removed
    let metadata = read_metadata(&repo);
    assert!(!metadata["branches"].as_object().unwrap().contains_key("feature/to-delete"));
}

#[test]
fn branch_delete_rejects_nonexistent_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "delete", "nonexistent"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Branch 'nonexistent' does not exist"));
}

#[test]
fn branch_delete_rejects_current_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "create", "feature/current"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["branch", "delete", "feature/current"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Cannot delete the currently checked out branch"));
}

#[test]
fn branch_delete_restacks_single_child() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second
    pk_cmd()
        .args(["branch", "create", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["branch", "create", "feature/second"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go back to main
    run_git(repo.path(), &["checkout", "main"]);

    // Delete the middle branch
    pk_cmd()
        .args(["branch", "delete", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Deleted branch 'feature/first' and restacked 1 child branch(es)"))
        .stdout(contains("Restacked 'feature/second' onto 'main'"));

    // Verify the child now points to main
    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/second"]["parent"].as_str(),
        Some("main"),
        "second should now be based on main"
    );

    // Verify first is gone
    assert!(!branch_exists(repo.path(), "feature/first"));
}

#[test]
fn branch_delete_restacks_multiple_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> parent -> child1
    //                            \-> child2
    pk_cmd()
        .args(["branch", "create", "feature/parent"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["branch", "create", "feature/child1"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go back to parent to create child2
    run_git(repo.path(), &["checkout", "feature/parent"]);

    pk_cmd()
        .args(["branch", "create", "feature/child2"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go to main to delete parent
    run_git(repo.path(), &["checkout", "main"]);

    // Delete the parent branch
    pk_cmd()
        .args(["branch", "delete", "feature/parent"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Deleted branch 'feature/parent' and restacked 2 child branch(es)"));

    // Verify both children now point to main
    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/child1"]["parent"].as_str(),
        Some("main"),
        "child1 should now be based on main"
    );
    assert_eq!(
        metadata["branches"]["feature/child2"]["parent"].as_str(),
        Some("main"),
        "child2 should now be based on main"
    );

    // Verify parent is gone
    assert!(!branch_exists(repo.path(), "feature/parent"));
}

#[test]
fn branch_delete_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["branch", "create", "feature/alias-test"])
        .current_dir(repo.path())
        .assert()
        .success();

    run_git(repo.path(), &["checkout", "main"]);

    // Test using alias 'd'
    pk_cmd()
        .args(["branch", "d", "feature/alias-test"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Deleted branch 'feature/alias-test'"));

    assert!(!branch_exists(repo.path(), "feature/alias-test"));
}

#[test]
fn bd_top_level_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/bd-test"])
        .current_dir(repo.path())
        .assert()
        .success();

    run_git(repo.path(), &["checkout", "main"]);

    // Test using top-level alias 'bd'
    pk_cmd()
        .args(["bd", "feature/bd-test"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Deleted branch 'feature/bd-test'"));

    assert!(!branch_exists(repo.path(), "feature/bd-test"));
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
