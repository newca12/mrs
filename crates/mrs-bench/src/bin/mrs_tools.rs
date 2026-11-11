use std::io::Read;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use mrs_bench::tooling::{bench, corpus, formal, prephase};

#[derive(Parser)]
#[command(
    name = "mrs-tools",
    bin_name = "mrs-tools",
    about = "Rust tools for mrs experiments and verification"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an immutable corpus manifest and family-disjoint split.
    FreezeCorpus(corpus::FreezeArgs),
    /// Run randomized, strictly audited comparisons of frozen builds.
    PairedCampaign(bench::PairedCampaignArgs),
    /// Compare frozen builds at identical selected-clause work.
    FixedWorkCompare(bench::FixedWorkArgs),
    /// Audit the CASC canaries for problem/include drift.
    CheckCanaryDrift(bench::CanaryDriftArgs),
    /// Render identity-bound audit snapshots as Markdown.
    CertificationDashboard(bench::DashboardArgs),
    /// Summarize a strict audit CSV (used by certification_campaign.sh).
    SummarizeCertification(bench::CertificationSummaryArgs),
    /// Count strictly certified refutations in an audit CSV.
    CountVerified(bench::CountVerifiedArgs),
    /// Add the resolved prover binary digest to run_meta.json.
    RecordBinaryHash(bench::RecordBinaryHashArgs),
    /// Read the prover binary digest from run_meta.json.
    ReadBinaryHash(bench::ReadBinaryHashArgs),
    /// Join EPR pre-pass telemetry from archived stderr into a CSV.
    EprTelemetry(bench::EprTelemetryArgs),
    /// Summarize the two run.csv files produced by prephase/ab.sh.
    PrephaseAbSummary(bench::PrephaseAbArgs),
    /// Compare replicate run.csv files and report per-problem spread.
    VarianceReport(bench::VarianceArgs),
    /// Count SZS and grading verdicts in a casc.sh run.csv.
    RunCsvSummary(bench::RunCsvSummaryArgs),
    /// Count unsolved CASC problems by family in a run.csv.
    FailureProfile(bench::FailureProfileArgs),
    /// List rows with an empty CSV field, printing another selected column.
    ListMissing(bench::ListMissingArgs),
    /// Reproduce the legacy raw-status overlap query against codex.db.
    CodexOverlap(bench::CodexOverlapArgs),
    /// Analyze the pre-phase feature and strategy-outcome CSVs.
    AnalyzePrephase(prephase::AnalyzeArgs),
    /// Compute exact fixed-width unions from archived solo sweep CSVs.
    ExactSoloBounds(prephase::ExactSoloBoundsArgs),
    /// Repair a legacy prephase_dump --probe CSV.
    RepairProbeColumns(prephase::RepairProbeArgs),
    /// Repair a legacy isolated prephase_dump CSV.
    RepairIsolatedDump(prephase::RepairIsolatedArgs),
    /// Audit Verus sources for prohibited proof escape hatches.
    AuditVerusSource(formal::AuditSourceArgs),
    /// Compile the verified executable twice and exercise its wire protocol.
    VerusRuntimeCheck(formal::RuntimeCheckArgs),
    /// Run verifier-negative and compiled-runtime mutation checks.
    VerusMutationCheck(formal::MutationCheckArgs),
    /// Write provenance JSON beside a compiled Verus runtime binary.
    VerusRuntimeEvidence(formal::RuntimeEvidenceArgs),
    /// Require a Verus executable to match the pinned version and toolchain.
    VerusCheckVersion(formal::VerusCheckVersionArgs),
    /// Read one field from the pinned Verus toolchain manifest.
    VerusPinField(VerusPinFieldArgs),
    /// Extract Verus's effective Rust toolchain from its version output.
    VerusEffectiveToolchain,
    /// Validate a JSON document without producing reformatted output.
    ValidateJson(ValidateJsonArgs),
}

#[derive(Args)]
struct VerusPinFieldArgs {
    manifest: PathBuf,
    field: String,
}

#[derive(Args)]
struct ValidateJsonArgs {
    path: PathBuf,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::FreezeCorpus(args) => mrs_bench::tooling::corpus::freeze_command(args),
        Command::PairedCampaign(args) => mrs_bench::tooling::bench::paired_campaign(args),
        Command::FixedWorkCompare(args) => mrs_bench::tooling::bench::fixed_work_compare(args),
        Command::CheckCanaryDrift(args) => mrs_bench::tooling::bench::check_canary_drift(args),
        Command::CertificationDashboard(args) => {
            mrs_bench::tooling::bench::certification_dashboard(args)
        }
        Command::SummarizeCertification(args) => {
            mrs_bench::tooling::bench::summarize_certification(args)
        }
        Command::CountVerified(args) => mrs_bench::tooling::bench::count_verified(args),
        Command::RecordBinaryHash(args) => mrs_bench::tooling::bench::record_binary_hash(args),
        Command::ReadBinaryHash(args) => mrs_bench::tooling::bench::read_binary_hash(args),
        Command::EprTelemetry(args) => mrs_bench::tooling::bench::epr_telemetry(args),
        Command::PrephaseAbSummary(args) => mrs_bench::tooling::bench::prephase_ab_summary(args),
        Command::VarianceReport(args) => mrs_bench::tooling::bench::variance_report(args),
        Command::RunCsvSummary(args) => mrs_bench::tooling::bench::run_csv_summary(args),
        Command::FailureProfile(args) => mrs_bench::tooling::bench::failure_profile(args),
        Command::ListMissing(args) => mrs_bench::tooling::bench::list_missing(args),
        Command::CodexOverlap(args) => mrs_bench::tooling::bench::codex_overlap(args),
        Command::AnalyzePrephase(args) => mrs_bench::tooling::prephase::analyze(args),
        Command::ExactSoloBounds(args) => mrs_bench::tooling::prephase::exact_solo_bounds(args),
        Command::RepairProbeColumns(args) => {
            mrs_bench::tooling::prephase::repair_probe_columns(args)
        }
        Command::RepairIsolatedDump(args) => {
            mrs_bench::tooling::prephase::repair_isolated_dump(args)
        }
        Command::AuditVerusSource(args) => mrs_bench::tooling::formal::audit_source(args),
        Command::VerusRuntimeCheck(args) => mrs_bench::tooling::formal::runtime_check(args),
        Command::VerusMutationCheck(args) => mrs_bench::tooling::formal::mutation_check(args),
        Command::VerusRuntimeEvidence(args) => {
            mrs_bench::tooling::formal::write_runtime_evidence(args)
        }
        Command::VerusCheckVersion(args) => mrs_bench::tooling::formal::check_verus_version(args),
        Command::VerusPinField(args) => {
            println!(
                "{}",
                mrs_bench::tooling::formal::pin_field(&args.manifest, &args.field)?
            );
            Ok(())
        }
        Command::VerusEffectiveToolchain => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            println!(
                "{}",
                mrs_bench::tooling::formal::effective_toolchain(&input)?
            );
            Ok(())
        }
        Command::ValidateJson(args) => mrs_bench::tooling::formal::validate_json(&args.path),
    }
}

fn main() {
    if let Err(error) = run() {
        let exit_code = if error
            .downcast_ref::<mrs_bench::tooling::bench::CanaryContaminated>()
            .is_some()
        {
            2
        } else {
            1
        };
        eprintln!("mrs-tools: {error}");
        std::process::exit(exit_code);
    }
}
