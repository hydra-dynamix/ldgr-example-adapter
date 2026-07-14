use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Output};

use ldgr::adapter_manifest::{parse_adapter_manifest, AdapterManifest};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_ldgr-example-adapter")
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ldgr-example-adapter-cli-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(binary());
    command.args(args).env_remove("LDGR_HOME");
    strip_entitlement_context(&mut command);
    command.output().expect("run ldgr-example-adapter")
}

fn run_with_env(args: &[&str], envs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(binary());
    command.args(args).env_remove("LDGR_HOME");
    for (key, value) in envs {
        command.env(key, value);
    }
    strip_entitlement_context(&mut command);
    command.output().expect("run ldgr-example-adapter")
}

fn strip_entitlement_context(command: &mut Command) {
    for key in [
        "LDGR_LICENSE",
        "LDGR_LICENSE_FILE",
        "LDGR_LICENSE_PATH",
        "LDGR_ENTITLEMENT",
        "LDGR_ENTITLEMENT_FILE",
        "LDGR_ENTITLEMENT_PATH",
        "LDGR_CUSTOMER_ID",
        "LDGR_PRODUCT",
        "LDGR_PRODUCT_FAMILY",
        "LDGR_SUBSCRIPTION",
    ] {
        command.env_remove(key);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn help_lists_adapter_command_extension() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("manifest-summary"));
    assert!(text.contains("adapter install"));
    assert!(text.contains("separate from core `ldgr` commands"));
}

#[test]
fn manifest_summary_reports_real_bundled_manifest() {
    let output = run(&["manifest-summary"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("adapter=example title=LDGR Example adapter"));
    assert!(text.contains("tools=1"));
    assert!(text.contains("example-manifest-summary :: ldgr-example-adapter manifest-summary"));
    assert!(text.contains("commands=1"));
    assert!(text.contains("example aliases=ldgr-example,reference :: ldgr-example-adapter"));
    assert!(text.contains("usage=ldgr example <command> [options]"));
    assert!(text.contains("target_profiles=1"));
    assert!(text.contains("example-adapter-lifecycle"));
}

#[test]
fn adapter_install_writes_discoverable_bundle() {
    let dir = fixture_dir("materialize");
    let adapter_root = dir.join("adapters");
    let install = adapter_root.join("example");
    let output = run_with_env(
        &[
            "adapter",
            "install",
            "--adapter-root",
            adapter_root.to_str().unwrap(),
        ],
        &[("HOME", &dir)],
    );
    assert!(output.status.success());
    assert!(install.join("adapter.toml").is_file());
    assert!(install.join("prompts/ldgr-loop-next-work.md").is_file());
    assert!(dir.join(".ldgr/prompts/ldgr-loop-next-work.md").is_file());
    assert!(install.join("templates/milestones.md").is_file());
    assert!(install.join("templates/example-spec.md").is_file());
    assert!(install.join("adapter-resources.json").is_file());
    assert!(install.join("skills/ldgr-example/SKILL.md").is_file());
    assert!(install.join("extensions/ldgr-example.ts").is_file());
    assert!(install.join("commands/ldgr-example.md").is_file());
    let manifest = fs::read_to_string(install.join("adapter.toml")).expect("manifest");
    assert!(manifest.contains("slug = \"example\""));
    assert!(manifest.contains("name = \"example-manifest-summary\""));
    assert!(manifest.contains("argv = [\"ldgr-example-adapter\", \"manifest-summary\"]"));
}

#[test]
fn open_adapter_install_and_commands_do_not_require_entitlement_context() {
    let dir = fixture_dir("unrestricted");
    let install = dir.join("example");
    let output = run_with_env(
        &[
            "adapter",
            "install",
            "--install-root",
            install.to_str().unwrap(),
        ],
        &[("HOME", &dir)],
    );
    assert!(output.status.success());

    let summary = run(&["manifest-summary"]);
    assert!(summary.status.success());
    assert!(stdout(&summary).contains("adapter=example"));

    let manifest = fs::read_to_string(install.join("adapter.toml")).expect("manifest");
    let manifest_lower = manifest.to_ascii_lowercase();
    for forbidden in [
        "license_public_key",
        "entitlement_claim",
        "entitlement_schema",
        "product_version_family",
        "version_family_enforcement",
    ] {
        assert!(
            !manifest_lower.contains(forbidden),
            "example manifest contains entitlement enforcement marker {forbidden}"
        );
    }
}

#[test]
fn profile_discover_finds_installed_example_adapter() {
    let dir = fixture_dir("discover");
    let adapter_root = dir.join("adapters");
    let install = adapter_root.join("example");
    let install_output = run_with_env(
        &[
            "adapter",
            "install",
            "--adapter-root",
            adapter_root.to_str().unwrap(),
        ],
        &[("HOME", &dir)],
    );
    assert!(install_output.status.success());
    assert!(install.join("adapter.toml").is_file());

    let mut command = Command::new(binary());
    strip_entitlement_context(&mut command);
    let output = command
        .args(["profile", "discover"])
        .env("LDGR_ADAPTER_PATH", adapter_root.to_str().unwrap())
        .env("HOME", &dir)
        .env_remove("LDGR_HOME")
        .output()
        .expect("run profile discover");
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("adapter=example"), "{text}");
    assert!(text.contains("aliases=ldgr-example,reference"), "{text}");
    assert!(
        text.contains("apply=\"ldgr-example-adapter profile apply\""),
        "{text}"
    );
}

