#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(f64, five_hour_util)]
        #[qproperty(QString, five_hour_resets_at)]
        #[qproperty(f64, seven_day_util)]
        #[qproperty(QString, seven_day_resets_at)]
        /// Per-model weekly limit (e.g. Fable); name is empty when absent
        #[qproperty(QString, seven_day_model_name)]
        #[qproperty(f64, seven_day_model_util)]
        #[qproperty(QString, seven_day_model_resets_at)]
        #[qproperty(f64, extra_usage_util)]
        #[qproperty(f64, extra_usage_used)]
        #[qproperty(f64, extra_usage_limit)]
        #[qproperty(bool, extra_usage_enabled)]
        #[qproperty(QString, error)]
        /// Epoch ms when the displayed data was received; 0 = never
        #[qproperty(f64, updated_at)]
        /// true once credentials are stored in KWallet
        #[qproperty(bool, configured)]
        type ClaudeUsage = super::ClaudeUsageRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        #[qinvokable]
        fn save_credentials(
            self: Pin<&mut Self>,
            url: &QString,
            username: &QString,
            password: &QString,
        );

        #[qinvokable]
        fn clear_credentials(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for ClaudeUsage {}
}

use crate::kwallet;
use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct ClaudeUsageRust {
    five_hour_util: f64,
    five_hour_resets_at: QString,
    seven_day_util: f64,
    seven_day_resets_at: QString,
    seven_day_model_name: QString,
    seven_day_model_util: f64,
    seven_day_model_resets_at: QString,
    extra_usage_util: f64,
    extra_usage_used: f64,  // in cents (divide by 100 for USD)
    extra_usage_limit: f64, // in cents (divide by 100 for USD)
    extra_usage_enabled: bool,
    error: QString,
    updated_at: f64,
    configured: bool,
    refresh_in_flight: Arc<AtomicBool>,
}

