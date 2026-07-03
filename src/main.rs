use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ldgr::adapter_command::{
    default_adapter_root, parse_adapter_install_command, AdapterInstallCommand,
};
use ldgr::adapter_manifest::{
    parse_adapter_manifest, AdapterManifest, ManifestCommandNamespace, ManifestTool,
};
use ldgr::adapter_profile::{apply_adapter_profile_prompt, AdapterProfileApplyOptions};
use ldgr::adapter_registry::AdapterRegistry;
use serde::Serialize;

const ADAPTER_TOML: &str = include_str!("../adapter.toml");
const LOOP_PROMPT: &str = include_str!("../prompts/ldgr-loop-next-work.md");
const MILESTONES: &str = include_str!("../templates/milestones.md");
const EXAMPLE_SPEC: &str = include_str!("../templates/example-spec.md");
const PROFILE_PROMPT_SLUG: &str = "example-loop";
const PROFILE_PROMPT_ROLE: &str = "example-adapter-loop";
const ADAPTER_INSTALL_DIR: &str = "example";

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
        println!("commands={}", summary.commands.len());
        for command in &summary.commands {
            let aliases = if command.aliases.is_empty() {
                String::new()
            } else {
                format!(" aliases={}", command.aliases.join(","))
            };
            println!(
                "- {}{} :: {}",
                command.namespace,
                aliases,
                command.argv.join(" ")
            );
            println!("  title={}", command.title);
            println!("  description={}", command.description);
            println!("  usage={}", command.help.usage);
            println!("  summary={}", command.help.summary);
            if let Some(details) = &command.help.details {
                println!("  details={details}");
            }
            if !command.capabilities.is_empty() {
                println!("  capabilities={}", command.capabilities.join(","));
            }
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
    let mut install_root = default_adapter_root().join(ADAPTER_INSTALL_DIR);
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
    let application = apply_adapter_profile_prompt(AdapterProfileApplyOptions {
        manifest_path: &manifest_path,
        db_path: &ldgr_db,
        artifact_root: &ldgr_artifact_root,
        prompt_slug: PROFILE_PROMPT_SLUG,
        prompt_role: PROFILE_PROMPT_ROLE,
        description: Some("Loop prompt installed by the LDGR example adapter."),
    })
    .map_err(|error| format!("failed to apply example adapter profile: {error:#}"))?;
    let prompt = application.prompt;
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

    let options =
        match parse_adapter_install_command(args[1..].iter().cloned(), ADAPTER_INSTALL_DIR)? {
            AdapterInstallCommand::Help => {
                print_adapter_install_help();
                return Ok(());
            }
            AdapterInstallCommand::Install(options) => options,
        };

    let manifest_path = install_bundle(&options.install_root)?;
    if options.print_path {
        println!("{}", manifest_path.display());
    } else {
        println!(
            "installed LDGR adapter `example`: {}",
            manifest_path.display()
        );
        println!("next: `ldgr-example-adapter profile discover` then `ldgr-example-adapter profile apply example`");
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

fn parse_manifest() -> Result<AdapterManifest, String> {
    parse_adapter_manifest(ADAPTER_TOML)
        .map_err(|error| format!("failed to parse bundled adapter.toml: {error:#}"))
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
    let registry = AdapterRegistry::discover();
    for warning in &registry.warnings {
        eprintln!(
            "warning: skipped adapter manifest {}: {}",
            warning.manifest_path.display(),
            warning.message
        );
    }

    Ok(registry
        .adapters
        .into_iter()
        .map(|adapter| DiscoveredAdapterManifest {
            slug: adapter.slug,
            title: adapter.title,
            core_version: adapter.core_version,
            aliases: adapter.aliases,
            manifest_path: adapter.manifest_path,
        })
        .collect())
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
    commands: Vec<ManifestCommandNamespace>,
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
            commands: manifest.commands.clone(),
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
        "ldgr-example-adapter adapter install\n\nOptions:\n      --adapter-root <PATH>  Adapter root; installs an example/ child [default: LDGR_HOME or ~/.ldgr]\n      --install-root <PATH>  Exact install directory for the example adapter bundle\n      --print-path           Print the installed adapter.toml path\n  -h, --help                 Print help"
    );
}

fn print_profile_discover_help() {
    println!(
        "ldgr-example-adapter profile discover\n\nSearches LDGR_ADAPTER_PATH, .ldgr, LDGR_HOME, LDGR_HOME, and ~/.ldgr for .<slug>/adapter.toml manifests.\n\nOptions:\n  -h, --help  Print help"
    );
}

fn print_profile_apply_help() {
    println!(
        "ldgr-example-adapter profile apply\n\nOptions:\n      --install-root <PATH>       Where to copy the bundled adapter files [default: LDGR_HOME/example or ~/.ldgr/example]\n      --ldgr-db <PATH>            LDGR database path [default: LDGR_DB or .ldgr/ldgr.db]\n      --ldgr-artifact-root <PATH> LDGR artifact root [default: LDGR_ARTIFACT_ROOT or .ldgr/artifacts]\n  -h, --help                      Print help"
    );
}
