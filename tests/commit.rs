use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn commit_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["commit", "-m", "test commit"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn commit_requires_message() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["commit"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("required"));
}

#[test]
fn commit_creates_commit_with_staged_changes() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create and stage a file
    fs::write(repo.path().join("test.txt"), "hello world").expect("write file");
    run_git(repo.path(), &["add", "test.txt"]);

    // Get initial commit count
    let initial_count = commit_count(repo.path());

    pk_cmd()
        .args(["commit", "-m", "Add test file"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created commit on branch 'main'"));

    // Verify commit was created
    assert_eq!(commit_count(repo.path()), initial_count + 1);

    // Verify commit message
    let last_message = last_commit_message(repo.path());
    assert_eq!(last_message, "Add test file");
}

#[test]
fn commit_with_all_flag_stages_and_commits() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create an unstaged file
    fs::write(repo.path().join("unstaged.txt"), "unstaged content").expect("write file");

    let initial_count = commit_count(repo.path());

    pk_cmd()
        .args(["commit", "-a", "-m", "Commit with --all"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created commit on branch 'main'"));

    // Verify commit was created
    assert_eq!(commit_count(repo.path()), initial_count + 1);

    // Verify the file was included in the commit
    let last_message = last_commit_message(repo.path());
    assert_eq!(last_message, "Commit with --all");

    // Verify file is tracked and committed
    let output = StdCommand::new("git")
        .args(["ls-tree", "-r", "HEAD", "--name-only"])
        .current_dir(repo.path())
        .output()
        .expect("git ls-tree");
    let files = String::from_utf8_lossy(&output.stdout);
    assert!(files.contains("unstaged.txt"), "file should be in commit");
}

#[test]
fn commit_amend_updates_last_commit() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create and commit a file
    fs::write(repo.path().join("file1.txt"), "content 1").expect("write file");
    run_git(repo.path(), &["add", "file1.txt"]);
    run_git(repo.path(), &["commit", "-m", "Original commit"]);

    let initial_count = commit_count(repo.path());

    // Create another file and stage it
    fs::write(repo.path().join("file2.txt"), "content 2").expect("write file");
    run_git(repo.path(), &["add", "file2.txt"]);

    pk_cmd()
        .args(["commit", "--amend", "-m", "Amended commit"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Amended commit on branch 'main'"));

    // Verify commit count didn't increase
    assert_eq!(commit_count(repo.path()), initial_count);

    // Verify commit message was updated
    let last_message = last_commit_message(repo.path());
    assert_eq!(last_message, "Amended commit");

    // Verify both files are in the commit
    let output = StdCommand::new("git")
        .args(["ls-tree", "-r", "HEAD", "--name-only"])
        .current_dir(repo.path())
        .output()
        .expect("git ls-tree");
    let files = String::from_utf8_lossy(&output.stdout);
    assert!(files.contains("file1.txt"), "first file should be in commit");
    assert!(files.contains("file2.txt"), "second file should be in commit");
}

#[test]
fn commit_alias_works() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    fs::write(repo.path().join("test.txt"), "hello").expect("write file");
    run_git(repo.path(), &["add", "test.txt"]);

    let initial_count = commit_count(repo.path());

    // Test using alias 'c'
    pk_cmd()
        .args(["c", "-m", "Using alias"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created commit"));

    assert_eq!(commit_count(repo.path()), initial_count + 1);
}

#[test]
fn commit_works_on_feature_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create a feature branch
    pk_cmd()
        .args(["branch", "create", "feature/test"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Make a commit on the feature branch
    fs::write(repo.path().join("feature.txt"), "feature content").expect("write file");
    run_git(repo.path(), &["add", "feature.txt"]);

    pk_cmd()
        .args(["commit", "-m", "Feature commit"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created commit on branch 'feature/test'"));

    // Verify commit message
    assert_eq!(last_commit_message(repo.path()), "Feature commit");
}

#[test]
fn commit_allows_empty_commit() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    let initial_count = commit_count(repo.path());

    // Create a commit without any changes (empty commit)
    pk_cmd()
        .args(["commit", "-m", "Empty commit"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Created commit on branch 'main'"));

    // Verify commit was created
    assert_eq!(commit_count(repo.path()), initial_count + 1);
    assert_eq!(last_commit_message(repo.path()), "Empty commit");
}

#[test]
fn commit_amend_with_all_flag() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    // Create and commit a file
    fs::write(repo.path().join("file1.txt"), "content 1").expect("write file");
    run_git(repo.path(), &["add", "file1.txt"]);
    run_git(repo.path(), &["commit", "-m", "Original commit"]);

    let initial_count = commit_count(repo.path());

    // Modify the file without staging
    fs::write(repo.path().join("file1.txt"), "modified content").expect("write file");

    pk_cmd()
        .args(["commit", "-a", "--amend", "-m", "Amended with --all"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Amended commit on branch 'main'"));

    // Verify commit count didn't increase
    assert_eq!(commit_count(repo.path()), initial_count);

    // Verify commit message
    assert_eq!(last_commit_message(repo.path()), "Amended with --all");

    // Verify file content was updated
    let content = fs::read_to_string(repo.path().join("file1.txt")).expect("read file");
    assert_eq!(content, "modified content");
}

#[test]
fn commit_message_can_have_spaces() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    fs::write(repo.path().join("test.txt"), "content").expect("write file");
    run_git(repo.path(), &["add", "test.txt"]);

    pk_cmd()
        .args(["commit", "-m", "This is a commit with multiple words"])
        .current_dir(repo.path())
        .assert()
        .success();

    assert_eq!(
        last_commit_message(repo.path()),
        "This is a commit with multiple words"
    );
}

#[test]
fn commit_message_can_have_special_characters() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    fs::write(repo.path().join("test.txt"), "content").expect("write file");
    run_git(repo.path(), &["add", "test.txt"]);

    let message = "Fix bug #123: handle edge case with foo/bar";
    pk_cmd()
        .args(["commit", "-m", message])
        .current_dir(repo.path())
        .assert()
        .success();

    assert_eq!(last_commit_message(repo.path()), message);
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

fn commit_count(dir: &Path) -> usize {
    let output = StdCommand::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-list");
    assert!(output.status.success(), "failed to count commits");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("parse commit count")
}

fn last_commit_message(dir: &Path) -> String {
    let output = StdCommand::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(dir)
        .output()
        .expect("git log");
    assert!(output.status.success(), "failed to get last commit message");
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
