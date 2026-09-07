//! Offline edge-bundle export. The SAME service the HTTP route uses, for when
//! the backend's port is not reachable from where the export runs (a Fastly
//! vendoring step on a laptop, a CI job with only a database URL).
//!
//!   cargo run --manifest-path backend/Cargo.toml --bin export_bundle -- \
//!       --site intrafish-com --env live --out edge/bundle.json
//!
//! Reads `DATABASE_URL` from the environment (or a `.env` beside the working
//! directory). It is CWD-independent on purpose, so it can be driven from the
//! repo root with `--manifest-path`.
//!
//! **stdout is the bundle and nothing else.** With `--out` the bundle goes to
//! that file and stdout stays empty; without it the bundle goes to stdout so the
//! command can be piped. Diagnostics — including the exporter's
//! unresolvable-reference warnings — always go to stderr.
//!
//! Every failure (unknown site, unreadable `DATABASE_URL`, DB error, unwritable
//! path) exits non-zero with a message on stderr. The destination file is
//! written via a temp file and an atomic rename, so an operator can never get
//! exit 0 alongside a truncated bundle, and a failed run never leaves a
//! half-written file where the previous good one was.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use rre_backend::{db, schemas::version::PublishEnvironment, services};

/// The database URL, from the environment (or a `.env` beside the working
/// directory).
///
/// Deliberately NOT `Settings::load()`: that resolves `config/default` relative
/// to the CWD, so it only works when the command runs from `backend/`. This
/// binary is invoked from the repo root with `--manifest-path`, and the export
/// needs exactly one setting, so it reads that one directly and stays
/// CWD-independent.
fn database_url() -> anyhow::Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL").map_err(|_| {
        anyhow!("DATABASE_URL is not set (export it, or run from a directory with a .env)")
    })
}

/// Parsed command line.
#[derive(Debug, PartialEq, Eq)]
struct Args {
    site: String,
    env: PublishEnvironment,
    /// `None` = write the bundle to stdout.
    out: Option<PathBuf>,
}

/// Parse `--env`. Deliberately strict: an unrecognised value is an error rather
/// than a fallback to `live`, so a typo cannot vendor the wrong environment's
/// rules into an edge deployment.
///
/// The accepted strings are `PublishEnvironment`'s serde representation — the
/// SAME two the `?env=` query parameter takes, so the CLI and the HTTP route
/// cannot drift. Note that "production" is NOT one of them; the live
/// environment is spelled `live` everywhere in RRE.
fn parse_env(raw: &str) -> anyhow::Result<PublishEnvironment> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "live" => Ok(PublishEnvironment::Live),
        "staging" => Ok(PublishEnvironment::Staging),
        other => Err(anyhow!(
            "--env must be 'live' or 'staging', got '{other}' \
             (the live environment is spelled 'live', not 'production')"
        )),
    }
}

/// Parse the argument vector (already skipping argv[0]).
fn parse_args(argv: impl IntoIterator<Item = String>) -> anyhow::Result<Args> {
    let mut site = None;
    let mut env_arg = "live".to_string();
    let mut out = None;

    let mut args = argv.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--site" => {
                site = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("--site requires a value"))?,
                );
            }
            "--env" => {
                env_arg = args
                    .next()
                    .ok_or_else(|| anyhow!("--env requires a value"))?;
            }
            "--out" => {
                out = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("--out requires a value"))?,
                ));
            }
            other => return Err(anyhow!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        site: site.ok_or_else(|| anyhow!("--site is required"))?,
        env: parse_env(&env_arg)?,
        out,
    })
}

