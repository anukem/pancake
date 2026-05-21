use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn branch_rename_updates_metadata_and_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/old"])
        .current_dir(repo.path())
        .assert()
        .success();
    pk_cmd()
        .args(["bc", "feature/child"])
        .current_dir(repo.path())
        .assert()
        .success();
    run_git(repo.path(), &["checkout", "feature/old"]);

    pk_cmd()
        .args(["branch", "rename", "feature/new"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Renamed branch 'feature/old' to 'feature/new'"));

    assert!(!branch_exists(repo.path(), "feature/old"));
    assert!(branch_exists(repo.path(), "feature/new"));
    assert_eq!(current_branch(repo.path()), "feature/new");

    let metadata = read_metadata(&repo);
    assert!(metadata["branches"].get("feature/old").is_none());
    assert!(metadata["branches"].get("feature/new").is_some());
    assert_eq!(
        metadata["branches"]["feature/child"]["parent"].as_str(),
        Some("feature/new")
    );
}

#[test]
fn br_top_level_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/old"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["br", "feature/new"])
        .current_dir(repo.path())
        .assert()
        .success();

    assert!(branch_exists(repo.path(), "feature/new"));
}

#[test]
fn branch_checkout_supports_exact_and_fuzzy_matches_in_current_stack() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();
    pk_cmd()
        .args(["bc", "feature/second"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["branch", "checkout", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/first'"));
    assert_eq!(current_branch(repo.path()), "feature/first");

    pk_cmd()
        .args(["co", "second"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/second'"));
    assert_eq!(current_branch(repo.path()), "feature/second");
}

#[test]
fn branch_checkout_rejects_ambiguous_fuzzy_match() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/alpha"])
        .current_dir(repo.path())
        .assert()
        .success();
    run_git(repo.path(), &["checkout", "main"]);
    pk_cmd()
        .args(["bc", "feature/beta"])
        .current_dir(repo.path())
        .assert()
        .success();
    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .args(["co", "feature"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("ambiguous"));
}

#[test]
fn branch_create_insert_before_places_new_branch_between_parent_and_target() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/target"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args([
            "bc",
            "feature/inserted",
            "--insert-before",
            "feature/target",
        ])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("inserted before 'feature/target'"));

    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/inserted"]["parent"].as_str(),
        Some("main")
    );
    assert_eq!(
        metadata["branches"]["feature/target"]["parent"].as_str(),
        Some("feature/inserted")
    );
}

#[test]
fn branch_create_insert_after_places_new_branch_between_target_and_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/target"])
        .current_dir(repo.path())
        .assert()
        .success();
    pk_cmd()
        .args(["bc", "feature/child"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["bc", "feature/inserted", "--insert-after", "feature/target"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("inserted after 'feature/target'"));

    let metadata = read_metadata(&repo);
    assert_eq!(
        metadata["branches"]["feature/inserted"]["parent"].as_str(),
        Some("feature/target")
    );
    assert_eq!(
        metadata["branches"]["feature/child"]["parent"].as_str(),
        Some("feature/inserted")
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
    let raw = fs::read_to_string(repo.path().join(".pancake/stacks.json"))
        .expect("metadata should exist");
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
