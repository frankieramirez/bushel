//! Unified-rail layout plan. Pure: area + facts → rects. Draw consumes this;
//! it does not re-decide stacked vs beside, collapse, or the width cap.

use ratatui::layout::{Constraint, Layout, Rect};

use crate::engine::state::{AppState, Pane};

/// Body width under this stacks the rail above the detail pane.
pub const STACK_BELOW: u16 = 80;
/// Rail never grows past this; spare width belongs to logs.
pub const RAIL_MAX: u16 = 36;
/// Stacked, or a rail shorter than this, uses tight (1-row) collapse.
pub const TIGHT_RAIL_H: u16 = 16;
/// Floor chrome band around ~55×20 (prototype-accepted): 1-row header, no
/// table headers, no tab row, no status cluster.
const FLOOR_H: u16 = 22;
const FLOOR_W: u16 = 60;

/// The facts layout needs from app state. Banner rows are filled in by draw.
#[derive(Clone, Copy, Debug)]
pub struct LayoutFacts {
    pub zoom: bool,
    pub active: Pane,
    pub visible: [u16; 3],
    pub banner_rows: u16,
}

impl LayoutFacts {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            zoom: state.zoom,
            active: state.pane,
            visible: [
                state.visible_rows_for(Pane::Containers).len() as u16,
                state.visible_rows_for(Pane::Images).len() as u16,
                state.visible_rows_for(Pane::Volumes).len() as u16,
            ],
            banner_rows: 0,
        }
    }
}

/// Where every region of the main screen lands for this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutPlan {
    pub header: Rect,
    pub banners: Rect,
    pub body: Rect,
    pub bottom: Rect,
    pub rail: Rect,
    pub detail: Rect,
    pub slots: [Rect; 3],
    pub stacked: bool,
    pub tight: bool,
    pub floor: bool,
    pub zoom: bool,
}

impl LayoutPlan {
    pub fn compute(area: Rect, facts: LayoutFacts) -> Self {
        let floor = area.height <= FLOOR_H || area.width <= FLOOR_W;
        let header_h = if floor { 1 } else { 2 };
        let mut constraints = vec![Constraint::Length(header_h)];
        for _ in 0..facts.banner_rows {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Min(3));
        constraints.push(Constraint::Length(1));
        let chunks = Layout::vertical(constraints).split(area);
        let header = chunks[0];
        let body = chunks[chunks.len() - 2];
        let bottom = chunks[chunks.len() - 1];
        let banners = if facts.banner_rows == 0 {
            Rect {
                x: header.x,
                y: header.y + header.height,
                width: header.width,
                height: 0,
            }
        } else {
            let first = chunks[1];
            let last = chunks[facts.banner_rows as usize];
            Rect {
                x: first.x,
                y: first.y,
                width: first.width,
                height: last.bottom().saturating_sub(first.y),
            }
        };

        if facts.zoom {
            return Self {
                header,
                banners,
                body,
                bottom,
                rail: body,
                detail: body,
                slots: [body, Rect::default(), Rect::default()],
                stacked: false,
                tight: floor,
                floor,
                zoom: true,
            };
        }

        let stacked = body.width < STACK_BELOW;
        let tight = stacked || body.height < TIGHT_RAIL_H;

        let (rail, detail) = if stacked {
            let rail_h = stacked_rail_height(body.height, facts.visible[facts.active.index()]);
            let parts =
                Layout::vertical([Constraint::Length(rail_h), Constraint::Min(3)]).split(body);
            (parts[0], parts[1])
        } else {
            let rail_w = RAIL_MAX;
            let parts =
                Layout::horizontal([Constraint::Length(rail_w), Constraint::Min(12)]).split(body);
            (parts[0], parts[1])
        };

        let slots = rail_slots(rail, facts, tight);
        Self {
            header,
            banners,
            body,
            bottom,
            rail,
            detail,
            slots,
            stacked,
            tight,
            floor,
            zoom: false,
        }
    }
}

fn stacked_rail_height(body_h: u16, active_rows: u16) -> u16 {
    let inactive = 2u16;
    let active_h = (active_rows + 2).clamp(4, 8);
    let want = inactive + active_h;
    let cap = (body_h / 2).max(6);
    want.clamp(6, cap.min(body_h.saturating_sub(4)))
}

