//! The map's ways out: the spot offered next to an area transition lies outside the
//! transition's trigger, with a margin, so a player moved there is not carried off as the
//! save loads (2026-11-06 audit; before, the floor at the trigger's middle). Read from every
//! module of the games installed on the PC; nothing is written. Without a game the test passes
//! without checking anything, and says so.

use kse_core::gamedata::GameData;
use kse_core::games::{find_installs, Game};
use kse_core::map::{Area, Kind};

fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut on = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            on = !on;
        }
        j = i;
    }
    on
}

#[test]
fn a_way_outs_spot_is_outside_its_trigger() {
    let installs = find_installs(&[]);
    let (mut exits, mut spots) = (0, 0);
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        for module in kse_core::map::game_modules(&install.path) {
            let Some(git) = kse_core::map::game_git(&data, &module) else { continue };
            let area = Area::build(&data, &module, &git.root, &|_| String::new());
            let triggers = git.root.list("TriggerList").cloned().unwrap_or_default();
            // a way out that is a trigger (a door leading out is a door, with its two sides)
            for o in area.objects.iter().filter(|o| o.kind == Kind::Exit && o.list == "TriggerList") {
                exits += 1;
                let Some(t) = triggers.get(o.index) else { continue };
                let (px, py) = (t.float("XPosition").unwrap_or(0.0), t.float("YPosition").unwrap_or(0.0));
                let poly: Vec<(f32, f32)> = t
                    .list("Geometry")
                    .map(|g| g.iter().map(|p| (px + p.float("PointX").unwrap_or(0.0), py + p.float("PointY").unwrap_or(0.0))).collect())
                    .unwrap_or_default();
                if poly.len() < 3 {
                    continue;
                }
                for s in &o.spots {
                    spots += 1;
                    assert!(!inside(&poly, s[0], s[1]), "{:?} {}: the spot by {} is inside its trigger: {:?}", game, module, o.tag, s);
                }
            }
        }
    }
    if exits == 0 {
        eprintln!("no game on this PC: nothing checked");
    } else {
        eprintln!("{} ways out, {} spots outside their triggers", exits, spots);
    }
}

/// A spot clicked on the map keeps out of the area's ways out too (2026-11-07 audit, W-4):
/// clicked at the middle of each way out, the floor picked is clear of every trigger, with the
/// margin. Before, the nearest floor was taken, inside the trigger.
#[test]
fn a_clicked_spot_keeps_out_of_the_ways_out() {
    let installs = find_installs(&[]);
    let (mut clicks, mut moved) = (0, 0);
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        for module in kse_core::map::game_modules(&install.path).into_iter().take(60) {
            let Some(git) = kse_core::map::game_git(&data, &module) else { continue };
            let area = Area::build(&data, &module, &git.root, &|_| String::new());
            for o in area.objects.iter().filter(|o| o.kind == Kind::Exit && o.list == "TriggerList") {
                let [x, y, z] = o.pos;
                let plain = area.floor.nearest(x, y, z, 1.5);
                let Some(spot) = area.floor.nearest_where(x, y, z, 1.5, &|px, py| area.clear_of_exits(px, py)) else { continue };
                clicks += 1;
                assert!(area.clear_of_exits(spot[0], spot[1]), "{} {}: the picked spot is in a way out", module, o.tag);
                if plain.map_or(false, |p| !area.clear_of_exits(p[0], p[1])) {
                    moved += 1;
                }
            }
        }
    }
    eprintln!("{} clicks at ways out; {} would have stood inside one before", clicks, moved);
}

/// A spot clicked on the map keeps out of the area's objects too (2026-11-07 audit, W-4, its
/// second half): clicked at the middle of each placeable, the floor picked is clear of every
/// placeable's walkmesh, with a character's reach. Before, only the ways out were kept clear.
#[test]
fn a_clicked_spot_keeps_out_of_the_objects() {
    let installs = find_installs(&[]);
    let (mut clicks, mut moved, mut shapes) = (0, 0, 0);
    for game in [Game::K1, Game::K2] {
        let Some(install) = installs.iter().find(|i| i.game == game) else { continue };
        let data = GameData::open(game, &install.path);
        for module in kse_core::map::game_modules(&install.path).into_iter().take(60) {
            let Some(git) = kse_core::map::game_git(&data, &module) else { continue };
            let area = Area::build(&data, &module, &git.root, &|_| String::new());
            shapes += area.object_shapes.len();
            for p in git.root.list("Placeable List").map(|l| l.to_vec()).unwrap_or_default() {
                let (x, y, z) = (p.float("X").unwrap_or(0.0), p.float("Y").unwrap_or(0.0), p.float("Z").unwrap_or(0.0));
                if area.clear_of_objects(x, y) {
                    continue; // no walkmesh here (an invisible marker)
                }
                let Some(spot) = area.floor.nearest_where(x, y, z, 1.5, &|px, py| area.clear_spot(px, py)) else { continue };
                clicks += 1;
                assert!(area.clear_of_objects(spot[0], spot[1]), "{} {}: the picked spot is inside an object", module, p.string("Tag").unwrap_or_default());
                assert!(area.clear_of_exits(spot[0], spot[1]), "{}: the picked spot is in a way out", module);
                if area.floor.nearest_where(x, y, z, 1.5, &|px, py| area.clear_of_exits(px, py)).map_or(false, |q| !area.clear_of_objects(q[0], q[1])) {
                    moved += 1;
                }
            }
        }
    }
    eprintln!("{} object triangles; {} clicks on objects; {} would have stood inside one before", shapes, clicks, moved);
}
