use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn log_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .arg("log")
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn log_notifies_when_empty() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .arg("log")
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("No tracked stacks yet"));
}

#[test]
fn log_displays_all_stacks_in_ascii_tree() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

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

    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .args(["branch", "create", "bugfix/hotfix"])
        .current_dir(repo.path())
        .assert()
        .success();

    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .arg("log")
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains(
            "main\n|-- bugfix/hotfix\n`-- feature/first\n    `-- feature/second",
        ));
}

#[test]
fn log_short_view_lists_paths() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

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

    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .args(["branch", "create", "bugfix/hotfix"])
        .current_dir(repo.path())
        .assert()
        .success();

    run_git(repo.path(), &["checkout", "main"]);

    pk_cmd()
        .args(["log", "--short"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("main -> bugfix/hotfix\nmain -> feature/first -> feature/second"));
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
