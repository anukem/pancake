use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn init_creates_config_and_detects_main_branch() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .arg("init")
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout(contains("main branch: main"));

    let config_path = repo.path().join(".pancake/config");
    let raw = fs::read_to_string(config_path).expect("config should exist");
    let doc: toml::Value = toml::from_str(&raw).expect("config should be valid toml");

    assert_eq!(
        doc["repository"]["main_branch"].as_str(),
        Some("main"),
        "main branch should be persisted"
    );
}

#[test]
fn init_requires_force_to_overwrite() {
    let repo = TestRepo::new("master");

    pk_cmd()
        .arg("init")
        .current_dir(repo.path())
        .assert()
        .success();

    pk_cmd()
        .arg("init")
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("already initialized"));

    pk_cmd()
        .args(["init", "--force", "--main-branch", "develop"])
        .current_dir(repo.path())
        .assert()
        .success();

    let raw = fs::read_to_string(repo.path().join(".pancake/config")).expect("config should exist");
    let doc: toml::Value = toml::from_str(&raw).expect("config should be valid toml");
    assert_eq!(doc["repository"]["main_branch"].as_str(), Some("develop"));
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
