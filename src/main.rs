use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ldgr::manifest_integrity::verify_manifest_digest;
use ldgr::store::{create_prompt, get_prompt, init_store, set_prompt_status, update_prompt};
use serde::{Deserialize, Serialize};

const ADAPTER_TOML: &str = include_str!("../adapter.toml");
const LOOP_PROMPT: &str = include_str!("../prompts/ldgr-loop-next-work.md");
const MILESTONES: &str = include_str!("../templates/milestones.md");
const EXAMPLE_SPEC: &str = include_str!("../templates/example-spec.md");
const PROFILE_PROMPT_SLUG: &str = "example-loop";
const PROFILE_PROMPT_ROLE: &str = "example-adapter-loop";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match args.first().and_then(|arg| arg.to_str()) {
        None | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some("manifest-summary") => manifest_summary(&args[1..]),
        Some("adapter") => adapter_install(&args[1..]),
        Some("profile") => profile(&args[1..]),
        Some(command) => Err(format!(
            "unknown ldgr-example-adapter command `{command}`. Try --help."
        )),
    }
}

fn manifest_summary(args: &[OsString]) -> Result<(), String> {
    let mut json = false;
    for arg in args {
        match arg.to_str() {
            Some("--json") => json = true,
            Some("--help") | Some("-h") => {
                print_manifest_summary_help();
                return Ok(());
            }
            Some(flag) => return Err(format!("unknown manifest-summary option `{flag}`")),
            None => return Err("manifest-summary arguments must be valid UTF-8".to_string()),
        }
    }

    let manifest = parse_manifest()?;
    let summary = ManifestSummary::from_manifest(&manifest);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary)
                .map_err(|error| format!("failed to render summary JSON: {error}"))?
        );
    } else {
        println!("adapter={} title={}", summary.slug, summary.title);
        println!("core_version={}", summary.core_version);
        println!("aliases={}", summary.aliases.join(","));
        println!("loop_prompt={}", summary.loop_prompt_path);
        println!(
            "default_milestone_template={}",
            summary.default_milestone_template
        );
        println!("spec_artifact={}", summary.spec_artifact_path);
        println!("readiness_policy={}", summary.readiness_policy);
        println!("tools={}", summary.tools.len());
        for tool in &summary.tools {
            println!("- {} :: {}", tool.name, tool.argv.join(" "));
        }
        println!("target_profiles={}", summary.target_profiles.len());
        for profile in &summary.target_profiles {
            println!(
                "- {} :: {} ({}) probes={}",
                profile.slug,
                profile.title,
                profile.target_type,
                profile.probes.len()
            );
            println!("  description={}", profile.description);
            for probe in &profile.probes {
                println!("  - probe {} :: {}", probe.slug, probe.title);
                println!("    description={}", probe.description);
                if let Some(kind) = &probe.evidence_artifact_kind {
                    println!("    evidence_artifact_kind={kind}");
                }
                if let Some(template) = &probe.expectation_template {
                    println!("    expectation_template={template}");
                }
                if let Some(hint) = &probe.validation_hint {
                    println!("    validation_hint={hint}");
                }
            }
        }
    }
    Ok(())
}

fn profile(args: &[OsString]) -> Result<(), String> {
    let subcommand = args.first().and_then(|arg| arg.to_str()).ok_or_else(|| {
        "profile requires a subcommand: `ldgr-example-adapter profile discover` or `ldgr-example-adapter profile apply`".to_string()
    })?;
    match subcommand {
        "discover" => profile_discover(&args[1..]),
        "apply" => profile_apply(&args[1..]),
        _ => Err(format!(
            "unknown profile subcommand `{subcommand}`. Try `ldgr-example-adapter profile discover` or `ldgr-example-adapter profile apply`."
        )),
    }
}

fn profile_discover(args: &[OsString]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.to_str(), Some("--help") | Some("-h")))
    {
        print_profile_discover_help();
        return Ok(());
    }
    if let Some(flag) = args.first().and_then(|arg| arg.to_str()) {
        return Err(format!("unknown profile discover option `{flag}`"));
    }

    let manifests = discover_adapter_manifests()?;
    if manifests.is_empty() {
        println!("No adapter manifests discovered.");
        return Ok(());
    }
    for manifest in manifests {
        let aliases = if manifest.aliases.is_empty() {
            String::new()
        } else {
            format!(" aliases={}", manifest.aliases.join(","))
        };
        println!(
            "adapter={} title={} core_version={}{} manifest={} apply=\"ldgr-example-adapter profile apply\"",
            manifest.slug,
            manifest.title,
            manifest.core_version,
            aliases,
            manifest.manifest_path.display()
        );
    }
    Ok(())
}

