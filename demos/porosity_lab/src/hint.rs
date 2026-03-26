use crate::scoring::{Analysis, Classification};
use crate::state::Puzzle;

pub struct Guidance {
    pub hint_title: String,
    pub hint_text: String,
    pub candidate_title: String,
    pub candidate_text: String,
    pub interesting: bool,
}

pub fn build_guidance(puzzle: &Puzzle, current: &Analysis, baseline: &Analysis) -> Guidance {
    let target = puzzle.active_target();
    let limiting_label = current
        .limiting_edge
        .and_then(|index| puzzle.edges.get(index))
        .map(|edge| edge.label)
        .unwrap_or("limiting aperture");

    let hint = match current.classification {
        Classification::Blocked => (
            "Reconnect".to_string(),
            format!("Open one lane, then widen the {}.", limiting_label),
        ),
        Classification::Bottlenecked => (
            "Widest path found".to_string(),
            format!("{} is still below {:.1}.", limiting_label, target),
        ),
        Classification::Open => {
            let bottleneck = current.best_bottleneck.unwrap_or(0.0);
            if (bottleneck - target).abs() < 0.12 {
                (
                    "Threshold hit".to_string(),
                    format!("Clearance is only {:.2}.", bottleneck - target),
                )
            } else {
                (
                    "Smooth run".to_string(),
                    "Now beat it in fewer moves.".to_string(),
                )
            }
        }
    };

    let classification_jump = current.classification.rank() - baseline.classification.rank();
    let score_gain = current.accessibility_score - baseline.accessibility_score;
    let near_threshold = current
        .best_bottleneck
        .map(|value| (value - target).abs() < 0.10)
        .unwrap_or(false);
    let low_edit_flip = current.changed_edges <= 2 && classification_jump > 0;
    let tradeoff_state = current.classification == Classification::Open
        && current.modification_cost > 0.18
        && current.modification_cost < 0.42;

    let interesting = near_threshold || low_edit_flip || tradeoff_state || score_gain > 18.0;
    let candidate_text = if low_edit_flip {
        format!("{} moves flipped transport.", current.changed_edges.max(1),)
    } else if near_threshold {
        format!("Best gate sits near {:.1}.", target)
    } else if tradeoff_state {
        "Open, but expensive. There is a real trade-off.".to_string()
    } else if score_gain > 18.0 {
        format!("Score jumped by {:.0}.", score_gain)
    } else {
        "No sharp transition yet.".to_string()
    };

    Guidance {
        hint_title: hint.0,
        hint_text: hint.1,
        candidate_title: if interesting {
            "Flagged".to_string()
        } else {
            "Not flagged".to_string()
        },
        candidate_text,
        interesting,
    }
}