/// Write the serialized bundle to `out`, or to stdout when `out` is `None`.
///
/// The file path is written temp-then-rename so the destination is either the
/// complete new bundle or the untouched previous one — never a partial file.
fn emit(json: &str, out: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = out else {
        println!("{json}");
        return Ok(());
    };

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to move {} into place at {}",
            tmp.display(),
            path.display()
        )
    })?;

    eprintln!("wrote {} ({} bytes)", path.display(), json.len());
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stderr, always: stdout belongs to the bundle so the command can be piped.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,sqlx=warn")),
        )
        .init();

    let args = parse_args(std::env::args().skip(1))?;

    let pool = db::connect(&database_url()?, 5)
        .await
        .context("failed to connect to database")?;

    // A site with no published features is NOT an error: it exports a valid
    // bundle with an empty `features` array, which is the documented no-op state
    // at the edge.
    let bundle = services::edge_bundle_service::build(&pool, &args.site, args.env)
        .await
        .with_context(|| format!("failed to build the edge bundle for site '{}'", args.site))?;

    let json = serde_json::to_string_pretty(&bundle).context("failed to serialize the bundle")?;
    emit(&json, args.out.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> anyhow::Result<Args> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_the_full_invocation() {
        let a = args(&[
            "--site",
            "intrafish-com",
            "--env",
            "staging",
            "--out",
            "b.json",
        ])
        .unwrap();
        assert_eq!(
            a,
            Args {
                site: "intrafish-com".into(),
                env: PublishEnvironment::Staging,
                out: Some(PathBuf::from("b.json")),
            }
        );
    }

    /// Omitting `--out` means stdout, which is what makes the command pipeable.
    #[test]
    fn out_is_optional_and_defaults_to_stdout() {
        assert_eq!(args(&["--site", "x"]).unwrap().out, None);
    }

    #[test]
    fn env_defaults_to_live() {
        assert_eq!(
            args(&["--site", "x"]).unwrap().env,
            PublishEnvironment::Live
        );
    }

    /// The accepted strings are `PublishEnvironment`'s serde form, so the CLI and
    /// the `?env=` query parameter can never drift.
    #[test]
    fn env_accepts_exactly_live_and_staging() {
        assert_eq!(parse_env("live").unwrap(), PublishEnvironment::Live);
        assert_eq!(parse_env("staging").unwrap(), PublishEnvironment::Staging);
        assert_eq!(parse_env("  LIVE ").unwrap(), PublishEnvironment::Live);
    }

    /// "production" is a plausible-looking wrong answer, so it gets an explicit
    /// test: it must FAIL rather than quietly mean `live`.
    #[test]
    fn env_rejects_production_and_other_values() {
        for bad in ["production", "prod", "", "dev", "Live "] {
            if bad.trim().eq_ignore_ascii_case("live") {
                continue;
            }
            let err = parse_env(bad).unwrap_err().to_string();
            assert!(
                err.contains("live") && err.contains("staging"),
                "error must name the valid values, got: {err}"
            );
        }
    }

    #[test]
    fn site_is_required() {
        assert!(args(&["--env", "live"]).is_err());
    }

    #[test]
    fn flags_require_values() {
        assert!(args(&["--site"]).is_err());
        assert!(args(&["--site", "x", "--env"]).is_err());
        assert!(args(&["--site", "x", "--out"]).is_err());
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(args(&["--site", "x", "--verbose"]).is_err());
    }

    #[test]
    fn emit_creates_parent_directories_and_writes_the_file() {
        let root = std::env::temp_dir().join(format!("rre-export-{}", uuid::Uuid::new_v4()));
        let path = root.join("nested/deeper/bundle.json");

        emit("{\"schema_version\":1}", Some(&path)).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"schema_version\":1}"
        );
        assert!(
            !path.with_extension("json.tmp").exists(),
            "the temp file must not survive a successful write"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// A rerun replaces the file wholesale rather than appending or truncating
    /// in place — the property that keeps a failed export from leaving a
    /// half-written bundle where a good one was.
    #[test]
    fn emit_replaces_an_existing_file() {
        let root = std::env::temp_dir().join(format!("rre-export-{}", uuid::Uuid::new_v4()));
        let path = root.join("bundle.json");

        emit("{\"a\":1111111111}", Some(&path)).unwrap();
        emit("{\"b\":2}", Some(&path)).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"b\":2}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn emit_reports_an_unwritable_destination() {
        // A path whose parent is an existing FILE cannot be created.
        let root = std::env::temp_dir().join(format!("rre-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let blocker = root.join("not-a-dir");
        std::fs::write(&blocker, "x").unwrap();

        assert!(emit("{}", Some(&blocker.join("bundle.json"))).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
