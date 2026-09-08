use std::collections::{BTreeMap, HashMap};

use super::state::{ActionKind, CONFIRM_TICKS, Pane, Pending, PendingPhase};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    pub pane: Pane,
    pub name: String,
}

impl Target {
    pub fn new(pane: Pane, name: impl Into<String>) -> Self {
        Self {
            pane,
            name: name.into(),
        }
    }
}

/// The destination and captured source identity for a tag action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagTarget {
    pub reference: String,
    pub digest: Option<String>,
}

/// A complete action, including the exact command arguments shown in its
/// confirmation preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlan {
    pub kind: ActionKind,
    pub target: Target,
    pub commands: Vec<Vec<String>>,
    pub tag: Option<TagTarget>,
}

impl ActionPlan {
    pub fn targets(&self) -> Vec<Target> {
        let mut targets = Vec::with_capacity(2);
        push_unique(&mut targets, self.target.clone());
        if let Some(tag) = &self.tag {
            push_unique(
                &mut targets,
                Target::new(Pane::Images, tag.reference.clone()),
            );
        }
        targets
    }

    pub fn command(&self) -> String {
        self.commands
            .iter()
            .map(|args| {
                let args = args.join(" ");
                if args.is_empty() {
                    "container".to_string()
                } else {
                    format!("container {args}")
                }
            })
            .collect::<Vec<_>>()
            .join(" && ")
    }
}

fn push_unique(targets: &mut Vec<Target>, target: Target) {
    if !targets.iter().any(|existing| existing == &target) {
        targets.push(target);
    }
}

pub type ActionId = u64;

/// One row from a successful list poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub name: String,
    pub state: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStatus {
    Confirmed,
    Failed,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub id: ActionId,
    pub plan: ActionPlan,
    pub status: OutcomeStatus,
}

/// The accepted completion of an action.  A successful command has no
/// outcome until a poll verifies it; a failed command has an immediate one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub plan: ActionPlan,
    pub outcome: Option<Outcome>,
}

#[derive(Debug, Clone)]
struct Attempt {
    plan: ActionPlan,
    phase: PendingPhase,
    completion_floor: Option<u64>,
    last_seen_sequence: Option<u64>,
}

/// Deterministic lifecycle and reservation state for individual-entity
/// actions.
///
/// The module has no I/O.  A caller records command completion with
/// [`Self::complete`], then supplies successful list-poll snapshots through
/// [`Self::observe`].  Reservations remain owned by this module until an outcome is
/// returned, even when a poll no longer contains the reserved entity.
#[derive(Debug)]
pub struct PendingActions {
    next_id: ActionId,
    attempts: BTreeMap<ActionId, Attempt>,
    reservations: HashMap<Target, ActionId>,
}

