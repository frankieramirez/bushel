use ratatui::layout::{Constraint, Layout, Rect};

use crate::engine::state::{ActionItem, AppState, Pane, UiAction};

pub const STACK_BELOW: u16 = 80;
pub const RAIL_MAX: u16 = 36;
pub const TIGHT_RAIL_H: u16 = 16;
const FLOOR_H: u16 = 22;
const FLOOR_W: u16 = 60;
const RAIL_MIN_H: u16 = Pane::all().len() as u16;
pub const SHEET_MAX_H: u16 = 9;
pub const CONFIRM_W: u16 = 48;
pub const CONFIRM_H: u16 = 7;
pub const HELP_W: u16 = 68;

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
                Layout::vertical([Constraint::Length(rail_h), Constraint::Fill(1)]).split(body);
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

pub fn sheet_items(mut actions: Vec<ActionItem>, floor: bool) -> Vec<ActionItem> {
    let is_jump = |a: &ActionItem| matches!(a.action, UiAction::LogsTab | UiAction::InspectTab);
    if floor {
        actions.retain(|a| !is_jump(a));
        return actions;
    }
    while actions.len() as u16 + 2 > SHEET_MAX_H {
        match actions.iter().rposition(is_jump) {
            Some(i) => {
                actions.remove(i);
            }
            None => break,
        }
    }
    actions
}

pub fn action_sheet(detail: Rect, items: u16) -> Rect {
    let h = (items + 2).min(SHEET_MAX_H).min(detail.height);
    Rect {
        x: detail.x,
        y: detail.bottom().saturating_sub(h),
        width: detail.width,
        height: h,
    }
}

pub fn confirm_modal(frame: Rect) -> Rect {
    centered(frame, CONFIRM_W, CONFIRM_H)
}

pub fn help_modal(frame: Rect, content_rows: u16) -> Rect {
    centered(frame, HELP_W, content_rows.saturating_add(2))
}

pub fn help_inner_width(frame: Rect) -> u16 {
    HELP_W.min(frame.width).saturating_sub(2)
}

pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn stacked_rail_height(body_h: u16, active_rows: u16) -> u16 {
    let inactive = 2u16;
    let active_h = (active_rows + 2).clamp(4, 8);
    let want = inactive + active_h;
    let spare = (body_h / 2).max(6).min(body_h.saturating_sub(4));
    let least = RAIL_MIN_H.min(body_h.saturating_sub(1));
    want.clamp(least, spare.max(least))
}

