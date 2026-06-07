use std::{fs, path::Path, process::Command as StdCommand};

use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn submit_requires_init() {
    let repo = TestRepo::new("main");

    pk_cmd()
        .args(["submit"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("Pancake is not initialized"));
}

#[test]
fn submit_errors_on_untracked_current_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    pk_cmd()
        .args(["submit"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("not tracked by Pancake"));
}

#[test]
fn submit_dry_run_default_shows_only_current_branch() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    create_stack(&repo, &["feature/base", "feature/mid", "feature/top"]);
    run_git(repo.path(), &["checkout", "feature/mid"]);

    let assert = pk_cmd()
        .args(["submit", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("Would submit 'feature/mid' (base: 'feature/base')"),
        "stdout was:\n{stdout}"
    );
    assert!(
        !stdout.contains("Would submit 'feature/top'"),
        "default should not include children, stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("Would submit 'feature/base'"),
        "default should not include parent, stdout:\n{stdout}"
    );
    assert!(stdout.contains("1 branch(es) would be submitted"));
}

#[test]
fn submit_dry_run_all_walks_whole_stack() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    create_stack(&repo, &["feature/base", "feature/mid", "feature/top"]);

    let assert = pk_cmd()
        .args(["submit", "--all", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("Would submit 'feature/base' (base: 'main')"));
    assert!(stdout.contains("Would submit 'feature/mid' (base: 'feature/base')"));
    assert!(stdout.contains("Would submit 'feature/top' (base: 'feature/mid')"));
    assert!(stdout.contains("3 branch(es) would be submitted"));
}

#[test]
fn submit_dry_run_from_branch_starts_there() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    create_stack(&repo, &["feature/base", "feature/mid", "feature/top"]);
    run_git(repo.path(), &["checkout", "feature/top"]);

    let assert = pk_cmd()
        .args(["submit", "--from", "feature/mid", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("Would submit 'feature/mid' (base: 'feature/base')"));
    assert!(stdout.contains("Would submit 'feature/top' (base: 'feature/mid')"));
    assert!(!stdout.contains("Would submit 'feature/base'"));
    assert!(stdout.contains("2 branch(es) would be submitted"));
}

#[test]
fn submit_all_and_from_conflict() {
    let repo = TestRepo::new("main");
    init_pk(&repo);
    create_stack(&repo, &["feature/base"]);

    pk_cmd()
        .args(["submit", "--all", "--from", "feature/base", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .failure();
}

#[test]
fn submit_from_unknown_branch_errors() {
    let repo = TestRepo::new("main");
    init_pk(&repo);
    create_stack(&repo, &["feature/base"]);

    pk_cmd()
        .args(["submit", "--from", "feature/nope", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stderr(contains("not tracked by Pancake"));
}

#[cfg(unix)]
#[test]
fn submit_creates_and_updates_prs_via_mock_gh() {
    let repo = TestRepo::new("main");
    init_pk(&repo);

    let bare = setup_bare_remote(&repo);
    let _ = &bare;

    create_stack(&repo, &["feature/base", "feature/top"]);
    run_git(repo.path(), &["checkout", "feature/base"]);

    let mock = install_mock_gh(repo.path());

    let assert = pk_cmd()
        .args(["submit", "--all"])
        .env("PANCAKE_GH_BIN", &mock.script)
        .current_dir(repo.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("Created PR #100 for 'feature/base'"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Created PR #101 for 'feature/top'"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Submitted 2 branch(es)."));

    let log = fs::read_to_string(&mock.log).expect("read mock gh log");
    assert!(
        log.contains("pr view feature/base"),
        "log missing pr view feature/base:\n{log}"
    );
    assert!(
        log.contains("pr view feature/top"),
        "log missing pr view feature/top:\n{log}"
    );
    assert!(
        log.contains("pr create --head feature/base --base main"),
        "log missing create for base:\n{log}"
    );
    assert!(
        log.contains("pr create --head feature/top --base feature/base"),
        "log missing create for top:\n{log}"
    );
    assert!(
        log.contains("pancake:stack"),
        "PR body should embed the stack marker:\n{log}"
    );

    let stacks_path = repo.path().join(".pancake/stacks.json");
    let stacks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&stacks_path).unwrap()).unwrap();
    let base_meta = &stacks["branches"]["feature/base"];
    assert_eq!(base_meta["pr_number"], 100);
    assert_eq!(base_meta["pr_url"], "https://github.com/example/repo/pull/100");
    let top_meta = &stacks["branches"]["feature/top"];
    assert_eq!(top_meta["pr_number"], 101);

    // Second run: PRs already exist. Mock should report existing PRs and we should hit edit.
    fs::write(&mock.log, "").unwrap();
    fs::write(repo.path().join(".gh-mode"), "existing").unwrap();

    let assert = pk_cmd()
        .args(["submit", "--all"])
        .env("PANCAKE_GH_BIN", &mock.script)
        .current_dir(repo.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("Updated PR #200 for 'feature/base'"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Updated PR #200 for 'feature/top'"),
        "stdout: {stdout}"
    );

    let log = fs::read_to_string(&mock.log).expect("read mock gh log");
    assert!(
        log.contains("pr edit 200 --base main"),
        "expected edit for base; log:\n{log}"
    );
    assert!(
        log.contains("pr edit 200 --base feature/base"),
        "expected edit for top; log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn submit_propagates_draft_flag_to_gh() {
    let repo = TestRepo::new("main");
    init_pk(&repo);
    let _bare = setup_bare_remote(&repo);

    create_stack(&repo, &["feature/base"]);
    let mock = install_mock_gh(repo.path());

    pk_cmd()
        .args(["submit", "--draft"])
        .env("PANCAKE_GH_BIN", &mock.script)
        .current_dir(repo.path())
        .assert()
        .success();

    let log = fs::read_to_string(&mock.log).expect("read mock gh log");
    assert!(
        log.contains("pr create --head feature/base --base main") && log.contains("--draft"),
        "expected --draft in pr create:\n{log}"
    );
}

// -- helpers ----------------------------------------------------------------

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

fn create_stack(repo: &TestRepo, branches: &[&str]) {
    for branch in branches {
        pk_cmd()
            .args(["bc", branch])
            .current_dir(repo.path())
            .assert()
            .success();
        let filename = format!("{}.txt", branch.replace('/', "_"));
        fs::write(repo.path().join(&filename), branch).expect("write branch file");
        run_git(repo.path(), &["add", &filename]);
        run_git(repo.path(), &["commit", "-m", &format!("{} commit", branch)]);
    }
}

fn setup_bare_remote(repo: &TestRepo) -> TempDir {
    let bare = TempDir::new().expect("bare remote");
    let bare_path = bare.path().to_string_lossy().into_owned();
    let status = StdCommand::new("git")
        .args(["init", "--bare", &bare_path])
        .status()
        .expect("init bare");
    assert!(status.success(), "bare git init failed");
    run_git(repo.path(), &["remote", "add", "origin", &bare_path]);
    run_git(repo.path(), &["push", "-u", "origin", "main"]);
    bare
}

struct MockGh {
    script: std::path::PathBuf,
    log: std::path::PathBuf,
}

#[cfg(unix)]
fn install_mock_gh(repo: &Path) -> MockGh {
    use std::os::unix::fs::PermissionsExt;

    let script_path = repo.join("mock-gh.sh");
    let log_path = repo.join("gh-calls.log");
    let mode_marker = repo.join(".gh-mode");
    fs::write(&log_path, "").expect("init mock gh log");

    let script = format!(
        r#"#!/usr/bin/env bash
set -e
LOG_FILE="{log}"
MODE_FILE="{mode_marker}"
MODE="new"
if [ -f "$MODE_FILE" ]; then
  MODE="$(cat "$MODE_FILE")"
fi
echo "$@" >> "$LOG_FILE"

if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  if [ "$MODE" = "existing" ]; then
    BRANCH="$3"
    echo "{{\"number\":200,\"url\":\"https://github.com/example/repo/pull/200\"}}"
    exit 0
  else
    echo "no pull requests found for branch $3" >&2
    exit 1
  fi
fi

if [ "$1" = "pr" ] && [ "$2" = "pr" ]; then
  exit 0
fi

if [ "$1" = "pr" ] && [ "$2" = "create" ]; then
  HEAD_BRANCH=""
  while [ $# -gt 0 ]; do
    if [ "$1" = "--head" ]; then HEAD_BRANCH="$2"; fi
    shift || true
  done
  case "$HEAD_BRANCH" in
    feature/base) NUM=100 ;;
    feature/top)  NUM=101 ;;
    feature/mid)  NUM=102 ;;
    *)            NUM=999 ;;
  esac
  echo "https://github.com/example/repo/pull/$NUM"
  exit 0
fi

if [ "$1" = "pr" ] && [ "$2" = "edit" ]; then
  exit 0
fi

exit 0
"#,
        log = log_path.display(),
        mode_marker = mode_marker.display(),
    );
    fs::write(&script_path, script).expect("write mock gh");
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    MockGh {
        script: script_path,
        log: log_path,
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
