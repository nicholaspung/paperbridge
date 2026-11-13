//! Host-side privacy checks (plan §7.2). Read-only: files and the interface
//! list only. They can raise a warning but can never prove isolation, so the
//! best level reachable here is `Unverified`.

use crate::platform;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PrivacyLevel {
    /// Nothing suspicious seen, but the tablet itself was not inspected.
    Unverified,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrivacyStatus {
    pub level: PrivacyLevel,
    pub findings: Vec<Finding>,
}

/// What the checks observed, separated from how it was read so the rules
/// can be tested.
#[derive(Debug, Default)]
pub struct HostFacts {
    pub checks_supported: bool,
    pub ip_forwarding: Option<bool>,
    /// reMarkable USB interfaces that do not hold a tablet-assigned address.
    pub misconfigured_interfaces: Vec<String>,
}

pub fn evaluate(facts: &HostFacts) -> PrivacyStatus {
    let mut findings = vec![Finding {
        code: "tablet-not-inspected".into(),
        severity: Severity::Info,
        message: "PaperBridge cannot see the tablet's own network settings over USB. \
                  Keep Wi-Fi switched off on the tablet."
            .into(),
    }];
    if !facts.checks_supported {
        findings.push(Finding {
            code: "host-checks-unavailable".into(),
            severity: Severity::Info,
            message: "Host network checks are not available on this operating system yet.".into(),
        });
    }
    for name in &facts.misconfigured_interfaces {
        findings.push(Finding {
            code: "usb-interface-misconfigured".into(),
            severity: Severity::Warning,
            message: format!(
                "The tablet's USB network interface ({name}) does not have an address from the \
                 tablet. This happens when the computer is set to share its internet connection \
                 over that interface. Turn sharing off for it."
            ),
        });
    }
    if facts.ip_forwarding == Some(true) {
        findings.push(Finding {
            code: "ip-forwarding-enabled".into(),
            severity: Severity::Info,
            message: "This computer forwards network traffic between interfaces (common with \
                      Docker or virtual machines). That alone does not give the tablet internet \
                      access."
                .into(),
        });
    }
    let level = if findings.iter().any(|f| f.severity == Severity::Warning) {
        PrivacyLevel::Warning
    } else {
        PrivacyLevel::Unverified
    };
    PrivacyStatus { level, findings }
}

pub fn check_host() -> PrivacyStatus {
    evaluate(&gather())
}

#[cfg(target_os = "linux")]
fn gather() -> HostFacts {
    let tablet: Vec<String> = platform::find_tablet_interfaces()
        .unwrap_or_default()
        .into_iter()
        .map(|i| i.name)
        .collect();
    HostFacts {
        checks_supported: true,
        ip_forwarding: std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
            .ok()
            .map(|s| s.trim() == "1"),
        misconfigured_interfaces: platform::known_usb_interfaces()
            .into_iter()
            .filter(|n| !tablet.contains(n))
            .collect(),
    }
}

#[cfg(not(target_os = "linux"))]
fn gather() -> HostFacts {
    let _ = platform::TABLET_IP;
    HostFacts::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_better_than_unverified() {
        let s = evaluate(&HostFacts {
            checks_supported: true,
            ip_forwarding: Some(false),
            misconfigured_interfaces: vec![],
        });
        assert_eq!(s.level, PrivacyLevel::Unverified);
    }

    #[test]
    fn forwarding_alone_is_not_a_warning() {
        let s = evaluate(&HostFacts {
            checks_supported: true,
            ip_forwarding: Some(true),
            misconfigured_interfaces: vec![],
        });
        assert_eq!(s.level, PrivacyLevel::Unverified);
        assert!(s.findings.iter().any(|f| f.code == "ip-forwarding-enabled"));
    }

    #[test]
    fn misconfigured_interface_warns() {
        let s = evaluate(&HostFacts {
            checks_supported: true,
            ip_forwarding: None,
            misconfigured_interfaces: vec!["usb0".into()],
        });
        assert_eq!(s.level, PrivacyLevel::Warning);
    }
}