#[test]
fn adapter_install_routes_prompts_through_harness_config() {
    let dir = fixture_dir("harness-config");
    let adapter_root = dir.join("adapters");
    let codex_prompts = dir.join(".codex/prompts");
    fs::create_dir_all(dir.join(".ldgr")).expect("create config dir");
    fs::write(
        dir.join(".ldgr/config.json"),
        format!(
            "{{\"harness\":\"codex\",\"prompt_paths\":[\"{}\"]}}",
            codex_prompts.display()
        ),
    )
    .expect("write config");

    let output = run_with_env(
        &[
            "adapter",
            "install",
            "--adapter-root",
            adapter_root.to_str().unwrap(),
        ],
        &[("HOME", &dir)],
    );

    assert!(output.status.success());
    assert!(codex_prompts.join("ldgr-loop-next-work.md").is_file());
    assert!(!dir.join(".ldgr/prompts/ldgr-loop-next-work.md").exists());
}

#[test]
fn profile_apply_activates_prompt_through_core_store() {
    let dir = fixture_dir("apply");
    let install_root = dir.join("adapter");
    let db = dir.join("ldgr.db");
    let artifact_root = dir.join("artifacts");
    let output = run(&[
        "profile",
        "apply",
        "--install-root",
        install_root.to_str().unwrap(),
        "--ldgr-db",
        db.to_str().unwrap(),
        "--ldgr-artifact-root",
        artifact_root.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = stdout(&output);
    assert!(text.contains("installed LDGR adapter `example`"), "{text}");
    assert!(
        text.contains("applied LDGR example adapter profile prompt=example-loop"),
        "{text}"
    );

    let connection = ldgr::store::open_store(&db).expect("open core store");
    let schema_version: i64 = connection
        .query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .expect("read core schema version");
    assert_eq!(
        schema_version,
        ldgr::store::CURRENT_SCHEMA_VERSION,
        "adapter must initialize the active Core schema"
    );
    let prompt = ldgr::store::active_prompt(&connection, "example-loop").expect("active prompt");
    assert_eq!(prompt.role, "example-adapter-loop");
    assert_eq!(prompt.status, "active");
    assert_eq!(
        prompt.source_path.as_deref(),
        Some(
            install_root
                .join("prompts/ldgr-loop-next-work.md")
                .to_str()
                .unwrap()
        )
    );
}

#[test]
fn open_adapter_fixtures_cover_public_api_scenarios() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/open-adapter");

    let valid_manifest_path = root.join("valid-manifest/adapter.toml");
    let valid_manifest =
        fs::read_to_string(&valid_manifest_path).expect("read valid fixture manifest");
    let valid = parse_adapter_manifest(&valid_manifest).expect("valid fixture parses");
    assert_eq!(valid.adapter.slug, "community-sample");
    assert_eq!(
        valid.profile.loop_prompt_path,
        "prompts/ldgr-loop-next-work.md"
    );
    assert_eq!(valid.tools.len(), 1);
    assert_eq!(valid.commands.len(), 1);
    assert_eq!(valid.commands[0].namespace, "community-sample");
    assert_eq!(valid.commands[0].argv, vec!["community-sample"]);
    assert_eq!(valid.commands[0].aliases, vec!["sample", "community"]);
    assert_eq!(
        valid.commands[0].help.usage,
        "ldgr community-sample <command> [options]"
    );
    assert_eq!(valid.target_profiles.len(), 1);
    assert_eq!(valid.target_profiles[0].probes.len(), 1);
    assert_referenced_files_exist(valid_manifest_path.parent().unwrap(), &valid);

    let malformed_manifest = fs::read_to_string(root.join("malformed-manifest/adapter.toml"))
        .expect("malformed fixture");
    assert!(
        parse_adapter_manifest(&malformed_manifest).is_err(),
        "malformed fixture should fail TOML parsing"
    );

    let expected_files = fs::read_to_string(root.join("bundle-materialization/expected-files.txt"))
        .expect("expected materialized file list");
    for relative in expected_files.lines().filter(|line| !line.is_empty()) {
        assert!(
            root.join("valid-manifest").join(relative).is_file(),
            "valid fixture missing expected materialized file {relative}"
        );
    }

    let discover_valid_path = root.join("profile-discover/adapters/community-sample/adapter.toml");
    let discover_valid =
        fs::read_to_string(&discover_valid_path).expect("read discovery fixture manifest");
    let discover =
        parse_adapter_manifest(&discover_valid).expect("discovery fixture valid manifest parses");
    assert_eq!(discover.adapter.aliases, vec!["sample", "community"]);
    assert_referenced_files_exist(discover_valid_path.parent().unwrap(), &discover);

    let discover_broken =
        fs::read_to_string(root.join("profile-discover/adapters/broken/adapter.toml"))
            .expect("read broken discovery fixture");
    assert!(
        parse_adapter_manifest(&discover_broken).is_err(),
        "broken discovery fixture should fail TOML parsing"
    );

    let apply_manifest_path = root.join("profile-apply/community-sample/adapter.toml");
    let apply_manifest =
        fs::read_to_string(&apply_manifest_path).expect("read profile apply fixture manifest");
    let apply =
        parse_adapter_manifest(&apply_manifest).expect("profile apply fixture manifest parses");
    assert_eq!(apply.adapter.slug, "community-sample");
    assert_referenced_files_exist(apply_manifest_path.parent().unwrap(), &apply);
}

fn assert_referenced_files_exist(manifest_dir: &Path, manifest: &AdapterManifest) {
    assert!(manifest_dir
        .join(&manifest.profile.loop_prompt_path)
        .is_file());
    assert!(manifest_dir
        .join(&manifest.profile.default_milestone_template)
        .is_file());
    assert!(manifest_dir
        .join(&manifest.profile.spec_artifact_path)
        .is_file());
}
