//! Makes one edited save to load in the game (2026-11-07 audit, in-game checks): copies
//! `source` into the next free slot of `saves_dir` (a test copy of the game, never the
//! player's), names it on the Load screen, applies one edit with the editor's own code, and
//! writes it as Save Changes does, backup and all.
//!
//!   cargo run --example ingame_fixture -- <game: k1|k2> <game folder> <source save> <saves dir> <name> <edit> [args]
//!
//! Edits: copy | finish <tag> <ending> [dark] | skip <planet> | back <planet> | level <n> |
//! item <resref> | xp <amount> | import <npc> <other save> | travel <module> | restore
//! (an XP edit written, then the save put back to its backup with save::restore).

use kse_core::catalog::{self, Catalog};
use kse_core::creature;
use kse_core::gamedata::GameData;
use kse_core::games::Game;
use kse_core::questdb;
use kse_core::save::{self, Save, Who};
use kse_core::story_preview::Plan;
use std::fs;
use std::path::{Path, PathBuf};

fn next_slot(dir: &Path) -> PathBuf {
    let highest = fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_string_lossy().split(" - ").next().and_then(|n| n.trim().parse::<u32>().ok())).max().unwrap_or(0))
        .unwrap_or(0);
    let n = highest + 1;
    dir.join(format!("{:06} - Game{}", n, n - 1))
}

fn upto(game: Game, planet: &str) -> Vec<serde_json::Value> {
    let order = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let all = questdb::milestones(game);
    let end = all.iter().find(|(p, _)| p.eq_ignore_ascii_case(planet)).map(|(_, m)| order(m)).expect("a chapter of that name");
    let mut path: Vec<_> = all.into_iter().map(|(_, m)| m).filter(|m| m.get("side_chapter").is_none() && order(m) <= end).collect();
    path.sort_by_key(order);
    path
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let game = if a[1] == "k2" { Game::K2 } else { Game::K1 };
    let data = GameData::open(game, Path::new(&a[2]));
    let source = PathBuf::from(&a[3]);
    let saves_dir = PathBuf::from(&a[4]);
    let name = &a[5];
    let edit = a[6].as_str();
    let arg = |i: usize| a.get(7 + i).cloned().unwrap_or_default();
    let target = next_slot(&saves_dir);
    // where the editor puts them: in the game folder, never inside KotOR II's cloudsaves,
    // which Steam syncs (the parent of a cloud save's folder is still inside it)
    let backups = save::backup_root(&target, Some(Path::new(&a[2])));
    fs::create_dir_all(&target).unwrap();
    for e in fs::read_dir(&source).unwrap().filter_map(|e| e.ok()) {
        if e.path().is_file() {
            fs::copy(e.path(), target.join(e.file_name())).unwrap();
        }
    }
    let mut sv = Save::open(&target).unwrap();
    sv.back_up(&backups).unwrap();
    let cat = Catalog::build(&data);
    let mut notes: Vec<String> = Vec::new();
    match edit {
        "copy" => {}
        "finish" => {
            let plan = Plan::Quest { tag: arg(0), ending: arg(1).parse().unwrap(), dark: arg(2) == "dark" };
            let p = plan.prepare(&sv, &data).unwrap();
            notes = p.notes;
            sv = p.save;
        }
        "skip" => {
            let plan = Plan::Skip { chapters: upto(game, &arg(0)), stage: None, class: Some(0), dark: false, prestige: None, train: false };
            let p = plan.prepare(&sv, &data).unwrap();
            notes = p.notes;
            sv = p.save;
        }
        "back" => {
            let path = upto(game, &arg(0));
            let kept = path[..path.len() - 1].to_vec();
            let undo = vec![path.last().unwrap().clone()];
            notes = questdb::go_back(&mut sv, &data, &kept, &undo, false, None).unwrap();
        }
        "level" => {
            let want: i64 = arg(0).parse().unwrap();
            sv.edit(Who::Player, "level", |c| {
                let classes = creature::classes(c);
                let idx = classes.len() - 1;
                let class = cat.class(classes[idx].0).unwrap().clone();
                let now = creature::level(c);
                let keep = creature::other_class_feats(c, idx, &|id| cat.class(id).cloned());
                creature::set_class_level_keeping(c, idx, classes[idx].1 + (want - now), &class, &|l| cat.xp_for(l), &keep);
            });
        }
        "item" => {
            let g = data.gff(&arg(0), "uti", kse_core::formats::erf::RES_TYPE_UTI).expect("the item's template");
            let like = sv.inventory().first().cloned();
            let item = catalog::item_from_template(&g.root, like.as_ref(), 1);
            sv.edit_inventory("add", |items| items.push(item));
        }
        "xp" => {
            let by: i64 = arg(0).parse().unwrap();
            sv.edit(Who::Player, "xp", |c| {
                let now = creature::experience(c);
                creature::set_experience(c, now + by);
            });
        }
        "import" => {
            let other = Save::open(Path::new(&arg(1))).unwrap();
            sv.import_npc(&other, arg(0).parse().unwrap()).unwrap();
        }
        "travel" => {
            let module = arg(0);
            // the ways the game leads into the area, as Travel Here offers them first
            let arrivals = kse_core::map::arrivals(&data);
            let way = arrivals
                .get(&module.to_ascii_lowercase())
                .and_then(|w| w.first().map(|(tag, _)| tag.clone()))
                .or_else(|| kse_core::map::any_waypoint(&data, &module))
                .expect("a way into that module");
            sv.queue_transition(&module, &way, &[]).unwrap();
            notes.push(format!("travel to {} by {}", module, way));
        }
        "restore" => {
            sv.edit(Who::Player, "xp", |c| {
                let now = creature::experience(c);
                creature::set_experience(c, now + 12345);
            });
            sv.rename(name);
            sv.write(&backups).unwrap();
            let first = save::backup_list(&backups, &target).into_iter().next().unwrap();
            save::restore(&target, &first, &backups, game).unwrap();
            let mut again = Save::open(&target).unwrap();
            again.rename(name);
            again.back_up(&backups).unwrap();
            again.write(&backups).unwrap();
            println!("{}\nrestored from {}", target.display(), first.display());
            return;
        }
        other => panic!("no edit {}", other),
    }
    sv.rename(name);
    let b = sv.write(&backups).unwrap();
    let check = Save::open(&target).unwrap();
    println!("{}", target.display());
    println!("backup {}", b.display());
    println!("level {} xp {} credits {} module {} quests {}", check.player().map(creature::level).unwrap_or(0), check.player().map(creature::experience).unwrap_or(0), check.party_int("PT_GOLD").unwrap_or(0), check.module_name(), check.quests().len());
    for n in notes {
        println!("note: {}", n);
    }
}