fn rail_slots(rail: Rect, facts: LayoutFacts, tight: bool) -> [Rect; 3] {
    let panes = Pane::all();
    let collapsed_rows = if rail.height < RAIL_MIN_H { 0 } else { 1 };
    let constraints: Vec<Constraint> = panes
        .iter()
        .map(|&p| {
            if p == facts.active {
                Constraint::Fill(1)
            } else if tight {
                Constraint::Length(collapsed_rows)
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
        assert_eq!(p.slots[Pane::Images.index()].height, 1);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 1);
        assert_eq!(p.slots[Pane::Containers.index()].height, 7);
    }

    #[test]
    fn body_width_under_80_stacks_otherwise_beside() {
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

    fn running_actions() -> Vec<ActionItem> {
        let mut s = AppState::new(true);
        s.containers.push(crate::engine::state::ContainerEntry {
            id: "web".into(),
            image: "alpine:latest".into(),
            state: "running".into(),
            created: None,
            cpus: None,
            volumes: vec![],
            cpu_percent: None,
            mem_bytes: None,
            telemetry: std::collections::VecDeque::new(),
            pending: None,
        });
        s.clamp_selection();
        s.available_actions()
    }

    fn keys(items: &[ActionItem]) -> Vec<char> {
        items.iter().map(|i| i.key).collect()
    }

    #[test]
    fn the_floor_sheet_omits_the_detail_tab_jumps() {
        let items = sheet_items(running_actions(), true);
        assert_eq!(keys(&items), vec!['s', 'r', 'K', 'd', 'P', 'e']);
        assert!(items.len() as u16 + 2 <= SHEET_MAX_H);
    }

    #[test]
    fn off_the_floor_the_sheet_trims_only_what_does_not_fit() {
        let all = running_actions();
        assert_eq!(all.len(), 8);
        let items = sheet_items(all, false);
        assert_eq!(items.len() as u16 + 2, SHEET_MAX_H);
        assert!(
            keys(&items).contains(&'l'),
            "trim from the end: logs survives"
        );
        assert!(!keys(&items).contains(&'i'));
    }

    #[test]
    fn off_the_floor_a_sheet_that_fits_keeps_every_jump() {
        let mut s = AppState::new(true);
        s.containers.push(crate::engine::state::ContainerEntry {
            id: "old".into(),
            image: "alpine:latest".into(),
            state: "stopped".into(),
            created: None,
            cpus: None,
            volumes: vec![],
            cpu_percent: None,
            mem_bytes: None,
            telemetry: std::collections::VecDeque::new(),
            pending: None,
        });
        s.clamp_selection();
        let items = sheet_items(s.available_actions(), false);
        assert_eq!(keys(&items), vec!['s', 'd', 'P', 'i']);
    }

    #[test]
    fn action_sheet_hugs_the_detail_floor_and_never_the_rail() {
        let p = plan(55, 20);
        let sheet = super::action_sheet(p.detail, 6);
        assert_eq!(sheet.height, 8);
        assert_eq!(sheet.bottom(), p.detail.bottom());
        assert_eq!(sheet.x, p.detail.x);
        assert_eq!(sheet.width, p.detail.width);
        assert!(
            sheet.y >= p.rail.bottom(),
            "sheet {sheet:?} must not cover the rail {:?}",
            p.rail
        );
    }

    #[test]
    fn action_sheet_height_is_n_plus_2_capped_at_9() {
        let detail = Rect::new(0, 0, 60, 30);
        assert_eq!(super::action_sheet(detail, 1).height, 3);
        assert_eq!(super::action_sheet(detail, 6).height, 8);
        assert_eq!(super::action_sheet(detail, 7).height, SHEET_MAX_H);
        assert_eq!(super::action_sheet(detail, 40).height, SHEET_MAX_H);
    }

    #[test]
    fn action_sheet_never_outgrows_a_short_detail_pane() {
        let detail = Rect::new(0, 10, 60, 4);
        let sheet = super::action_sheet(detail, 8);
        assert_eq!(sheet.height, 4);
        assert_eq!(sheet.y, detail.y);
    }

    #[test]
    fn confirm_modal_is_a_fixed_7_row_box_centered_in_the_frame() {
        let a = super::confirm_modal(Rect::new(0, 0, 55, 20));
        assert_eq!((a.width, a.height), (CONFIRM_W, CONFIRM_H));
        let wide = super::confirm_modal(Rect::new(0, 0, 200, 50));
        assert_eq!((wide.width, wide.height), (CONFIRM_W, CONFIRM_H));
        assert_eq!(wide.x, (200 - CONFIRM_W) / 2);
        assert_eq!(wide.y, (50 - CONFIRM_H) / 2);
    }

    #[test]
    fn help_clamps_to_the_frame_and_is_full_screen_at_the_floor() {
        let floor = super::help_modal(Rect::new(0, 0, 55, 20), 24);
        assert_eq!((floor.width, floor.height), (55, 20));
        assert_eq!((floor.x, floor.y), (0, 0));
        let roomy = super::help_modal(Rect::new(0, 0, 200, 50), 21);
        assert_eq!((roomy.width, roomy.height), (HELP_W, 23));
    }

    #[test]
    fn tiny_frames_split_the_body_instead_of_panicking() {
        for h in 1..=40u16 {
            for w in [1, 2, 20, 40, 55, 59, 60, 61, 79, 80, 81, 100] {
                let p = plan(w, h);
                if p.stacked {
                    assert_eq!(
                        p.rail.height + p.detail.height,
                        p.body.height,
                        "{w}x{h}: rail {:?} + detail {:?} must tile body {:?}",
                        p.rail,
                        p.detail,
                        p.body
                    );
                } else {
                    assert_eq!(
                        p.rail.width + p.detail.width,
                        p.body.width,
                        "{w}x{h}: rail {:?} + detail {:?} must tile body {:?}",
                        p.rail,
                        p.detail,
                        p.body
                    );
                }
                assert!(
                    p.rail.y >= p.body.y && p.rail.bottom() <= p.body.bottom(),
                    "{w}x{h}: rail {:?} escapes body {:?}",
                    p.rail,
                    p.body
                );
                assert!(
                    p.detail.y >= p.body.y && p.detail.bottom() <= p.body.bottom(),
                    "{w}x{h}: detail {:?} escapes body {:?}",
                    p.detail,
                    p.body
                );
                if p.body.height >= 1 {
                    assert!(
                        p.detail.height >= 1,
                        "{w}x{h}: detail pane lost the body's last row: {:?}",
                        p.detail
                    );
                }
                if p.body.height >= 2 {
                    assert!(
                        p.rail.height >= 1,
                        "{w}x{h}: rail vanished from a {}-row body",
                        p.body.height
                    );
                }
                let slots: u16 = p.slots.iter().map(|s| s.height).sum();
                assert_eq!(
                    slots, p.rail.height,
                    "{w}x{h}: slots {:?} must tile rail {:?}",
                    p.slots, p.rail
                );
            }
        }
    }

    #[test]
    fn the_stacked_rail_leaves_the_detail_pane_a_row_on_a_short_body() {
        let p = plan(79, 11);
        assert!(p.stacked);
        assert_eq!(p.body.height, 9);
        assert!(p.rail.height > 0, "rail vanished: {:?}", p.rail);
        assert!(p.detail.height > 0, "detail vanished: {:?}", p.detail);
        assert_eq!(p.rail.bottom(), p.detail.y, "rail and detail must tile");
        assert_eq!(p.detail.bottom(), p.body.bottom());
    }

    #[test]
    fn a_rail_too_short_for_three_panes_keeps_the_active_one() {
        let p = plan(40, 5);
        assert!(p.stacked);
        assert_eq!(p.rail.height, 2);
        assert_eq!(p.detail.height, 1);
        assert_eq!(p.slots[Pane::Containers.index()].height, 2);
        assert_eq!(p.slots[Pane::Images.index()].height, 0);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 0);
    }

    #[test]
    fn the_stacked_rail_never_shrinks_as_the_body_grows() {
        let mut prev = 0;
        for h in 1..=60u16 {
            let p = plan(40, h);
            assert!(
                p.rail.height >= prev,
                "rail shrank from {prev} to {} at height {h}",
                p.rail.height
            );
            prev = p.rail.height;
        }
    }

    #[test]
    fn roomy_inactive_cap_is_max_8_or_height_over_4() {
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
