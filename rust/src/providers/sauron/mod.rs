//! Sauron provider implementation.

use async_trait::async_trait;

use crate::core::{
    FetchContext, Provider, ProviderError, ProviderFetchResult, ProviderId, ProviderMetadata,
    RateWindow, SourceMode, UsageSnapshot,
};
use crate::sauron::SauronManager;
use crate::settings::Settings;

pub struct SauronProvider {
    metadata: ProviderMetadata,
}

impl SauronProvider {
    pub fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                id: ProviderId::Sauron,
                display_name: "Sauron",
                session_label: "Status",
                weekly_label: "Agent",
                supports_opus: false,
                supports_credits: false,
                default_enabled: false,
                is_primary: false,
                dashboard_url: Some(SauronManager::repo_url()),
                status_page_url: None,
            },
        }
    }

    async fn fetch_status(&self) -> Result<UsageSnapshot, ProviderError> {
        let settings = Settings::load();
        let report = SauronManager::status(&settings).map_err(|error| {
            let message = error.to_string();
            if message.contains("was not found") {
                ProviderError::NotInstalled(
                    "sauron-sees.exe was not found on PATH or in %LOCALAPPDATA%\\Programs\\sauron-sees"
                        .to_string(),
                )
            } else {
                ProviderError::Other(message)
            }
        })?;

        let primary = RateWindow::with_details(
            report.state.progress_percent(),
            None,
            None,
            Some(report.detail.clone()),
        );

        let mut usage = UsageSnapshot::new(primary).with_login_method(report.state.status_label());
        if let Some(dir) = report.screenshots_dir {
            usage = usage.with_organization(dir.display().to_string());
        }

        Ok(usage)
    }
}

impl Default for SauronProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for SauronProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Sauron
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<ProviderFetchResult, ProviderError> {
        let usage = self.fetch_status().await?;
        Ok(ProviderFetchResult::new(usage, "cli"))
    }

    fn available_sources(&self) -> Vec<SourceMode> {
        vec![SourceMode::Auto, SourceMode::Cli]
    }

    fn supports_cli(&self) -> bool {
        true
    }
}
