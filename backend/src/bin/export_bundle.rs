//! Offline edge-bundle export. The SAME service the HTTP route uses, for when
//! the backend's port is not reachable from where the export runs (a Fastly
//! vendoring step on a laptop, a CI job with only a database URL).
//!
//!   cargo run --bin export_bundle -- --site intrafish-com --env live > bundle.json
//!
//! Reads `DATABASE_URL` from the environment / `.env` via `Settings::load`.
//! Writes the bundle to stdout and nothing else, so it is safe to redirect.

#![forbid(unsafe_code)]

use anyhow::{anyhow, Context};
use rre_backend::{config::Settings, db, schemas::version::PublishEnvironment, services};

/// Parse `--env`. Deliberately strict: an unrecognised value is an error rather
/// than a fallback to `live`, so a typo cannot vendor the wrong environment's
/// rules into an edge deployment.
fn parse_env(raw: &str) -> anyhow::Result<PublishEnvironment> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "live" => Ok(PublishEnvironment::Live),
        "staging" => Ok(PublishEnvironment::Staging),
        other => Err(anyhow!("--env must be live or staging, got '{other}'")),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut site = None;
    let mut env_arg = "live".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--site" => site = args.next(),
            "--env" => {
                env_arg = args
                    .next()
                    .ok_or_else(|| anyhow!("--env requires a value"))?;
            }
            other => return Err(anyhow!("unknown argument: {other}")),
        }
    }

    let site = site.ok_or_else(|| anyhow!("--site is required"))?;
    let env = parse_env(&env_arg)?;

    let settings = Settings::load().context("failed to load settings")?;
    let pool = db::connect(&settings.database_url, 5)
        .await
        .context("failed to connect to database")?;

    let bundle = services::edge_bundle_service::build(&pool, &site, env)
        .await
        .with_context(|| format!("failed to build the edge bundle for site '{site}'"))?;

    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
