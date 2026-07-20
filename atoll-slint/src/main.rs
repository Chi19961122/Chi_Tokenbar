//! Minimal Atoll-like shell on Slint: compact island + expandable limits list.
//! Experimental — mock data only. Uses Slint's software renderer (no GPU).

mod mock;

use mock::{demo_snapshot, worst_for, Provider};
use slint::ComponentHandle;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let snap = demo_snapshot();
    let window = AtollWindow::new()?;

    window.set_limits(mock::limit_rows(&snap));
    window.set_updated_label(snap.updated_label.clone().into());

    let claude = worst_for(&snap, Provider::Claude);
    let codex = worst_for(&snap, Provider::Codex);
    window.set_claude_remaining(claude.map(|l| 100.0 - l.util).unwrap_or(0.0) as f32);
    window.set_codex_remaining(codex.map(|l| 100.0 - l.util).unwrap_or(0.0) as f32);
    window.set_claude_color(
        claude
            .map(|l| l.status.color())
            .unwrap_or(slint::Color::from_rgb_u8(120, 120, 140)),
    );
    window.set_codex_color(
        codex
            .map(|l| l.status.color())
            .unwrap_or(slint::Color::from_rgb_u8(120, 120, 140)),
    );

    let weak = window.as_weak();
    window.on_expand_requested(move || {
        if let Some(w) = weak.upgrade() {
            w.set_expanded(true);
        }
    });

    let weak = window.as_weak();
    window.on_collapse_requested(move || {
        if let Some(w) = weak.upgrade() {
            w.set_expanded(false);
            w.set_open_index(-1);
        }
    });

    let weak = window.as_weak();
    window.on_row_toggled(move |i| {
        if let Some(w) = weak.upgrade() {
            let cur = w.get_open_index();
            w.set_open_index(if cur == i { -1 } else { i });
        }
    });

    window.run()
}