#[derive(Deserialize)]
struct UsageLimit {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct ExtraUsage {
    is_enabled: bool,
    monthly_limit: Option<f64>,
    used_credits: Option<f64>,
    utilization: Option<f64>,
}

#[derive(Deserialize)]
struct LimitScopeModel {
    id: Option<String>,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct LimitScope {
    model: Option<LimitScopeModel>,
}

/// One entry of the `limits` array; per-model weekly limits arrive as
/// `weekly_scoped` entries here (the legacy `seven_day_sonnet` field is null
/// on current accounts but kept as a fallback for older proxy snapshots).
#[derive(Deserialize)]
struct LimitEntry {
    kind: Option<String>,
    percent: Option<f64>,
    resets_at: Option<String>,
    scope: Option<LimitScope>,
}

#[derive(Deserialize)]
struct SubscriptionUsage {
    five_hour: Option<UsageLimit>,
    seven_day: Option<UsageLimit>,
    seven_day_sonnet: Option<UsageLimit>,
    #[serde(default)]
    limits: Vec<LimitEntry>,
    extra_usage: Option<ExtraUsage>,
}

/// (name, utilization, resets_at) of the per-model weekly limit, preferring
/// the scoped `limits` entry and falling back to legacy `seven_day_sonnet`.
/// Name is empty when no per-model limit is present.
fn model_weekly_limit(usage: &SubscriptionUsage) -> (String, f64, String) {
    if let Some(l) = usage
        .limits
        .iter()
        .find(|l| l.kind.as_deref() == Some("weekly_scoped") && l.percent.is_some())
    {
        let name = l
            .scope
            .as_ref()
            .and_then(|s| s.model.as_ref())
            .and_then(|m| m.display_name.clone().or_else(|| m.id.clone()))
            .unwrap_or_else(|| "Model".to_string());
        return (
            name,
            l.percent.unwrap(),
            l.resets_at.clone().unwrap_or_default(),
        );
    }
    if let Some(u) = usage.seven_day_sonnet.as_ref() {
        if let Some(pct) = u.utilization {
            return (
                "Sonnet".to_string(),
                pct,
                u.resets_at.clone().unwrap_or_default(),
            );
        }
    }
    (String::new(), -1.0, String::new())
}

enum RefreshError {
    NoCredentials(String), // KWallet empty, locked, or unavailable
    Http(String),
}

impl qobject::ClaudeUsage {
    fn refresh(self: Pin<&mut Self>) {
        #[cfg(debug_assertions)]
        eprintln!("[claude-plasmoid] refresh() called");
        // One refresh at a time: overlapping worker threads could queue their
        // results out of order and let a stale response overwrite a fresh one.
        let in_flight = self.refresh_in_flight.clone();
        if in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        // Move both KWallet and HTTP off the Qt thread. A locked wallet can
        // block kwalletd6's open() on a password dialog for an arbitrary
        // amount of time, which would freeze plasmashell just like a slow
        // HTTP request would.
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = kwallet::read_credentials()
                .map_err(RefreshError::NoCredentials)
                .and_then(|c| {
                    #[cfg(debug_assertions)]
                    eprintln!("[claude-plasmoid] KWallet ok, url={}", c.url);
                    fetch_usage(&c.url, &c.username, &c.password).map_err(RefreshError::Http)
                });

            let flag = in_flight.clone();
            let queued = qt_thread.queue(move |mut qobj| {
                match result {
                    Ok(usage) => {
                        let five_h = usage
                            .five_hour
                            .as_ref()
                            .and_then(|u| u.utilization)
                            .unwrap_or(-1.0);
                        let five_h_reset = usage
                            .five_hour
                            .as_ref()
                            .and_then(|u| u.resets_at.clone())
                            .unwrap_or_default();
                        let seven_d = usage
                            .seven_day
                            .as_ref()
                            .and_then(|u| u.utilization)
                            .unwrap_or(-1.0);
                        let seven_d_reset = usage
                            .seven_day
                            .as_ref()
                            .and_then(|u| u.resets_at.clone())
                            .unwrap_or_default();
                        let (model_name, model_util, model_reset) = model_weekly_limit(&usage);
                        let (ex_en, ex_util, ex_used, ex_limit) = match usage.extra_usage.as_ref() {
                            Some(e) => (
                                e.is_enabled,
                                e.utilization.unwrap_or(-1.0),
                                e.used_credits.unwrap_or(0.0),
                                e.monthly_limit.unwrap_or(0.0),
                            ),
                            None => (false, -1.0, 0.0, 0.0),
                        };

                        qobj.as_mut().set_configured(true);
                        qobj.as_mut().set_five_hour_util(five_h);
                        qobj.as_mut()
                            .set_five_hour_resets_at(QString::from(&five_h_reset));
                        qobj.as_mut().set_seven_day_util(seven_d);
                        qobj.as_mut()
                            .set_seven_day_resets_at(QString::from(&seven_d_reset));
                        qobj.as_mut()
                            .set_seven_day_model_name(QString::from(&model_name));
                        qobj.as_mut().set_seven_day_model_util(model_util);
                        qobj.as_mut()
                            .set_seven_day_model_resets_at(QString::from(&model_reset));
                        qobj.as_mut().set_extra_usage_enabled(ex_en);
                        qobj.as_mut().set_extra_usage_util(ex_util);
                        qobj.as_mut().set_extra_usage_used(ex_used);
                        qobj.as_mut().set_extra_usage_limit(ex_limit);
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as f64)
                            .unwrap_or(0.0);
                        qobj.as_mut().set_updated_at(now_ms);
                        qobj.as_mut().set_error(QString::from(""));
                    }
                    Err(RefreshError::NoCredentials(e)) => {
                        #[cfg(debug_assertions)]
                        eprintln!("[claude-plasmoid] KWallet err: {e}");
                        qobj.as_mut().set_configured(false);
                        qobj.as_mut().set_error(QString::from(&e));
                    }
                    Err(RefreshError::Http(e)) => {
                        qobj.as_mut().set_error(QString::from(&e));
                    }
                }
                flag.store(false, Ordering::Release);
            });
            // If queueing failed the closure never runs; release the guard.
            if queued.is_err() {
                in_flight.store(false, Ordering::Release);
            }
        });
    }

    fn save_credentials(
        mut self: Pin<&mut Self>,
        url: &QString,
        username: &QString,
        password: &QString,
    ) {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_string();

        match kwallet::write_credentials(&url, &username, &password) {
            Ok(()) => {
                self.as_mut().set_configured(true);
                self.as_mut().set_error(QString::from(""));
                self.refresh();
            }
            Err(e) => self.as_mut().set_error(QString::from(&e)),
        }
    }

    fn clear_credentials(mut self: Pin<&mut Self>) {
        match kwallet::delete_credentials() {
            Ok(()) => {
                self.as_mut().set_configured(false);
                self.as_mut().set_error(QString::from(""));
            }
            Err(e) => {
                self.as_mut().set_error(QString::from(&e));
            }
        }
    }
}

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_http_client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            // Safe to expect: Client::build() only fails with custom TLS backends. Panic occurs
            // in spawned thread, so it won't cross FFI — thread just terminates silently.
            .expect("HTTP client initialization failed")
    })
}

fn fetch_usage(
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<SubscriptionUsage, String> {
    let base = base_url.trim_end_matches('/').trim_end_matches("/admin");
    let url = format!("{base}/admin/oauth/usage");
    #[cfg(debug_assertions)]
    eprintln!("[claude-plasmoid] GET {url} as {username}");
    let client = get_http_client();
    let resp = client
        .get(&url)
        .basic_auth(username, Some(password))
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.json::<SubscriptionUsage>().map_err(|e| e.to_string())
}
