use ratatui::layout::{Constraint, Layout, Rect};

use crate::config::LayoutMode;
use crate::engine::state::{ActionItem, AppState, Pane};

pub const STACK_BELOW: u16 = 80;
/// Rail column *including* the rule that separates it from the detail pane.
pub const RAIL_MAX: u16 = 36;
pub const TIGHT_RAIL_H: u16 = 16;
const FLOOR_H: u16 = 22;
const FLOOR_W: u16 = 60;
const RAIL_MIN_H: u16 = Pane::all().len() as u16;
const FOOTER_MIN_RAIL_H: u16 = 8;
/// Column header plus its rule plus the gap above the detail pane.
const TABLE_CHROME: u16 = 3;
pub const SHEET_MAX_H: u16 = 9;
pub const CONFIRM_W: u16 = 48;
pub const CONFIRM_H: u16 = 7;
pub const HELP_W: u16 = 72;
pub const SETTINGS_W: u16 = 52;

#[derive(Clone, Copy, Debug)]
pub struct LayoutFacts {
    pub zoom: bool,
    pub active: Pane,
    pub visible: [u16; Pane::COUNT],
    pub banner_rows: u16,
    pub mode: LayoutMode,
}

impl LayoutFacts {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            zoom: state.zoom,
            active: state.pane,
            visible: Pane::all().map(|p| state.visible_rows_for(p).len() as u16),
            banner_rows: 0,
            mode: state.layout(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutPlan {
    pub header: Rect,
    pub banners: Rect,
    pub body: Rect,
    pub bottom: Rect,
    pub list: Rect,
    /// The single rule between list and detail. Zero-sized when there is none.
    pub divider: Rect,
    pub detail: Rect,
    pub slots: [Rect; Pane::COUNT],
    /// The rail's last row, holding the reclaimable/prune line. May be empty.
    pub footer: Rect,
    pub stacked: bool,
    pub tight: bool,
    pub floor: bool,
    pub zoom: bool,
    pub mode: LayoutMode,
}

fn empty_at(r: Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y,
        width: 0,
        height: 0,
    }
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

        let base = Self {
            header,
            banners,
            body,
            bottom,
            list: body,
            divider: empty_at(body),
            detail: body,
            slots: [empty_at(body); Pane::COUNT],
            footer: empty_at(body),
            stacked: false,
            tight: floor,
            floor,
            zoom: false,
            mode: facts.mode,
        };

        if facts.zoom {
            let mut slots = [empty_at(body); Pane::COUNT];
            slots[0] = body;
            return Self {
                slots,
                zoom: true,
                ..base
            };
        }

        match facts.mode {
            LayoutMode::Table => Self::table(base, facts),
            LayoutMode::Rail => Self::rail(base, facts),
        }
    }

    fn table(base: Self, facts: LayoutFacts) -> Self {
        let body = base.body;
        let rows = facts.visible[facts.active.index()];
        let table_h = table_height(body.height, rows, base.floor);
        let parts =
            Layout::vertical([Constraint::Length(table_h), Constraint::Fill(1)]).split(body);
        let mut slots = [empty_at(body); Pane::COUNT];
        slots[facts.active.index()] = parts[0];
        Self {
            list: parts[0],
            detail: parts[1],
            slots,
            stacked: true,
            tight: base.floor,
            ..base
        }
    }

