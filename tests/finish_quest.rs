//! Finish Quest as if played (2026-11-06): on the saves on the PC, in memory, every quest
//! under way finished at each of its endings on both sides is prepared without error, the
//! prepared save has the quest at that ending, the player's XP never goes down, and the save it
//! was prepared from is untouched. A test that needs a game or a save passes without checking
//! anything where there is none, and says so.

use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, find_saves, Game};
use kse_core::save::Save;
use kse_core::story_preview::Plan;

fn finish_all(game: Game) {
    let installs = find_installs(&[]);
    let Some(install) = installs.iter().find(|i| i.game == game) else {
        return eprintln!("{:?} is not installed here: nothing checked", game);
    };
    let data = GameData::open(game, &install.path);
    let journal = kse_core::quests::all(&data);
    let mut checked = 0;
    for folder in find_saves(&installs, game, &[]).into_iter().take(3) {
        let Ok(save) = Save::open(&folder.path) else { continue };
        let Some(pc) = save.player() else { continue };
        let xp = kse_core::creature::experience(pc);
        let quests = save.quests();
        for (tag, stage) in &quests {
            let Some(q) = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)) else { continue };
            if q.stage(*stage).map_or(true, |s| s.end) {
                continue;
            }
            for ending in q.endings() {
                for dark in [false, true] {
                    let plan = Plan::Quest { tag: tag.clone(), ending: ending.id, dark };
                    let prepared = plan.prepare(&save, &data).unwrap_or_else(|e| panic!("{} {} ending {}: {}", folder.path.display(), tag, ending.id, e));
                    let now = prepared.save.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, v)| v);
                    assert_eq!(now, Some(ending.id), "{} {}: not at the ending", folder.path.display(), tag);
                    let after = prepared.save.player().map(kse_core::creature::experience).unwrap_or(0);
                    assert!(after >= xp, "{} {} ending {}: XP went down ({} to {})", folder.path.display(), tag, ending.id, xp, after);
                    checked += 1;
                }
            }
        }
        // the save it was prepared from is as it was
        assert_eq!(save.quests(), quests, "{}: the original changed", folder.path.display());
        assert_eq!(save.player().map(kse_core::creature::experience), Some(xp));
    }
    if checked == 0 {
        eprintln!("no {:?} save with a quest under way here: nothing checked", game);
    } else {
        eprintln!("{:?}: {} finishes prepared", game, checked);
    }
}

#[test]
fn kotor_quests_finish_as_played() {
    finish_all(Game::K1);
}

#[test]
fn kotor_ii_quests_finish_as_played() {
    finish_all(Game::K2);
}
