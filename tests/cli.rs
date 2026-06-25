use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Output};

use serde::Deserialize;

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
    assert!(text.contains("target_profiles=1"));
    assert!(text.contains("example-adapter-lifecycle"));
}

#[test]
fn adapter_install_writes_discoverable_bundle() {
    let dir = fixture_dir("materialize");
    let adapter_root = dir.join("adapters");
    let install = adapter_root.join("example");
    let output = run(&[
        "adapter",
        "install",
        "--adapter-root",
        adapter_root.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(install.join("adapter.toml").is_file());
    assert!(install.join("prompts/ldgr-loop-next-work.md").is_file());
    assert!(install.join("templates/milestones.md").is_file());
    assert!(install.join("templates/example-spec.md").is_file());
    let manifest = fs::read_to_string(install.join("adapter.toml")).expect("manifest");
    assert!(manifest.contains("slug = \"example\""));
    assert!(manifest.contains("name = \"example-manifest-summary\""));
    assert!(manifest.contains("argv = [\"ldgr-example-adapter\", \"manifest-summary\"]"));
}

#[test]
fn open_adapter_install_and_commands_do_not_require_entitlement_context() {
    let dir = fixture_dir("unrestricted");
    let install = dir.join("example");
    let output = run(&[
        "adapter",
        "install",
        "--install-root",
        install.to_str().unwrap(),
    ]);
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
    let install_output = run(&[
        "adapter",
        "install",
        "--adapter-root",
        adapter_root.to_str().unwrap(),
    ]);
    assert!(install_output.status.success());
    assert!(install.join("adapter.toml").is_file());

    let mut command = Command::new(binary());
    strip_entitlement_context(&mut command);
    let output = command
        .args(["profile", "discover"])
        .env("LDGR_ADAPTER_PATH", adapter_root.to_str().unwrap())
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
    let valid: FixtureManifest = toml::from_str(&valid_manifest).expect("valid fixture parses");
    assert_eq!(valid.adapter.slug, "community-sample");
    assert_eq!(
        valid.profile.loop_prompt_path,
        "prompts/ldgr-loop-next-work.md"
    );
    assert_eq!(valid.tools.len(), 1);
    assert_eq!(valid.target_profiles.len(), 1);
    assert_eq!(valid.target_profiles[0].probes.len(), 1);
    assert_referenced_files_exist(valid_manifest_path.parent().unwrap(), &valid);

    let malformed_manifest = fs::read_to_string(root.join("malformed-manifest/adapter.toml"))
        .expect("malformed fixture");
    assert!(
        toml::from_str::<FixtureManifest>(&malformed_manifest).is_err(),
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
    let discover: FixtureManifest =
        toml::from_str(&discover_valid).expect("discovery fixture valid manifest parses");
    assert_eq!(discover.adapter.aliases, vec!["sample", "community"]);
    assert_referenced_files_exist(discover_valid_path.parent().unwrap(), &discover);

    let discover_broken =
        fs::read_to_string(root.join("profile-discover/adapters/broken/adapter.toml"))
            .expect("read broken discovery fixture");
    assert!(
        toml::from_str::<FixtureManifest>(&discover_broken).is_err(),
        "broken discovery fixture should fail TOML parsing"
    );

    let apply_manifest_path = root.join("profile-apply/community-sample/adapter.toml");
    let apply_manifest =
        fs::read_to_string(&apply_manifest_path).expect("read profile apply fixture manifest");
    let apply: FixtureManifest =
        toml::from_str(&apply_manifest).expect("profile apply fixture manifest parses");
    assert_eq!(apply.adapter.slug, "community-sample");
    assert_referenced_files_exist(apply_manifest_path.parent().unwrap(), &apply);
}

fn assert_referenced_files_exist(manifest_dir: &Path, manifest: &FixtureManifest) {
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

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    adapter: FixtureAdapter,
    profile: FixtureProfile,
    #[serde(default)]
    tools: Vec<FixtureTool>,
    #[serde(default)]
    target_profiles: Vec<FixtureTargetProfile>,
}

#[derive(Debug, Deserialize)]
struct FixtureAdapter {
    slug: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    core_version: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureProfile {
    loop_prompt_path: String,
    default_milestone_template: String,
    spec_artifact_path: String,
    #[allow(dead_code)]
    readiness_policy: String,
}

#[derive(Debug, Deserialize)]
struct FixtureTool {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    argv: Vec<String>,
    #[allow(dead_code)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureTargetProfile {
    #[allow(dead_code)]
    slug: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    target_type: String,
    #[allow(dead_code)]
    description: String,
    #[serde(default)]
    probes: Vec<FixtureProbe>,
}

#[derive(Debug, Deserialize)]
struct FixtureProbe {
    #[allow(dead_code)]
    slug: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    evidence_artifact_kind: Option<String>,
    #[allow(dead_code)]
    expectation_template: Option<String>,
    #[allow(dead_code)]
    validation_hint: Option<String>,
}
