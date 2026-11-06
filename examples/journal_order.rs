//! Development aid, read only: prints a save's journal in the order the save keeps it, with
//! each entry's date and time and the chapter whose quest it is, to see whether the order
//! of the entries tells the order the planets were played in.
//!   cargo run --example journal_order -- <save folder> [<save folder> ...]
use kse_core::questdb;
use kse_core::save::Save;
use std::path::PathBuf;

fn main() {
    for arg in std::env::args().skip(1) {
        let Ok(save) = Save::open(&PathBuf::from(&arg)) else {
            println!("{}: does not open", arg);
            continue;
        };
        println!("== {} ({:?})", arg, save.game);
        let chapters: Vec<(String, Vec<String>)> = questdb::milestones_ref(save.game)
            .iter()
            .map(|(p, _)| (p.to_string(), questdb::chapter_quests(save.game, p).into_iter().map(|(t, _)| t).collect()))
            .collect();
        let list = save.party.root.list("JNL_Entries").cloned().unwrap_or_default();
        for (i, e) in list.iter().enumerate() {
            let tag = e.string("JNL_PlotID").unwrap_or_default();
            let owners: Vec<&str> = chapters.iter().filter(|(_, q)| q.iter().any(|t| t.eq_ignore_ascii_case(&tag))).map(|(p, _)| p.as_str()).collect();
            println!(
                "{:3} {:28} {:4} date {:6} time {:9} {}",
                i,
                tag,
                e.int("JNL_State").unwrap_or(-1),
                e.int("JNL_Date").unwrap_or(-1),
                e.int("JNL_Time").unwrap_or(-1),
                owners.join(",")
            );
        }
        let played = questdb::chapters_played(save.game, &save.quests());
        let names: Vec<&str> = questdb::milestones_ref(save.game).iter().zip(&played).filter(|(_, p)| **p).map(|((n, _), _)| *n).collect();
        println!("played: {}", names.join(", "));
    }
}
