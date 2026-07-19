//! Demo data only — not wired to real Claude/Codex scanners yet.

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
    pub fn color(self) -> egui::Color32 {
        match self {
            Status::Ok => egui::Color32::from_rgb(74, 222, 128),
            Status::Near => egui::Color32::from_rgb(251, 191, 36),
            Status::Locked => egui::Color32::from_rgb(248, 113, 113),
            Status::Failed => egui::Color32::from_rgb(148, 163, 184),
        }
    }
}
