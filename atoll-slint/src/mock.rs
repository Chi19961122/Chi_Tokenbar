//! Demo data only — not wired to real Claude/Codex scanners yet.
//! Mirrors atoll-egui/src/mock.rs, adapted to feed Slint's LimitRow struct.

use slint::{Color, ModelRc, SharedString, VecModel};

use crate::LimitRow;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
    Grok,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Near,
    Locked,
    Failed,
}

#[derive(Clone)]
pub struct Limit {
    pub id: &'static str,
    pub provider: Provider,
    pub label: &'static str,
    /// 0..=100 utilization
    pub util: f64,
    pub status: Status,
    pub runway_label: &'static str,
}

#[derive(Clone)]
pub struct Snapshot {
    pub limits: Vec<Limit>,
    pub updated_label: String,
}

pub fn demo_snapshot() -> Snapshot {
    Snapshot {
        limits: vec![
            Limit {
                id: "cc.5h",
                provider: Provider::Claude,
                label: "Claude 5h",
                util: 42.0,
                status: Status::Ok,
                runway_label: "~3h left",
            },
            Limit {
                id: "cc.week",
                provider: Provider::Claude,
                label: "Claude week",
                util: 78.0,
                status: Status::Near,
                runway_label: "~1d left",
            },
            Limit {
                id: "codex.5h",
                provider: Provider::Codex,
                label: "Codex 5h",
                util: 55.0,
                status: Status::Ok,
                runway_label: "~2h left",
            },
            Limit {
                id: "codex.week",
                provider: Provider::Codex,
                label: "Codex week",
                util: 100.0,
                status: Status::Locked,
                runway_label: "locked",
            },
            Limit {
                id: "grok.ctx",
                provider: Provider::Grok,
                label: "Grok context",
                util: 33.0,
                status: Status::Ok,
                runway_label: "session",
            },
        ],
        updated_label: "demo data · not live APIs".into(),
    }
}

impl Provider {
    pub fn short(self) -> &'static str {
        match self {
            Provider::Claude => "Claude",
            Provider::Codex => "Codex",
            Provider::Grok => "Grok",
        }
    }
}

impl Status {
    pub fn color(self) -> Color {
        match self {
            Status::Ok => Color::from_rgb_u8(74, 222, 128),
            Status::Near => Color::from_rgb_u8(251, 191, 36),
            Status::Locked => Color::from_rgb_u8(248, 113, 113),
            Status::Failed => Color::from_rgb_u8(148, 163, 184),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Near => "near limit",
            Status::Locked => "locked",
            Status::Failed => "failed",
        }
    }
}

pub fn worst_for(snap: &Snapshot, p: Provider) -> Option<&Limit> {
    snap.limits
        .iter()
        .filter(|l| l.provider == p)
        .max_by(|a, b| {
            a.util
                .partial_cmp(&b.util)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Build the Slint LimitRow model from the mock snapshot.
pub fn limit_rows(snap: &Snapshot) -> ModelRc<LimitRow> {
    let rows: Vec<LimitRow> = snap
        .limits
        .iter()
        .map(|l| LimitRow {
            label: SharedString::from(l.label),
            provider: SharedString::from(l.provider.short()),
            status_label: SharedString::from(l.status.label()),
            remaining_pct: (100.0 - l.util) as f32,
            util_pct: l.util as f32,
            runway_label: SharedString::from(l.runway_label),
            color: l.status.color(),
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}
