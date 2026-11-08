//! Story rewinds must not leave removed companions active or mutate an inconsistent
//! character. Tests write only to temporary copies and use Save::write's backup.
use kse_core::{
    creature,
    formats::gff::Value,
    gamedata::GameData,
    games::{find_installs, find_saves, Game},
    questdb,
    save::{Save, Who},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[test]
fn rewind_alignment_is_neutral_by_default_and_honors_the_chosen_target() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some((mut save, work, data)) = copy_save(game, "rewind-alignment") else {
            continue;
        };
        save.edit(Who::Player, "test.alignment", |p| {
            creature::set_alignment(p, 79)
        });
        let all = questdb::milestones_ref(game);
        let first = all.first().unwrap().1.clone();
        let pending = save.clone();
        let mut neutral = save.clone();
        questdb::go_back(&mut neutral, &data, &[], &[first.clone()], false, None).unwrap();
        assert_eq!(neutral.player().map(creature::alignment), Some(50));
        for target in [0, 23, 50, 79, 100] {
            let plan = kse_core::story_preview::Plan::Back {
                kept: vec![],
                undo: vec![first.clone()],
                stage: None,
                alignment: target,
                unfinished: Vec::new(),
                keep_items: false,
            };
            let prepared = plan.prepare(&save, &data).unwrap();
            assert_eq!(
                prepared.save.player().map(creature::alignment),
                Some(target)
            );
        }
        if game == Game::K1 {
            let get = |name: &str| {
                all.iter()
                    .find(|(p, _)| p.eq_ignore_ascii_case(name))
                    .unwrap()
                    .1
                    .clone()
            };
            for target in [0, 50, 100] {
                let mut partial = save.clone();
                questdb::go_back_with_alignment(
                    &mut partial,
                    &data,
                    &[get("Endar Spire")],
                    &[get("Taris")],
                    false,
                    Some(("tar_selvenbounty".into(), 1)),
                    target,
                )
                .unwrap();
                assert_eq!(partial.player().map(creature::alignment), Some(target));
            }
        }
        for target in [-1, 101] {
            let mut invalid = save.clone();
            assert!(questdb::go_back_with_alignment(
                &mut invalid,
                &data,
                &[],
                &[first.clone()],
                false,
                None,
                target
            )
            .is_err());
            assert_eq!(invalid.player(), pending.player());
            assert_eq!(invalid.party, pending.party);
            assert_eq!(invalid.changes, pending.changes);
        }
        neutral
            .write(&work.with_file_name(format!(
                "{}-backups",
                work.file_name().unwrap().to_string_lossy()
            )))
            .unwrap();
        assert_eq!(
            Save::open(&work).unwrap().player().map(creature::alignment),
            Some(50)
        );
        assert_eq!(save.player(), pending.player());
        checked += 1;
    }
    eprintln!("Rewind alignment checked on {} games", checked);
}

