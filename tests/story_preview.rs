//! Preparing a timeline change uses pending edits, agrees with the story engine and
//! never writes the save. Real saves are read only to make isolated test copies.
use kse_core::{
    creature,
    gamedata::GameData,
    games::{find_installs, find_saves, Game},
    questdb,
    save::{Save, Who},
    story_preview::{accessible_items, companion_name, inventory_changes, Plan},
};
use std::{fs, path::Path};

fn files(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<_> = fs::read_dir(folder)
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
    out.sort();
    out
}

fn same_result(a: &Save, b: &Save) {
    assert_eq!(a.nfo, b.nfo);
    assert_eq!(a.party, b.party);
    assert_eq!(a.globals, b.globals);
    assert_eq!(a.player(), b.player());
    assert_eq!(a.inventory(), b.inventory());
    assert_eq!(a.changes, b.changes);
}

#[test]
fn preview_matches_apply_and_preserves_pending_edits_and_files() {
    let installs = find_installs(&[]);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut checked = 0;
    for game in [Game::K1, Game::K2] {
        let (Some(install), Some(folder)) = (
            installs.iter().find(|i| i.game == game),
            find_saves(&installs, game, &[]).into_iter().filter(|f| !kse_core::save::player_outside_archive(&f.path) && kse_core::save::Save::open(&f.path).is_ok()).find(|f| {
                Save::open(&f.path)
                    .ok()
                    .and_then(|s| s.player().cloned())
                    .map_or(false, |p| {
                        creature::uses_level_records(&p) && creature::level_records_match(&p)
                    })
            }),
        ) else {
            continue;
        };
        let work = std::env::temp_dir()
            .join("kmse-tests")
            .join(format!("story-preview-{}-{:?}", unique, game));
        fs::create_dir_all(&work).unwrap();
        for e in fs::read_dir(&folder.path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            fs::copy(e.path(), work.join(e.file_name())).unwrap();
        }
        let disk = files(&work);
        let data = GameData::open(game, &install.path);
        let mut save = Save::open(&work).unwrap();
        let xp = save.player().map(creature::experience).unwrap_or(0) + 137;
        save.edit(Who::Player, "preview.pending", |c| {
            creature::set_experience(c, xp)
        });
        save.set_party_int("PT_GOLD", save.party_int("PT_GOLD").unwrap_or(0) + 321);
        let pending = save.clone();
        if let Some(i) = save.npcs().first().copied() {
            let mut named = save.clone();
            named.edit(Who::Npc(i), "preview.name", |c| {
                creature::set_name(c, "Prepared companion", 0)
            });
            assert_eq!(
                companion_name(&named, &data, i, "Fallback"),
                "Prepared companion"
            );
            assert_ne!(
                companion_name(&save, &data, i, "Fallback"),
                "Prepared companion"
            );
        }
        let chapter = questdb::milestones_ref(game)
            .iter()
            .filter(|(_, m)| m.get("side_chapter").and_then(|v| v.as_bool()) != Some(true))
            .min_by_key(|(_, m)| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99))
            .unwrap()
            .1
            .clone();
        let chapters = vec![chapter];
        let skip = Plan::Skip {
            chapters: chapters.clone(),
            stage: None,
            class: None,
            dark: false,
            prestige: None,
            train: false,
        };
        let prepared = skip.prepare(&save, &data).unwrap();
        let mut expected = save.clone();
        let notes =
            questdb::skip_through_with(&mut expected, &data, &chapters, None, false, None, false)
                .unwrap();
        same_result(&prepared.save, &expected);
        assert_eq!(prepared.notes, notes);
        same_result(&save, &pending);
        assert_eq!(save.player().map(creature::experience), Some(xp));
        assert_eq!(files(&work), disk);

        if game == Game::K1 {
            let all = questdb::milestones_ref(game);
            let get = |name: &str| {
                all.iter()
                    .find(|(p, _)| p.eq_ignore_ascii_case(name))
                    .unwrap()
                    .1
                    .clone()
            };
            let partial_chapters = vec![get("Endar Spire"), get("Taris")];
            let stage = ("tar_escape".to_string(), 30);
            let plan = Plan::Skip {
                chapters: partial_chapters.clone(),
                stage: Some(stage.clone()),
                class: None,
                dark: false,
                prestige: None,
                train: false,
            };
            let partial = plan.prepare(&save, &data).unwrap();
            let mut expected = save.clone();
            questdb::skip_to_stage(
                &mut expected,
                &data,
                &partial_chapters,
                &stage.0,
                stage.1,
                None,
                false,
                None,
                false,
            )
            .unwrap();
            same_result(&partial.save, &expected);
            let plan = Plan::Back {
                alignment: 50,
                unfinished: Vec::new(),
                kept: vec![get("Endar Spire")],
                undo: vec![get("Taris")],
                stage: Some(stage.clone()),
                keep_items: false,
            };
            let partial_back = plan.prepare(&partial.save, &data).unwrap();
            let mut expected = partial.save.clone();
            questdb::go_back(
                &mut expected,
                &data,
                &[get("Endar Spire")],
                &[get("Taris")],
                false,
                Some(stage),
            )
            .unwrap();
            same_result(&partial_back.save, &expected);
            same_result(&save, &pending);
            assert_eq!(files(&work), disk);
        }

        // Back takes the chapters' items and credits back unless "Keep my items and credits"
        // is on (off when the pop-up opens).
        let back = Plan::Back {
            alignment: 50,
            unfinished: Vec::new(),
            kept: Vec::new(),
            undo: chapters.clone(),
            stage: None,
            keep_items: false,
        };
        let rewound = back.prepare(&prepared.save, &data).unwrap();
        let mut expected = prepared.save.clone();
        let notes = questdb::go_back(&mut expected, &data, &[], &chapters, false, None).unwrap();
        same_result(&rewound.save, &expected);
        assert_eq!(rewound.notes, notes);
        // kept: the engine's keep_items, and no inventory or credit change at all (2026-11-07)
        let keep = Plan::Back {
            alignment: 50,
            unfinished: Vec::new(),
            kept: Vec::new(),
            undo: chapters.clone(),
            stage: None,
            keep_items: true,
        };
        let kept_items = keep.prepare(&prepared.save, &data).unwrap();
        let mut expected = prepared.save.clone();
        questdb::go_back(&mut expected, &data, &[], &chapters, true, None).unwrap();
        same_result(&kept_items.save, &expected);
        let still: Vec<String> = inventory_changes(&accessible_items(&prepared.save), &accessible_items(&kept_items.save))
            .iter()
            .map(|c| format!("{} {}", c.item.string("TemplateResRef").or_else(|| c.item.string("EquippedRes")).unwrap_or_default(), c.difference))
            .collect();
        assert!(still.is_empty(), "items kept: no inventory change, not {:?}", still);
        assert_eq!(kept_items.save.party_int("PT_GOLD"), prepared.save.party_int("PT_GOLD"), "credits kept");
        let taken = inventory_changes(&accessible_items(&prepared.save), &accessible_items(&rewound.save)).len();
        eprintln!("going back takes {} kinds of item back; with them kept, none", taken);
        assert!(Plan::Back {
            alignment: 50,
            unfinished: Vec::new(),
            kept: Vec::new(),
            undo: Vec::new(),
            stage: None,
            keep_items: false,
        }
        .prepare(&save, &data)
        .is_err());
        same_result(&save, &pending);
        assert_eq!(files(&work), disk);
        checked += 1;
    }
    eprintln!("Story preview checked on {} games", checked);
}
