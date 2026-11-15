//! Property-based generated command sequence tests for VentaCore.
//!
//! Conforms to Stage 1 exit criteria:
//! - Generated random command sequences never leave an invalid timeline.
//! - An invalid command is rejected and changes nothing.

use proptest::prelude::*;
use ventacore_core::*;

// Strategy to generate arbitrary valid or invalid commands
fn arb_asset_id() -> impl Strategy<Value = AssetId> {
    (1u64..10u64).prop_map(AssetId)
}

fn arb_track_id() -> impl Strategy<Value = TrackId> {
    (1u64..5u64).prop_map(TrackId)
}

fn arb_item_id() -> impl Strategy<Value = ItemId> {
    (1u64..20u64).prop_map(ItemId)
}

fn arb_tick() -> impl Strategy<Value = Tick> {
    (0i64..50i64).prop_map(|s| Tick::from_seconds(s))
}

fn arb_duration() -> impl Strategy<Value = Tick> {
    (-2i64..10i64).prop_map(|s| Tick::from_seconds(s))
}

fn arb_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        (arb_asset_id(), "[a-z0-9_]{3,8}\\.mp4").prop_map(|(id, hint)| {
            Command::AddAsset(Asset::new(id, hint))
        }),
        arb_asset_id().prop_map(Command::RemoveAsset),
        (arb_track_id(), "[A-Za-z0-9 ]{3,10}").prop_map(|(id, name)| {
            Command::AddTrack(Track::new(id, name, TrackKind::Video))
        }),
        arb_track_id().prop_map(Command::RemoveTrack),
        (arb_track_id(), any::<bool>()).prop_map(|(track_id, locked)| {
            Command::SetTrackLock { track_id, locked }
        }),
        (
            arb_item_id(),
            arb_track_id(),
            arb_asset_id(),
            arb_tick(),
            arb_duration(),
            arb_tick()
        )
            .prop_map(|(id, track_id, asset_id, start, dur, src_in)| {
                Command::InsertItem(Item::new(id, track_id, asset_id, start, dur, src_in))
            }),
        arb_item_id().prop_map(Command::RemoveItem),
        (arb_item_id(), arb_tick()).prop_map(|(item_id, new_start)| {
            Command::MoveItem { item_id, new_start }
        }),
        (arb_item_id(), arb_tick(), arb_duration()).prop_map(
            |(item_id, new_source_in, new_duration)| {
                Command::TrimItem {
                    item_id,
                    new_source_in,
                    new_duration,
                }
            }
        ),
        (arb_item_id(), any::<bool>()).prop_map(|(item_id, pinned)| {
            Command::SetItemPin { item_id, pinned }
        }),
        (arb_item_id(), arb_tick(), arb_item_id()).prop_map(
            |(item_id, split_point, new_item_id)| {
                Command::SplitItem {
                    item_id,
                    split_point,
                    new_item_id,
                }
            }
        ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn test_random_command_sequences_never_leave_invalid_timeline(
        commands in prop::collection::vec(arb_command(), 1..30)
    ) {
        let mut project = Project::new("Fuzz Project");

        for cmd in &commands {
            let snapshot_before = project.clone();
            let result = project.apply_command(cmd);

            // Invariant 1: Project must ALWAYS be valid after command execution
            prop_assert!(
                project.validate().is_ok(),
                "Project left in invalid state after command {:?}: {:?}",
                cmd,
                project.validate()
            );

            // Invariant 2: An invalid command is rejected and changes nothing
            if result.is_err() {
                prop_assert_eq!(
                    &project,
                    &snapshot_before,
                    "Project state mutated by failing command {:?}",
                    cmd
                );
            }
        }
    }
}