impl Default for PendingActions {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingActions {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            attempts: BTreeMap::new(),
            reservations: HashMap::new(),
        }
    }

    pub fn can_begin(&self, plan: &ActionPlan) -> Result<(), Target> {
        for target in plan.targets() {
            if self.reservations.contains_key(&target) {
                return Err(target);
            }
        }
        Ok(())
    }

    pub fn begin(&mut self, plan: ActionPlan) -> Result<ActionId, Target> {
        self.can_begin(&plan)?;

        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("action identity exhausted");
        for target in plan.targets() {
            self.reservations.insert(target, id);
        }
        self.attempts.insert(
            id,
            Attempt {
                plan,
                phase: PendingPhase::InFlight,
                completion_floor: None,
                last_seen_sequence: None,
            },
        );
        Ok(id)
    }

    /// Record command completion.  A successful command enters confirmation;
    /// its first eligible poll must have a sequence greater than
    /// `poll_floor`.  A failed command produces its failure immediately.
    pub fn complete(&mut self, id: ActionId, success: bool, poll_floor: u64) -> Option<Completion> {
        let attempt = self.attempts.get_mut(&id)?;
        match attempt.phase {
            PendingPhase::InFlight if success => {
                let plan = attempt.plan.clone();
                attempt.phase = PendingPhase::Confirming(CONFIRM_TICKS);
                attempt.completion_floor = Some(poll_floor);
                attempt.last_seen_sequence = None;
                Some(Completion {
                    plan,
                    outcome: None,
                })
            }
            PendingPhase::InFlight => {
                let outcome = self.finish(id, OutcomeStatus::Failed)?;
                Some(Completion {
                    plan: outcome.plan.clone(),
                    outcome: Some(outcome),
                })
            }
            PendingPhase::Confirming(_) => None,
        }
    }

    /// Apply one successful poll snapshot to all attempts for `pane`.
    ///
    /// Polls at or before command completion, and duplicate or out-of-order
    /// poll sequences for an attempt, are ignored.  A relevant unsuccessful
    /// snapshot consumes one confirmation allowance.  After the second such
    /// snapshot the action is released as `Unconfirmed`.
    pub fn observe(&mut self, pane: Pane, sequence: u64, rows: &[Observation]) -> Vec<Outcome> {
        let ids: Vec<ActionId> = self
            .attempts
            .iter()
            .filter_map(|(&id, attempt)| {
                attempt
                    .plan
                    .targets()
                    .iter()
                    .any(|target| target.pane == pane)
                    .then_some(id)
            })
            .collect();

        let mut outcomes = Vec::new();
        for id in ids {
            let Some(attempt) = self.attempts.get_mut(&id) else {
                continue;
            };
            let PendingPhase::Confirming(remaining) = attempt.phase else {
                continue;
            };
            let Some(floor) = attempt.completion_floor else {
                continue;
            };
            if sequence <= floor
                || attempt
                    .last_seen_sequence
                    .is_some_and(|last| sequence <= last)
            {
                continue;
            }

            attempt.last_seen_sequence = Some(sequence);
            let confirmed = confirms(&attempt.plan, rows);
            if confirmed {
                if let Some(outcome) = self.finish(id, OutcomeStatus::Confirmed) {
                    outcomes.push(outcome);
                }
            } else if remaining <= 1 {
                if let Some(outcome) = self.finish(id, OutcomeStatus::Unconfirmed) {
                    outcomes.push(outcome);
                }
            } else {
                attempt.phase = PendingPhase::Confirming(remaining - 1);
            }
        }
        outcomes
    }

    pub fn pending_for(&self, target: &Target) -> Option<Pending> {
        let id = self.reservations.get(target)?;
        let attempt = self.attempts.get(id)?;
        Some(Pending {
            kind: attempt.plan.kind,
            phase: attempt.phase,
        })
    }

    pub fn has_kind(&self, pane: Pane) -> bool {
        self.reservations.keys().any(|target| target.pane == pane)
    }

    fn finish(&mut self, id: ActionId, status: OutcomeStatus) -> Option<Outcome> {
        let attempt = self.attempts.remove(&id)?;
        for target in attempt.plan.targets() {
            self.reservations.remove(&target);
        }
        Some(Outcome {
            id,
            plan: attempt.plan,
            status,
        })
    }
}

fn confirms(plan: &ActionPlan, rows: &[Observation]) -> bool {
    match plan.kind {
        ActionKind::Start | ActionKind::Restart => {
            row_for(&plan.target, rows).and_then(|row| row.state.as_deref()) == Some("running")
        }
        ActionKind::Stop | ActionKind::Kill => {
            row_for(&plan.target, rows).and_then(|row| row.state.as_deref()) == Some("stopped")
        }
        ActionKind::DeleteContainer | ActionKind::DeleteImage | ActionKind::DeleteVolume => {
            row_for(&plan.target, rows).is_none()
        }
        ActionKind::CreateVolume => row_for(&plan.target, rows).is_some(),
        ActionKind::TagImage => {
            let Some(tag) = &plan.tag else { return false };
            let Some(digest) = tag.digest.as_deref().filter(|digest| !digest.is_empty()) else {
                return false;
            };
            rows.iter()
                .any(|row| row.name == tag.reference && row.digest.as_deref() == Some(digest))
        }
        ActionKind::PruneContainers | ActionKind::PruneImages | ActionKind::PruneVolumes => false,
    }
}

