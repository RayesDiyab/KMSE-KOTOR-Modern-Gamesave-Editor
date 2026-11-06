//! Adding and removing feats and Force powers the way the Abilities tab's buttons do, in
//! memory, on the saves on the PC: a feat and a power added, then both removed, leave the
//! creature as it was, in both games (2026-11-06: in KotOR II nothing could be added or
//! removed, which turned out to be the list's layout; this keeps the edits themselves checked).
//! A test that needs a game or a save passes without checking anything where there is none,
//! and says so.

use kse_core::catalog::Catalog;
use kse_core::creature;
use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::{Save, Who};

fn round_trip(game: Game) {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == game) else {
        return eprintln!("{:?} is not installed here: nothing checked", game);
    };
    let data = GameData::open(game, &install.path);
    let cat = Catalog::build(&data);
    let force = |k: i64| cat.is_force_class(k);
    let mut checked = 0;
    for folder in find_saves(&installs, game, &[]).into_iter().take(6) {
        let Ok(mut save) = Save::open(&folder.path) else { continue };
        let Some(pc) = save.creature(Who::Player).cloned() else { continue };
        let (feats, powers) = (creature::feats(&pc), creature::powers(&pc));
        let Some(feat) = cat.feats.iter().map(|e| e.id).find(|id| !feats.contains(id)) else { continue };
        let power = cat.powers.iter().map(|e| e.id).find(|id| !powers.contains(id));
        let can_power = creature::can_have_powers(&pc, &force);
        save.edit(Who::Player, &format!("feat{}", feat), |c| {
            assert!(creature::add_feat(c, feat));
        });
        if let (Some(p), true) = (power, can_power) {
            save.edit(Who::Player, &format!("power{}", p), |c| {
                assert!(creature::add_power(c, p, &force));
            });
            let now = save.creature(Who::Player).unwrap();
            assert_eq!(creature::powers(now).len(), powers.len() + 1, "{}: the power was not added", folder.path.display());
        }
        let now = save.creature(Who::Player).unwrap();
        assert_eq!(creature::feats(now).len(), feats.len() + 1, "{}: the feat was not added", folder.path.display());
        save.edit(Who::Player, &format!("feat{}", feat), |c| {
            assert!(creature::remove_feat(c, feat));
        });
        if let (Some(p), true) = (power, can_power) {
            save.edit(Who::Player, &format!("power{}", p), |c| {
                assert!(creature::remove_power(c, p));
            });
        }
        let after = save.creature(Who::Player).unwrap();
        assert_eq!(creature::feats(after), feats, "{}: feats not as before", folder.path.display());
        assert_eq!(creature::powers(after), powers, "{}: powers not as before", folder.path.display());
        checked += 1;
    }
    if checked == 0 {
        eprintln!("no {:?} save here: nothing checked", game);
    }
}

#[test]
fn kotor_feats_and_powers_add_and_remove() {
    round_trip(Game::K1);
}

#[test]
fn kotor_ii_feats_and_powers_add_and_remove() {
    round_trip(Game::K2);
}
