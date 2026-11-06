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