fn row_for<'a>(target: &Target, rows: &'a [Observation]) -> Option<&'a Observation> {
    rows.iter().find(|row| row.name == target.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(pane: Pane, name: &str) -> Target {
        Target::new(pane, name)
    }

    fn plan(kind: ActionKind, pane: Pane, name: &str) -> ActionPlan {
        ActionPlan {
            kind,
            target: target(pane, name),
            commands: vec![vec![kind.past_tense().to_string(), name.to_string()]],
            tag: None,
        }
    }

    fn row(name: &str, state: Option<&str>, digest: Option<&str>) -> Observation {
        Observation {
            name: name.to_string(),
            state: state.map(str::to_string),
            digest: digest.map(str::to_string),
        }
    }

    #[test]
    fn plan_renders_sequential_commands_and_deduplicates_tag_target() {
        let source = target(Pane::Images, "alpine:latest");
        let plan = ActionPlan {
            kind: ActionKind::TagImage,
            target: source.clone(),
            commands: vec![
                vec!["image".into(), "tag".into(), "alpine:latest".into()],
                vec!["image".into(), "inspect".into()],
            ],
            tag: Some(TagTarget {
                reference: "alpine:dev".into(),
                digest: Some("sha256:abc".into()),
            }),
        };

        assert_eq!(
            plan.command(),
            "container image tag alpine:latest && container image inspect"
        );
        assert_eq!(
            plan.targets(),
            vec![source, target(Pane::Images, "alpine:dev")]
        );

        let same = ActionPlan {
            tag: Some(TagTarget {
                reference: "alpine:latest".into(),
                digest: Some("sha256:abc".into()),
            }),
            ..plan
        };
        assert_eq!(same.targets(), vec![target(Pane::Images, "alpine:latest")]);
    }

    #[test]
    fn reservations_are_scoped_by_pane_and_conflicts_return_exact_target() {
        let mut pending = PendingActions::new();
        let _id = pending
            .begin(plan(ActionKind::DeleteContainer, Pane::Containers, "same"))
            .unwrap();
        assert!(pending.has_kind(Pane::Containers));
        assert!(
            pending
                .can_begin(&plan(ActionKind::DeleteImage, Pane::Images, "same"))
                .is_ok()
        );
        let conflict = pending
            .can_begin(&plan(ActionKind::Stop, Pane::Containers, "same"))
            .unwrap_err();
        assert_eq!(conflict, target(Pane::Containers, "same"));
        assert_eq!(
            pending.pending_for(&target(Pane::Containers, "same")),
            Some(Pending {
                kind: ActionKind::DeleteContainer,
                phase: PendingPhase::InFlight,
            })
        );
    }

    #[test]
    fn tag_reserves_source_and_destination_and_allows_distinct_targets() {
        let mut pending = PendingActions::default();
        let first = ActionPlan {
            kind: ActionKind::TagImage,
            target: target(Pane::Images, "source-a"),
            commands: vec![],
            tag: Some(TagTarget {
                reference: "dest-a".into(),
                digest: Some("sha256:a".into()),
            }),
        };
        let second = ActionPlan {
            kind: ActionKind::TagImage,
            target: target(Pane::Images, "source-b"),
            commands: vec![],
            tag: Some(TagTarget {
                reference: "dest-b".into(),
                digest: Some("sha256:b".into()),
            }),
        };
        let first_id = pending.begin(first.clone()).unwrap();
        let second_id = pending.begin(second.clone()).unwrap();
        assert_ne!(first_id, second_id);
        assert_eq!(
            pending
                .pending_for(&target(Pane::Images, "source-a"))
                .unwrap()
                .kind,
            ActionKind::TagImage
        );
        assert_eq!(
            pending
                .pending_for(&target(Pane::Images, "dest-a"))
                .unwrap()
                .kind,
            ActionKind::TagImage
        );

        let conflicting = ActionPlan {
            target: target(Pane::Images, "source-c"),
            tag: Some(TagTarget {
                reference: "dest-a".into(),
                digest: Some("sha256:c".into()),
            }),
            ..first
        };
        assert_eq!(
            pending.begin(conflicting),
            Err(target(Pane::Images, "dest-a"))
        );
    }

    #[test]
    fn stale_and_duplicate_completions_and_polls_do_not_change_attempt() {
        let mut pending = PendingActions::new();
        let id = pending
            .begin(plan(ActionKind::Start, Pane::Containers, "worker"))
            .unwrap();
        assert_eq!(pending.complete(id, true, 5).unwrap().outcome, None);
        assert_eq!(pending.complete(id, false, 5), None);
        assert_eq!(pending.complete(id, true, 5), None);
        assert_eq!(
            pending.pending_for(&target(Pane::Containers, "worker")),
            Some(Pending {
                kind: ActionKind::Start,
                phase: PendingPhase::Confirming(CONFIRM_TICKS),
            })
        );

        assert!(pending.observe(Pane::Images, 6, &[]).is_empty());
        assert!(
            pending
                .observe(Pane::Containers, 5, &[row("worker", Some("running"), None)])
                .is_empty()
        );
        assert!(
            pending
                .observe(Pane::Containers, 7, &[row("other", Some("running"), None)])
                .is_empty()
        );
        assert_eq!(
            pending.pending_for(&target(Pane::Containers, "worker")),
            Some(Pending {
                kind: ActionKind::Start,
                phase: PendingPhase::Confirming(1),
            })
        );
        assert!(
            pending
                .observe(Pane::Containers, 6, &[row("worker", Some("running"), None)])
                .is_empty()
        );
        let outcomes =
            pending.observe(Pane::Containers, 8, &[row("worker", Some("running"), None)]);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].id, id);
        assert_eq!(outcomes[0].status, OutcomeStatus::Confirmed);
        assert!(pending.complete(id, false, 8).is_none());
    }

    #[test]
    fn failed_commands_release_reservations_without_poll_confirmation() {
        let mut pending = PendingActions::new();
        let id = pending
            .begin(plan(ActionKind::Stop, Pane::Containers, "worker"))
            .unwrap();
        let completion = pending.complete(id, false, 0).unwrap();
        let outcome = completion.outcome.unwrap();
        assert_eq!(outcome.status, OutcomeStatus::Failed);
        assert_eq!(completion.plan.kind, ActionKind::Stop);
        assert!(!pending.has_kind(Pane::Containers));
        assert!(
            pending
                .pending_for(&target(Pane::Containers, "worker"))
                .is_none()
        );
    }

    #[test]
    fn action_specific_evidence_and_two_poll_expiry() {
        let mut pending = PendingActions::new();
        let stop = pending
            .begin(plan(ActionKind::Stop, Pane::Containers, "worker"))
            .unwrap();
        pending.complete(stop, true, 10);
        assert!(
            pending
                .observe(
                    Pane::Containers,
                    11,
                    &[row("worker", Some("running"), None)]
                )
                .is_empty()
        );
        let outcome = pending.observe(
            Pane::Containers,
            12,
            &[row("worker", Some("running"), None)],
        );
        assert_eq!(outcome[0].status, OutcomeStatus::Unconfirmed);

        let delete = pending
            .begin(plan(ActionKind::DeleteImage, Pane::Images, "old"))
            .unwrap();
        pending.complete(delete, true, 20);
        let outcome = pending.observe(Pane::Images, 21, &[]);
        assert_eq!(outcome[0].status, OutcomeStatus::Confirmed);

        let volume = pending
            .begin(plan(ActionKind::CreateVolume, Pane::Volumes, "new"))
            .unwrap();
        pending.complete(volume, true, 30);
        assert!(pending.observe(Pane::Volumes, 31, &[]).is_empty());
        let outcome = pending.observe(Pane::Volumes, 32, &[row("new", None, None)]);
        assert_eq!(outcome[0].status, OutcomeStatus::Confirmed);
    }

    #[test]
    fn tag_requires_captured_nonempty_digest_and_matching_destination() {
        let mut pending = PendingActions::new();
        let id = pending
            .begin(ActionPlan {
                kind: ActionKind::TagImage,
                target: target(Pane::Images, "source"),
                commands: vec![],
                tag: Some(TagTarget {
                    reference: "dest".into(),
                    digest: Some("sha256:source".into()),
                }),
            })
            .unwrap();
        pending.complete(id, true, 40);
        assert!(
            pending
                .observe(Pane::Images, 41, &[row("dest", None, None)])
                .is_empty()
        );
        let outcome = pending.observe(Pane::Images, 42, &[row("dest", None, Some("sha256:other"))]);
        assert_eq!(outcome[0].status, OutcomeStatus::Unconfirmed);
        assert!(
            pending
                .pending_for(&target(Pane::Images, "source"))
                .is_none()
        );

        let id = pending
            .begin(ActionPlan {
                kind: ActionKind::TagImage,
                target: target(Pane::Images, "source"),
                commands: vec![],
                tag: Some(TagTarget {
                    reference: "dest".into(),
                    digest: None,
                }),
            })
            .unwrap();
        pending.complete(id, true, 50);
        assert!(
            pending
                .observe(
                    Pane::Images,
                    51,
                    &[row("dest", None, Some("sha256:source"))]
                )
                .is_empty()
        );
        let outcome = pending.observe(
            Pane::Images,
            52,
            &[row("dest", None, Some("sha256:source"))],
        );
        assert_eq!(outcome[0].status, OutcomeStatus::Unconfirmed);
    }
    #[test]
    fn a_retired_completion_cannot_release_a_new_action_on_the_same_entity() {
        let mut pending = PendingActions::new();
        let old = pending
            .begin(plan(ActionKind::Stop, Pane::Containers, "worker"))
            .unwrap();
        pending.complete(old, false, 1).unwrap();
        let current = pending
            .begin(plan(ActionKind::Restart, Pane::Containers, "worker"))
            .unwrap();
        assert_ne!(old, current);
        assert!(pending.complete(old, true, 2).is_none());
        assert_eq!(
            pending.pending_for(&target(Pane::Containers, "worker")),
            Some(Pending {
                kind: ActionKind::Restart,
                phase: PendingPhase::InFlight,
            })
        );
        pending.complete(current, true, 3).unwrap();
        let outcomes =
            pending.observe(Pane::Containers, 4, &[row("worker", Some("running"), None)]);
        assert_eq!(outcomes[0].id, current);
        assert_eq!(outcomes[0].status, OutcomeStatus::Confirmed);
    }

    #[test]
    fn container_state_is_evidence_only_after_the_command_succeeds() {
        for (kind, expected) in [
            (ActionKind::Start, "running"),
            (ActionKind::Restart, "running"),
            (ActionKind::Stop, "stopped"),
            (ActionKind::Kill, "stopped"),
        ] {
            let mut pending = PendingActions::new();
            let id = pending
                .begin(plan(kind, Pane::Containers, "worker"))
                .unwrap();
            let rows = [row("worker", Some(expected), None)];
            assert!(pending.observe(Pane::Containers, 1, &rows).is_empty());
            pending.complete(id, true, 1).unwrap();
            assert!(
                pending.observe(Pane::Containers, 2, &[]).is_empty(),
                "disappearance cannot prove {kind:?}"
            );
            assert!(
                pending.observe(Pane::Containers, 2, &rows).is_empty(),
                "duplicate polls cannot supply new evidence"
            );
            let outcomes = pending.observe(Pane::Containers, 3, &rows);
            assert_eq!(outcomes[0].status, OutcomeStatus::Confirmed);
        }
    }
}