fn profile_apply(args: &[OsString]) -> Result<(), String> {
    let mut install_root = default_adapter_root().join("example");
    let mut ldgr_db = env::var_os("LDGR_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".ldgr/ldgr.db"));
    let mut ldgr_artifact_root = env::var_os("LDGR_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".ldgr/artifacts"));

    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--install-root") => {
                install_root = next_path(args, index, "--install-root")?;
                index += 2;
            }
            Some("--ldgr-db") => {
                ldgr_db = next_path(args, index, "--ldgr-db")?;
                index += 2;
            }
            Some("--ldgr-artifact-root") => {
                ldgr_artifact_root = next_path(args, index, "--ldgr-artifact-root")?;
                index += 2;
            }
            Some("--help") | Some("-h") => {
                print_profile_apply_help();
                return Ok(());
            }
            Some(flag) => return Err(format!("unknown profile apply option `{flag}`")),
            None => return Err("profile apply arguments must be valid UTF-8".to_string()),
        }
    }

    let manifest_path = install_bundle(&install_root)?;
    init_store(&ldgr_db, &ldgr_artifact_root)
        .map_err(|error| format!("failed to initialize LDGR store: {error:#}"))?;
    let connection = ldgr::store::open_store(&ldgr_db)
        .map_err(|error| format!("failed to open LDGR store: {error:#}"))?;
    let prompt_path = install_root.join("prompts/ldgr-loop-next-work.md");
    let source_path = prompt_path.to_string_lossy();
    if get_prompt(&connection, PROFILE_PROMPT_SLUG)
        .map_err(|error| format!("failed to inspect existing prompt: {error:#}"))?
        .is_some()
    {
        update_prompt(
            &connection,
            PROFILE_PROMPT_SLUG,
            LOOP_PROMPT,
            Some(source_path.as_ref()),
            Some("Loop prompt installed by the LDGR example adapter."),
        )
        .map_err(|error| format!("failed to update example adapter prompt: {error:#}"))?;
    } else {
        create_prompt(
            &connection,
            PROFILE_PROMPT_SLUG,
            PROFILE_PROMPT_ROLE,
            LOOP_PROMPT,
            Some(source_path.as_ref()),
            Some("Loop prompt installed by the LDGR example adapter."),
        )
        .map_err(|error| format!("failed to create example adapter prompt: {error:#}"))?;
    }
    let prompt = set_prompt_status(&connection, PROFILE_PROMPT_SLUG, "active")
        .map_err(|error| format!("failed to activate example adapter prompt: {error:#}"))?;
    println!(
        "installed LDGR adapter `example`: {}",
        manifest_path.display()
    );
    println!(
        "applied LDGR example adapter profile prompt={} version={} status={}",
        prompt.slug, prompt.current_version, prompt.status
    );
    Ok(())
}

fn adapter_install(args: &[OsString]) -> Result<(), String> {
    let subcommand = args.first().and_then(|arg| arg.to_str()).ok_or_else(|| {
        "adapter requires a subcommand: `ldgr-example-adapter adapter install`".to_string()
    })?;
    if subcommand != "install" {
        return Err(format!(
            "unknown adapter subcommand `{subcommand}`. Try `ldgr-example-adapter adapter install`."
        ));
    }

    let mut install_root = default_adapter_root().join("example");
    let mut print_path = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].to_str() {
            Some("--adapter-root") => {
                install_root = next_path(args, index, "--adapter-root")?.join("example");
                index += 2;
            }
            Some("--install-root") => {
                install_root = next_path(args, index, "--install-root")?;
                index += 2;
            }
            Some("--print-path") => {
                print_path = true;
                index += 1;
            }
            Some("--help") | Some("-h") => {
                print_adapter_install_help();
                return Ok(());
            }
            Some(flag) => return Err(format!("unknown adapter install option `{flag}`")),
            None => return Err("adapter install arguments must be valid UTF-8".to_string()),
        }
    }

    let manifest_path = install_bundle(&install_root)?;
    if print_path {
        println!("{}", manifest_path.display());
    } else {
        println!(
            "installed LDGR adapter `example`: {}",
            manifest_path.display()
        );
        println!("next: `ldgr profile discover` then `ldgr profile apply example`");
    }
    Ok(())
}

