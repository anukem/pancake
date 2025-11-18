use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn sync_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["sync"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn sync_rebases_current_branch_and_children() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/base"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "base.txt", "base branch", "base commit");

    pk_cmd()
        .args(["bc", "feature/top"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "top.txt", "top branch", "top commit");

    run_git(repo.path(), &["checkout", "main"]);
    write_and_commit(&repo, "README.md", "main updated", "main update");

    run_git(repo.path(), &["checkout", "feature/base"]);

    pk_cmd()
        .args(["sync"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Synced 2 branch(es)"));

    assert_eq!(
        merge_base(repo.path(), "feature/base", "main"),
        rev_parse(repo.path(), "main")
    );
    assert_eq!(
        merge_base(repo.path(), "feature/top", "feature/base"),
        rev_parse(repo.path(), "feature/base")
    );
    assert_eq!(current_branch(repo.path()), "feature/base");
}

#[test]
fn sync_all_rebases_entire_stack() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/first"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "first.txt", "first", "first commit");

    pk_cmd()
        .args(["bc", "feature/second"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "second.txt", "second", "second commit");

    pk_cmd()
        .args(["bc", "feature/third"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "third.txt", "third", "third commit");

    run_git(repo.path(), &["checkout", "main"]);
    write_and_commit(&repo, "README.md", "main sync all", "main sync all");

    run_git(repo.path(), &["checkout", "feature/third"]);

    pk_cmd()
        .args(["sync", "--all"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Synced 3 branch(es)"));

    assert_eq!(
        merge_base(repo.path(), "feature/first", "main"),
        rev_parse(repo.path(), "main")
    );
    assert_eq!(
        merge_base(repo.path(), "feature/second", "feature/first"),
        rev_parse(repo.path(), "feature/first")
    );
    assert_eq!(
        merge_base(repo.path(), "feature/third", "feature/second"),
        rev_parse(repo.path(), "feature/second")
    );
    assert_eq!(current_branch(repo.path()), "feature/third");
}

#[test]
fn restack_rebases_entire_stack() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["bc", "feature/alpha"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "alpha.txt", "alpha", "alpha commit");

    pk_cmd()
        .args(["bc", "feature/beta"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "beta.txt", "beta", "beta commit");

    pk_cmd()
        .args(["bc", "feature/gamma"])
        .current_dir(repo.path())
        .assert()
        .success();
    write_and_commit(&repo, "gamma.txt", "gamma", "gamma commit");

    run_git(repo.path(), &["checkout", "main"]);
    write_and_commit(&repo, "README.md", "main restack", "main restack");

    run_git(repo.path(), &["checkout", "feature/beta"]);

    pk_cmd()
        .args(["restack"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("Restacked 3 branch(es)"));

    assert_eq!(
        merge_base(repo.path(), "feature/alpha", "main"),
        rev_parse(repo.path(), "main")
    );
    assert_eq!(
        merge_base(repo.path(), "feature/beta", "feature/alpha"),
        rev_parse(repo.path(), "feature/alpha")
    );
    assert_eq!(
        merge_base(repo.path(), "feature/gamma", "feature/beta"),
        rev_parse(repo.path(), "feature/beta")
    );
    assert_eq!(current_branch(repo.path()), "feature/beta");
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

fn write_and_commit(repo: &TestRepo, filename: &str, contents: &str, message: &str) {
    fs::write(repo.path().join(filename), contents).expect("write file");
    run_git(repo.path(), &["add", filename]);
    run_git(repo.path(), &["commit", "-m", message]);
}

fn merge_base(dir: &Path, left: &str, right: &str) -> String {
    let output = StdCommand::new("git")
        .args(["merge-base", left, right])
        .current_dir(dir)
        .output()
        .expect("git merge-base");
    assert!(output.status.success(), "merge-base failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn rev_parse(dir: &Path, rev: &str) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", rev])
        .current_dir(dir)
        .output()
        .expect("git rev-parse");
    assert!(output.status.success(), "rev-parse failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
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

fn checkout_branch(dir: &Path, branch: &str) {
    if current_branch(dir) == branch {
        return;
    }
    run_git(dir, &["checkout", "-b", branch]);
}

fn pk_cmd() -> assert_cmd::Command {
    #[allow(deprecated)]
    {
        assert_cmd::Command::cargo_bin("pk").expect("pk binary")
    }
}