    fn rail(base: Self, facts: LayoutFacts) -> Self {
        let body = base.body;
        let stacked = body.width < STACK_BELOW;
        let tight = stacked || body.height < TIGHT_RAIL_H;

        let (rail, divider, detail) = if stacked {
            let rail_h = stacked_rail_height(body.height, facts.visible[facts.active.index()]);
            if body.height >= rail_h + 2 {
                let parts = Layout::vertical([
                    Constraint::Length(rail_h),
                    Constraint::Length(1),
                    Constraint::Fill(1),
                ])
                .split(body);
                (parts[0], parts[1], parts[2])
            } else {
                let parts =
                    Layout::vertical([Constraint::Length(rail_h), Constraint::Fill(1)]).split(body);
                (parts[0], empty_at(parts[1]), parts[1])
            }
        } else {
            let parts = Layout::horizontal([
                Constraint::Length(RAIL_MAX.saturating_sub(1)),
                Constraint::Length(1),
                Constraint::Min(12),
            ])
            .split(body);
            (parts[0], parts[1], parts[2])
        };

        let footer_h = u16::from(!tight && rail.height >= FOOTER_MIN_RAIL_H);
        let footer = Rect {
            x: rail.x,
            y: rail.bottom().saturating_sub(footer_h),
            width: rail.width,
            height: footer_h,
        };
        let sections = Rect {
            height: rail.height - footer_h,
            ..rail
        };

        Self {
            list: rail,
            divider,
            detail,
            slots: rail_slots(sections, facts, tight),
            footer,
            stacked,
            tight,
            ..base
        }
    }
}

/// How many rows the table block gets, gap row included.
///
/// Half the body is the ceiling: the point of this layout is that the detail
/// pane gets every column *and* most of the rows.
fn table_height(body_h: u16, rows: u16, floor: bool) -> u16 {
    if body_h <= 1 {
        return 0;
    }
    let chrome = if floor { 1 } else { TABLE_CHROME };
    let want = rows.saturating_add(chrome);
    let cap = body_h.saturating_sub(1).min((body_h / 2).max(4));
    want.clamp(1, cap.max(1))
}