fn rail_slots(rail: Rect, facts: LayoutFacts, tight: bool) -> [Rect; 3] {
    let panes = Pane::all();
    let constraints: Vec<Constraint> = panes
        .iter()
        .map(|&p| {
            if p == facts.active {
                Constraint::Fill(1)
            } else if tight {
                Constraint::Length(1)
            } else {
                let need = facts.visible[p.index()] + 2;
                let cap = (rail.height / 4).max(8);
                Constraint::Length(need.clamp(3, cap))
            }
        })
        .collect();
    let split = Layout::vertical(constraints).split(rail);
    [split[0], split[1], split[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prototype seed: 8 containers, 5 images, 3 volumes, containers active.
    fn facts() -> LayoutFacts {
        LayoutFacts {
            zoom: false,
            active: Pane::Containers,
            visible: [8, 5, 3],
            banner_rows: 0,
        }
    }

    fn plan(w: u16, h: u16) -> LayoutPlan {
        LayoutPlan::compute(Rect::new(0, 0, w, h), facts())
    }

    #[test]
    fn floor_55x20_stacks_the_rail_above_detail() {
        let p = plan(55, 20);
        assert!(p.stacked);
        assert!(p.tight);
        assert!(p.floor);
        assert_eq!(p.header.height, 1);
        assert_eq!(p.rail.width, 55);
        assert_eq!(p.rail.height, 9);
        assert_eq!(p.detail.width, 55);
        assert_eq!(p.detail.height, 9);
        assert!(p.rail.y < p.detail.y);
        assert_eq!(p.rail.x, p.detail.x);
        // tight inactive panes are one row; the active panel takes the rest
        assert_eq!(p.slots[Pane::Images.index()].height, 1);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 1);
        assert_eq!(p.slots[Pane::Containers.index()].height, 7);
    }

    #[test]
    fn body_width_under_80_stacks_otherwise_beside() {
        // 79×30 is not floor (h>22, w>60); header 2 + bottom 1 → body 27×79
        let stacked = plan(79, 30);
        assert!(stacked.stacked, "79-col body must stack");
        let beside = plan(80, 30);
        assert!(!beside.stacked, "80-col body sits beside");
        assert_eq!(beside.rail.width, 36);
    }

    #[test]
    fn medium_100x30_sits_beside_capped_at_36() {
        let p = plan(100, 30);
        assert!(!p.stacked);
        assert!(!p.tight);
        assert!(!p.floor);
        assert_eq!(p.header.height, 2);
        assert_eq!(p.rail.width, RAIL_MAX);
        assert_eq!(p.rail.height, 27);
        assert_eq!(p.detail.width, 64);
        assert_eq!(p.detail.height, 27);
        assert_eq!(p.rail.y, p.detail.y);
        assert!(p.rail.x < p.detail.x);
        // roomy: shrink-to-fit names, cap max(8, height/4)=8
        // images 5+2=7, volumes 3+2=5, containers fill
        assert_eq!(p.slots[Pane::Images.index()].height, 7);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 5);
        assert_eq!(p.slots[Pane::Containers.index()].height, 15);
    }

    #[test]
    fn wide_200x50_keeps_the_rail_cap_spare_goes_to_detail() {
        let p = plan(200, 50);
        assert!(!p.stacked);
        assert_eq!(p.rail.width, RAIL_MAX);
        assert_eq!(p.detail.width, 164);
        assert_eq!(p.detail.height, 47);
    }

    #[test]
    fn rail_height_under_16_is_tight_even_when_beside() {
        // floor (h<=22): header 1 + bottom 1. Frame 17 → body 15 < TIGHT_RAIL_H.
        let p = plan(100, 17);
        assert!(!p.stacked);
        assert_eq!(p.body.height, 15);
        assert!(p.tight);
        assert_eq!(p.slots[Pane::Images.index()].height, 1);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 1);
    }

    #[test]
    fn zoom_gives_the_whole_body_to_one_side() {
        let mut f = facts();
        f.zoom = true;
        let p = LayoutPlan::compute(Rect::new(0, 0, 100, 30), f);
        assert!(p.zoom);
        assert_eq!(p.rail, p.body);
        assert_eq!(p.detail, p.body);
        assert_eq!(p.slots[0], p.body);
    }

    #[test]
    fn floor_when_height_leq_22_or_width_leq_60() {
        assert!(plan(100, 22).floor);
        assert!(plan(60, 30).floor);
        assert!(!plan(61, 23).floor);
    }

    #[test]
    fn roomy_inactive_cap_is_max_8_or_height_over_4() {
        // 100×40: not floor, header 2 + bottom 1 → body 37. cap = max(8, 37/4)=9
        // images need 7, under cap; invent a long inactive list via facts
        let f = LayoutFacts {
            zoom: false,
            active: Pane::Containers,
            visible: [2, 40, 40],
            banner_rows: 0,
        };
        let p = LayoutPlan::compute(Rect::new(0, 0, 100, 40), f);
        let cap = (p.rail.height / 4).max(8);
        assert_eq!(cap, 9);
        assert_eq!(p.slots[Pane::Images.index()].height, cap);
        assert_eq!(p.slots[Pane::Volumes.index()].height, cap);
    }
}
