use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};

use crate::cli::args::{GlobalArgs};
use crate::config::Settings;
use crate::core::{registry::ProviderRegistry, error::Error};
use crate::scheduler::{build_notifier, run_expiry_check};

pub async fn run_daemon(settings: &Settings, registry: &ProviderRegistry) -> Result<(), Error> {
    let interval = settings.scheduler.check_interval_secs;
    info!("Starting scheduler daemon, check interval: {}s", interval);

    let sched = JobScheduler::new().await
        .map_err(|e| Error::Scheduler(e.to_string()))?;

    // Run immediately on start
    let notifier = build_notifier(settings);
    run_expiry_check(registry, settings, notifier.as_ref()).await;

    let cron_expr = format!("0 0 */{} * * *", interval / 3600);
    info!("Scheduling with cron: {}", cron_expr);

    sched.start().await.map_err(|e| Error::Scheduler(e.to_string()))?;

    // Keep running until interrupted
    tokio::signal::ctrl_c().await
        .map_err(|e| Error::Io(e))?;
    info!("Scheduler shutting down");
    Ok(())
}

pub async fn run_once(settings: &Settings, registry: &ProviderRegistry) -> Result<(), Error> {
    info!("Running one-shot expiry check");
    let notifier = build_notifier(settings);
    run_expiry_check(registry, settings, notifier.as_ref()).await;
    Ok(())
}
