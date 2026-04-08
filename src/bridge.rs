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
        #[qproperty(f64, seven_day_sonnet_util)]
        #[qproperty(QString, seven_day_sonnet_resets_at)]
        #[qproperty(f64, extra_usage_util)]
        #[qproperty(f64, extra_usage_used)]
        #[qproperty(f64, extra_usage_limit)]
        #[qproperty(bool, extra_usage_enabled)]
        #[qproperty(QString, error)]
        /// true once credentials are stored in KWallet
        #[qproperty(bool, configured)]
        type ClaudeUsage = super::ClaudeUsageRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        #[qinvokable]
        fn save_credentials(self: Pin<&mut Self>, url: &QString, username: &QString, password: &QString);
    }

    impl cxx_qt::Threading for ClaudeUsage {}
}

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use serde::Deserialize;
use crate::kwallet;

#[derive(Default)]
pub struct ClaudeUsageRust {
    five_hour_util: f64,
    five_hour_resets_at: QString,
    seven_day_util: f64,
    seven_day_resets_at: QString,
    seven_day_sonnet_util: f64,
    seven_day_sonnet_resets_at: QString,
    extra_usage_util: f64,
    extra_usage_used: f64,
    extra_usage_limit: f64,
    extra_usage_enabled: bool,
    error: QString,
    configured: bool,
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
struct SubscriptionUsage {
    five_hour: Option<UsageLimit>,
    seven_day: Option<UsageLimit>,
    seven_day_sonnet: Option<UsageLimit>,
    extra_usage: Option<ExtraUsage>,
}

impl qobject::ClaudeUsage {
    fn refresh(mut self: Pin<&mut Self>) {
        eprintln!("[claude-plasmoid] refresh() called");
        let creds = match kwallet::read_credentials() {
            Ok(c) => { eprintln!("[claude-plasmoid] KWallet ok, url={}", c.url); c }
            Err(e) => {
                eprintln!("[claude-plasmoid] KWallet err: {e}");
                self.as_mut().set_configured(false);
                self.as_mut().set_error(QString::from(&e));
                return;
            }
        };

        self.as_mut().set_configured(true);

        // Move the HTTP call off the Qt thread so plasmashell doesn't freeze.
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = fetch_usage(&creds.url, &creds.username, &creds.password);
            let _ = qt_thread.queue(move |mut qobj| match result {
                Ok(usage) => {
                    let five_h = usage.five_hour.as_ref().and_then(|u| u.utilization).unwrap_or(-1.0);
                    let five_h_reset = usage.five_hour.as_ref().and_then(|u| u.resets_at.clone()).unwrap_or_default();
                    let seven_d = usage.seven_day.as_ref().and_then(|u| u.utilization).unwrap_or(-1.0);
                    let seven_d_reset = usage.seven_day.as_ref().and_then(|u| u.resets_at.clone()).unwrap_or_default();
                    let seven_d_s = usage.seven_day_sonnet.as_ref().and_then(|u| u.utilization).unwrap_or(-1.0);
                    let seven_d_s_reset = usage.seven_day_sonnet.as_ref().and_then(|u| u.resets_at.clone()).unwrap_or_default();
                    let (ex_en, ex_util, ex_used, ex_limit) = match usage.extra_usage.as_ref() {
                        Some(e) => (e.is_enabled, e.utilization.unwrap_or(-1.0), e.used_credits.unwrap_or(0.0), e.monthly_limit.unwrap_or(0.0)),
                        None => (false, -1.0, 0.0, 0.0),
                    };

                    qobj.as_mut().set_five_hour_util(five_h);
                    qobj.as_mut().set_five_hour_resets_at(QString::from(&five_h_reset));
                    qobj.as_mut().set_seven_day_util(seven_d);
                    qobj.as_mut().set_seven_day_resets_at(QString::from(&seven_d_reset));
                    qobj.as_mut().set_seven_day_sonnet_util(seven_d_s);
                    qobj.as_mut().set_seven_day_sonnet_resets_at(QString::from(&seven_d_s_reset));
                    qobj.as_mut().set_extra_usage_enabled(ex_en);
                    qobj.as_mut().set_extra_usage_util(ex_util);
                    qobj.as_mut().set_extra_usage_used(ex_used);
                    qobj.as_mut().set_extra_usage_limit(ex_limit);
                    qobj.as_mut().set_error(QString::from(""));
                }
                Err(e) => qobj.as_mut().set_error(QString::from(&e)),
            });
        });
    }

    fn save_credentials(mut self: Pin<&mut Self>, url: &QString, username: &QString, password: &QString) {
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
}

fn fetch_usage(base_url: &str, username: &str, password: &str) -> Result<SubscriptionUsage, String> {
    let base = base_url.trim_end_matches('/').trim_end_matches("/admin");
    let url = format!("{base}/admin/oauth/usage");
    eprintln!("[claude-plasmoid] GET {url} as {username}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
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
