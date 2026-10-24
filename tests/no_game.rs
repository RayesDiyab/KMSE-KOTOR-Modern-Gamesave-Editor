//! A save opened on a PC without the game (2026-10-24): the catalogue still has what is the
//! same in every copy of the game, and says it has no class rules, so the editor refuses
//! level and class changes instead of guessing them.

use kse_core::catalog::Catalog;
use kse_core::gamedata::GameData;
use kse_core::games::Game;

#[test]
fn kotor_without_its_files() {
    let data = GameData::none(Game::K1);
    assert!(!data.is_usable());
    let cat = Catalog::build(&data);
    assert_eq!(cat.skills.len(), 8);
    assert_eq!(cat.skills[3].name, "Awareness");
    assert_eq!(cat.class_name(3), "Jedi Guardian");
    assert_eq!(cat.max_level(), 20);
    assert_eq!(cat.xp_for(2), Some(1000));
    assert_eq!(cat.xp_for(20), Some(190_000));
    assert_eq!(cat.level_for(6000), 4);
    assert!(!cat.has_rules());
    assert!(!cat.is_force_class(3));
    assert_eq!(cat.npc_names[0], "Bastila");
}

#[test]
fn kotor_two_without_its_files() {
    let cat = Catalog::build(&GameData::none(Game::K2));
    assert_eq!(cat.max_level(), 50);
    assert_eq!(cat.xp_for(50), Some(1_225_000));
    assert_eq!(cat.class_name(16), "Sith Assassin");
    assert!(!cat.has_rules());
}