pub fn sheet_items(mut actions: Vec<ActionItem>, floor: bool) -> Vec<ActionItem> {
    if floor {
        actions.retain(|a| !a.is_tab_jump());
        return actions;
    }
    while actions.len() as u16 + 2 > SHEET_MAX_H {
        match actions.iter().rposition(ActionItem::is_tab_jump) {
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

pub fn settings_modal(frame: Rect, content_rows: u16) -> Rect {
    centered(frame, SETTINGS_W, content_rows.saturating_add(2))
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
    let inactive = Pane::COUNT as u16 - 1;
    let active_h = (active_rows + 2).clamp(4, 8);
    let want = inactive + active_h;
    let spare = (body_h / 2).max(6).min(body_h.saturating_sub(4));
    let least = RAIL_MIN_H.min(body_h.saturating_sub(1));
    want.clamp(least, spare.max(least))
}

/// A roomy section wants a label row, its rows, and a blank row after it.
fn section_want(visible: u16) -> u16 {
    2 + visible.max(1)
}

fn rail_slots(rail: Rect, facts: LayoutFacts, tight: bool) -> [Rect; Pane::COUNT] {
    let panes = Pane::all();
    let collapsed_rows = if rail.height < RAIL_MIN_H { 0 } else { 1 };
    let wants: [u16; Pane::COUNT] = panes.map(|p| section_want(facts.visible[p.index()]));
    let total: u16 = wants.iter().sum();
    let fits = !tight && total <= rail.height;

    let constraints: Vec<Constraint> = panes
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            if tight {
                if p == facts.active {
                    Constraint::Fill(1)
                } else {
                    Constraint::Length(collapsed_rows)
                }
            } else if fits {
                if i == Pane::COUNT - 1 {
                    Constraint::Min(wants[i])
                } else {
                    Constraint::Length(wants[i])
                }
            } else if p == facts.active {
                Constraint::Fill(1)
            } else {
                let cap = (rail.height / 4).max(4);
                Constraint::Length(wants[i].clamp(2, cap))
            }
        })
        .collect();
    let split = Layout::vertical(constraints).split(rail);
    std::array::from_fn(|i| split.get(i).copied().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> LayoutFacts {
        LayoutFacts {
            zoom: false,
            active: Pane::Containers,
            visible: [8, 5, 3, 2],
            banner_rows: 0,
            mode: LayoutMode::Rail,
        }
    }

    fn plan(w: u16, h: u16) -> LayoutPlan {
        LayoutPlan::compute(Rect::new(0, 0, w, h), facts())
    }

    fn table_plan(w: u16, h: u16) -> LayoutPlan {
        LayoutPlan::compute(
            Rect::new(0, 0, w, h),
            LayoutFacts {
                mode: LayoutMode::Table,
                ..facts()
            },
        )
    }

    fn tiles(p: &LayoutPlan) -> u16 {
        if p.stacked {
            p.list.height + p.divider.height + p.detail.height
        } else {
            p.list.width + p.divider.width + p.detail.width
        }
    }

    #[test]
    fn floor_55x20_stacks_the_rail_above_detail() {
        let p = plan(55, 20);
        assert!(p.stacked);
        assert!(p.tight);
        assert!(p.floor);
        assert_eq!(p.header.height, 1);
        assert_eq!(p.list.width, 55);
        assert_eq!(p.list.height, 9);
        assert_eq!(p.detail.width, 55);
        assert_eq!(p.detail.height, 8);
        assert_eq!(p.divider.height, 1, "a rule separates the stack");
        assert!(p.list.y < p.detail.y);
        assert_eq!(p.list.x, p.detail.x);
        assert_eq!(p.slots[Pane::Images.index()].height, 1);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 1);
        assert_eq!(p.slots[Pane::Networks.index()].height, 1);
        assert_eq!(p.slots[Pane::Containers.index()].height, 6);
        assert_eq!(p.footer.height, 0, "no prune footer at the floor");
    }

    #[test]
    fn body_width_under_80_stacks_otherwise_beside() {
        let stacked = plan(79, 30);
        assert!(stacked.stacked, "79-col body must stack");
        let beside = plan(80, 30);
        assert!(!beside.stacked, "80-col body sits beside");
        assert_eq!(beside.list.width, 35);
        assert_eq!(beside.divider.width, 1);
        assert_eq!(beside.list.width + beside.divider.width, RAIL_MAX);
    }

    #[test]
    fn medium_100x30_sits_beside_capped_at_36() {
        let p = plan(100, 30);
        assert!(!p.stacked);
        assert!(!p.tight);
        assert!(!p.floor);
        assert_eq!(p.header.height, 2);
        assert_eq!(p.list.width + p.divider.width, RAIL_MAX);
        assert_eq!(p.list.height, 27);
        assert_eq!(p.detail.width, 64);
        assert_eq!(p.detail.height, 27);
        assert_eq!(p.list.y, p.detail.y);
        assert!(p.list.x < p.detail.x);
    }

    #[test]
    fn a_roomy_rail_shrinks_every_section_and_pools_the_slack_at_the_bottom() {
        let p = plan(100, 30);
        assert_eq!(p.footer.height, 1);
        assert_eq!(p.footer.bottom(), p.list.bottom());
        assert_eq!(p.slots[Pane::Containers.index()].height, 10);
        assert_eq!(p.slots[Pane::Images.index()].height, 7);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 5);
        assert_eq!(p.slots[Pane::Networks.index()].height, 4);

        let tall = LayoutPlan::compute(Rect::new(0, 0, 100, 50), facts());
        assert_eq!(
            tall.slots[Pane::Containers.index()].height,
            10,
            "the active section does not swell to fill a tall rail"
        );
        assert!(
            tall.slots[Pane::Networks.index()].height > 4,
            "the last section absorbs the slack so it pools at the bottom"
        );
    }

    #[test]
    fn a_rail_too_short_for_every_section_gives_the_active_one_the_room() {
        let f = LayoutFacts {
            visible: [40, 40, 40, 40],
            ..facts()
        };
        let p = LayoutPlan::compute(Rect::new(0, 0, 100, 40), f);
        let cap = (p.list.height - 1) / 4;
        for pane in [Pane::Images, Pane::Volumes, Pane::Networks] {
            assert_eq!(p.slots[pane.index()].height, cap.max(4));
        }
        assert!(
            p.slots[Pane::Containers.index()].height >= cap.max(4),
            "the active section takes what the capped ones leave"
        );
        let total: u16 = p.slots.iter().map(|s| s.height).sum();
        assert_eq!(total, p.list.height - p.footer.height);
    }

    #[test]
    fn wide_200x50_keeps_the_rail_cap_spare_goes_to_detail() {
        let p = plan(200, 50);
        assert!(!p.stacked);
        assert_eq!(p.list.width + p.divider.width, RAIL_MAX);
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
        assert_eq!(p.slots[Pane::Networks.index()].height, 1);
    }

    #[test]
    fn zoom_gives_the_whole_body_to_one_side_in_either_mode() {
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            let f = LayoutFacts {
                zoom: true,
                mode,
                ..facts()
            };
            let p = LayoutPlan::compute(Rect::new(0, 0, 100, 30), f);
            assert!(p.zoom);
            assert_eq!(p.list, p.body);
            assert_eq!(p.detail, p.body);
            assert_eq!(p.slots[0], p.body);
            assert_eq!(p.divider.height, 0);
        }
    }

    #[test]
    fn floor_when_height_leq_22_or_width_leq_60() {
        assert!(plan(100, 22).floor);
        assert!(plan(60, 30).floor);
        assert!(!plan(61, 23).floor);
    }

    #[test]
    fn the_table_layout_splits_horizontally_and_caps_the_table_at_half() {
        let p = table_plan(120, 40);
        assert_eq!(p.mode, LayoutMode::Table);
        assert!(p.stacked, "table mode is always a top/bottom split");
        assert_eq!(p.list.width, 120, "the table gets every column");
        assert_eq!(p.detail.width, 120, "so does the detail");
        assert_eq!(p.list.height, 8 + TABLE_CHROME);
        assert_eq!(p.list.height + p.detail.height, p.body.height);
        assert_eq!(p.slots[Pane::Containers.index()], p.list);
        assert_eq!(p.slots[Pane::Images.index()].height, 0);

        let many = LayoutPlan::compute(
            Rect::new(0, 0, 120, 40),
            LayoutFacts {
                mode: LayoutMode::Table,
                visible: [400, 0, 0, 0],
                ..facts()
            },
        );
        assert_eq!(
            many.list.height,
            many.body.height / 2,
            "a long list still leaves the detail half the body"
        );
    }

    #[test]
    fn the_table_layout_follows_the_active_pane() {
        let p = LayoutPlan::compute(
            Rect::new(0, 0, 120, 40),
            LayoutFacts {
                mode: LayoutMode::Table,
                active: Pane::Volumes,
                ..facts()
            },
        );
        assert_eq!(p.list.height, 3 + TABLE_CHROME);
        assert_eq!(p.slots[Pane::Volumes.index()], p.list);
    }

    #[test]
    fn tiny_frames_split_the_body_instead_of_panicking() {
        for mode in [LayoutMode::Rail, LayoutMode::Table] {
            for h in 1..=40u16 {
                for w in [1, 2, 20, 40, 55, 59, 60, 61, 79, 80, 81, 100] {
                    let p =
                        LayoutPlan::compute(Rect::new(0, 0, w, h), LayoutFacts { mode, ..facts() });
                    let span = if p.stacked {
                        p.body.height
                    } else {
                        p.body.width
                    };
                    assert_eq!(
                        tiles(&p),
                        span,
                        "{mode:?} {w}x{h}: rail {:?} + divider {:?} + detail {:?} must tile body {:?}",
                        p.list,
                        p.divider,
                        p.detail,
                        p.body
                    );
                    assert!(
                        p.list.y >= p.body.y && p.list.bottom() <= p.body.bottom(),
                        "{mode:?} {w}x{h}: rail {:?} escapes body {:?}",
                        p.list,
                        p.body
                    );
                    assert!(
                        p.detail.y >= p.body.y && p.detail.bottom() <= p.body.bottom(),
                        "{mode:?} {w}x{h}: detail {:?} escapes body {:?}",
                        p.detail,
                        p.body
                    );
                    if p.body.height >= 1 {
                        assert!(
                            p.detail.height >= 1,
                            "{mode:?} {w}x{h}: detail pane lost the body's last row: {:?}",
                            p.detail
                        );
                    }
                    if p.body.height >= 3 {
                        assert!(
                            p.list.height >= 1,
                            "{mode:?} {w}x{h}: rail vanished from a {}-row body",
                            p.body.height
                        );
                    }
                    assert!(
                        p.footer.height <= p.list.height,
                        "{mode:?} {w}x{h}: footer {:?} escapes rail {:?}",
                        p.footer,
                        p.list
                    );
                    let slots: u16 = p.slots.iter().map(|s| s.height).sum();
                    assert_eq!(
                        slots,
                        p.list.height - p.footer.height,
                        "{mode:?} {w}x{h}: slots {:?} must tile rail {:?} above footer {:?}",
                        p.slots,
                        p.list,
                        p.footer
                    );
                }
            }
        }
    }

    #[test]
    fn the_stacked_rail_leaves_the_detail_pane_a_row_on_a_short_body() {
        let p = plan(79, 11);
        assert!(p.stacked);
        assert_eq!(p.body.height, 9);
        assert!(p.list.height > 0, "rail vanished: {:?}", p.list);
        assert!(p.detail.height > 0, "detail vanished: {:?}", p.detail);
        assert_eq!(p.detail.bottom(), p.body.bottom());
    }

    #[test]
    fn a_rail_too_short_for_four_panes_keeps_the_active_one() {
        let p = plan(40, 5);
        assert!(p.stacked);
        assert_eq!(p.list.height, 2);
        assert_eq!(p.slots[Pane::Containers.index()].height, 2);
        assert_eq!(p.slots[Pane::Images.index()].height, 0);
        assert_eq!(p.slots[Pane::Volumes.index()].height, 0);
        assert_eq!(p.slots[Pane::Networks.index()].height, 0);
    }

    #[test]
    fn the_stacked_rail_never_shrinks_as_the_body_grows() {
        let mut prev = 0;
        for h in 1..=60u16 {
            let p = plan(40, h);
            assert!(
                p.list.height >= prev,
                "rail shrank from {prev} to {} at height {h}",
                p.list.height
            );
            prev = p.list.height;
        }
    }

    fn running_actions() -> Vec<ActionItem> {
        let mut s = AppState::new(true);
        s.containers.push(crate::engine::state::ContainerEntry {
            id: "web".into(),
            image: "alpine:latest".into(),
            state: "running".into(),
            created: None,
            started: None,
            cpus: None,
            mem_limit: None,
            volumes: vec![],
            networks: vec![],
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
            started: None,
            cpus: None,
            mem_limit: None,
            volumes: vec![],
            networks: vec![],
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
            sheet.y >= p.list.bottom(),
            "sheet {sheet:?} must not cover the rail {:?}",
            p.list
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
    fn settings_clamps_to_the_frame_too() {
        let floor = super::settings_modal(Rect::new(0, 0, 40, 8), 40);
        assert_eq!(
            (floor.width, floor.height),
            (40, 8),
            "a frame narrower than the panel clamps it, it does not overflow"
        );
        let roomy = super::settings_modal(Rect::new(0, 0, 200, 50), 10);
        assert_eq!((roomy.width, roomy.height), (SETTINGS_W, 12));
        assert!(
            SETTINGS_W as usize >= " saved to ~/.config/bushel/config.toml".len() + 2,
            "the panel must hold its longest row without eliding"
        );
    }
}
