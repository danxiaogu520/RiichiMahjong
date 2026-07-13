use riichi_core::game::{CallOption, CallType, ResponseAction};
use riichi_core::player::PlayerId;

/// AI 决策：有合法荣和就荣和，否则一律 Pass（不副露）。
pub fn decide_call(_player: PlayerId, options: &[CallOption]) -> Option<ResponseAction> {
    if options
        .iter()
        .any(|option| matches!(option.call_type, CallType::Ron))
    {
        Some(ResponseAction::Ron)
    } else {
        Some(ResponseAction::Pass)
    }
}

#[cfg(test)]
mod tests {
    use super::decide_call;
    use riichi_core::game::{CallOption, CallType, ResponseAction};
    use riichi_core::player::PlayerId;

    #[test]
    fn always_takes_ron_when_available() {
        let options = vec![CallOption {
            player: PlayerId(0),
            call_type: CallType::Ron,
        }];
        assert!(matches!(
            decide_call(PlayerId(0), &options),
            Some(ResponseAction::Ron)
        ));
    }

    #[test]
    fn passes_non_ron_calls() {
        let options = vec![CallOption {
            player: PlayerId(0),
            call_type: CallType::Pon {
                hand_tiles: [
                    riichi_core::tile::Tile::from_raw(0),
                    riichi_core::tile::Tile::from_raw(1),
                ],
            },
        }];
        assert!(matches!(
            decide_call(PlayerId(0), &options),
            Some(ResponseAction::Pass)
        ));
    }
}
