use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn up_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn up_navigates_to_child() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> feature/first -> feature/second
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

    // Go back to first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Now on feature/first, go up to second
    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/second'"));

    assert_eq!(current_branch(repo.path()), "feature/second");
}

#[test]
fn up_with_count() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second -> third
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
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go down to first
    pk_cmd()
        .args(["down", "2"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Now on feature/first, go up 2 to third
    pk_cmd()
        .args(["up", "2"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/third'"));

    assert_eq!(current_branch(repo.path()), "feature/third");
}

#[test]
fn up_fails_when_no_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Try to go up from feature/first (no children, it's a leaf)
    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("has no children in the stack"));
}

#[test]
fn up_alias_works() {
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

    // Go down first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Use 'u' alias to go back up
    pk_cmd()
        .args(["u"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/second'"));
}

#[test]
fn down_navigates_to_parent() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> feature/first -> feature/second
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

    // Now on feature/second, go down to first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/first'"));

    assert_eq!(current_branch(repo.path()), "feature/first");
}

#[test]
fn down_with_count() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second -> third
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
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Now on third, go down 2 to first
    pk_cmd()
        .args(["down", "2"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/first'"));

    assert_eq!(current_branch(repo.path()), "feature/first");
}

#[test]
fn down_fails_when_no_parent() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Try to go down from feature/first (parent is main which is not tracked)
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("has no parent in the stack"));
}

#[test]
fn up_fails_with_multiple_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create branch with multiple children
    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .args(["bc", "feature/second-a"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go back to first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Create another child
    pk_cmd()
        .args(["bc", "feature/second-b"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go back to first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Try to go up (should fail with multiple children)
    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stdout(contains("has multiple children"));
}

#[test]
fn down_alias_works() {
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

    // Use 'd' alias to go down from second to first
    pk_cmd()
        .args(["d"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/first'"));
}

#[test]
fn top_navigates_to_topmost_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second -> third
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
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go back to first
    pk_cmd()
        .args(["down", "2"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Now go to top
    pk_cmd()
        .args(["top"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/third' (top of stack)"));

    assert_eq!(current_branch(repo.path()), "feature/third");
}

#[test]
fn top_when_already_at_top() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Already at top
    pk_cmd()
        .args(["top"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Already at the top of the stack"));

    assert_eq!(current_branch(repo.path()), "feature/first");
}

#[test]
fn bottom_navigates_to_bottom_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second -> third
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
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Now go to bottom
    pk_cmd()
        .args(["bottom"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Switched to branch 'feature/first' (bottom of stack)"));

    assert_eq!(current_branch(repo.path()), "feature/first");
}

#[test]
fn bottom_when_already_at_bottom() {
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

    // Go back to first
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Already at bottom
    pk_cmd()
        .args(["bottom"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Already at the bottom of the stack"));

    assert_eq!(current_branch(repo.path()), "feature/first");
}

#[test]
fn navigation_round_trip() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a stack: main -> first -> second -> third
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
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Go to bottom
    pk_cmd()
        .args(["bottom"])
        .current_dir(repo.path())
        .assert()
        .success();
    assert_eq!(current_branch(repo.path()), "feature/first");

    // Go to top
    pk_cmd()
        .args(["top"])
        .current_dir(repo.path())
        .assert()
        .success();
    assert_eq!(current_branch(repo.path()), "feature/third");

    // Go up one from top (should fail - no children)
    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .failure();

    // Go down one
    pk_cmd()
        .args(["down"])
        .current_dir(repo.path())
        .assert()
        .success();
    assert_eq!(current_branch(repo.path()), "feature/second");

    // Go up one
    pk_cmd()
        .args(["up"])
        .current_dir(repo.path())
        .assert()
        .success();
    assert_eq!(current_branch(repo.path()), "feature/third");
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