fn install_bundle(install_root: &Path) -> Result<PathBuf, String> {
    write_parented(&install_root.join("adapter.toml"), ADAPTER_TOML)?;
    write_parented(
        &install_root.join("prompts/ldgr-loop-next-work.md"),
        LOOP_PROMPT,
    )?;
    write_parented(&install_root.join("templates/milestones.md"), MILESTONES)?;
    write_parented(
        &install_root.join("templates/example-spec.md"),
        EXAMPLE_SPEC,
    )?;
    Ok(install_root.join("adapter.toml"))
}

fn write_parented(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn next_path(args: &[OsString], index: usize, flag: &str) -> Result<PathBuf, String> {
    args.get(index + 1)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a path"))
}

fn default_adapter_root() -> PathBuf {
    env::var_os("LDGR_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".ldgr")
        })
        .join("adapters")
}

fn parse_manifest() -> Result<AdapterManifest, String> {
    toml::from_str(ADAPTER_TOML)
        .map_err(|error| format!("failed to parse bundled adapter.toml: {error}"))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterManifest {
    adapter: ManifestAdapter,
    profile: ManifestProfile,
    #[serde(default)]
    tools: Vec<ManifestTool>,
    #[serde(default)]
    target_profiles: Vec<ManifestTargetProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAdapter {
    slug: String,
    title: String,
    core_version: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug)]
struct DiscoveredAdapterManifest {
    slug: String,
    title: String,
    core_version: String,
    aliases: Vec<String>,
    manifest_path: PathBuf,
}

fn discover_adapter_manifests() -> Result<Vec<DiscoveredAdapterManifest>, String> {
    let mut discovered = Vec::new();
    for root in adapter_search_roots() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("failed to read {}: {error}", root.display()))?;
            let manifest_path = entry.path().join("adapter.toml");
            if !manifest_path.is_file() {
                continue;
            }
            let manifest_text = match fs::read_to_string(&manifest_path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!(
                        "warning: skipped adapter manifest {}: failed to read: {error}",
                        manifest_path.display()
                    );
                    continue;
                }
            };
            let manifest: AdapterManifest = match toml::from_str(&manifest_text) {
                Ok(manifest) => manifest,
                Err(error) => {
                    eprintln!(
                        "warning: skipped adapter manifest {}: failed to parse: {error}",
                        manifest_path.display()
                    );
                    continue;
                }
            };
            if let Err(error) = verify_manifest_digest(&manifest_text) {
                eprintln!(
                    "warning: skipped adapter manifest {}: failed to verify: {error}",
                    manifest_path.display()
                );
                continue;
            }
            discovered.push(DiscoveredAdapterManifest {
                slug: manifest.adapter.slug,
                title: manifest.adapter.title,
                core_version: manifest.adapter.core_version,
                aliases: manifest.adapter.aliases,
                manifest_path: manifest_path.canonicalize().unwrap_or(manifest_path),
            });
        }
    }
    discovered.sort_by(|left, right| left.slug.cmp(&right.slug));
    discovered.dedup_by(|left, right| left.slug == right.slug);
    Ok(discovered)
}