fn copy_save(game: Game, label: &str) -> Option<(Save, PathBuf, GameData)> {
    let installs = find_installs(&[]);
    let install = installs.iter().find(|i| i.game == game)?;
    let folder = find_saves(&installs, game, &[]).into_iter().filter(|f| !kse_core::save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok()).next()?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let work = std::env::temp_dir()
        .join("kmse-tests")
        .join(format!("{}-{:?}-{}", label, game, unique));
    fs::create_dir_all(&work).unwrap();
    for e in fs::read_dir(&folder.path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        fs::copy(e.path(), work.join(e.file_name())).unwrap();
    }
    Some((
        Save::open(&work).unwrap(),
        work,
        GameData::open(game, &install.path),
    ))
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn removing_a_companion_updates_the_active_party_and_control() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some((mut save, work, _)) = copy_save(game, "party-removal-safety") else {
            continue;
        };
        let members: Vec<_> = save.npcs().into_iter().filter(|n| save.in_party(*n) || save.selectable(*n)).take(2).collect();
        if members.is_empty() {
            continue;
        }
        save.set_party_members(&members).unwrap();
        let removed = members[0];
        save.set_party_int("PT_CONTROLLED_NP", removed as i64);
        save.set_recruited(removed, false);
        assert!(!save.recruited(removed));
        assert!(!save.in_party(removed));
        assert_eq!(save.party_members(), members[1..]);
        assert_eq!(
            save.party_int("PT_NUM_MEMBERS"),
            Some((members.len() - 1) as i64)
        );
        assert_eq!(save.party_int("PT_CONTROLLED_NP"), Some(-1));
        save.write(&work.with_file_name(format!(
            "{}-backups",
            work.file_name().unwrap().to_string_lossy()
        )))
        .unwrap();
        let again = Save::open(&work).unwrap();
        assert!(!again.recruited(removed));
        assert!(!again.in_party(removed));
        assert_eq!(again.party_members(), members[1..]);
        assert_eq!(
            again.party_int("PT_NUM_MEMBERS"),
            Some((members.len() - 1) as i64)
        );
        assert_eq!(again.party_int("PT_CONTROLLED_NP"), Some(-1));
        checked += 1;
    }
    eprintln!("Party removal safety checked on {} games", checked);
}

#[test]
fn inconsistent_history_is_rejected_before_any_rewind_change() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some((mut save, work, data)) = copy_save(game, "rewind-history-safety") else {
            continue;
        };
        save.edit(Who::Player, "test.invalid-history", |p| {
            if let Some(r) = p.list_mut("LvlStatList").and_then(|r| r.first_mut()) {
                r.set("LvlStatClass", Value::Byte(250));
            }
        });
        assert!(!creature::level_records_match(save.player().unwrap()));
        let before = save.clone();
        let disk = files(&work);
        let first = questdb::milestones_ref(game).first().unwrap().1.clone();
        let error = questdb::go_back(&mut save, &data, &[], &[first], false, None).unwrap_err();
        assert!(error.contains("saved level history"));
        assert_eq!(save.party, before.party);
        assert_eq!(save.globals, before.globals);
        assert_eq!(save.nfo, before.nfo);
        assert_eq!(save.player(), before.player());
        assert_eq!(save.inventory(), before.inventory());
        assert_eq!(save.current_module_parts(), before.current_module_parts());
        assert_eq!(save.changes, before.changes);
        assert_eq!(files(&work), disk);
        checked += 1;
    }
    eprintln!("Unsafe rewind refusal checked on {} games", checked);
}

#[test]
fn rewind_reconstructs_earlier_classes_from_the_level_records() {
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let Some((mut save, work, data)) = copy_save(game, "rewind-recorded-class") else {
            continue;
        };
        let first_class = save
            .player()
            .and_then(|p| p.list("LvlStatList"))
            .and_then(|r| r.first())
            .and_then(|r| r.int("LvlStatClass"))
            .unwrap();
        save.edit(Who::Player, "test.changed-class", |p| {
            if let Some(c) = p.list_mut("ClassList").and_then(|c| c.first_mut()) {
                c.set("Class", Value::Int(250));
            }
        });
        assert!(!creature::level_records_match(save.player().unwrap()));
        let first = questdb::milestones_ref(game).first().unwrap().1.clone();
        questdb::go_back(&mut save, &data, &[], &[first], false, None).unwrap();
        let player = save.player().unwrap();
        assert_eq!(creature::level(player), 1);
        assert_eq!(creature::classes(player), vec![(first_class, 1)]);
        assert!(creature::level_records_match(player));
        save.write(&work.with_file_name(format!(
            "{}-backups",
            work.file_name().unwrap().to_string_lossy()
        )))
        .unwrap();
        let again = Save::open(&work).unwrap();
        assert_eq!(creature::level(again.player().unwrap()), 1);
        assert!(creature::level_records_match(again.player().unwrap()));
        checked += 1;
    }
    eprintln!("Recorded-class rewind checked on {} games", checked);
}