fn adapter_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(paths) = env::var_os("LDGR_ADAPTER_PATH") {
        roots.extend(env::split_paths(&paths));
    }
    if let Some(home) = env::var_os("LDGR_HOME") {
        roots.push(PathBuf::from(home).join("adapters"));
    }
    if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".ldgr/adapters"));
    }
    roots
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProfile {
    loop_prompt_path: String,
    default_milestone_template: String,
    spec_artifact_path: String,
    readiness_policy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestTool {
    name: String,
    argv: Vec<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestTargetProfile {
    slug: String,
    title: String,
    target_type: String,
    description: String,
    #[serde(default)]
    probes: Vec<ManifestProbeFamily>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProbeFamily {
    slug: String,
    title: String,
    description: String,
    evidence_artifact_kind: Option<String>,
    expectation_template: Option<String>,
    validation_hint: Option<String>,
}

#[derive(Debug, Serialize)]
struct ManifestSummary {
    slug: String,
    title: String,
    core_version: String,
    aliases: Vec<String>,
    loop_prompt_path: String,
    default_milestone_template: String,
    spec_artifact_path: String,
    readiness_policy: String,
    tools: Vec<ManifestTool>,
    target_profiles: Vec<TargetProfileSummary>,
}

#[derive(Debug, Serialize)]
struct TargetProfileSummary {
    slug: String,
    title: String,
    target_type: String,
    description: String,
    probes: Vec<ProbeFamilySummary>,
}

#[derive(Debug, Serialize)]
struct ProbeFamilySummary {
    slug: String,
    title: String,
    description: String,
    evidence_artifact_kind: Option<String>,
    expectation_template: Option<String>,
    validation_hint: Option<String>,
}

impl ManifestSummary {
    fn from_manifest(manifest: &AdapterManifest) -> Self {
        Self {
            slug: manifest.adapter.slug.clone(),
            title: manifest.adapter.title.clone(),
            core_version: manifest.adapter.core_version.clone(),
            aliases: manifest.adapter.aliases.clone(),
            loop_prompt_path: manifest.profile.loop_prompt_path.clone(),
            default_milestone_template: manifest.profile.default_milestone_template.clone(),
            spec_artifact_path: manifest.profile.spec_artifact_path.clone(),
            readiness_policy: manifest.profile.readiness_policy.clone(),
            tools: manifest.tools.clone(),
            target_profiles: manifest
                .target_profiles
                .iter()
                .map(|profile| TargetProfileSummary {
                    slug: profile.slug.clone(),
                    title: profile.title.clone(),
                    target_type: profile.target_type.clone(),
                    description: profile.description.clone(),
                    probes: profile
                        .probes
                        .iter()
                        .map(|probe| ProbeFamilySummary {
                            slug: probe.slug.clone(),
                            title: probe.title.clone(),
                            description: probe.description.clone(),
                            evidence_artifact_kind: probe.evidence_artifact_kind.clone(),
                            expectation_template: probe.expectation_template.clone(),
                            validation_hint: probe.validation_hint.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

fn print_help() {
    println!(
        "ldgr-example-adapter\n\nUsage:\n  ldgr-example-adapter manifest-summary [--json]\n  ldgr-example-adapter adapter install [OPTIONS]\n  ldgr-example-adapter profile discover [OPTIONS]\n  ldgr-example-adapter profile apply [OPTIONS]\n\nCommands:\n  manifest-summary  Summarize the bundled reference adapter manifest and command extension.\n  adapter install   Install the bundled LDGR example adapter.\n  profile discover  List installed LDGR adapter manifests.\n  profile apply     Install the bundle and activate its loop prompt through ldgr-core.\n\nThe adapter-owned command surface is intentionally separate from core `ldgr` commands."
    );
}

fn print_manifest_summary_help() {
    println!(
        "ldgr-example-adapter manifest-summary\n\nOptions:\n      --json  Emit machine-readable JSON\n  -h, --help  Print help"
    );
}

fn print_adapter_install_help() {
    println!(
        "ldgr-example-adapter adapter install\n\nOptions:\n      --adapter-root <PATH>  Adapter root; installs an example/ child [default: LDGR_HOME/adapters or ~/.ldgr/adapters]\n      --install-root <PATH>  Exact install directory for the example adapter bundle\n      --print-path           Print the installed adapter.toml path\n  -h, --help                 Print help"
    );
}

fn print_profile_discover_help() {
    println!(
        "ldgr-example-adapter profile discover\n\nSearches LDGR_ADAPTER_PATH, LDGR_HOME/adapters, and ~/.ldgr/adapters for <slug>/adapter.toml manifests.\n\nOptions:\n  -h, --help  Print help"
    );
}

fn print_profile_apply_help() {
    println!(
        "ldgr-example-adapter profile apply\n\nOptions:\n      --install-root <PATH>       Where to copy the bundled adapter files [default: LDGR_HOME/adapters/example or ~/.ldgr/adapters/example]\n      --ldgr-db <PATH>            LDGR database path [default: LDGR_DB or .ldgr/ldgr.db]\n      --ldgr-artifact-root <PATH> LDGR artifact root [default: LDGR_ARTIFACT_ROOT or .ldgr/artifacts]\n  -h, --help                      Print help"
    );
}
