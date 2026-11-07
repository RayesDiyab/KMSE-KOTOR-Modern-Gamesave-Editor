//! The quest database: for each quest, the steps the game takes from its start to each
//! ending, as tools/questdb/build.py drafts them from the game's own scripts and dialogs and
//! a person then checks by playing (data/quests/*.json). Finishing a quest with "as if
//! played" applies every step up to the chosen ending to the save, in order: story
//! switches, items, experience, alignment, credits, and the people and doors of each area
//! the save has been to. Nothing here runs a script; it writes what the script would have.

use crate::formats::gff::{Struct, Value};
use crate::gamedata::GameData;
use crate::games::Game;
use crate::save::{Save, Who};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct File {
    pub game: String,
    #[serde(default)]
    pub planet: String,
    #[serde(default)]
    pub checked: bool,
    pub quests: Vec<Quest>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Quest {
    pub tag: String,
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub notes: String,
    /// The chapter's own quest (not one another chapter also sets stages of).
    #[serde(default)]
    pub own: bool,
    /// The ending a full run of the chapter reaches (tools/questdb/assemble.py).
    #[serde(default)]
    pub run_ending: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Stage {
    pub stage: i64,
    #[serde(default)]
    pub end: bool,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Step {
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub via: Option<serde_json::Value>,
    #[serde(default)]
    pub needs: Vec<serde_json::Value>,
    #[serde(default)]
    pub does: Vec<serde_json::Value>,
    #[serde(default)]
    pub branch: Option<String>,
    /// Item templates the game keeps only in an area's own files (KotOR 2 has many), found
    /// there ahead of time for Skip Ahead; looked up after the game's global files.
    #[serde(skip)]
    pub found: std::collections::HashMap<String, crate::formats::gff::Gff>,
}

/// The databases the editor carries, by game: whether the file is a chapter's milestones or
/// rewards (the rest are its quests), and its text, deflated by build.rs.
const FILES: &[(Game, bool, &[u8])] = include!(concat!(env!("OUT_DIR"), "/quest_files.rs"));

fn text(packed: &[u8]) -> String {
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed).unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_default()
}

/// The quest files (data/quests/*.json read as quest data), parsed once: the editor asks
/// for them on every refresh of the story, and parsing the 13 MB each time made it slow.
fn quest_files() -> &'static [(Game, File)] {
    static CACHE: std::sync::OnceLock<Vec<(Game, File)>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        FILES
            .iter()
            .filter(|(_, kept, _)| !kept)
            .filter_map(|(g, _, t)| serde_json::from_str::<File>(&text(t)).ok().map(|f| (*g, f)))
            .collect()
    })
}

/// The chapters' milestones and rewards as JSON, parsed once.
fn json_files() -> &'static [(Game, serde_json::Value)] {
    static CACHE: std::sync::OnceLock<Vec<(Game, serde_json::Value)>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        FILES
            .iter()
            .filter(|(_, kept, _)| *kept)
            .filter_map(|(g, _, t)| serde_json::from_str::<serde_json::Value>(&text(t)).ok().map(|v| (*g, v)))
            .filter(|(_, v)| v.get("next_chapter").is_some() || v.get("xp_at_end").is_some())
            .collect()
    })
}

/// The quest's chain, when the database has it.
pub fn quest(game: Game, tag: &str) -> Option<(Quest, bool)> {
    // A quest that runs over several chapters (the Star Forge, the party members' own
    // quests) is in each chapter's file with the stages set there: merged into one chain.
    let mut found: Option<(Quest, bool)> = None;
    for (g, file) in quest_files() {
        if *g != game {
            continue;
        }
        let Some(q) = file.quests.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)).cloned() else { continue };
        match found.as_mut() {
            None => found = Some((q, file.checked)),
            Some((have, checked)) => {
                *checked &= file.checked;
                for st in q.stages {
                    match have.stages.iter_mut().find(|x| x.stage == st.stage) {
                        Some(x) => x.steps.extend(st.steps),
                        None => have.stages.push(st),
                    }
                }
                have.stages.sort_by_key(|x| x.stage);
            }
        }
    }
    found
}

/// The steps that lead to `ending`: every step of the stages before it (by number) that
/// belongs to no branch, then the ending's own steps. Branched steps of earlier stages are
/// returned apart, for the player to choose.
pub fn path(q: &Quest, ending: i64) -> (Vec<Step>, Vec<Step>) {
    let mut sure = Vec::new();
    let mut choices = Vec::new();
    let mut stages: Vec<&Stage> = q.stages.iter().collect();
    stages.sort_by_key(|s| s.stage);
    for s in stages {
        if s.stage == ending {
            sure.extend(s.steps.iter().cloned());
        } else if s.stage < ending && !s.end {
            for step in &s.steps {
                if step.branch.as_deref().map_or(true, |b| b.trim().is_empty()) {
                    sure.push(step.clone());
                } else {
                    choices.push(step.clone());
                }
            }
        }
    }
    (sure, choices)
}

/// Whether a step's "before this line: if player alignment ..." holds at `alignment`. The
/// game's alignment-scaled rewards are one step per band, and exactly one of them runs.
fn alignment_fits(branch: Option<&str>, alignment: i64) -> bool {
    let Some(text) = branch.and_then(|b| b.split("if player alignment ").nth(1)) else { return true };
    let numbers: Vec<i64> = text.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    match numbers.as_slice() {
        [high, ..] if text.starts_with("at most") => alignment <= *high,
        [low, ..] if text.starts_with("at least") => alignment >= *low,
        [low, high, ..] if text.starts_with("from") => (*low..=*high).contains(&alignment),
        _ => true,
    }
}

/// The ways through one stage are told apart by the line that runs them: the conversation
/// node or script (`via`). Steps with neither belong to every way.
fn way_of(step: &Step) -> String {
    match &step.via {
        Some(v) => v.to_string(),
        None => step.branch.as_deref().filter(|b| b.starts_with("way ")).map(|b| b.split(':').next().unwrap_or(b).to_string()).unwrap_or_default(),
    }
}

/// What a way moves alignment by, light above dark.
fn way_alignment(steps: &[&Step]) -> i64 {
    let of = |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    steps.iter().flat_map(|s| &s.does).map(|e| of(e, "light_side") - of(e, "dark_side")).sum()
}

/// The steps one play of a quest takes to `ending`. `path` keeps every way of the ending's
/// stage, so a stage with five ways gave five rewards (measured 2026-11-02: 5,000 XP where one
/// way gives 1,000; alignment 50 to 0). Here each stage takes one way, the one that suits the
/// side played, ties to the first the data lists, and of the alignment-scaled steps only the
/// band the player is in.
pub fn one_way(q: &Quest, ending: i64, dark: bool, alignment: i64) -> Vec<Step> {
    one_way_in(q, ending, dark, alignment, &[])
}

/// A step's condition, the part of its branch after the way ("way 1: Kodin, ... / if not
/// GetLocalBoolean(...)", "choice: reply R13", "line E66", "otherwise"); empty for a step every
/// path through its way takes. Alignment bands are not counted here: `alignment_fits` picks
/// the one band the player is in.
fn condition_of(step: &Step) -> String {
    let b = step.branch.as_deref().unwrap_or("").trim();
    let cond = if b.starts_with("way ") { b.split_once(" / ").map(|(_, c)| c).unwrap_or("") } else { b };
    if cond.contains("player alignment") {
        return String::new();
    }
    cond.trim().to_string()
}

/// Whether a condition holds for the save's story values, where every clause of it names a
/// story value and compares it ("if DAN_ELISE_PLOT != 0", "if not 403DXN_Missing_Mand",
/// "if K_SWG_BASTILA >= 2; not EBO_GIZKA_PLOT"): Some(true or false). None where any clause
/// is something else (a conversation's local flag, an item held, a reply), which only the
/// game can tell.
fn condition_holds(cond: &str, globals: &[crate::globals::Global]) -> Option<bool> {
    let text = cond.trim().strip_prefix("before this line: ").unwrap_or(cond.trim());
    // "if not (1 && GetIsObjectValid(OBJECT_SELF))": the script's own object always exists
    if text.contains("not (1 && GetIsObjectValid(") {
        return Some(false);
    }
    let text = text.strip_prefix("if ")?;
    if globals.is_empty() {
        return None;
    }
    let find = |n: &str| globals.iter().find(|g| g.name.eq_ignore_ascii_case(n));
    let mut all = true;
    for clause in text.split(';').map(str::trim).filter(|c| !c.is_empty()) {
        let (negated, clause) = match clause.strip_prefix("not ") {
            Some(c) => (true, c.trim()),
            None => (false, clause),
        };
        let words: Vec<&str> = clause.split_whitespace().collect();
        let truth = |name: &str| -> Option<bool> {
            match find(name)?.value {
                crate::globals::GlobalValue::Bool(b) => Some(b),
                crate::globals::GlobalValue::Number(n) => Some(n != 0),
                _ => None,
            }
        };
        let holds = match words.as_slice() {
            // "501_FIRST_ENTER is TRUE"
            [name, "is", "TRUE"] => truth(name)?,
            [name, "is", "FALSE"] => !truth(name)?,
            [name] => match find(name)?.value {
                crate::globals::GlobalValue::Bool(b) => b,
                crate::globals::GlobalValue::Number(n) => n != 0,
                _ => return None,
            },
            [name, op, value] => {
                let v: i64 = value.parse().ok()?;
                let have = match find(name)?.value {
                    crate::globals::GlobalValue::Number(n) => n,
                    crate::globals::GlobalValue::Bool(b) => b as i64,
                    _ => return None,
                };
                match *op {
                    "==" => have == v,
                    "!=" => have != v,
                    ">=" => have >= v,
                    "<=" => have <= v,
                    ">" => have > v,
                    "<" => have < v,
                    _ => return None,
                }
            }
            _ => return None,
        };
        all &= holds != negated;
    }
    Some(all)
}

/// The group of alternatives a condition belongs to within its way: what comes before its last
/// clause, and the kind of that clause (the node's own script, the line's own test, the line
/// that follows a reply).
fn alternative_group(cond: &str) -> String {
    let (before, last) = cond.rsplit_once(" / ").unwrap_or(("", cond));
    let last = last.trim();
    let kind = if last.starts_with("before this line:") {
        "script"
    } else if last.starts_with("line ") {
        "line"
    } else if last.starts_with("choice:") {
        "choice"
    } else {
        "test"
    };
    format!("{}|{}", before.trim(), kind)
}

/// Whether a step with a condition runs, by the story values as the run stands: its condition
/// alone. Its `needs` are not read: they hold what reaching the line takes (KotOR II's
/// escape_merchant recruits Mandalore only with 502OND_End_First at 2, which the run to the
/// ending sets on the way), so judged by the save as it is now they refused steps the run
/// itself makes possible (2026-11-07). None where only the game can tell.
fn step_holds(t: &Step, values: &[crate::globals::Global]) -> Option<bool> {
    condition_holds(&condition_of(t), values)
}

/// A story value an effect sets, put into the running values.
fn set_running_value(values: &mut [crate::globals::Global], e: &serde_json::Value) {
    let (name, value) = if let Some((n, v)) = e.get("set_global_bool").and_then(pair) {
        (n, crate::globals::GlobalValue::Bool(v.as_bool().unwrap_or(false)))
    } else if let Some((n, v)) = e.get("set_global_number").and_then(pair) {
        let Some(v) = v.as_i64() else { return };
        (n, crate::globals::GlobalValue::Number(v))
    } else {
        return;
    };
    if let Some(g) = values.iter_mut().find(|g| g.name.eq_ignore_ascii_case(&name)) {
        g.value = value;
    }
}

/// The save's story values, for `one_way_in`.
fn story_values(save: &Save) -> Vec<crate::globals::Global> {
    save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default()
}

/// `one_way` with the save's story values: of the steps one way takes, each is one path through
/// its conversation or script, and the game runs exactly one path (2026-11-07 audit: KotOR II's
/// DroidRetrieval stage 20 lists 63 paths of one line, each paying Kodin; taken together they
/// took 46,400 credits). A step without a condition is on every path. A condition the save's
/// story values decide keeps its step only when it holds; of the rest, one condition is taken,
/// the first the data lists, as `one_way` takes the first of equal ways.
pub fn one_way_in(q: &Quest, ending: i64, dark: bool, alignment: i64, globals: &[crate::globals::Global]) -> Vec<Step> {
    let mut stages: Vec<&Stage> = q.stages.iter().filter(|s| s.stage == ending || (s.stage < ending && !s.end)).collect();
    stages.sort_by_key(|s| s.stage);
    let mut out = Vec::new();
    // the story values as the run goes: what an earlier stage sets, a later one reads
    // (2026-11-07 second review: conditions read the values before the run)
    let mut values: Vec<crate::globals::Global> = globals.to_vec();
    for s in stages {
        let fits: Vec<&Step> = s.steps.iter().filter(|t| alignment_fits(t.branch.as_deref(), alignment)).collect();
        let sets_stage = |t: &Step| t.does.iter().any(|e| e.get("journal").and_then(|j| j.get(0)).and_then(|v| v.as_str()).map_or(false, |x| x.eq_ignore_ascii_case(&q.tag)));
        let mut ways: Vec<String> = Vec::new();
        for t in fits.iter().filter(|t| sets_stage(t)) {
            let w = way_of(t);
            if !ways.contains(&w) {
                ways.push(w);
            }
        }
        let score = |w: &String| way_alignment(&fits.iter().copied().filter(|t| way_of(t) == *w).collect::<Vec<_>>());
        let mut pick = ways.first().cloned();
        for w in &ways {
            if pick.as_ref().map_or(false, |p| if dark { score(w) < score(p) } else { score(w) > score(p) }) {
                pick = Some(w.clone());
            }
        }
        let kept = |t: &Step| {
            let w = way_of(t);
            let chosen = w.is_empty() || Some(&w) == pick.as_ref();
            // a stage with no line of its own: what `path` takes from it
            let plain = pick.is_none() && (s.stage == ending || t.branch.as_deref().map_or(true, |b| b.trim().is_empty()));
            chosen || plain
        };
        // Alternatives are the steps of one way at one point of it: the same line reached the
        // same way (the part of the condition before its last clause), and the same kind of
        // last clause: the node's own script ("before this line: ..."), the line's own test
        // ("if ...", "otherwise"), or which line follows a reply ("line E66"). Of each such
        // group one is taken: the first the save says holds, else the first that does
        // something, else the first. Steps at different points are parts of one run and are
        // all kept (2026-11-07 second review: taking one step per way dropped the 110 XP of
        // KotOR II's tutorial_3CFD and of KotOR's dan_murder). Steps without a way of their
        // own are separate scripts: each is kept unless the save says its condition fails.
        let decide = |t: &Step| step_holds(t, &values);
        let mut chosen: Vec<(String, usize)> = Vec::new();
        for want in [Some(true), None] {
            for prefer_effect in [true, false] {
                for (i, t) in fits.iter().enumerate() {
                    let w = way_of(t);
                    let cond = condition_of(t);
                    if w.is_empty() || cond.is_empty() || !kept(t) {
                        continue;
                    }
                    let group = format!("{}|{}", w, alternative_group(&cond));
                    if chosen.iter().any(|(g, _)| *g == group) || decide(t) != want || (prefer_effect && t.does.is_empty()) {
                        continue;
                    }
                    chosen.push((group, i));
                }
            }
        }
        let mut taken: Vec<&Step> = Vec::new();
        for (i, t) in fits.iter().enumerate() {
            if !kept(t) {
                continue;
            }
            let w = way_of(t);
            let cond = condition_of(t);
            if !cond.is_empty() {
                if decide(t) == Some(false) {
                    continue;
                }
                if !w.is_empty() && !chosen.iter().any(|(_, ci)| *ci == i) {
                    continue;
                }
            }
            taken.push(t);
            out.push((*t).clone());
        }
        // the story values this stage sets, for the conditions of the stages after it
        for t in taken {
            for e in &t.does {
                set_running_value(&mut values, e);
            }
        }
    }
    out
}

fn str_of(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn pair(v: &serde_json::Value) -> Option<(String, &serde_json::Value)> {
    let a = v.as_array()?;
    Some((a.first()?.as_str()?.to_string(), a.get(1)?))
}

/// One effect in plain words, for the pop-up.
pub fn describe(effect: &serde_json::Value, data: &GameData) -> String {
    let Some((kind, arg)) = effect.as_object().and_then(|o| o.iter().next()) else { return String::new() };
    let item_name = |resref: &str| -> String {
        data.gff(resref, "uti", crate::formats::erf::RES_TYPE_UTI)
            .and_then(|g| match g.root.get("LocalizedName") {
                Some(Value::LocString(l)) => Some(data.loc(l)),
                _ => None,
            })
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| resref.to_string())
    };
    match kind.as_str() {
        "set_global_bool" => pair(arg).map(|(n, v)| format!("Story switch {} {}", n, if v.as_bool() == Some(true) { "on" } else { "off" })).unwrap_or_default(),
        "set_global_number" => pair(arg).map(|(n, v)| format!("Story counter {} = {}", n, v)).unwrap_or_default(),
        "give_item" => pair(arg).map(|(r, n)| format!("You get {}{}", item_name(&r), if n.as_i64().unwrap_or(1) > 1 { format!(" x{}", n) } else { String::new() })).unwrap_or_default(),
        "take_item" => pair(arg).map(|(r, _)| format!("{} is taken from you", item_name(&r))).unwrap_or_default(),
        "xp" => format!("+{} XP", arg),
        "light_side" => format!("Light side +{}", arg),
        "dark_side" => format!("Dark side +{}", arg),
        "credits" => format!("{} credits", arg),
        "destroy" => format!("{} leaves the area", str_of(arg)),
        "door_open" => format!("Door {} opens", str_of(arg)),
        "door_unlock" => format!("Door {} unlocks", str_of(arg)),
        "journal" => pair(arg).map(|(t, s)| format!("Journal: {} stage {}", t, s)).unwrap_or_default(),
        "local" => arg.as_array().map(|a| format!("Local flag {} on {}", a.get(2).cloned().unwrap_or_default(), str_of(a.first().unwrap_or(&serde_json::Value::Null)))).unwrap_or_default(),
        "other" => str_of(arg),
        k => k.to_string(),
    }
}

/// Whether the editor can write this effect into a save (cutscenes and the like it cannot).
pub fn can_apply(effect: &serde_json::Value) -> bool {
    effect.as_object().and_then(|o| o.keys().next()).map_or(false, |k| k != "other")
}

/// Applies one step's effects to the save. Returns what could not be applied, in plain
/// words (an area not visited yet: the game builds it fresh from its own files there).
pub fn apply(save: &mut Save, data: &GameData, step: &Step, key: &str) -> Vec<String> {
    let mut skipped = Vec::new();
    for (i, effect) in step.does.iter().enumerate() {
        let Some((kind, arg)) = effect.as_object().and_then(|o| o.iter().next()) else { continue };
        let k = format!("{}.{}", key, i);
        match kind.as_str() {
            "set_global_bool" | "set_global_number" => {
                let Some((name, v)) = pair(arg) else { continue };
                let value = if kind == "set_global_bool" { v.as_bool().map(|b| b as i64) } else { v.as_i64() };
                let Some(value) = value else { continue };
                let mut found = false;
                save.edit_globals(&k, |root| {
                    let all = crate::globals::list(root);
                    if let Some(g) = all.iter().find(|g| g.name.eq_ignore_ascii_case(&name)) {
                        found = true;
                        match g.kind {
                            crate::globals::Kind::Boolean => crate::globals::set_bool(root, g.index, value != 0),
                            // The game keeps booleans and numbers apart: a boolean write never
                            // reaches a number of that name (MAN_MISSING_PLOT in the quest data).
                            crate::globals::Kind::Number if kind == "set_global_bool" => {}
                            crate::globals::Kind::Number => crate::globals::set_number(root, g.index, value),
                            crate::globals::Kind::String => {}
                        }
                    }
                });
                if !found {
                    skipped.push(format!("The save has no story switch {}", name));
                }
            }
            "give_item" => {
                let Some((resref, n)) = pair(arg) else { continue };
                let Some(uti) = data.gff(&resref, "uti", crate::formats::erf::RES_TYPE_UTI).or_else(|| step.found.get(&resref.to_ascii_lowercase()).cloned()) else {
                    skipped.push(format!("Item {} not found in the game", resref));
                    continue;
                };
                let count = n.as_i64().unwrap_or(1);
                // Credits (baseitems.2da row 57): picking them up adds their stack size to the
                // party's credits, as the game does; they never stay in the inventory.
                if uti.root.int("BaseItem") == Some(57) {
                    let amount = uti.root.int("StackSize").unwrap_or(1).max(1) * count;
                    let gold = save.party_int("PT_GOLD").unwrap_or(0);
                    save.set_party_int("PT_GOLD", gold + amount);
                    continue;
                }
                let like = save.inventory().first().cloned();
                let item = crate::catalog::item_from_template(&uti.root, like.as_ref(), count);
                // Onto a stack of the same item the party already has, as the game stacks a
                // pick-up, up to baseitems.2da's stacking limit.
                let base = item.int("BaseItem").unwrap_or(0) as usize;
                let limit = data.table("baseitems").int(base, "stacking").unwrap_or(1).max(1);
                let tag = item.string("Tag").unwrap_or_default();
                save.edit_inventory(&k, |items| {
                    if limit > 1 {
                        if let Some(same) = items.iter_mut().find(|it| {
                            it.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&tag))
                                && it.int("StackSize").unwrap_or(1) + count <= limit
                        }) {
                            let now = same.int("StackSize").unwrap_or(1);
                            same.set_int("StackSize", now + count, Value::Word(1));
                            return;
                        }
                    }
                    items.push(item);
                });
            }
            "take_item" => {
                let Some((resref, _)) = pair(arg) else { continue };
                save.edit_inventory(&k, |items| {
                    if let Some(at) = items.iter().position(|it| {
                        it.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&resref))
                            || it.string("TemplateResRef").map_or(false, |t| t.eq_ignore_ascii_case(&resref))
                    }) {
                        let stack = items[at].int("StackSize").unwrap_or(1);
                        if stack > 1 {
                            items[at].set_int("StackSize", stack - 1, Value::Word(1));
                        } else {
                            items.remove(at);
                        }
                    }
                });
            }
            "xp" => {
                let n = arg.as_i64().unwrap_or(0);
                save.edit(Who::Player, &k, |c| {
                    let now = crate::creature::experience(c);
                    crate::creature::set_experience(c, now + n);
                });
                let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
                save.set_party_int("PT_XP_POOL", pool + n);
            }
            "light_side" | "dark_side" => {
                let n = arg.as_i64().unwrap_or(0) * if kind == "light_side" { 1 } else { -1 };
                save.edit(Who::Player, &k, |c| {
                    let now = crate::creature::alignment(c);
                    crate::creature::set_alignment(c, (now + n).clamp(0, 100));
                });
            }
            "credits" => {
                let n = arg.as_i64().unwrap_or(0);
                let gold = save.party_int("PT_GOLD").unwrap_or(0);
                save.set_party_int("PT_GOLD", (gold + n).max(0));
            }
            "destroy" | "door_open" | "door_unlock" => {
                let tag = str_of(arg);
                let what = kind.clone();
                let done = save.edit_module_git(&step.module, &k, |git| {
                    let tag_is = |s: &Struct| s.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&tag));
                    match what.as_str() {
                        "destroy" => {
                            for list in [crate::area::CREATURES, crate::area::PLACEABLES] {
                                if let Some(l) = git.list_mut(list) {
                                    l.retain(|s| !tag_is(s));
                                }
                            }
                        }
                        _ => {
                            if let Some(l) = git.list_mut(crate::area::DOORS) {
                                for d in l.iter_mut().filter(|d| tag_is(d)) {
                                    if what == "door_open" {
                                        d.set_int("OpenState", 1, Value::Byte(0));
                                    }
                                    d.set_int("Locked", 0, Value::Byte(0));
                                }
                            }
                        }
                    }
                });
                if !done {
                    skipped.push(format!("{} happens when you first enter {} (the save has not been there; the game reads the story switches then)", describe(effect, data), step.module));
                }
            }
            "local" => {
                // A flag the game keeps on an object, the area or the module (its NPCs read
                // theirs to know a talk is done). Objects and the area: in the area's stored
                // copy; the module's own flags are not reachable here.
                let Some(a) = arg.as_array() else { continue };
                let (Some(on), Some(kind), Some(index), Some(value)) = (a.first().and_then(|v| v.as_str()), a.get(1).and_then(|v| v.as_str()), a.get(2).and_then(|v| v.as_u64()), a.get(3).and_then(|v| v.as_i64())) else { continue };
                if on.eq_ignore_ascii_case("module") {
                    continue;
                }
                let (game, number, on) = (save.game, kind == "number", on.to_string());
                let done = save.edit_module_git(&step.module, &k, |git| {
                    if on.eq_ignore_ascii_case("area") {
                        crate::stored::set_local(git, game, number, index as u32, value);
                        return;
                    }
                    for list in [crate::area::CREATURES, crate::area::DOORS, crate::area::PLACEABLES, "TriggerList"] {
                        if let Some(l) = git.list_mut(list) {
                            for o in l.iter_mut().filter(|o| o.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&on))) {
                                crate::stored::set_local(o, game, number, index as u32, value);
                            }
                        }
                    }
                });
                if !done {
                    skipped.push(format!("{} happens when you first enter {}", describe(effect, data), step.module));
                }
            }
            "journal" => {
                if let Some((tag, s)) = pair(arg) {
                    let s = s.as_i64().unwrap_or(0);
                    // The game only moves a quest forward.
                    let now = save.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(&tag)).map(|(_, v)| v).unwrap_or(-1);
                    if s > now {
                        save.set_quest(&tag, s);
                    }
                }
            }
            _ => skipped.push(describe(effect, data)),
        }
    }
    skipped
}

// ---- skipping ahead ----
//
// A milestone (data/quests/*-milestone.json, written by tools/questdb/build.py --milestone)
// is the story state every finished run of a chapter ends with: the journal, the story
// switches the rest of the game reads, the items the player holds, the party, and where the
// next chapter begins. Skipping ahead applies it to the save and then sends the player to
// that beginning the way the game's own scripts do (Save::queue_transition).

/// The milestones the editor carries for a game: (chapter, the file).
pub fn milestones(game: Game) -> Vec<(String, serde_json::Value)> {
    milestones_ref(game).into_iter().map(|(p, v)| (p.to_string(), v.clone())).collect()
}

/// `milestones` without copying them: the editor's timeline reads them on every refresh.
pub fn milestones_ref(game: Game) -> Vec<(&'static str, &'static serde_json::Value)> {
    json_files()
        .iter()
        .filter(|(g, v)| *g == game && v.get("next_chapter").is_some())
        .map(|(_, v)| (v.get("planet").and_then(|p| p.as_str()).unwrap_or_default(), v))
        .collect()
}

/// Applies a milestone to the save and queues the trip to the next chapter. Returns what
/// could not be applied, and the values the data marks as not certain, in plain words.
pub fn skip_to(save: &mut Save, data: &GameData, m: &serde_json::Value) -> Result<Vec<String>, String> {
    skip_through(save, data, &[m.clone()], None, false)
}

/// Whether the light or the dark side changes anything in these chapters: the story (a
/// chapter's "dark" block), or the credits and alignment a run of each side picks up.
pub fn side_matters(game: Game, chapters: &[serde_json::Value]) -> bool {
    chapters.iter().any(|m| {
        if m.get("dark").is_some() {
            return true;
        }
        let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default();
        rewards(game, planet).and_then(|r| r.get("credits").cloned()).map_or(false, |c| {
            let side = |s: &str| c.get(s).map(|v| (v.get("credits").and_then(|x| x.as_i64()), v.get("light_points").and_then(|x| x.as_i64()), v.get("dark_points").and_then(|x| x.as_i64())));
            side("light") != side("dark")
        })
    })
}

/// The chapter as a run of the chosen side ends it: for the dark side, the milestone with
/// its "dark" block in place of the light-side journal, story values, party and route.
fn for_side(m: &serde_json::Value, dark: bool) -> serde_json::Value {
    let Some(dk) = m.get("dark").filter(|_| dark) else { return m.clone() };
    let mut out = m.clone();
    if let Some(j) = dk.get("journal") {
        out["journal"] = j.clone();
    }
    let overrides = dk.get("globals").and_then(|g| g.as_array()).cloned().unwrap_or_default();
    if let Some(globals) = out.get_mut("globals").and_then(|g| g.as_array_mut()) {
        for o in &overrides {
            let name = o.get("name").and_then(|n| n.as_str()).unwrap_or_default();
            match globals.iter_mut().find(|g| g.get("name").and_then(|n| n.as_str()).map_or(false, |n| n.eq_ignore_ascii_case(name))) {
                Some(g) => {
                    g["value"] = o.get("value").cloned().unwrap_or(serde_json::Value::Null);
                    g["certain"] = o.get("certain").cloned().unwrap_or(serde_json::Value::Bool(false));
                }
                None => globals.push(o.clone()),
            }
        }
    }
    if let Some(party) = dk.get("party") {
        let gone: Vec<i64> = party.get("not_recruited").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        let mut recruit: Vec<serde_json::Value> = out["party"]["recruit"].as_array().cloned().unwrap_or_default();
        recruit.retain(|r| !r.get("slot").and_then(|s| s.as_i64()).map_or(false, |s| gone.contains(&s)));
        recruit.extend(party.get("recruit").and_then(|r| r.as_array()).cloned().unwrap_or_default());
        let mut remove: Vec<serde_json::Value> = out["party"]["remove"].as_array().cloned().unwrap_or_default();
        remove.extend(party.get("remove").and_then(|r| r.as_array()).cloned().unwrap_or_default());
        out["party"]["recruit"] = serde_json::Value::Array(recruit);
        out["party"]["remove"] = serde_json::Value::Array(remove);
    }
    if let Some(next) = dk.get("next_chapter").filter(|n| !n.is_null()) {
        out["next_chapter"] = next.clone();
    }
    out
}

/// Skips through several chapters in story order: each one's state applied in turn (so
/// later chapters build on earlier ones, as in play), the XP of the last, every chapter's
/// areas stored as played through, then the last chapter's own route on. `jedi_class` is
/// the choice (0, 1, 2) for a chapter that makes the player a Jedi (Dantooine); `dark` picks
/// the dark side's story, credits and alignment where they differ.
pub fn skip_through(save: &mut Save, data: &GameData, chapters: &[serde_json::Value], jedi_class: Option<usize>, dark: bool) -> Result<Vec<String>, String> {
    skip_through_with(save, data, chapters, jedi_class, dark, None, false)
}

/// KotOR 2's companions who can be trained as Jedi, by what the end of their own conversation
/// chain does (research 2026-10-23): (party slot, the class a_makejedi gives, the player's
/// gender it needs, the story values the chain ends with, a local flag on the companion).
/// Mira's value is 2 on the light side (her dark training sets 4; slot 7 is light-only anyway).
const TRAINABLE: [(usize, i64, Option<bool>, &[(&str, i64)], Option<u32>); 5] = [
    (0, 5, None, &[("000_Atton_ID_Reveal", 6), ("003EBO_Atton_End", 1)], None),
    (1, 3, None, &[], Some(35)), // baodur.dlg E123 / E132 set his local 35 when the offer is made
    (4, 3, Some(false), &[("000_Handmaiden_Jedi", 1), ("000_Kreia_Hand_Mom", 5)], None),
    (7, 5, None, &[("000_Mira_Jedi", 2)], None),
    (11, 4, Some(true), &[("000_Disc_Padawan", 5)], None),
];

/// Trains the companions in the party as Jedi the way the end of each one's conversation
/// chain does (a_makejedi: AddMultiClass, `creature::add_multiclass`), and sets the chain's values to where it ends, so the
/// game does not offer those conversations again. Returns notes.
pub fn train_companions(save: &mut Save, data: &GameData) -> Vec<String> {
    let mut notes = Vec::new();
    if save.game != Game::K2 {
        return notes;
    }
    let female = save.player().map_or(false, |p| p.int("Gender") == Some(1));
    let cat = crate::catalog::Catalog::build(data);
    for (slot, class_id, gender, values, local) in TRAINABLE {
        if gender.map_or(false, |f| f != female) || !save.recruited(slot) {
            continue;
        }
        let Some(class) = cat.class(class_id).cloned() else { continue };
        let already = save.creature(Who::Npc(slot)).map_or(true, |c| crate::creature::classes(c).iter().any(|k| [3, 4, 5].contains(&k.0)));
        if !already {
            save.edit(Who::Npc(slot), &format!("skip.train.{}", slot), |c| {
                // AddMultiClass as the engine runs it: the class at level 0 and the XP for
                // the next level, which the player then takes in the new class in the game.
                let next = cat.xp_for(crate::creature::level(c) + 1);
                crate::creature::add_multiclass(c, &class, next);
                if let Some(bit) = local {
                    crate::stored::set_local(c, Game::K2, false, bit, 1);
                }
            });
            notes.push(format!("Party member {} trained as {}: the level-up into it is waiting in the game", slot, class.name));
        }
        let does = values.iter().map(|(n, v)| serde_json::json!({ "set_global_number": [n, v] })).collect();
        notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does, branch: None, found: Default::default() }, &format!("skip.train.{}.values", slot)));
    }
    notes
}


/// KotOR 2's AddMultiClass on the player (a_pc_jedi; swkotor2.exe 0066C7A0, the
/// PlayerCreated branch): the class at level 0 (`creature::add_multiclass`, XP left to the
/// party), then the XP the next level needs handed out by the party's DistributeExperience
/// (005FE0C0) as `distribute_experience` does it. Returns the XP handed out (before the
/// bonus and shares), 0 when the player already had it.
pub fn add_multiclass_to_player(save: &mut Save, data: &GameData, cat: &crate::catalog::Catalog, class: &crate::catalog::Class) -> i64 {
    let Some(p) = save.player() else { return 0 };
    let (level, xp) = (crate::creature::level(p), crate::creature::experience(p));
    if crate::creature::classes(p).len() != 1 || crate::creature::classes(p).iter().any(|(k, _)| *k == class.id) {
        return 0;
    }
    save.edit(Who::Player, "skip.prestige", |c| {
        crate::creature::add_multiclass(c, class, None);
    });
    let Some(need) = cat.xp_for(level + 1) else { return 0 };
    let delta = need - xp;
    if delta > 0 {
        distribute_experience(save, data, cat, delta);
    }
    delta.max(0)
}

/// The share of `xp` ReceiveExperience (swkotor2.exe 0058A9F0) gives a creature: npc.2da's
/// PercentXP for its party slot (none for the player), times row 13 (GAME_XP_General), in
/// single precision as the engine computes it, rounded up. Tags "204_b4d4" and "remote" get
/// nothing.
fn xp_share(data: &GameData, slot: Option<usize>, tag: &str, xp: i64) -> i64 {
    if tag.eq_ignore_ascii_case("204_b4d4") || tag.eq_ignore_ascii_case("remote") {
        return 0;
    }
    let table = data.table("npc");
    let percent = |row: usize| table.int(row, "percentxp").unwrap_or(100);
    experience_share(slot.map(percent), percent(13), xp)
}

/// ReceiveExperience's arithmetic (swkotor2.exe 0058A9F0): the factor built in single
/// precision from the slot's percentage (if any) and the general one, `xp` times it rounded
/// to single precision, then rounded up.
pub fn experience_share(slot_percent: Option<i64>, general_percent: i64, xp: i64) -> i64 {
    let mut m: f32 = 1.0;
    if let Some(p) = slot_percent {
        m = ((p as f64 / 100.0) as f32 as f64 * m as f64) as f32;
    }
    m = ((general_percent as f64 / 100.0) as f32 as f64 * m as f64) as f32;
    let product = (xp as f64 * m as f64) as f32;
    (product as f64).ceil() as i64
}

/// DistributeExperience's Mentor bonus (swkotor2.exe 005FE1A2..005FE20A) for a member of
/// `level`: 3%, 5% from level 7, 7% from level 13, as single-precision constants, times
/// `xp`, rounded to single precision, then up; 0 for level 0.
pub fn mentor_bonus(level: i64, xp: i64) -> i64 {
    if level <= 0 {
        return 0;
    }
    let factor: f32 = if level >= 13 { 0.07 } else if level >= 7 { 0.05 } else { 0.03 };
    let bonus = (factor as f64 * xp as f64) as f32;
    (bonus as f64).ceil() as i64
}

/// KotOR 2's CSWPartyTable::DistributeExperience (swkotor2.exe 005FE0C0) for a positive
/// `xp`, as it reaches a save:
///
/// - the party's XP pool (PT_XP_POOL) takes `xp` as given;
/// - the first member of the active party (PT_MEMBERS order) with feat 202 (Mentor) and a
///   level above 0 adds a bonus: 3% of `xp` below level 7, 5% from 7, 7% from 13, the
///   percentage in single precision and the product rounded up;
/// - each active member and the player receive their share of that total (`xp_share`:
///   80% for the companions, 100% for the player, rounded up), XP never lowered;
/// - the player's receipt sets G_PC_LEVEL to the level that XP reaches (006B7310).
///
/// Not reproduced: the auto level-up option (the game levels members up on the spot when it
/// is on) and the k_level_dlg script the player's first possible level-up runs.
pub fn distribute_experience(save: &mut Save, data: &GameData, cat: &crate::catalog::Catalog, xp: i64) {
    if xp <= 0 {
        return;
    }
    let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
    save.set_party_int("PT_XP_POOL", pool + xp);
    let members: Vec<usize> = save.party_members().into_iter().filter(|m| save.creature(Who::Npc(*m)).is_some()).collect();
    let mentor = members.iter().filter_map(|m| save.creature(Who::Npc(*m))).find(|c| crate::creature::feats(c).contains(&202)).map(crate::creature::level).unwrap_or(0);
    let total = xp + mentor_bonus(mentor, xp);
    for m in members {
        let tag = save.creature(Who::Npc(m)).and_then(|c| c.string("Tag")).unwrap_or_default();
        let share = xp_share(data, Some(m), &tag, total);
        save.edit(Who::Npc(m), "xp.distribute", |c| {
            let now = crate::creature::experience(c);
            crate::creature::set_experience(c, now + share);
        });
    }
    let tag = save.player().and_then(|c| c.string("Tag")).unwrap_or_default();
    let share = xp_share(data, None, &tag, total);
    save.edit(Who::Player, "xp.distribute", |c| {
        let now = crate::creature::experience(c);
        crate::creature::set_experience(c, now + share);
    });
    if let Some(p) = save.player() {
        let xp_now = crate::creature::experience(p);
        let mut l = crate::creature::level(p);
        while l < cat.max_level() && cat.xp_for(l + 1).map_or(false, |need| need <= xp_now) {
            l += 1;
        }
        let step = Step { module: String::new(), via: None, needs: Vec::new(), does: vec![serde_json::json!({ "set_global_number": ["G_PC_LEVEL", l] })], branch: None, found: Default::default() };
        let _ = apply(save, data, &step, "xp.distribute.level");
    }
}

/// The prestige classes KotOR 2's Kreia offers (dialog kreia E598 / E590, a_pc_jedi):
/// classes.2da rows by side, in the order Skip Ahead lists them.
pub const PRESTIGE_LIGHT: [i64; 3] = [11, 13, 12]; // Weapon Master, Watchman, Jedi Master
pub const PRESTIGE_DARK: [i64; 3] = [14, 16, 15]; // Marauder, Assassin, Sith Lord

/// Whether Kreia would offer the player a prestige class once the skip is done: KotOR 2, one
/// class so far, a Jedi one, 000_Prestige not 2, level 15 or more at `xp` (c_pc_prestige; the
/// alignment test is the side the player picks).
pub fn prestige_due(save: &Save, data: &GameData, xp: i64) -> bool {
    if save.game != Game::K2 {
        return false;
    }
    let Some(p) = save.player() else { return false };
    let classes = crate::creature::classes(p);
    if classes.len() != 1 || ![3, 4, 5].contains(&classes[0].0) {
        return false;
    }
    let taken = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default().iter().any(|g| {
        g.name.eq_ignore_ascii_case("000_Prestige") && g.value == crate::globals::GlobalValue::Number(2)
    });
    let table = data.table("exptable");
    let level = (0..table.rows.len()).filter(|&r| table.int(r, "xp").map_or(false, |need| xp >= need)).count() as i64;
    !taken && level >= 15
}

/// What a chapter's conversations and scripts give a run of one side (credits.py: credits,
/// alignment, influence per party slot), less the conversations whose quests the save had
/// already finished; and the payments the story needs (credits_fixed), less a purchase whose
/// companion the save already had (its "slot").
struct ChapterGains {
    credits: i64,
    net_alignment: i64,
    influence: std::collections::BTreeMap<usize, i64>,
}

fn chapter_gains(c: &serde_json::Value, side: &str, planet: &str, game: Game, before: &Before) -> ChapterGains {
    let quests = dialog_quests(game, planet);
    let done = |dialog: &str| {
        quests.get(&dialog.to_ascii_lowercase()).map_or(false, |qs| {
            !qs.is_empty() && qs.iter().all(|(t, end)| end.map_or(false, |e| before.stage(t) >= e))
        })
    };
    let int = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let total = &c[side];
    let mut g = ChapterGains { credits: int(total, "credits"), net_alignment: int(total, "light_points") - int(total, "dark_points"), influence: Default::default() };
    let mut add_influence = |v: &serde_json::Value, sign: i64| {
        for (slot, x) in v.get("influence").and_then(|v| v.as_object()).cloned().unwrap_or_default() {
            if let (Ok(slot), Some(x)) = (slot.parse::<usize>(), x.as_i64()) {
                *g.influence.entry(slot).or_default() += sign * x;
            }
        }
    };
    add_influence(total, 1);
    for src in c.get("sources").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let Some(d) = src.get("dialog").and_then(|v| v.as_str()) else { continue };
        if done(d) {
            let v = &src[side];
            g.credits -= int(v, "credits");
            g.net_alignment -= int(v, "light_points") - int(v, "dark_points");
            add_influence(v, -1);
        }
    }
    for f in c.get("fixed").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let bought = f.get("slot").and_then(|v| v.as_u64()).map_or(false, |sl| before.recruited.get(sl as usize).copied().unwrap_or(false));
        if bought {
            g.credits -= int(&f, "credits"); // the total includes it; the save paid it already
        }
    }
    g
}

/// What the save already had before a skip, so nothing a real run gives once is given twice
/// (2026-10-23): the journal, who was in the party, the items held.
struct Before {
    quests: Vec<(String, i64)>,
    recruited: Vec<bool>,
    items: std::collections::HashSet<String>,
}

impl Before {
    fn of(save: &Save) -> Before {
        let mut items = std::collections::HashSet::new();
        for it in save.inventory() {
            for f in ["TemplateResRef", "Tag"] {
                if let Some(v) = it.string(f) {
                    items.insert(v.to_ascii_lowercase());
                }
            }
        }
        Before { quests: save.quests(), recruited: (0..12).map(|i| save.recruited(i)).collect(), items }
    }

    fn stage(&self, tag: &str) -> i64 {
        self.quests.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map_or(-1, |(_, v)| *v)
    }
}

/// Whether the save already met the chapter's Jedi master (KotOR 2, "master_met": a story
/// value, or a flag the master sets on himself in his own conversation): then his count and his
/// teaching are not given again.
fn master_met(save: &Save, m: &serde_json::Value) -> bool {
    let globals = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    m.get("master_met").and_then(|v| v.as_array()).cloned().unwrap_or_default().iter().any(|c| {
        if let Some(name) = c.get("global").and_then(|v| v.as_str()) {
            return globals.iter().any(|g| g.name.eq_ignore_ascii_case(name) && match g.value {
                crate::globals::GlobalValue::Number(n) => n > 0,
                crate::globals::GlobalValue::Bool(b) => b,
                _ => false,
            });
        }
        let (Some(module), Some(dialog), Some(bit)) = (c.get("module").and_then(|v| v.as_str()), c.get("dialog").and_then(|v| v.as_str()), c.get("local").and_then(|v| v.as_u64())) else { return false };
        let Some(git) = save.module_git(module) else { return false };
        git.root.list(crate::area::CREATURES).map_or(false, |l| l.iter().any(|o| {
            o.string("Conversation").map_or(false, |d| d.eq_ignore_ascii_case(dialog)) && local_bit(o, bit as u32)
        }))
    })
}

fn local_bit(o: &Struct, bit: u32) -> bool {
    let Some(Value::Struct(t)) = o.get("SWVarTable") else { return false };
    t.list("BitArray").and_then(|l| l.get((bit / 32) as usize)).and_then(|w| w.int("Variable")).map_or(false, |w| (w as u32) & (1 << (bit % 32)) != 0)
}

/// The chapter's own quests each conversation moves on (from the quest file), for the credits
/// a save that already finished those quests has had.
fn dialog_quests(game: Game, planet: &str) -> std::collections::HashMap<String, Vec<(String, Option<i64>)>> {
    let mut out: std::collections::HashMap<String, Vec<(String, Option<i64>)>> = Default::default();
    for (g, file) in quest_files() {
        if *g != game {
            continue;
        }
        if !file.planet.eq_ignore_ascii_case(planet) {
            continue;
        }
        for q in file.quests.iter().filter(|q| q.own) {
            let end = q.run_ending.or_else(|| q.stages.iter().filter(|s| s.end).map(|s| s.stage).min());
            for st in q.stages.iter().flat_map(|s| s.steps.iter()) {
                if let Some(d) = st.via.as_ref().and_then(|v| v.get("dialog")).and_then(|v| v.as_str()) {
                    let e = out.entry(d.to_ascii_lowercase()).or_default();
                    if !e.iter().any(|(t, _)| t.eq_ignore_ascii_case(&q.tag)) {
                        e.push((q.tag.clone(), end));
                    }
                }
            }
        }
    }
    out
}

/// skip_through, and the prestige class the player picked (a classes.2da row, KotOR 2).
pub fn skip_through_with(save: &mut Save, data: &GameData, chapters: &[serde_json::Value], jedi_class: Option<usize>, dark: bool, prestige: Option<i64>, train: bool) -> Result<Vec<String>, String> {
    skip_core(save, data, chapters, jedi_class, dark, prestige, train, None)
}

/// Skips to a quest's stage inside a chapter: every chapter before it as skip_through does,
/// then in this chapter the quest up to that stage as if played (its story values, journal,
/// the NPCs' flags, its XP, items and credits), the main-story stages its steps need, and the
/// companions who join on the way; then the trip to where that stage happens. The chapter's
/// other quests stay open. `chapters` ends with the chapter of the quest.
pub fn skip_to_stage(save: &mut Save, data: &GameData, chapters: &[serde_json::Value], tag: &str, stage: i64, jedi_class: Option<usize>, dark: bool, prestige: Option<i64>, train: bool) -> Result<Vec<String>, String> {
    skip_core(save, data, chapters, jedi_class, dark, prestige, train, Some((tag.to_string(), stage)))
}

#[allow(clippy::too_many_arguments)]
fn skip_core(save: &mut Save, data: &GameData, chapters: &[serde_json::Value], jedi_class: Option<usize>, dark: bool, prestige: Option<i64>, train: bool, partial: Option<(String, i64)>) -> Result<Vec<String>, String> {
    let all: Vec<serde_json::Value> = leviathan_in_place(save, chapters).iter().map(|m| for_side(m, dark)).collect();
    let target = all.last().cloned().ok_or("no chapter to skip to")?;
    // the player leads at the new point of the story: a party member left marked as the one
    // controlled crashed the game (Save::release_controlled_npc)
    save.release_controlled_npc();
    // a skip to a quest stage plays the chapters before its own in full
    let chapters: Vec<serde_json::Value> = if partial.is_some() { all[..all.len() - 1].to_vec() } else { all.clone() };
    let last = chapters.last().cloned().unwrap_or_else(|| target.clone());
    let mut notes = Vec::new();
    let before = Before::of(save);
    let met: Vec<bool> = chapters.iter().map(|m| master_met(save, m)).collect();
    // Read before the chapters change the journal.
    let played = chapters_played_in(save);
    let mut run: Vec<String> = milestones_ref(save.game).iter().zip(&played).filter(|(_, p)| **p).map(|((planet, _), _)| planet.to_string()).collect();
    run.extend(chapters.iter().filter_map(|m| m.get("planet").and_then(|p| p.as_str()).map(str::to_string)));
    // What the game itself took out of the journal stays out (the chapters already played too).
    let mut removed: Vec<String> = milestones_ref(save.game).iter().zip(&played).filter(|(_, p)| **p).flat_map(|((_, m), _)| removed_by(m)).collect();
    removed.extend(all.iter().flat_map(removed_by));
    let (mut credits, mut shift) = (0i64, 0i64);
    // KotOR 2: influence won or lost with each companion over the chapters (credits.py).
    let mut influence: std::collections::BTreeMap<usize, i64> = Default::default();
    for (ci, m) in chapters.iter().enumerate() {
        notes.extend(apply_chapter(save, data, m, jedi_class, &before, met[ci]));
        notes.extend(apply_k2_powers(save, data, m, dark, met[ci]));
        // The credits and alignment the chapter's conversations and scripts give a run of
        // this side (tools/questdb/credits.py).
        let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default();
        if let Some(c) = rewards(save.game, planet).and_then(|r| r.get("credits").cloned()) {
            let sname = if dark { "dark" } else { "light" };
            let gains = chapter_gains(&c, sname, planet, save.game, &before);
            credits += gains.credits;
            // A chapter played on one side never moves the player toward the other.
            let net = gains.net_alignment;
            shift += if dark { net.min(0) } else { net.max(0) };
            for (slot, v) in gains.influence {
                *influence.entry(slot).or_default() += v;
            }
        }
    }
    // Influence moves from what the save holds (the game starts a companion at 50 when met),
    // for the companions the party has once the skip is done; 0 to 100.
    for (slot, delta) in influence {
        if !save.recruited(slot) {
            continue;
        }
        let now = save.influence(slot).filter(|v| *v >= 0).unwrap_or(50);
        save.set_influence(slot, now + delta);
    }
    if credits != 0 {
        let gold = save.party_int("PT_GOLD").unwrap_or(0);
        save.set_party_int("PT_GOLD", (gold + credits).max(0));
        notes.push(format!("{} credits from the chapters' quests and conversations ({} side)", credits, if dark { "dark" } else { "light" }));
    }
    if shift != 0 {
        save.edit(Who::Player, "skip.alignment", |c| {
            let now = crate::creature::alignment(c);
            crate::creature::set_alignment(c, (now + shift).clamp(0, 100));
        });
    }
    // The XP of the chapters played, each counted once whatever the order, for the player and
    // the party pool; never lowered.
    let planet = last.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_string();
    let full_xp = if chapters.is_empty() { None } else { Some(xp_of(save.game, &run)) };
    if let Some(xp) = full_xp {
        save.edit(Who::Player, &format!("skip.{}.xp", planet.to_ascii_lowercase()), |c| {
            let now = crate::creature::experience(c);
            crate::creature::set_experience(c, now.max(xp));
        });
        let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
        save.set_party_int("PT_XP_POOL", pool.max(xp));
    }
    if train {
        notes.extend(train_companions(save, data));
    }
    // KotOR 2's prestige class: what a_pc_jedi does (AddMultiClass), and 000_Prestige 2 as
    // the dialog sets after the choice, when Kreia would have offered it by now.
    if let Some(class_id) = prestige {
        let xp = save.player().map(crate::creature::experience).unwrap_or(0);
        if prestige_due(save, data, xp) {
            let cat = crate::catalog::Catalog::build(data);
            if let Some(class) = cat.class(class_id).cloned() {
                add_multiclass_to_player(save, data, &cat, &class);
                notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does: vec![serde_json::json!({ "set_global_number": ["000_Prestige", 2] })], branch: None, found: Default::default() }, "skip.prestige.global"));
                notes.push(format!("Prestige class {} taken, as Kreia offers it at level 15", class.name));
            }
        }
    }
    if let Some((tag, stage)) = partial {
        // the stage's chapter: the quest up to it, then the trip to where it happens
        let (more, module, became_jedi) = apply_partial(save, data, &target, &tag, stage, dark);
        notes.extend(more);
        if became_jedi {
            notes.extend(apply_jedi(save, data, &target, jedi_class, &before));
        }
        notes.extend(count_star_maps(save, data, &target));
        let trip = vec![module.to_ascii_lowercase()];
        for m in &chapters {
            notes.extend(write_cleared(save, data, m, &trip));
        }
        for m in &chapters {
            notes.extend(close_quests(save, data, m, dark, &removed));
        }
        let waypoint = crate::map::any_waypoint(data, &module).unwrap_or_default();
        save.queue_transition(&module, &waypoint, &[])?;
        notes.dedup();
        return Ok(notes);
    }
    // The game's own first hop of the last chapter, with its movies.
    let next = last.get("next_chapter").ok_or("this milestone has no next chapter")?;
    let hop = next.get("chain").and_then(|c| c.as_array()).and_then(|c| c.first()).unwrap_or(next);
    let module = hop.get("module").and_then(|v| v.as_str()).ok_or("this milestone names no next area")?;
    let waypoint = hop.get("waypoint").and_then(|v| v.as_str()).unwrap_or_default();
    let movies: Vec<&str> = hop.get("movies").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
    let mut trip = vec![module.to_ascii_lowercase()];
    if let Some(final_module) = next.get("module").and_then(|v| v.as_str()) {
        trip.push(final_module.to_ascii_lowercase());
    }
    for m in &chapters {
        notes.extend(write_cleared(save, data, m, &trip));
    }
    for m in &chapters {
        notes.extend(close_quests(save, data, m, dark, &removed));
    }
    save.queue_transition(module, waypoint, &movies)?;
    notes.dedup();
    Ok(notes)
}

/// A quest of a chapter up to a stage, as if played, with the main-story stages its steps
/// need. Returns the notes, the module where the stage happens, and whether the route passes
/// the script that makes the player a Jedi.
fn apply_partial(save: &mut Save, data: &GameData, m: &serde_json::Value, tag: &str, stage: i64, dark: bool) -> (Vec<String>, String, bool) {
    let mut notes = Vec::new();
    let alignment = save.player().map_or(50, crate::creature::alignment);
    let mut became_jedi = false;
    let makes_jedi = m.get("jedi_class").and_then(|j| j.get("class_by")).and_then(|v| v.as_str()).and_then(|s| s.split(':').next()).map(|s| s.trim().to_ascii_lowercase());
    let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_string();
    let file_quests: Vec<Quest> = quest_files()
        .iter()
        .filter(|(g, f)| *g == save.game && f.planet.eq_ignore_ascii_case(&planet))
        .flat_map(|(_, f)| f.quests.iter().cloned())
        .collect();
    let find = |t: &str| file_quests.iter().find(|q| q.tag.eq_ignore_ascii_case(t)).cloned().or_else(|| quest(save.game, t).map(|(q, _)| q));
    // the quest and, before it, the stages of other quests its route needs (main story first)
    let mut plan: Vec<(Quest, i64)> = Vec::new();
    let mut want = vec![(tag.to_string(), stage)];
    while let Some((t, st)) = want.pop() {
        let Some(q) = find(&t) else { continue };
        if plan.iter().any(|(p, s)| p.tag.eq_ignore_ascii_case(&q.tag) && *s >= st) {
            continue;
        }
        for step in &one_way_in(&q, st, dark, alignment, &story_values(save)) {
            for n in &step.needs {
                if let (Some(sv), Some(op)) = (n.get("stage").and_then(|v| v.as_array()), n.get("op").and_then(|v| v.as_str())) {
                    if let (Some(nt), Some(nv)) = (sv.first().and_then(|v| v.as_str()), sv.get(1).and_then(|v| v.as_i64())) {
                        let need = match op { ">" => nv + 1, ">=" | "==" => nv, _ => continue };
                        if !nt.eq_ignore_ascii_case(&q.tag) && need > 0 {
                            want.push((nt.to_string(), need));
                        }
                    }
                }
            }
        }
        plan.insert(0, (q, st));
    }
    let mut module = String::new();
    let mut touched: Vec<String> = Vec::new();
    let before_journal = save.quests();
    for (q, st) in &plan {
        let now = before_journal.iter().find(|(t, _)| t.eq_ignore_ascii_case(&q.tag)).map_or(-1, |(_, v)| *v);
        if now >= *st {
            continue; // the save is there already: nothing given twice
        }
        for (i, step) in one_way_in(q, *st, dark, alignment, &story_values(save)).iter().enumerate() {
            // stages the save has passed already gave their rewards
            let reached = step.does.iter().any(|e| e.get("journal").and_then(|j| j.get(1)).and_then(|v| v.as_i64()).map_or(false, |v| v <= now));
            if reached {
                continue;
            }
            let found = area_items(data, m, &step.does);
            let step = Step { found, does: without_star_map_branches(save.game, m, &step.does), ..step.clone() };
            if makes_jedi.as_ref().map_or(false, |s| step.does.iter().any(|e| e.to_string().to_ascii_lowercase().contains(s.as_str()))) {
                became_jedi = true;
            }
            notes.extend(apply(save, data, &step, &format!("skip.stage.{}.{}", q.tag.to_ascii_lowercase(), i)).into_iter().filter(|n| !n.contains("happens when you first enter")));
            if !step.module.is_empty() {
                module = step.module.clone();
                if !touched.iter().any(|t| t.eq_ignore_ascii_case(&step.module)) {
                    touched.push(step.module.to_ascii_lowercase());
                }
            }
            notes.extend(recruits_in(save, data, m, &step));
        }
        if *st > save.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(&q.tag)).map_or(-1, |(_, v)| v) {
            save.set_quest(&q.tag, *st);
        }
    }
    // The main story's own events up to here: the critical path is in story order; every
    // companion whose joining script comes on it no later than the furthest area the quest
    // reached joins (a companion who joins on entering an area, not in a quest step).
    let critical = m.get("critical_path").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let reach = critical.iter().rposition(|e| e.get("module").and_then(|v| v.as_str()).map_or(false, |x| touched.iter().any(|t| t.eq_ignore_ascii_case(x))));
    // A side room need not be on the critical path, but reaching any quest stage in
    // the chapter still passes its entry (for example Carth joining at Taris's hideout).
    let entry_modules: Vec<String> = milestones_ref(save.game).iter()
        .filter(|(_, previous)| previous.get("order").and_then(|v| v.as_i64()).unwrap_or(99) < m.get("order").and_then(|v| v.as_i64()).unwrap_or(0))
        .filter_map(|(_, previous)| previous.get("next_chapter").and_then(|n| n.get("module")).and_then(|v| v.as_str()).map(str::to_ascii_lowercase)).collect();
    for r in m.get("party").and_then(|p| p.get("recruit")).and_then(|r| r.as_array()).cloned().unwrap_or_default() {
        let script = r.get("script").and_then(|v| v.as_str()).unwrap_or_default().split('|').next().unwrap_or_default().to_ascii_lowercase();
        let at = critical.iter().position(|e| e.get("source").and_then(|v| v.as_str()).map_or(false, |x| x.split('|').next().unwrap_or_default().eq_ignore_ascii_case(&script)));
        let at_entry = r.get("module").and_then(|v| v.as_str()).map_or(false, |module| entry_modules.iter().any(|e| e.eq_ignore_ascii_case(module)));
        if !at_entry && !at.zip(reach).map_or(false, |(a, reach)| a <= reach) {
            continue;
        }
        let (Some(slot), Some(template)) = (r.get("slot").and_then(|v| v.as_u64()), r.get("template").and_then(|v| v.as_str())) else { continue };
        let female = save.player().map_or(false, |p| p.int("Gender") == Some(1));
        match r.get("when").and_then(|w| w.as_str()) {
            Some("male") if female => continue,
            Some("female") if !female => continue,
            _ => {}
        }
        let step = Step { module: r.get("module").and_then(|v| v.as_str()).unwrap_or_default().to_string(), via: None, needs: Vec::new(),
            does: vec![serde_json::json!({ "other": format!("AddAvailableNPCByTemplate({}, \"{}\")", slot, template) })], branch: None, found: Default::default() };
        notes.extend(recruits_in(save, data, m, &step));
    }
    if module.is_empty() {
        module = m.get("cleared").and_then(|c| c.as_array()).and_then(|c| c.first()).and_then(|c| c.get("module")).and_then(|v| v.as_str()).unwrap_or_default().to_string();
    }
    notes.push(format!("{}: {} quests played up to the picked stage", planet, plan.len()));
    (notes, module, became_jedi)
}

fn removed_by(m: &serde_json::Value) -> Vec<String> {
    m.get("journal_remove").and_then(|j| j.as_array()).into_iter().flatten().filter_map(|t| t.as_str().map(str::to_string)).collect()
}

/// The XP a full run of `planets` (chapters, in any order) ends with: each chapter's own share
/// once. What Skip Ahead gives (never less than the player has).
pub fn xp_after(game: Game, planets: &[String]) -> i64 {
    xp_of(game, planets)
}

/// The XP of a set of chapters. The rewards files hold running totals along the timeline; one
/// chapter's own share is its total less the one before it, so a save that played the free
/// planets in another order counts each planet once, and the Leviathan where it belongs. A
/// side chapter's share is its total less the largest main total under it.
fn xp_of(game: Game, planets: &[String]) -> i64 {
    let total = |p: &str| rewards(game, p).and_then(|r| r.get("xp_at_end").and_then(|v| v.as_i64()));
    let mut mains: Vec<(i64, &str, i64)> = milestones_ref(game).iter()
        .filter(|(_, m)| m.get("side_chapter").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|(p, m)| Some((m.get("order").and_then(|v| v.as_i64()).unwrap_or(99), *p, total(p)?)))
        .collect();
    mains.sort_by_key(|x| x.0);
    let share = |planet: &str| -> i64 {
        if let Some(k) = mains.iter().position(|x| x.1.eq_ignore_ascii_case(planet)) {
            return mains[k].2 - if k == 0 { 0 } else { mains[k - 1].2 };
        }
        let Some(t) = total(planet) else { return 0 };
        t - mains.iter().map(|x| x.2).filter(|x| *x <= t).max().unwrap_or(0)
    };
    let mut seen: Vec<String> = Vec::new();
    planets.iter().filter(|p| {
        let new = !seen.iter().any(|s| s.eq_ignore_ascii_case(p));
        if new {
            seen.push(p.to_string());
        }
        new
    }).map(|p| share(p)).sum()
}

/// Which chapters a save has played, by milestone index: those whose journal entries it has
/// reached on either side, and those whose journal quests a chapter it has played takes out
/// of the journal (the game removes the Endar Spire's quest on Taris, so a save past Taris no
/// longer shows it). `k_starforge` is left out on a star map planet: it holds the last map
/// found, so an earlier planet would read unplayed. KotOR 2's chapters count their main-path
/// quests alone, those their critical path names: most of Telos's list is side content.
pub fn chapters_played(game: Game, quests: &[(String, i64)]) -> Vec<bool> {
    let all = milestones_ref(game);
    let tags = |m: &serde_json::Value, journal: Option<&serde_json::Value>| -> Vec<(String, i64)> {
        let mut out: Vec<(String, i64)> = journal
            .and_then(|j| j.as_array())
            .map(|a| a.iter().filter_map(|e| Some((e.get("tag")?.as_str()?.to_string(), e.get("stage").and_then(|v| v.as_i64()).unwrap_or(0)))).collect())
            .unwrap_or_default();
        if m.get("star_map").map_or(false, |s| !s.is_null()) {
            out.retain(|(t, _)| !t.eq_ignore_ascii_case("k_starforge"));
        }
        if game == Game::K2 {
            let main = critical_stages(m);
            let kept: Vec<(String, i64)> = out.iter().filter_map(|(t, _)| main.iter().find(|(c, _)| c.eq_ignore_ascii_case(t)).cloned()).collect();
            if !kept.is_empty() {
                return kept;
            }
        }
        out
    };
    let reached = |tag: &str, stage: i64| quests.iter().any(|(t, v)| t.eq_ignore_ascii_case(tag) && *v >= stage);
    let needs: Vec<Vec<(String, i64)>> = all.iter().map(|(_, m)| tags(m, m.get("journal"))).collect();
    let darks: Vec<Vec<(String, i64)>> = all.iter().map(|(_, m)| tags(m, m.get("dark").and_then(|d| d.get("journal")))).collect();
    let removes: Vec<Vec<String>> = all.iter().map(|(_, m)| removed_by(m)).collect();
    let done = |n: &Vec<(String, i64)>| !n.is_empty() && n.iter().all(|(t, s)| reached(t, *s));
    let mut played: Vec<bool> = needs.iter().zip(&darks).map(|(n, d)| done(n) || done(d)).collect();
    loop {
        let mut more = false;
        for i in 0..all.len() {
            if played[i] || needs[i].is_empty() {
                continue;
            }
            let gone = |t: &String| (0..all.len()).any(|j| played[j] && removes[j].iter().any(|r| r.eq_ignore_ascii_case(t)));
            if needs[i].iter().all(|(t, s)| reached(t, *s) || gone(t)) && needs[i].iter().any(|(t, _)| gone(t)) {
                played[i] = true;
                more = true;
            }
        }
        if !more {
            break;
        }
    }
    let known = played.clone();
    for (i, k) in known.iter().enumerate() {
        if *k {
            line_before(game, &mut played, i);
        }
    }
    played
}

/// Outside the free planets the story is one line, so being past chapter `i` (milestone index)
/// means every chapter before it on that line was played, though the game took its quests out
/// of the journal (a KotOR 1 save on Korriban had no Dantooine entry left). A chapter after the
/// free planets means all of them, except KotOR 1's Leviathan, which comes after the third.
fn line_before(game: Game, played: &mut [bool], i: usize) {
    let all = milestones_ref(game);
    let order_of = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let side = |m: &serde_json::Value| m.get("side_chapter").and_then(|v| v.as_bool()) == Some(true);
    let Some((planet, m)) = all.get(i) else { return };
    if side(m) {
        return;
    }
    // a free planet's group comes as one: what comes before it in its group was played
    if let Some((_, group, _)) = FREE.iter().find(|(g, ps, _)| *g == game && ps.iter().any(|p| p.eq_ignore_ascii_case(planet))) {
        for p in group.iter().take_while(|p| !p.eq_ignore_ascii_case(planet)) {
            if let Some(j) = all.iter().position(|(x, _)| x.eq_ignore_ascii_case(p)) {
                played[j] = true;
            }
        }
    }
    let free_max = all.iter().filter(|(p, _)| is_free(game, p)).map(|(_, m)| order_of(m)).max().unwrap_or(0);
    let after_free = order_of(m) > free_max && !(game == Game::K1 && planet.eq_ignore_ascii_case("Leviathan"));
    for (j, (other, o)) in all.iter().enumerate() {
        if side(o) || order_of(o) >= order_of(m) {
            continue;
        }
        // KotOR 1's Leviathan sits after the free planets in the files, but before the fourth in
        // the game: what comes after it says nothing about it unless after_free
        let leviathan = game == Game::K1 && other.eq_ignore_ascii_case("Leviathan");
        if (!is_free(game, other) && !leviathan) || after_free {
            if let Some(x) = played.get_mut(j) {
                *x = true;
            }
        }
    }
}

/// Which chapters a save has played: chapters_played by its journal, KotOR 1's free planets
/// whose star map is found, and every chapter before the one it stands in on the story's line
/// (a KotOR II save on Telos has finished Peragus, though the journal may not reach Peragus's
/// end).
pub fn chapters_played_in(save: &Save) -> Vec<bool> {
    let mut played = chapters_played(save.game, &save.quests());
    // KotOR 1: a planet whose star map is found has had its story played, though side quests
    // there may be open; Skip Ahead must not play it again (its loot and credits twice)
    if save.game == Game::K1 {
        let globals = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
        let all = milestones_ref(save.game);
        for (g, planets, flag) in FREE {
            if *g != Game::K1 || !globals.iter().any(|v| v.name.eq_ignore_ascii_case(flag) && v.value == crate::globals::GlobalValue::Bool(true)) {
                continue;
            }
            for p in planets.iter() {
                if let Some(i) = all.iter().position(|(x, _)| x.eq_ignore_ascii_case(p)) {
                    played[i] = true;
                    line_before(save.game, &mut played, i);
                }
            }
        }
    }
    // Where the save stands says what was played only while no trip waits in its queue. Go
    // Back leaves the save in the area gone back from and queues a trip (mostly to the Ebon
    // Hawk or a cutscene area, not the chapter gone back to), and Travel Here can queue one to
    // a later planet: either place would read chapters played that were not. With a trip
    // waiting the journal alone decides. (2026-11-06, corrected twice: first the trip's area
    // was used, then the earlier of the two; both read undone chapters played again.)
    let here = if save.queued_trip().is_some() { None } else { chapter_of_module(save.game, save.module_name()) };
    if let Some(here) = here {
        line_before(save.game, &mut played, here);
    }
    played
}

/// The journal stages a chapter's critical path names ("the arrest: journal arrested 10"),
/// the highest of each quest.
fn critical_stages(m: &serde_json::Value) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    for e in m.get("critical_path").and_then(|c| c.as_array()).into_iter().flatten() {
        let Some(what) = e.get("what").and_then(|v| v.as_str()) else { continue };
        for part in what.split("journal ").skip(1) {
            let mut w = part.split_whitespace();
            let (Some(tag), Some(stage)) = (w.next(), w.next().and_then(|s| s.trim_end_matches(|c: char| !c.is_ascii_digit()).parse::<i64>().ok())) else { continue };
            match out.iter_mut().find(|(t, _)| t.eq_ignore_ascii_case(tag)) {
                Some(x) => x.1 = x.1.max(stage),
                None => out.push((tag.to_string(), stage)),
            }
        }
    }
    out
}

/// A chapter on the timeline, as far as what a skip or a return covers depends on it.
#[derive(Clone, Copy, Debug)]
pub struct ChapterState {
    pub index: i32,
    pub order: i64,
    pub side: bool,
    pub played: bool,
    /// not played, but the save has been there (the chapter it is in, or a free planet left
    /// part way)
    pub begun: bool,
}

/// The chapters a skip to chapter `index` applies, in story order: every main chapter before
/// it this save has not played, then it. A side chapter (the Ebon Hawk, Yavin Station) stands
/// alone: it comes last on the timeline only because it has no place on it, so everything
/// counted as before it would be the whole game.
pub fn skip_path(chapters: &[ChapterState], index: i32) -> Vec<i32> {
    let Some(target) = chapters.iter().find(|c| c.index == index) else { return Vec::new() };
    if target.side {
        return vec![index];
    }
    let mut out: Vec<i32> = chapters.iter().filter(|c| c.index != index && !c.played && !c.side && c.order < target.order).map(|c| c.index).collect();
    out.push(index);
    out
}

/// What a return to chapter `index` keeps and undoes, in route order, and the chapters among
/// those undone that were begun and never finished.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Return {
    pub kept: Vec<i32>,
    pub undo: Vec<i32>,
    pub unfinished: Vec<i32>,
}

/// Side chapters stay played when a return goes to a main chapter, and a return to a side
/// chapter undoes that chapter alone: neither lies on the line of the main story. A return to
/// a main chapter undoes it and every chapter after it on the route that is played or begun.
///
/// Correction, 2026-11-06 (audit): until then the chapter the player was in was taken to be the
/// first unplayed one on the route. With two free planets begun, a return to the second undid
/// nothing ("nothing to go back over"), and a return to the first left the second as it was.
pub fn return_to(chapters: &[ChapterState], index: i32) -> Return {
    let Some(target) = chapters.iter().find(|c| c.index == index) else { return Return::default() };
    if target.side {
        let kept = chapters.iter().filter(|c| c.played && c.index != index).map(|c| c.index).collect();
        return Return { kept, undo: vec![index], unfinished: Vec::new() };
    }
    let mut kept: Vec<i32> = chapters.iter().filter(|c| c.played && !c.side && c.order < target.order).map(|c| c.index).collect();
    kept.extend(chapters.iter().filter(|c| c.played && c.side).map(|c| c.index));
    let undo: Vec<i32> = chapters
        .iter()
        .filter(|c| !c.side && c.order >= target.order && (c.played || c.begun || c.index == index))
        .map(|c| c.index)
        .collect();
    let unfinished = chapters.iter().filter(|c| undo.contains(&c.index) && !c.played).map(|c| c.index).collect();
    Return { kept, undo, unfinished }
}

/// The planets a player may fly to in any order, each group moving as one (KotOR 2's Dxun
/// leads straight into Onderon), with the story value the game leaves once the planet is done:
/// KotOR 1's star map flag (TRUE), KotOR 2's marker of the master found there (above 0; the
/// list in tools/questdb/chapters_k2/the_jedi_masters.py, STAR_MAP_RULE).
const FREE: &[(Game, &[&str], &str)] = &[
    (Game::K1, &["Tatooine"], "K_STAR_MAP_TATOOINE"),
    (Game::K1, &["Kashyyyk"], "K_STAR_MAP_KASHYYYK"),
    (Game::K1, &["Manaan"], "K_STAR_MAP_MANAAN"),
    (Game::K1, &["Korriban"], "K_STAR_MAP_KORRIBAN"),
    (Game::K2, &["Nar Shaddaa"], "351NAR_Ebon_Released"),
    (Game::K2, &["Dxun", "Onderon"], "506OND_End"),
    (Game::K2, &["Dantooine"], "602DAN_End"),
    (Game::K2, &["Korriban"], "702KOR_Vash"),
];

/// Whether `planet` is one of the planets the player may visit in any order.
pub fn is_free(game: Game, planet: &str) -> bool {
    FREE.iter().any(|(g, planets, _)| *g == game && planets.iter().any(|p| p.eq_ignore_ascii_case(planet)))
}

/// The chapter (milestone index) whose areas include `module`, by the areas each chapter's data
/// lists as its own ("cleared"); the Ebon Hawk's areas give the Ebon Hawk's side chapter.
pub fn chapter_of_module(game: Game, module: &str) -> Option<usize> {
    milestones_ref(game).iter().position(|(_, m)| {
        m.get("cleared").and_then(|c| c.as_array()).map_or(false, |c| {
            c.iter().any(|e| e.get("module").and_then(|v| v.as_str()).map_or(false, |x| x.eq_ignore_ascii_case(module)))
        })
    })
}

/// The order this save plays the story in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    /// The main chapters by milestone index, in the order of the story as this save plays it.
    pub order: Vec<usize>,
    /// The free planets as groups (milestone indices), in the route's order.
    pub groups: Vec<Vec<usize>>,
    /// Per group: 0 done, 1 begun, 2 not yet visited. What is done stays before the rest, and
    /// the player's choice moves a done group among the done and any other among the others
    /// (a planet just landed on can wait while another is skipped to). Without a choice, what
    /// is begun comes before what is not visited.
    pub kinds: Vec<u8>,
}

/// Whether a save has been to any of the chapter's own areas (milestone index `i`): `visited`
/// is what the save keeps of the areas it has been to (Save::visited_modules) and the area it
/// is in. A quest of the chapter's in the journal says less: KotOR II hands out a Nar Shaddaa
/// quest on Telos.
pub fn been_to(game: Game, i: usize, visited: &[String]) -> bool {
    milestones_ref(game).get(i).map_or(false, |(_, m)| {
        m.get("cleared").and_then(|c| c.as_array()).map_or(false, |c| {
            c.iter().filter_map(|e| e.get("module").and_then(|v| v.as_str())).any(|x| visited.iter().any(|v| v.eq_ignore_ascii_case(x)))
        })
    })
}

/// The story's main chapters in the order this save plays them. Outside the free planets the
/// story is one line. The free planets come in the order the save went to them: those done
/// first (the game's flag, or the chapter played), then those begun (the save has been to one
/// of their areas, `visited`: see been_to), each in the order their own quests first appear in
/// the journal, which keeps new entries at its end; then the ones not yet visited in the order
/// of `plan` (planet names; the timeline's order without one). `plan` also reorders the done
/// and the begun among themselves, where the journal guessed wrong. KotOR 1's Leviathan comes
/// after the third free planet, as the game plays it (the capture starts at the take-off after
/// the third star map). `played` is chapters_played_in, or chapters_played without a save.
pub fn story_route(game: Game, quests: &[(String, i64)], played: &[bool], globals: &[crate::globals::Global], visited: &[String], plan: &[String]) -> Route {
    let all = milestones_ref(game);
    let order_of = |m: &serde_json::Value| m.get("order").and_then(|v| v.as_i64()).unwrap_or(99);
    let side = |m: &serde_json::Value| m.get("side_chapter").and_then(|v| v.as_bool()) == Some(true);
    let mut mains: Vec<(i64, usize)> = all.iter().enumerate().filter(|(_, (_, m))| !side(m)).map(|(i, (_, m))| (order_of(m), i)).collect();
    mains.sort();
    let index = |planet: &str| all.iter().position(|(p, _)| p.eq_ignore_ascii_case(planet));
    // every chapter's own quests, once
    let owns: Vec<Vec<String>> = all.iter().map(|(p, _)| chapter_quests(game, p).into_iter().map(|(t, _)| t.to_ascii_lowercase()).collect()).collect();
    let mut groups: Vec<(u8, usize, usize, i64, Vec<usize>, String)> = Vec::new();
    for (_, planets, flag) in FREE.iter().filter(|(g, _, _)| *g == game) {
        let members: Vec<usize> = planets.iter().filter_map(|p| index(p)).collect();
        if members.is_empty() {
            continue;
        }
        let flag_on = globals.iter().any(|v| {
            v.name.eq_ignore_ascii_case(flag)
                && match v.value {
                    crate::globals::GlobalValue::Bool(b) => b,
                    crate::globals::GlobalValue::Number(n) => n > 0,
                    _ => false,
                }
        });
        // the group's own quests (a quest more than one chapter shares says nothing)
        let at = quests.iter().position(|(t, _)| {
            let t = t.to_ascii_lowercase();
            members.iter().any(|i| owns[*i].contains(&t)) && owns.iter().filter(|o| o.contains(&t)).count() == 1
        });
        // done is what Skip Ahead counts as played (chapters_played_in, where KotOR 1's star map
        // flag already counts); KotOR 2's marker can be set before its planet's end, so there
        // it says begun
        let done = members.iter().all(|i| played.get(*i).copied().unwrap_or(false));
        let kind = if done { 0 } else if flag_on || members.iter().any(|i| played.get(*i).copied().unwrap_or(false) || been_to(game, *i, visited)) { 1 } else { 2 };
        let base = members.iter().map(|i| order_of(all[*i].1)).min().unwrap_or(99);
        groups.push((kind, at.unwrap_or(usize::MAX), 0, base, members, planets[0].to_string()));
    }
    for g in groups.iter_mut() {
        g.2 = plan.iter().position(|p| p.eq_ignore_ascii_case(&g.5)).unwrap_or(usize::MAX);
    }
    // the plan first where it names the group; else the journal's order (done and begun) or
    // the timeline's (not yet visited)
    groups.sort_by(|a, b| {
        let done = |g: &(u8, usize, usize, i64, Vec<usize>, String)| g.0 != 0;
        done(a).cmp(&done(b))
            .then(a.2.cmp(&b.2))
            .then(a.0.cmp(&b.0))
            .then(if a.0 < 2 { a.1.cmp(&b.1) } else { std::cmp::Ordering::Equal })
            .then(a.3.cmp(&b.3))
    });
    let flat: Vec<usize> = groups.iter().flat_map(|g| g.4.clone()).collect();
    // the free planets take the places the timeline gives them, in the route's order
    let mut slots = mains.iter().enumerate().filter(|(_, (_, i))| flat.contains(i)).map(|(k, _)| k);
    let mut order: Vec<usize> = mains.iter().map(|(_, i)| *i).collect();
    for i in &flat {
        if let Some(k) = slots.next() {
            order[k] = *i;
        }
    }
    if game == Game::K1 {
        if let Some(lev) = index("Leviathan") {
            let third = groups.get(2).and_then(|g| g.4.last().copied());
            if let (Some(from), Some(third)) = (order.iter().position(|i| *i == lev), third) {
                order.remove(from);
                let to = order.iter().position(|i| *i == third).map_or(order.len(), |k| k + 1);
                order.insert(to, lev);
            }
        }
    }
    Route { order, groups: groups.iter().map(|g| g.4.clone()).collect(), kinds: groups.iter().map(|g| g.0).collect() }
}

/// The companions a quest step brings into the party (AddAvailableNPCByTemplate, or KotOR 2's
/// AddAvailableNPCByObject with the template the chapter's milestone names for that slot).
fn recruits_in(save: &mut Save, data: &GameData, m: &serde_json::Value, step: &Step) -> Vec<String> {
    let mut notes = Vec::new();
    for e in &step.does {
        let Some(text) = e.get("other").and_then(|v| v.as_str()) else { continue };
        let by_template = text.strip_prefix("AddAvailableNPCByTemplate(");
        let by_object = text.strip_prefix("AddAvailableNPCByObject(");
        let Some(rest) = by_template.or(by_object) else { continue };
        let Some(slot) = rest.split(',').next().and_then(|n| n.trim().parse::<usize>().ok()) else { continue };
        let template = if by_template.is_some() {
            rest.split('"').nth(1).map(|t| t.to_string())
        } else {
            m.get("party").and_then(|p| p.get("recruit")).and_then(|r| r.as_array()).and_then(|r| {
                r.iter().find(|x| x.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64)).and_then(|x| x.get("template").and_then(|v| v.as_str()).map(|t| t.to_string()))
            })
        };
        let Some(template) = template else {
            // KotOR II's AddAvailableNPCByObject names a creature in the area, and the chapter
            // has no template for the slot: said, as before, not dropped (second review)
            notes.push(format!("Left to the game: party member {} joins", slot));
            continue;
        };
        // recruit_from_template keeps the same companion and replaces anyone else in the slot
        let module = step.module.clone();
        let utc = (!module.is_empty())
            .then(|| crate::map::ModuleFiles::open(&data.root, &module))
            .and_then(|f| f.read(&template, crate::formats::erf::RES_TYPE_UTC))
            .and_then(|d| crate::formats::gff::Gff::read(&d).ok())
            .or_else(|| data.gff(&template, "utc", crate::formats::erf::RES_TYPE_UTC));
        match utc {
            Some(u) => {
                if save.recruit_from_template(slot, &u.root).is_ok() {
                    notes.push(format!("Party member {} joins ({})", slot, template));
                }
            }
            None => notes.push(format!("Party member template {} not found in the game", template)),
        }
    }
    notes
}

/// The form a master teaches (a_give_form, KotOR 2): by the prestige class (`pre`, 255 none),
/// the base class, how many forms were taught before (`form`, G_PC_FORM), the side, and the
/// forms already known. spells.2da rows.
fn form_rule(pre: i64, base: i64, form: i64, evil: bool, known: &[i64]) -> Option<i64> {
    let acq = |s: i64| known.contains(&s);
    let consular = |f: i64| -> Option<i64> {
        match f {
            0 => Some(if evil { 266 } else { 267 }),
            1 => Some(if evil { if !acq(266) { 266 } else { 267 } } else if !acq(267) { 267 } else { 266 }),
            2 => Some(268),
            _ => None,
        }
    };
    let spell = match (pre, base, form) {
        (255, 3, 0) => Some(261),
        (255, 3, 1) => Some(262),
        (255, 3, 2) => Some(264),
        (255, 5, 0) => Some(262),
        (255, 5, 1) => Some(261),
        (255, 5, 2) => Some(263),
        (255, 4, f) => consular(f),
        (11 | 14, _, 0) => Some(261),
        (11 | 14, _, 1) => Some(if !acq(261) { 261 } else { 262 }),
        (11 | 14, _, 2) => Some(264),
        (13 | 16, _, 0) => Some(262),
        (13 | 16, _, 1) => Some(if !acq(262) { 262 } else { 261 }),
        (13 | 16, _, 2) => Some(263),
        (12 | 15, _, f) => consular(f),
        _ => None,
    };
    spell
}

/// KotOR 2: the form a Jedi master teaches in the chapter (a_give_form, read 2026-10-23: the
/// player's classes, G_PC_FORM, the alignment and the forms already known decide; spells.2da
/// rows) and the powers and feats the story grants ("grants", by side).
fn apply_k2_powers(save: &mut Save, data: &GameData, m: &serde_json::Value, dark: bool, met: bool) -> Vec<String> {
    let mut notes = Vec::new();
    let side = if dark { "dark" } else { "light" };
    let force_class = |id: i64| [3, 4, 5, 11, 12, 13, 14, 15, 16].contains(&id);
    let add = |save: &mut Save, spell: Option<i64>, feat: Option<i64>, key: &str| {
        save.edit(Who::Player, key, |c| {
            if let Some(sp) = spell {
                crate::creature::add_power(c, sp, &force_class);
            }
            if let Some(f) = feat {
                crate::creature::add_feat(c, f);
            }
        });
    };
    for g in m.get("grants").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        if g.get("side").and_then(|v| v.as_str()).map_or(false, |s| s != side) {
            continue;
        }
        let spell = g.get("spell").and_then(|v| v.as_i64());
        let feat = g.get("feat").and_then(|v| v.as_i64());
        add(save, spell, feat, &format!("skip.grant.{:?}.{:?}", spell, feat));
    }
    if m.get("teach_form").and_then(|v| v.as_bool()) != Some(true) || met {
        return notes;
    }
    let Some(p) = save.player().cloned() else { return notes };
    let classes = crate::creature::classes(&p);
    let base = classes.first().map(|c| c.0).unwrap_or(-1);
    let pre = classes.get(1).map(|c| c.0).unwrap_or(255);
    let globals = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    let form = globals.iter().find(|g| g.name.eq_ignore_ascii_case("G_PC_FORM")).and_then(|g| match g.value {
        crate::globals::GlobalValue::Number(n) => Some(n),
        _ => None,
    }).unwrap_or(0);
    // the alignment a run of this side has reached (GetGoodEvilValue < 50 is the dark branch)
    let evil = dark;
    let known = crate::creature::powers(&p);
    let spell = form_rule(pre, base, form, evil, &known);
    let planet = m.get("planet").and_then(|v| v.as_str()).unwrap_or_default().to_ascii_lowercase();
    match spell {
        Some(sp) => {
            add(save, Some(sp), None, &format!("skip.{}.form", planet));
            notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does: vec![serde_json::json!({ "set_global_number": ["G_PC_FORM", form + 1] })], branch: None, found: Default::default() }, &format!("skip.{}.formcount", planet)));
        }
        None => notes.push("No form taught: the player's class or the teaching count is outside the game's table".to_string()),
    }
    notes
}

/// Closes the chapter's own quests the way a full run leaves them, so their NPCs do not offer
/// the talks again whose rewards the skip already gave: the state effects of the route to each
/// quest's ending (the milestone's stage for it, else the run's ending): the journal, story
/// values, the NPCs' own flags, doors, people who leave. Credits, items, XP and alignment are
/// left out (the rewards file counts them). Runs after the areas are stored, so the flags of an
/// area stored as played through land in it too.
fn close_quests(save: &mut Save, data: &GameData, m: &serde_json::Value, dark: bool, removed: &[String]) -> Vec<String> {
    const STATE: [&str; 7] = ["journal", "set_global_bool", "set_global_number", "local", "destroy", "door_open", "door_unlock"];
    let alignment = save.player().map_or(50, crate::creature::alignment);
    let gone = |tag: &str| removed.iter().any(|r| r.eq_ignore_ascii_case(tag));
    let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default();
    let milestone_stage = |tag: &str| {
        m.get("journal").and_then(|j| j.as_array()).and_then(|j| {
            j.iter().find(|e| e.get("tag").and_then(|t| t.as_str()).map_or(false, |t| t.eq_ignore_ascii_case(tag))).and_then(|e| e.get("stage").and_then(|s| s.as_i64()))
        })
    };
    // What the milestone sets stands: this runs after every chapter of the skip, so a route's
    // write here would undo a later chapter's (Dantooine's K_STAR_MAP 10 over the count).
    let mut settled: Vec<String> = m.get("globals").and_then(|g| g.as_array()).into_iter().flatten()
        .filter(|g| g.get("value").map_or(false, |v| !v.is_null()))
        .filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(str::to_string)).collect();
    // The Jedi path is the player's pick: a route's write would leave it always the last one.
    settled.extend(m.get("jedi_class").and_then(|j| j.get("variable")).and_then(|v| v.as_str()).map(str::to_string));
    let mut closed = 0;
    let mut later = 0;
    for (g, file) in quest_files() {
        if *g != save.game {
            continue;
        }
        if !file.planet.eq_ignore_ascii_case(planet) {
            continue;
        }
        for q in file.quests.iter().filter(|q| q.own) {
            let Some(ending) = milestone_stage(&q.tag).or(q.run_ending) else { continue };
            // no story values: here they stand at the chapter's end already, so tests of what the
            // chapter's start held would read wrong (2026-11-07 second review)
            for (i, st) in one_way_in(q, ending, dark, alignment, &[]).iter().enumerate() {
                let does: Vec<serde_json::Value> = without_star_map_branches(save.game, m, &st.does).into_iter()
                    .filter(|e| e.as_object().and_then(|o| o.keys().next()).map_or(false, |k| STATE.contains(&k.as_str())))
                    .filter(|e| !e.get("journal").and_then(|j| j.get(0)).and_then(|t| t.as_str()).map_or(false, |t| gone(t)))
                    // a route's step does not take a quest past where the milestone leaves it
                    .filter(|e| e.get("journal").and_then(|j| Some((j.get(0)?.as_str()?, j.get(1)?.as_i64()?))).map_or(true, |(t, s)| milestone_stage(t).map_or(true, |top| s <= top)))
                    .filter(|e| !global_written(e).map_or(false, |n| settled.iter().any(|s| s.eq_ignore_ascii_case(n))))
                    .collect();
                if does.is_empty() {
                    continue;
                }
                let step = Step { module: st.module.clone(), via: None, needs: Vec::new(), does, branch: None, found: Default::default() };
                later += apply(save, data, &step, &format!("skip.close.{}.{}", q.tag.to_ascii_lowercase(), i)).len();
            }
            let now = save.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(&q.tag)).map(|(_, v)| v).unwrap_or(-1);
            if ending > now && !gone(&q.tag) {
                save.set_quest(&q.tag, ending);
            }
            closed += 1;
        }
    }
    let mut notes = Vec::new();
    if closed > 0 {
        notes.push(format!("{}: {} quests closed as a full run ends them, so their people do not offer them again{}", planet, closed,
            if later > 0 { format!(" ({} of their changes wait in areas the save has not been to; the game sets those itself there)", later) } else { String::new() }));
    }
    notes
}

/// A global the step writes, by name.
fn global_written(effect: &serde_json::Value) -> Option<&str> {
    effect.get("set_global_bool").or_else(|| effect.get("set_global_number")).and_then(|a| a.get(0)).and_then(|n| n.as_str())
}

/// A quest step's effects, without the other branches of the game's star map script. The
/// script (UT_StarMap1VariableSet) sets one planet's flag and what the new count calls for,
/// but the quest data lists every branch in one step, so replaying that step found all four
/// maps, opened the Unknown World and queued the capture at the first planet (measured
/// 2026-11-02). What is left of such a step is the chapter's own flag; `count_star_maps`
/// sets the rest. A step that is not the script's is returned as it is.
fn without_star_map_branches(game: Game, m: &serde_json::Value, does: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let all = milestones_ref(game);
    let Some(sm) = all.iter().find_map(|(_, v)| v.get("star_map").filter(|s| !s.is_null())) else { return does.to_vec() };
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_ascii_lowercase();
    let counter = sm.get("counter").map(text).unwrap_or_default();
    let flags: Vec<String> = sm.get("planet_flags").and_then(|f| f.as_array()).map(|f| f.iter().map(text).collect()).unwrap_or_default();
    let mut names = flags.clone();
    names.push(counter.clone());
    for on in sm.get("on_count").and_then(|v| v.as_array()).into_iter().flatten() {
        names.extend(on.get("set").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|st| st.get("name")).map(text));
    }
    let is_script = does.iter().any(|e| {
        global_written(e).map_or(false, |n| flags.contains(&n.to_ascii_lowercase()))
            || e.get("other").and_then(|o| o.as_str()).map_or(false, |o| o.to_ascii_lowercase().contains(&format!("\"{}\"", counter)))
    });
    if !is_script {
        return does.to_vec();
    }
    let own = m.get("star_map").and_then(|s| s.get("planet_flag")).map(text).unwrap_or_default();
    let journal = sm.get("journal").and_then(|j| j.get(0)).map(text).unwrap_or_default();
    does.iter()
        .filter(|e| {
            if let Some(n) = global_written(e) {
                let n = n.to_ascii_lowercase();
                return n == own || !names.contains(&n);
            }
            e.get("journal").and_then(|j| j.get(0)).map(text) != Some(journal.clone())
        })
        .cloned()
        .collect()
}

/// Chapters in the order the game plays them. The timeline lists KotOR 1's four free planets
/// and then the Leviathan, but the game plays the capture after the third map, and its
/// take-off script only starts it while K_STAR_MAP is 40: past that it never plays, and
/// Bastila stays. So the Leviathan goes in before a fourth planet, and a skip to the
/// Leviathan itself leaves the fourth planet to play, as the game does.
fn leviathan_in_place(save: &Save, chapters: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let name = |m: &serde_json::Value| m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().eq_ignore_ascii_case("Leviathan");
    let globals = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    let is_on = |flag: &str| globals.iter().any(|g| g.name.eq_ignore_ascii_case(flag) && g.value == crate::globals::GlobalValue::Bool(true));
    let captured = globals.iter().any(|g| g.name.eq_ignore_ascii_case("K_CAPTURED_LEV") && matches!(g.value, crate::globals::GlobalValue::Number(n) if n >= 10));
    let all = milestones_ref(save.game);
    let (Some(leviathan), false) = (all.iter().find(|(_, m)| name(m)).map(|(_, m)| (*m).clone()), captured) else { return chapters.to_vec() };
    let flag = |m: &serde_json::Value| m.get("star_map").and_then(|s| s.get("planet_flag")).and_then(|f| f.as_str()).map(str::to_string);
    let to_leviathan = chapters.last().map_or(false, |m| name(m));
    let mut found = all.iter().filter_map(|(_, m)| flag(m)).filter(|f| is_on(f)).count();
    let mut out: Vec<serde_json::Value> = Vec::new();
    for m in chapters {
        if name(m) {
            continue;
        }
        if flag(m).map_or(false, |f| !is_on(&f)) {
            if found == 3 {
                if to_leviathan {
                    continue;
                }
                if !out.iter().any(|o| name(o)) {
                    out.push(leviathan.clone());
                }
            }
            found += 1;
        }
        out.push(m.clone());
    }
    if chapters.iter().any(|m| name(m)) && !out.iter().any(|o| name(o)) {
        out.push(leviathan);
    }
    out
}

/// The star maps, counted the game's way: the counter is the base and so much per planet
/// whose map is found, and reaching a count sets what the game's own script sets then
/// (the Leviathan capture after the third planet, the Unknown World after the fourth).
fn count_star_maps(save: &mut Save, data: &GameData, m: &serde_json::Value) -> Vec<String> {
    let mut notes = Vec::new();
    let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_ascii_lowercase();
    if let Some(sm) = m.get("star_map").filter(|v| !v.is_null()) {
        let flags: Vec<String> = sm.get("planet_flags").and_then(|f| serde_json::from_value(f.clone()).ok()).unwrap_or_default();
        let found = save
            .globals
            .as_ref()
            .map(|g| crate::globals::list(&g.root))
            .unwrap_or_default()
            .iter()
            .filter(|g| flags.iter().any(|f| f.eq_ignore_ascii_case(&g.name)) && g.value == crate::globals::GlobalValue::Bool(true))
            .count() as i64;
        let count = sm.get("base").and_then(|v| v.as_i64()).unwrap_or(10) + sm.get("per_planet").and_then(|v| v.as_i64()).unwrap_or(10) * found;
        let mut does = Vec::new();
        if let Some(counter) = sm.get("counter").and_then(|v| v.as_str()) {
            does.push(serde_json::json!({ "set_global_number": [counter, count] }));
        }
        for on in sm.get("on_count").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            if on.get("count").and_then(|v| v.as_i64()) != Some(count) {
                continue;
            }
            let now = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
            for st in on.get("set").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
                let (Some(name), Some(value)) = (st.get("name").and_then(|v| v.as_str()), st.get("value").and_then(|v| v.as_i64())) else { continue };
                if let Some(below) = st.get("only_if_below").and_then(|v| v.as_i64()) {
                    let cur = now.iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| match g.value {
                        crate::globals::GlobalValue::Number(n) => Some(n),
                        _ => None,
                    });
                    if cur.map_or(false, |c| c >= below) {
                        continue;
                    }
                }
                does.push(serde_json::json!({ "set_global_number": [name, value] }));
            }
            if let Some(j) = on.get("journal").and_then(|v| v.as_array()) {
                if let (Some(tag), Some(stage)) = (j.first().and_then(|v| v.as_str()), j.get(1).and_then(|v| v.as_i64())) {
                    does.push(serde_json::json!({ "journal": [tag, stage] }));
                }
            }
        }
        notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does, branch: None, found: Default::default() }, &format!("skip.{}.starmap", planet)));
    }
    notes
}

/// One chapter's end state: the journal (and the quests the game itself removes), the story
/// values, the star map count, the items, the loot, the party, the Jedi class.
/// Whether the companions a KotOR II Ebon Hawk scene needs have joined: the scenes on the ship
/// with slot 3 (`003EBO_G0T0_*`, `CUT_*G0T0*`, named in the Dxun, Onderon, Dantooine and Korriban
/// milestones: "needs slots 3 and 1 available") and, for those with Bao-Dur, slot 1 too. Any
/// other story value is not a scene and counts as present.
fn scene_party_present(save: &Save, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let scene = (n.starts_with("003ebo_") || n.starts_with("cut_")) && n.contains("g0t0");
    if !scene {
        return true;
    }
    save.recruited(3) && (!n.contains("bao") || save.recruited(1))
}

fn apply_chapter(save: &mut Save, data: &GameData, m: &serde_json::Value, jedi_class: Option<usize>, before: &Before, met: bool) -> Vec<String> {
    let mut does = Vec::new();
    for j in m.get("journal").and_then(|j| j.as_array()).cloned().unwrap_or_default() {
        if let (Some(tag), Some(stage)) = (j.get("tag").and_then(|t| t.as_str()), j.get("stage").and_then(|s| s.as_i64())) {
            does.push(serde_json::json!({ "journal": [tag, stage] }));
        }
    }
    let mut unsure = Vec::new();
    for g in m.get("globals").and_then(|g| g.as_array()).cloned().unwrap_or_default() {
        let Some(name) = g.get("name").and_then(|n| n.as_str()) else { continue };
        let value = g.get("value").cloned().unwrap_or(serde_json::Value::Null);
        if value.is_null() {
            continue;
        }
        // KotOR II's Ebon Hawk scenes with slot 3 (and slot 1): the chapters after Nar Shaddaa,
        // where slot 3 joins, set them as seen. Played before Nar Shaddaa, which the galaxy
        // map's order allows since 2026-11-06, they wait for the game to play them.
        if save.game == Game::K2 && !scene_party_present(save, name) {
            continue;
        }
        if g.get("certain").and_then(|c| c.as_bool()) == Some(false) {
            unsure.push(format!("Story value {} = {} (worked out from the scripts, not seen in play)", name, value));
        }
        let kind = if value.is_boolean() { "set_global_bool" } else { "set_global_number" };
        does.push(serde_json::json!({ kind: [name, value] }));
    }
    let items = m.get("items").and_then(|i| i.get("held_at_end")).and_then(|h| h.as_array()).cloned().unwrap_or_default();
    for it in items {
        let (Some(resref), n) = (it.get("resref").and_then(|r| r.as_str()), it.get("count").and_then(|c| c.as_i64()).unwrap_or(1)) else { continue };
        if before.items.contains(&resref.to_ascii_lowercase()) {
            continue; // the story gave it already
        }
        does.push(serde_json::json!({ "give_item": [resref, n] }));
    }
    // The chapter's areas the save has been to, as it holds them, and as the game ships them:
    // a container already emptied there, or every enemy of a kind already dead, gave its loot.
    let visited: Vec<(String, Struct)> = cleared_of(m).into_iter().filter_map(|(module, _, _)| save.module_git(&module).map(|g| (module, g.root))).collect();
    let templates = |git: &Struct| -> std::collections::HashMap<String, i64> {
        let mut out = std::collections::HashMap::new();
        for c in git.list(crate::area::CREATURES).cloned().unwrap_or_default() {
            if let Some(t) = c.string("TemplateResRef") {
                *out.entry(t.to_ascii_lowercase()).or_insert(0) += 1;
            }
        }
        out
    };
    let mut dead: std::collections::HashMap<String, (i64, i64)> = Default::default(); // template: (shipped in visited, alive)
    for (module, git) in &visited {
        let now = templates(git);
        if let Some(shipped) = crate::map::game_git(data, module) {
            for (t, n) in templates(&shipped.root) {
                let e = dead.entry(t.clone()).or_insert((0, 0));
                e.0 += n;
                e.1 += now.get(&t).copied().unwrap_or(0);
            }
        }
    }
    let all_killed = |from: &[String]| !from.is_empty() && from.iter().all(|t| dead.get(&t.to_ascii_lowercase()).map_or(false, |(n, alive)| *n > 0 && *alive == 0));
    let looted = |module: &str, tag: &str| {
        visited.iter().find(|(m, _)| m.eq_ignore_ascii_case(module)).map_or(false, |(_, git)| {
            git.list(crate::area::PLACEABLES).map_or(false, |l| l.iter().any(|p| {
                p.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(tag)) && p.list("ItemList").map_or(true, |i| i.is_empty())
            }))
        })
    };
    // What a full run leaves besides the story: the enemies' drops and the containers' loot,
    // the containers' class gear by the game's own tests.
    let planet_name = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_string();
    if let Some(r) = rewards(save.game, &planet_name) {
        for d in r.get("drops").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
            let from: Vec<String> = d.get("from").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            if all_killed(&from) {
                continue; // the player killed them and took this already
            }
            if let (Some(res), n) = (d.get("resref").and_then(|v| v.as_str()), d.get("count").and_then(|v| v.as_i64()).unwrap_or(1)) {
                does.push(serde_json::json!({ "give_item": [res, n] }));
            }
        }
        let player = save.player().cloned();
        for c in r.get("containers").and_then(|c| c.as_array()).cloned().unwrap_or_default() {
            let (cm, ct) = (c.get("module").and_then(|v| v.as_str()).unwrap_or_default(), c.get("container").and_then(|v| v.as_str()).unwrap_or_default());
            if !ct.is_empty() && looted(cm, ct) {
                continue; // emptied in the save already
            }
            for it in c.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default() {
                if let (Some(res), n) = (it.get("resref").and_then(|v| v.as_str()), it.get("count").and_then(|v| v.as_i64()).unwrap_or(1)) {
                    does.push(serde_json::json!({ "give_item": [res, n] }));
                }
            }
            for it in c.get("created_by_script").and_then(|i| i.as_array()).cloned().unwrap_or_default() {
                let all: Vec<String> = it.get("when").and_then(|w| serde_json::from_value(w.clone()).ok()).unwrap_or_default();
                let any: Vec<Vec<String>> = it.get("when_any").and_then(|w| serde_json::from_value(w.clone()).ok()).unwrap_or_default();
                let holds = |t: &String| player.as_ref().map_or(false, |p| condition(p, t));
                if all.iter().all(holds) && (any.is_empty() || any.iter().any(|g| g.iter().all(holds))) {
                    if let Some(res) = it.get("resref").and_then(|v| v.as_str()) {
                        does.push(serde_json::json!({ "give_item": [res, 1] }));
                    }
                }
            }
        }
    }
    let planet = planet_name.to_ascii_lowercase();
    let found = area_items(data, m, &does);
    let mut notes = apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does, branch: None, found }, &format!("skip.{}", planet));

    // The quests the game itself takes out of the journal at the chapter's end.
    for tag in m.get("journal_remove").and_then(|j| j.as_array()).cloned().unwrap_or_default() {
        if let Some(tag) = tag.as_str() {
            save.remove_quest(tag);
        }
    }

    // A number several chapters raise, each by its own amount (KotOR 2's masters found,
    // 000_Jedi_Found): added to what the save holds, once per chapter skipped.
    for inc in m.get("increments").and_then(|v| v.as_array()).cloned().unwrap_or_default().into_iter().filter(|_| !met) {
        let (Some(name), Some(add)) = (inc.get("name").and_then(|v| v.as_str()), inc.get("add").and_then(|v| v.as_i64())) else { continue };
        let now = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default().iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| match g.value {
            crate::globals::GlobalValue::Number(n) => Some(n),
            _ => None,
        });
        let value = now.unwrap_or(0) + add;
        notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does: vec![serde_json::json!({ "set_global_number": [name, value] })], branch: None, found: Default::default() }, &format!("skip.{}.inc", planet)));
    }

    notes.extend(count_star_maps(save, data, m));

    // Companions: who joins (AddAvailableNPCByTemplate) and who leaves.
    let party = m.get("party").cloned().unwrap_or_default();
    // KotOR 2's Handmaiden joins a male player and the Disciple a female one ("when").
    let female = save.player().map_or(false, |p| p.int("Gender") == Some(1));
    for r in party.get("recruit").and_then(|r| r.as_array()).cloned().unwrap_or_default() {
        let (Some(slot), Some(template)) = (r.get("slot").and_then(|v| v.as_i64()), r.get("template").and_then(|v| v.as_str())) else { continue };
        match r.get("when").and_then(|w| w.as_str()) {
            Some("male") if female => continue,
            Some("female") if !female => continue,
            _ => {}
        }
        // A template some chapters keep in the recruiting area's own files (the dark side's
        // p_bastilla004 is only in unk_m44ac): the area's copy first, as the game loads it there.
        let module = r.get("module").and_then(|v| v.as_str()).unwrap_or_default();
        let from_module = (!module.is_empty())
            .then(|| crate::map::ModuleFiles::open(&data.root, module))
            .and_then(|f| f.read(template, crate::formats::erf::RES_TYPE_UTC))
            .and_then(|d| crate::formats::gff::Gff::read(&d).ok());
        match from_module.or_else(|| data.gff(template, "utc", crate::formats::erf::RES_TYPE_UTC)) {
            Some(utc) => {
                if let Err(e) = save.recruit_from_template(slot.max(0) as usize, &utc.root) {
                    notes.push(format!("Party member {} not recruited: {}", template, e));
                }
            }
            None => notes.push(format!("Party member template {} not found in the game", template)),
        }
    }
    for r in party.get("remove").and_then(|r| r.as_array()).cloned().unwrap_or_default() {
        if let Some(slot) = r.as_i64().or_else(|| r.get("slot").and_then(|v| v.as_i64())) {
            save.set_recruited(slot.max(0) as usize, false);
        }
    }

    notes.extend(apply_jedi(save, data, m, jedi_class, before));
    notes.extend(unsure);
    notes
}

/// Becoming a Jedi: the class the player picks, a level of it, its lightsaber, and the story
/// value the game sets for that path.
fn apply_jedi(save: &mut Save, data: &GameData, m: &serde_json::Value, jedi_class: Option<usize>, before: &Before) -> Vec<String> {
    let mut notes = Vec::new();
    let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_ascii_lowercase();
    if let (Some(jc), Some(pick)) = (m.get("jedi_class"), jedi_class) {
        if let Some(choice) = jc.get("choices").and_then(|c| c.as_array()).and_then(|c| c.get(pick)).cloned() {
            let variable = jc.get("variable").and_then(|v| v.as_str()).unwrap_or("DAN_PATH_STATE");
            let mut does = Vec::new();
            if let Some(ps) = choice.get("path_state").and_then(|v| v.as_i64()) {
                does.push(serde_json::json!({ "set_global_number": [variable, ps] }));
            }
            if let Some(saber) = choice.get("lightsaber").and_then(|v| v.as_str()).filter(|s| !before.items.contains(&s.to_ascii_lowercase())) {
                does.push(serde_json::json!({ "give_item": [saber, 1] }));
            }
            notes.extend(apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does, branch: None, found: Default::default() }, &format!("skip.{}.jedi", planet)));
            if let Some(class_id) = choice.get("class").and_then(|v| v.as_i64()) {
                let cat = crate::catalog::Catalog::build(data);
                if let Some(class) = cat.class(class_id).cloned() {
                    save.edit(Who::Player, &format!("skip.{}.class", planet), |c| {
                        crate::creature::add_class(c, &class, None);
                    });
                }
            }
        }
    }
    notes
}

/// The items of `does` the game's global files lack, from the chapter's own areas (the
/// milestone's "cleared" modules): KotOR 2 keeps many loot and quest items only there.
fn area_items(data: &GameData, m: &serde_json::Value, does: &[serde_json::Value]) -> std::collections::HashMap<String, crate::formats::gff::Gff> {
    let mut want: Vec<String> = does
        .iter()
        .filter_map(|e| e.get("give_item").and_then(|g| g.get(0)).and_then(|r| r.as_str()))
        .map(|r| r.to_ascii_lowercase())
        .filter(|r| data.gff(r, "uti", crate::formats::erf::RES_TYPE_UTI).is_none())
        .collect();
    want.sort();
    want.dedup();
    let mut out = std::collections::HashMap::new();
    if want.is_empty() {
        return out;
    }
    let modules: Vec<String> = m.get("cleared").and_then(|c| c.as_array()).map(|c| c.iter().filter_map(|x| x.get("module").and_then(|v| v.as_str()).map(|s| s.to_string())).collect()).unwrap_or_default();
    for module in modules {
        let files = crate::map::ModuleFiles::open(&data.root, &module);
        for r in &want {
            if out.contains_key(r) {
                continue;
            }
            if let Some(g) = files.read(r, crate::formats::erf::RES_TYPE_UTI).and_then(|d| crate::formats::gff::Gff::read(&d).ok()) {
                out.insert(r.clone(), g);
            }
        }
        if out.len() == want.len() {
            break;
        }
    }
    out
}

/// A chapter's rewards file (tools/questdb/rewards.py), when the editor carries one.
pub fn rewards_of(game: Game, planet: &str) -> Option<serde_json::Value> {
    rewards(game, planet)
}

fn rewards(game: Game, planet: &str) -> Option<serde_json::Value> {
    json_files().iter().filter(|(g, _)| *g == game).find_map(|(_, v)| {
        let same = v.get("planet").and_then(|p| p.as_str()).map_or(false, |p| p.eq_ignore_ascii_case(planet));
        (same && v.get("xp_at_end").is_some()).then(|| v.clone())
    })
}

/// One of the tests the game's scripts make before handing out class gear, against the
/// player: "class is CLASS_TYPE_SOLDIER", "has skill SKILL_STEALTH" (a rank above 0).
fn condition(player: &Struct, test: &str) -> bool {
    const CLASSES: [&str; 6] = ["SOLDIER", "SCOUT", "SCOUNDREL", "JEDIGUARDIAN", "JEDICONSULAR", "JEDISENTINEL"];
    const SKILLS: [&str; 8] = ["COMPUTER_USE", "DEMOLITIONS", "STEALTH", "AWARENESS", "PERSUADE", "REPAIR", "SECURITY", "TREAT_INJURY"];
    if let Some(class) = test.strip_prefix("class is CLASS_TYPE_") {
        let Some(id) = CLASSES.iter().position(|c| *c == class) else { return false };
        return crate::creature::classes(player).iter().any(|(c, _)| *c == id as i64);
    }
    if let Some(skill) = test.strip_prefix("has skill SKILL_") {
        let Some(i) = SKILLS.iter().position(|s| *s == skill) else { return false };
        return crate::creature::skill(player, i) > 0;
    }
    false
}

/// The chapter's per-area "played through" data (the milestone's "cleared"), read leniently:
/// a map or a list of modules, each with remove, doors, containers_looted, locals and
/// can_return. Tags may be plain strings or objects with a "tag".
fn cleared_of(m: &serde_json::Value) -> Vec<(String, crate::stored::Cleared, bool)> {
    let list = m.get("cleared").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let tags = |v: Option<&serde_json::Value>| -> Vec<String> {
        v.and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|e| e.as_str().map(|s| s.to_string()).or_else(|| e.get("tag").and_then(|t| t.as_str()).map(|s| s.to_string()))).collect())
            .unwrap_or_default()
    };
    list.into_iter()
        .filter_map(|v| {
            let module = v.get("module").and_then(|x| x.as_str())?.to_ascii_lowercase();
            let mut c = crate::stored::Cleared::default();
            c.remove = tags(v.get("remove"));
            c.remove.extend(tags(v.get("spawned_and_killed")));
            for d in v.get("doors").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
                let Some(tag) = d.get("tag").and_then(|t| t.as_str()) else { continue };
                if d.get("open").and_then(|x| x.as_bool()).unwrap_or(false) {
                    c.open.push(tag.to_string());
                }
                if d.get("unlocked").and_then(|x| x.as_bool()).unwrap_or(false) {
                    c.unlock.push(tag.to_string());
                }
            }
            c.looted = tags(v.get("containers_looted"));
            // Locals with a known value: booleans that end TRUE, numbers with their value.
            for l in v.get("locals").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
                let Some(index) = l.get("index").and_then(|x| x.as_u64()).map(|x| x as u32) else { continue };
                let object = l.get("object").and_then(|x| x.as_str()).unwrap_or_default();
                let tag = l.get("tag").and_then(|x| x.as_str()).unwrap_or_default().to_string();
                let value = l.get("value").cloned().unwrap_or_default();
                let number = match &value {
                    serde_json::Value::Bool(true) => None,
                    serde_json::Value::Number(n) => n.as_i64(),
                    _ => continue,
                };
                match (object, number) {
                    ("area", None) => c.area_bits.push(index),
                    ("module", None) => c.module_bits.push(index),
                    ("area", Some(n)) => c.area_numbers.push((index, n)),
                    ("module", Some(n)) => c.module_numbers.push((index, n)),
                    (_, None) if !tag.is_empty() => c.object_bits.push((tag, index)),
                    (_, Some(n)) if !tag.is_empty() => c.object_numbers.push((tag, index, n)),
                    _ => {}
                }
            }
            let can_return = v.get("returnable").or_else(|| v.get("can_return")).and_then(|x| x.as_bool()).unwrap_or(false);
            Some((module, c, can_return))
        })
        .collect()
}

/// Brings a stored area the player has been to to the state a full run leaves it in.
fn reconcile_visited(save: &mut Save, module: &str, c: &crate::stored::Cleared) {
    let game = save.game;
    let c = c.clone();
    save.edit_module_git(module, &format!("skip.visited.{}", module), |git| {
        let tag_in = |o: &Struct, list: &[String]| o.string("Tag").map_or(false, |t| list.iter().any(|x| x.eq_ignore_ascii_case(&t)));
        if let Some(l) = git.list_mut(crate::area::CREATURES) {
            l.retain(|o| !tag_in(o, &c.remove));
        }
        if let Some(l) = git.list_mut(crate::area::DOORS) {
            for d in l.iter_mut() {
                if tag_in(d, &c.open) {
                    d.set_int("OpenState", 1, Value::Byte(0));
                }
                if tag_in(d, &c.open) || tag_in(d, &c.unlock) {
                    d.set_int("Locked", 0, Value::Byte(0));
                }
            }
        }
        if let Some(l) = git.list_mut(crate::area::PLACEABLES) {
            for p in l.iter_mut().filter(|p| tag_in(p, &c.looted)) {
                if p.get("ItemList").is_some() {
                    p.set("ItemList", Value::List(Vec::new()));
                }
            }
        }
        for &b in &c.area_bits {
            crate::stored::set_local(git, game, false, b, 1);
        }
        for &(i, n) in &c.area_numbers {
            crate::stored::set_local(git, game, true, i, n);
        }
        for list in [crate::area::CREATURES, crate::area::DOORS, crate::area::PLACEABLES, "TriggerList"] {
            if let Some(l) = git.list_mut(list) {
                for o in l.iter_mut() {
                    let Some(t) = o.string("Tag") else { continue };
                    for (tag, b) in c.object_bits.iter().chain(c.trigger_bits.iter()) {
                        if tag.eq_ignore_ascii_case(&t) {
                            crate::stored::set_local(o, game, false, *b, 1);
                        }
                    }
                    for (tag, i, n) in &c.object_numbers {
                        if tag.eq_ignore_ascii_case(&t) {
                            crate::stored::set_local(o, game, true, *i, *n);
                        }
                    }
                }
            }
        }
    });
}

/// Writes the played-through stored copies of the chapter's areas the player can come back
/// to, never over an area the save has been to, and only for save groups the trip on keeps
/// (modulesave.2da: entering a module deletes its deletesavegrouponenter group, so a copy of
/// that group would be thrown away). Returns notes.
fn write_cleared(save: &mut Save, data: &GameData, m: &serde_json::Value, trip: &[String]) -> Vec<String> {
    let table = data.table("modulesave");
    let row_of = |module: &str| (0..table.rows.len()).find(|&r| table.cell(r, "modulename").map_or(false, |n| n.eq_ignore_ascii_case(module)));
    let deleted: Vec<i64> = trip.iter().filter_map(|t| row_of(t).and_then(|r| table.int(r, "deletesavegrouponenter"))).collect();
    let Some((last_ifo, base)) = save.current_module_parts() else { return Vec::new() };
    let mut notes = Vec::new();
    let mut written = 0;
    for (module, cleared, can_return) in cleared_of(m) {
        if save.module_git(&module).is_some() {
            // An area the player has been to keeps its stored state; brought to how a full run
            // leaves it: the one-time enemies still there gone, doors opened, the looted
            // containers emptied (their loot is in the skip's), the flags set.
            reconcile_visited(save, &module, &cleared);
            continue;
        }
        if !can_return {
            continue;
        }
        let Some(r) = row_of(&module) else { continue };
        if table.int(r, "includeinsave") != Some(1) || table.int(r, "savegroup").map_or(true, |g| deleted.contains(&g)) {
            continue;
        }
        match crate::stored::played_through(data, &module, &cleared, &last_ifo, &base) {
            Some(bytes) => {
                if save.add_stored_module(&module, bytes) {
                    written += 1;
                }
            }
            None => notes.push(format!("The area {} could not be prepared as played (its game files were not found)", module)),
        }
    }
    if written > 0 {
        notes.push(format!("{} areas of the chapter are stored as played through: come back and its one-time enemies are gone, its doors open, its containers looted", written));
    }
    notes
}

// ---- finishing one quest ----

/// The stages of a quest the game sets out of number order: a step whose journal call may
/// lower the stage (`bAllowOverrideHigher`), with the areas that step runs in. KotOR's Carth
/// sets stage 3 ("Carth's revenge is complete") on the Leviathan, after stage 65; ordered by
/// number it came first and read as the quest's beginning (2026-11-06).
pub fn lowered_stages(game: Game, tag: &str) -> Vec<(i64, Vec<String>)> {
    let Some((q, _)) = quest(game, tag) else { return Vec::new() };
    q.stages
        .iter()
        .filter(|s| s.steps.iter().any(|t| t.does.iter().any(|e| e.get("other").and_then(|v| v.as_str()).map_or(false, |o| o.contains("may also lower the stage")))))
        .map(|s| (s.stage, s.steps.iter().map(|t| t.module.to_ascii_lowercase()).filter(|m| !m.is_empty()).collect()))
        .collect()
}

/// The steps finishing `tag` at `ending` takes from where the save stands: one way through
/// each stage after the quest's current stage (`one_way`, the way that suits the side), the
/// ending's own steps last.
fn finish_steps(save: &Save, q: &Quest, ending: i64, dark: bool) -> Vec<Step> {
    let now = save.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(&q.tag)).map(|(_, v)| v).unwrap_or(-1);
    let alignment = save.player().map(crate::creature::alignment).unwrap_or(50);
    let after: std::collections::HashSet<i64> = q.stages.iter().filter(|s| s.stage > now || s.stage == ending).map(|s| s.stage).collect();
    let mut trimmed = q.clone();
    trimmed.stages.retain(|s| after.contains(&s.stage));
    one_way_in(&trimmed, ending, dark, alignment, &story_values(save))
}

/// Whether the side played changes what finishing `tag` at `ending` does from where the save
/// stands (a light and a dark way through one of its stages).
pub fn finish_side_matters(save: &Save, tag: &str, ending: i64) -> bool {
    let Some((q, _)) = quest(save.game, tag) else { return false };
    let key = |steps: Vec<Step>| steps.iter().map(|s| format!("{:?}{:?}", s.via, s.does)).collect::<Vec<_>>();
    key(finish_steps(save, &q, ending, false)) != key(finish_steps(save, &q, ending, true))
}

/// The side an ending belongs to: 1 light, -1 dark, 0 neither. Light when the ending's own
/// stage raises alignment on both its light and its dark way, dark when it lowers it on both;
/// an ending whose ways go both ways is neither (the pop-up asks the side for it). Only the
/// ending's own stage counts: an earlier stage both endings share says nothing about either
/// (Genoharadan: Ithorak's -4 comes before both its endings; counting it read both "dark").
pub fn ending_side(save: &Save, tag: &str, ending: i64) -> i32 {
    let Some((mut q, _)) = quest(save.game, tag) else { return 0 };
    q.stages.retain(|s| s.stage == ending);
    let alignment = save.player().map(crate::creature::alignment).unwrap_or(50);
    let of = |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    let moved = |dark: bool| -> i64 { one_way(&q, ending, dark, alignment).iter().flat_map(|s| &s.does).map(|e| of(e, "light_side") - of(e, "dark_side")).sum() };
    match (moved(false), moved(true)) {
        (a, b) if a > 0 && b > 0 => 1,
        (a, b) if a < 0 && b < 0 => -1,
        _ => 0,
    }
}

/// Finishes a quest as if played from where the save stands: the steps the game takes on one
/// way to `ending` (journal stages, story values, items, credits, XP, alignment, doors and
/// the flags people read), the journal's own experience for every quest stage those steps
/// reach that is new to the save (plot.2da `xp` times the stage's `XP_Percentage`, as the
/// game gives it with the entry; measured on 2026-11-06, the quest database's XP and the
/// journal's are separate: in KotOR 74 stages give XP by their steps alone, 135 by the
/// journal alone, 3 both and never the same amount), and the quest at the ending. Returns what
/// could not be done, in plain words: what only the game can do (a cutscene, a conversation)
/// and what waits for an area the save has not been to.
pub fn finish_quest(save: &mut Save, data: &GameData, tag: &str, ending: i64, dark: bool) -> Result<Vec<String>, String> {
    let journal = crate::quests::all(data);
    let xp_of = |t: &str, s: i64| journal.iter().find(|q| q.tag.eq_ignore_ascii_case(t)).and_then(|q| q.stage(s)).map_or(0, |x| x.xp);
    let before: Vec<(String, i64)> = save.quests();
    let was = |t: &str| before.iter().find(|(x, _)| x.eq_ignore_ascii_case(t)).map(|(_, v)| *v);
    // A quest at an ending already, or at the stage asked, has nothing left to play: finishing
    // it again gave the ending's items, credits and XP a second time, and an ending of a lower
    // number moved the journal back (2026-11-07 audit).
    if let Some(now) = was(tag) {
        if now == ending {
            return Err("The quest is at this point already.".into());
        }
        let ended = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)).and_then(|q| q.stage(now)).map_or(false, |s| s.end);
        if ended {
            return Err("This quest is finished already.".into());
        }
    }
    let mut notes = Vec::new();
    let mut reached: Vec<(String, i64)> = Vec::new();
    if let Some((q, _)) = quest(save.game, tag) {
        // the chapter the quest is in: its star map and the templates of its recruits
        let chapter = chapter_of_quest(save.game, tag).unwrap_or(serde_json::Value::Null);
        let mut star_map = false;
        let steps = finish_steps(save, &q, ending, dark);
        for (k, step) in steps.iter().enumerate() {
            // KotOR's star map script lists all four planets' branches in one step: only this
            // planet's flag is kept, and the maps are counted the game's way below (2026-11-07
            // audit: finishing Tatooine's map found all four and opened the Unknown World)
            let mut step = step.clone();
            let kept = without_star_map_branches(save.game, &chapter, &step.does);
            if kept.len() != step.does.len() {
                star_map = true;
                step.does = kept;
            }
            // companions the step brings or takes, as Skip Ahead does (they were only listed)
            notes.extend(recruits_in(save, data, &chapter, &step));
            for e in &step.does {
                if let Some((t, s)) = e.get("journal").and_then(pair).and_then(|(t, s)| s.as_i64().map(|s| (t, s))) {
                    if !reached.iter().any(|(x, y)| x.eq_ignore_ascii_case(&t) && *y == s) {
                        reached.push((t, s));
                    }
                }
                if let Some(text) = e.get("other").and_then(|v| v.as_str()) {
                    if text.starts_with("AddAvailableNPCByTemplate(") || text.starts_with("AddAvailableNPCByObject(") {
                        continue; // done or noted by recruits_in
                    }
                    if let Some(slot) = text.strip_prefix("RemoveAvailableNPC(").and_then(|r| r.split(')').next()).and_then(|n| n.trim().parse::<usize>().ok()) {
                        if save.recruited(slot) && save.set_recruited(slot, false) {
                            notes.push(format!("Party member {} leaves", slot));
                        }
                        continue;
                    }
                    if !apply_script_line(save, text, &format!("finish.{}.{}.other", tag, k)) {
                        if let Some(line) = only_the_game(text) {
                            notes.push(line);
                        }
                    }
                }
            }
            // the script lines are done or told above; the rest as Skip Ahead does it
            let mut rest = step.clone();
            rest.does.retain(|e| e.get("other").is_none());
            notes.extend(apply(save, data, &rest, &format!("finish.{}.{}", tag, k)));
        }
        if star_map {
            notes.extend(count_star_maps(save, data, &chapter));
        }
    } else {
        notes.push("The editor has no record of this quest's steps: only the journal and its experience change.".to_string());
    }
    if !reached.iter().any(|(x, y)| x.eq_ignore_ascii_case(tag) && *y == ending) {
        reached.push((tag.to_string(), ending));
    }
    // the journal's experience for each stage reached that is new to the save, once
    let xp: i64 = reached.iter().filter(|(t, s)| was(t).map_or(true, |w| *s > w || (t.eq_ignore_ascii_case(tag) && *s == ending && *s != w))).map(|(t, s)| xp_of(t, *s)).sum();
    save.set_quest(tag, ending);
    if xp != 0 {
        save.edit(Who::Player, &format!("finish.{}.xp", tag), |c| {
            let now = crate::creature::experience(c);
            crate::creature::set_experience(c, now + xp);
        });
        let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
        save.set_party_int("PT_XP_POOL", pool + xp);
    }
    notes.dedup();
    Ok(notes)
}

/// The chapter milestone whose quests include `tag` (its star map, its recruits' templates).
fn chapter_of_quest(game: Game, tag: &str) -> Option<serde_json::Value> {
    milestones_ref(game).into_iter().find(|(planet, _)| chapter_quests(game, planet).iter().any(|(t, _)| t.eq_ignore_ascii_case(tag))).map(|(_, m)| m.clone())
}

/// What a script line the quest data keeps as text does, done where the editor can do it as
/// the script does (2026-11-06, for Finish Quest; counted in the data that day): a journal
/// quest removed (134 lines), a story counter raised by a number (Carth's, Bastila's, the HK
/// parts), set to the player's level or to another counter, and XP of the player's level times
/// a number. Returns whether it was done.
fn apply_script_line(save: &mut Save, text: &str, key: &str) -> bool {
    let quoted = |t: &str| t.split('"').nth(1).map(|x| x.to_string());
    if let Some(rest) = text.strip_prefix("removes the journal quest ") {
        if let Some(tag) = quoted(rest) {
            save.remove_quest(&tag);
            return true;
        }
    }
    let level = save.player().map(crate::creature::level).unwrap_or(1);
    if let Some(rest) = text.strip_prefix("gives XP to player: (GetHitDice(player) * ") {
        if let Ok(n) = rest.trim_end_matches(')').trim().parse::<i64>() {
            let xp = level * n;
            save.edit(Who::Player, key, |c| {
                let now = crate::creature::experience(c);
                crate::creature::set_experience(c, now + xp);
            });
            let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
            save.set_party_int("PT_XP_POOL", pool + xp);
            return true;
        }
    }
    if let Some(rest) = text.strip_prefix("sets global number ") {
        let Some((name, how)) = rest.split_once(" to a value that is not constant: ") else { return false };
        let name = name.trim().to_string();
        let how = how.trim();
        let number = |root: &Struct, n: &str| crate::globals::list(root).into_iter().find(|g| g.kind == crate::globals::Kind::Number && g.name.eq_ignore_ascii_case(n));
        let value_of = |g: &crate::globals::Global| match g.value {
            crate::globals::GlobalValue::Number(v) => v,
            _ => 0,
        };
        // (GetGlobalNumber("NAME") + N), GetHitDice(player), GetGlobalNumber("OTHER")
        let plan: Option<Box<dyn Fn(&Struct) -> Option<i64>>> = if let Some(inner) = how.strip_prefix("(GetGlobalNumber(\"").and_then(|x| x.strip_suffix(')')) {
            let Some((from, Some(add))) = inner.split_once("\") + ").map(|(a, b)| (a.to_string(), b.trim().parse::<i64>().ok())) else { return false };
            let name = name.clone();
            if !from.eq_ignore_ascii_case(&name) {
                return false;
            }
            Some(Box::new(move |root: &Struct| number(root, &name).map(|g| value_of(&g) + add)))
        } else if how == "GetHitDice(player)" {
            Some(Box::new(move |_: &Struct| Some(level)))
        } else if let Some(other) = how.strip_prefix("GetGlobalNumber(\"").and_then(|x| x.strip_suffix("\")")) {
            let other = other.to_string();
            Some(Box::new(move |root: &Struct| number(root, &other).map(|g| value_of(&g))))
        } else {
            None
        };
        let Some(plan) = plan else { return false };
        let mut done = false;
        save.edit_globals(key, |root| {
            if let (Some(g), Some(v)) = (number(root, &name), plan(root)) {
                crate::globals::set_number(root, g.index, v.clamp(0, 255));
                done = true;
            }
        });
        return done;
    }
    false
}

/// A script line only the game can do, in words a player can use; None for what says nothing to
/// a player (the quest data's own remarks, faction tags, empty conversation markers, flags
/// kept on objects, the long "depending on other tests" lists).
fn only_the_game(text: &str) -> Option<String> {
    let t = text.trim();
    const QUIET: [&str; 7] = ["may also lower the stage", "depending on other tests", "changes the faction of", "starts conversation \"\"", "SetLocal", "runs script", "AddPartyMember(GetGlobalNumber"];
    if t.is_empty() || QUIET.iter().any(|q| t.contains(q)) {
        return None;
    }
    if let Some(movie) = t.strip_prefix("PlayMovie(\"") {
        return Some(format!("A cutscene plays ({})", movie.split('"').next().unwrap_or_default()));
    }
    if let Some(rest) = t.strip_prefix("starts conversation ") {
        return Some(format!("A conversation starts ({})", rest.trim_matches('"')));
    }
    if let Some(rest) = t.strip_prefix("moves the party to module ") {
        return Some(format!("The party is taken to {}", rest.trim_matches('"')));
    }
    Some(format!("Left to the game: {}", t))
}

/// A chapter's own quests for Skip Ahead's list, in the order a player meets them: the main
/// story (the milestone's journal) first, then the rest as the quest file lists them. (tag,
/// main story)
pub fn chapter_quests(game: Game, planet: &str) -> Vec<(String, bool)> {
    // borrowed, not `milestones`: that copies every chapter's file (12 ms a call, and the
    // galaxy map's route asks for each chapter's quests)
    let main: Vec<String> = milestones_ref(game)
        .into_iter()
        .find(|(p, _)| p.eq_ignore_ascii_case(planet))
        .and_then(|(_, m)| m.get("journal").and_then(|j| j.as_array()))
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("tag").and_then(|t| t.as_str()).map(|t| t.to_string()))
        .collect();
    let mut out: Vec<(String, bool)> = Vec::new();
    for (g, file) in quest_files() {
        if *g != game {
            continue;
        }
        if !file.planet.eq_ignore_ascii_case(planet) {
            continue;
        }
        for q in file.quests.iter().filter(|q| q.own) {
            if !out.iter().any(|(t, _)| t.eq_ignore_ascii_case(&q.tag)) {
                out.push((q.tag.clone(), main.iter().any(|m| m.eq_ignore_ascii_case(&q.tag))));
            }
        }
    }
    out.sort_by_key(|(_, m)| !*m);
    out
}

// ---- going back ----
//
// A save holds where the story is, not how it got there; but every chapter's data says what
// the chapter sets, so going back undoes the later chapters one by one, newest first: their
// story values return to what the chapters before them left (or unset), their quests leave the
// journal, the companions they brought leave and the ones they took away return, their areas
// are forgotten (the game builds them fresh), the XP returns to the end of the chapter before
// and the levels gained since come off newest first (each level's own record: its feats,
// powers, skill points and attribute point). Items and credits the later chapters gave go too,
// unless the player keeps what they have now.

/// Goes back to the start of the earliest chapter in `undo` (chapters the save has played,
/// in story order), or, with `stage`, to a quest's stage in that chapter. `kept` are the
/// chapters that stay played, in story order. `keep_items` keeps the items and credits.
/// Alignment defaults to 50, Neutral; use go_back_with_alignment for a chosen value.
pub fn go_back(save: &mut Save, data: &GameData, kept: &[serde_json::Value], undo: &[serde_json::Value], keep_items: bool, stage: Option<(String, i64)>) -> Result<Vec<String>, String> {
    go_back_with_alignment(save, data, kept, undo, keep_items, stage, 50)
}

/// Alignment is a chosen target, not an estimate of the player's past choices.
pub fn go_back_with_alignment(save: &mut Save, data: &GameData, kept: &[serde_json::Value], undo: &[serde_json::Value], keep_items: bool, stage: Option<(String, i64)>, alignment: i64) -> Result<Vec<String>, String> {
    go_back_part_played(save, data, kept, undo, keep_items, stage, alignment, &[])
}

/// The side a chapter was played on, by the journal where the sides end differently, and by
/// alignment where the journal cannot tell.
fn played_dark(m: &serde_json::Value, quests: &[(String, i64)], leans_dark: bool) -> bool {
    let stage_of = |j: Option<&serde_json::Value>, tag: &str| j.and_then(|j| j.as_array()).and_then(|a| a.iter().find(|e| e.get("tag").and_then(|t| t.as_str()).map_or(false, |t| t.eq_ignore_ascii_case(tag)))).and_then(|e| e.get("stage").and_then(|s| s.as_i64()));
    for e in m.get("dark").and_then(|d| d.get("journal")).and_then(|j| j.as_array()).into_iter().flatten() {
        let Some(tag) = e.get("tag").and_then(|t| t.as_str()) else { continue };
        let (Some(dark), Some(light)) = (e.get("stage").and_then(|s| s.as_i64()), stage_of(m.get("journal"), tag)) else { continue };
        if dark == light {
            continue;
        }
        match quests.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, v)| *v) {
            Some(v) if v == dark => return true,
            Some(v) if v == light => return false,
            _ => {}
        }
    }
    leans_dark
}

/// As go_back_with_alignment. `unfinished` names the chapters of `undo` begun and never
/// finished: their values and journal go back, but what only a finished chapter gave is not
/// taken away.
#[allow(clippy::too_many_arguments)]
pub fn go_back_part_played(save: &mut Save, data: &GameData, kept: &[serde_json::Value], undo: &[serde_json::Value], keep_items: bool, stage: Option<(String, i64)>, alignment: i64, unfinished: &[String]) -> Result<Vec<String>, String> {
    let is_unfinished = |p: &str| unfinished.iter().any(|u| u.eq_ignore_ascii_case(p));
    if !(0..=100).contains(&alignment) { return Err("Alignment must be between 0 and 100.".into()); }
    let first = undo.first().cloned().ok_or("nothing to go back over")?;
    // the player leads at the earlier point of the story (Save::release_controlled_npc)
    save.release_controlled_npc();
    let asked = undo;
    let (kept, undo) = fourth_planet_with_the_leviathan(save, kept, undo);
    let dark = save.player().map_or(false, |p| crate::creature::alignment(p) < 50);
    let quests = save.quests();
    let mut notes = Vec::new();
    for m in undo.iter().filter(|m| m.get("star_map").map_or(false, |s| !s.is_null()) && !asked.iter().any(|a| a.get("planet") == m.get("planet"))) {
        notes.push(format!("{} is undone too: the capture comes before the fourth planet", m.get("planet").and_then(|p| p.as_str()).unwrap_or_default()));
    }
    let kept_side: Vec<serde_json::Value> = kept.iter().map(|m| for_side(m, played_dark(m, &quests, dark))).collect();
    let names: Vec<String> = kept.iter().filter_map(|m| m.get("planet").and_then(|p| p.as_str()).map(str::to_string)).collect();
    // Never raised: a return gives no XP the player did not have.
    let xp = xp_of(save.game, &names).min(save.player().map_or(0, crate::creature::experience));
    let table = data.table("exptable");
    let mut target = (1..=60).take_while(|l| table.int((l - 1) as usize, "xp").map_or(false, |need| xp >= need)).last().unwrap_or(1);
    // A story-granted class from an undone chapter cannot remain merely because the
    // player had unspent XP before taking it. Stop before its first recorded level.
    let lost_classes: Vec<i64> = undo.iter().filter(|m| m.get("planet").and_then(|p| p.as_str()).map_or(true, |p| !is_unfinished(p))).flat_map(|m| m.get("jedi_class").and_then(|j| j.get("choices")).and_then(|c| c.as_array()).cloned().unwrap_or_default())
        .filter_map(|c| c.get("class").and_then(|id| id.as_i64())).collect();
    if let Some(stop) = save.player().and_then(|p| p.list("LvlStatList")).and_then(|r| r.iter().position(|r| r.int("LvlStatClass").map_or(false, |id| lost_classes.contains(&id)))).filter(|i| *i > 0) {
        target = target.min(stop as i64);
    }
    let cat = crate::catalog::Catalog::build(data);
    crate::creature::validate_rewind_prefix(save.player().ok_or("This save has no player to rewind.")?, target, &cat)?;
    let before = Before::of(save);
    for m in undo.iter().rev() {
        let on_dark = played_dark(m, &quests, dark);
        let side = if on_dark { "dark" } else { "light" };
        let m = for_side(m, on_dark);
        let part = m.get("planet").and_then(|p| p.as_str()).map_or(false, is_unfinished);
        notes.extend(undo_chapter(save, data, &m, &kept_side, keep_items, side, &before, part));
    }
    restore_star_maps(save, data, &kept_side, &undo);
    // Prestige class story state goes with the class (KotOR 2).
    let prestige_left = save.player().map_or(false, |p| crate::creature::classes(p).iter().any(|c| PRESTIGE_LIGHT.contains(&c.0) || PRESTIGE_DARK.contains(&c.0)));
    let levels_stay = save.player().and_then(|p| p.list("LvlStatList")).map_or(false, |r| r.iter().take(target.max(0) as usize).any(|x| x.int("LvlStatClass").map_or(false, |c| PRESTIGE_LIGHT.contains(&c) || PRESTIGE_DARK.contains(&c))));
    if save.game == Game::K2 && prestige_left && !levels_stay {
        set_value(save, data, "000_Prestige", serde_json::json!(0), "goback.prestige");
    }
    // Companions a later chapter trained (KotOR 2): the chain's values, for those no longer here.
    if save.game == Game::K2 {
        for (slot, _, _, values, _) in TRAINABLE {
            if !save.recruited(slot) {
                for (name, _) in values.iter() {
                    set_value(save, data, name, serde_json::json!(0), "goback.train");
                }
            }
        }
    }
    // Rebuild the earlier class list from the retained records, even when a later
    // edit changed the current classes. Never leave old levels behind with reduced XP.
    let restored = std::cell::RefCell::new(Ok(0));
    save.edit(Who::Player, "goback.levels", |c| {
        let result = crate::creature::rewind_level_history(c, target, &cat);
        if result.is_ok() { crate::creature::set_experience(c, xp); }
        if restored.borrow().is_ok() { *restored.borrow_mut() = result; }
    });
    let level = restored.into_inner()?;
    save.set_party_int("PT_XP_POOL", xp);
    notes.push(format!("XP back to {} (level {}), later recorded levels taken away", xp, level));
    if keep_items {
        notes.push("Items and credits kept as they are now".to_string());
    }
    // where the story starts again: a stage inside the first undone chapter, or its start
    if let Some((tag, st)) = stage {
        notes.extend(skip_to_stage(save, data, &[first.clone()], &tag, st, None, dark, None, false)?);
        save.edit(Who::Player, "goback.alignment", |c| crate::creature::set_alignment(c, alignment));
        notes.push(format!("Alignment set to {} (chosen for this return)", alignment));
        return Ok(notes);
    }
    let start = kept_side.iter().rev().find(|m| m.get("side_chapter").and_then(|v| v.as_bool()) != Some(true)).and_then(|m| m.get("next_chapter").cloned());
    match start {
        Some(next) => {
            let hop = next.get("chain").and_then(|c| c.as_array()).and_then(|c| c.first()).cloned().unwrap_or(next.clone());
            let module = hop.get("module").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let waypoint = hop.get("waypoint").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let movies: Vec<String> = hop.get("movies").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            let mv: Vec<&str> = movies.iter().map(|s| s.as_str()).collect();
            save.queue_transition(&module, &waypoint, &mv)?;
        }
        None => {
            // the game's first chapter: its first area
            let module = first.get("cleared").and_then(|c| c.as_array()).and_then(|c| c.first()).and_then(|c| c.get("module")).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let waypoint = crate::map::any_waypoint(data, &module).unwrap_or_default();
            save.queue_transition(&module, &waypoint, &[])?;
        }
    }
    save.edit(Who::Player, "goback.alignment", |c| crate::creature::set_alignment(c, alignment));
    notes.push(format!("Alignment set to {} (chosen for this return)", alignment));
    notes.dedup();
    Ok(notes)
}

/// The capture plays after the third map, so with all four found the Leviathan was played
/// before the last planet. A return to it takes that planet back too, or the count stays past
/// the 40 the capture's script tests and neither it nor the Unknown World can come. Which
/// planet was last the save cannot say: it is the last on the timeline.
fn fourth_planet_with_the_leviathan(save: &Save, kept: &[serde_json::Value], undo: &[serde_json::Value]) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let (mut kept, mut undo) = (kept.to_vec(), undo.to_vec());
    let leviathan = undo.iter().any(|m| m.get("planet").and_then(|p| p.as_str()).map_or(false, |p| p.eq_ignore_ascii_case("Leviathan")));
    let globals = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    let on = |m: &serde_json::Value| m.get("star_map").and_then(|s| s.get("planet_flag")).and_then(|f| f.as_str()).map_or(false, |f| globals.iter().any(|g| g.name.eq_ignore_ascii_case(f) && g.value == crate::globals::GlobalValue::Bool(true)));
    if leviathan && kept.iter().filter(|m| on(m)).count() == 4 {
        if let Some(at) = kept.iter().rposition(on) {
            undo.push(kept.remove(at));
        }
    }
    (kept, undo)
}

/// The star maps after a return: the planets undone lose their flags and the counter is
/// counted from those left. Each value the count sets (K_SWG_BASTILA, K_CAPTURED_LEV,
/// K_KOTOR_MASTER) is worked out on its own: it starts from what the kept chapters leave (0
/// where they leave nothing) when an undone chapter wrote it, or when it stands at or past
/// what a count no longer reached sets; otherwise it keeps the value it has. Then each count
/// still reached sets its value the game's way: only below `only_if_below`, and never lower
/// than it is.
///
/// Corrections, 2026-11-06 (audit): until then every KotOR 1 return reset these values to the
/// kept chapters' (0 where none gives one) and ran the count's effects again. Any return wrote
/// `K_SWG_BASTILA` 0, or 99 with two maps kept, over a save that had it at 4; and a return to the
/// Star Forge, with all four maps kept, ran the count of 50 again, which wrote `K_KOTOR_MASTER`
/// 30 (the Unknown World opened) over the 50 the Unknown World leaves.
fn restore_star_maps(save: &mut Save, data: &GameData, kept: &[serde_json::Value], undo: &[serde_json::Value]) {
    let all = milestones_ref(save.game);
    let Some(carrier) = all.iter().map(|(_, m)| *m).find(|m| m.get("star_map").map_or(false, |s| !s.is_null())) else { return };
    let sm = carrier.get("star_map").cloned().unwrap_or_default();
    let value_in = |list: &[serde_json::Value], name: &str| list.iter().filter_map(|k| k.get("globals").and_then(|g| g.as_array()).and_then(|g| {
        g.iter().find(|x| x.get("name").and_then(|n| n.as_str()).map_or(false, |n| n.eq_ignore_ascii_case(name))).and_then(|x| x.get("value").cloned()).filter(|v| !v.is_null())
    })).last();
    for m in undo {
        if let Some(flag) = m.get("star_map").and_then(|s| s.get("planet_flag")).and_then(|f| f.as_str()) {
            set_value(save, data, flag, serde_json::json!(false), "goback.starmap.flag");
        }
    }
    let flags: Vec<String> = sm.get("planet_flags").and_then(|f| serde_json::from_value(f.clone()).ok()).unwrap_or_default();
    let now = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    let found = now.iter().filter(|g| flags.iter().any(|f| f.eq_ignore_ascii_case(&g.name)) && g.value == crate::globals::GlobalValue::Bool(true)).count() as i64;
    let counter = sm.get("counter").and_then(|v| v.as_str()).unwrap_or_default();
    let count = if found > 0 { sm.get("base").and_then(|v| v.as_i64()).unwrap_or(10) + sm.get("per_planet").and_then(|v| v.as_i64()).unwrap_or(10) * found } else { 0 };
    if !counter.is_empty() {
        let value = if found > 0 { serde_json::json!(count) } else { value_in(kept, counter).unwrap_or(serde_json::json!(0)) };
        set_value(save, data, counter, value, "goback.starmap");
    }
    let number = |name: &str| now.iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| match g.value {
        crate::globals::GlobalValue::Number(n) => Some(n),
        _ => None,
    });
    // every value the count sets, with each count's (count, value, only_if_below)
    let mut entries: Vec<(String, i64, i64, Option<i64>)> = Vec::new();
    for on in sm.get("on_count").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let Some(at) = on.get("count").and_then(|v| v.as_i64()) else { continue };
        for st in on.get("set").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            let (Some(name), Some(value)) = (st.get("name").and_then(|n| n.as_str()), st.get("value").and_then(|v| v.as_i64())) else { continue };
            entries.push((name.to_string(), at, value, st.get("only_if_below").and_then(|v| v.as_i64())));
        }
    }
    entries.sort_by_key(|e| e.1);
    let mut names: Vec<String> = entries.iter().map(|e| e.0.clone()).collect();
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    for name in names {
        let Some(cur) = number(&name) else { continue };
        let mine: Vec<&(String, i64, i64, Option<i64>)> = entries.iter().filter(|e| e.0.eq_ignore_ascii_case(&name)).collect();
        let undone_wrote = value_in(undo, &name).is_some();
        let past_unreached = mine.iter().any(|e| e.1 > count && cur >= e.2);
        let mut value = if undone_wrote || past_unreached { value_in(kept, &name).and_then(|v| v.as_i64()).unwrap_or(0) } else { cur };
        for e in mine.iter().filter(|e| e.1 <= count) {
            value = match e.3 {
                Some(below) if value < below => e.2,
                Some(_) => value,
                None => value.max(e.2),
            };
        }
        if value != cur {
            set_value(save, data, &name, serde_json::json!(value), "goback.starmap.set");
        }
    }
}

fn set_value(save: &mut Save, data: &GameData, name: &str, value: serde_json::Value, key: &str) {
    let kind = if value.is_boolean() { "set_global_bool" } else { "set_global_number" };
    let _ = apply(save, data, &Step { module: String::new(), via: None, needs: Vec::new(), does: vec![serde_json::json!({ kind: [name, value] })], branch: None, found: Default::default() }, key);
}

/// Takes one played chapter back out of the save.
#[allow(clippy::too_many_arguments)]
fn undo_chapter(save: &mut Save, data: &GameData, m: &serde_json::Value, kept: &[serde_json::Value], keep_items: bool, side: &str, before: &Before, unfinished: bool) -> Vec<String> {
    let mut notes = Vec::new();
    let planet = m.get("planet").and_then(|p| p.as_str()).unwrap_or_default().to_string();
    let key = format!("goback.{}", planet.to_ascii_lowercase());
    // story values: what the chapters that stay left, else unset (a value an earlier chapter
    // also writes without a known end value is left as it is)
    let earlier_value = |name: &str| -> Option<serde_json::Value> {
        kept.iter().filter_map(|k| k.get("globals").and_then(|g| g.as_array()).and_then(|g| {
            g.iter().find(|x| x.get("name").and_then(|n| n.as_str()).map_or(false, |n| n.eq_ignore_ascii_case(name))).and_then(|x| x.get("value").cloned()).filter(|v| !v.is_null())
        })).last()
    };
    let earlier_writes = |name: &str| kept.iter().any(|k| k.get("globals").and_then(|g| g.as_array()).map_or(false, |g| g.iter().any(|x| x.get("name").and_then(|n| n.as_str()).map_or(false, |n| n.eq_ignore_ascii_case(name)))));
    let all = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
    let mut values = 0;
    let mut left: Vec<String> = Vec::new();
    for g in m.get("globals").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let Some(name) = g.get("name").and_then(|n| n.as_str()) else { continue };
        let Some(cur) = all.iter().find(|x| x.name.eq_ignore_ascii_case(name)) else { continue };
        let value = match earlier_value(name) {
            Some(v) => v,
            None if earlier_writes(name) => {
                left.push(name.to_string());
                continue;
            }
            None => match cur.value {
                crate::globals::GlobalValue::Bool(_) => serde_json::json!(false),
                crate::globals::GlobalValue::Number(_) => serde_json::json!(0),
                _ => continue,
            },
        };
        set_value(save, data, name, value, &format!("{}.g.{}", key, name));
        values += 1;
    }
    if !left.is_empty() {
        notes.push(format!("{}: story values left as they are, the data has no earlier value for them: {}", planet, left.join(", ")));
    }
    // the masters found and the forms taught (KotOR 2): a chapter not finished gave neither
    for inc in m.get("increments").and_then(|v| v.as_array()).cloned().unwrap_or_default().into_iter().filter(|_| !unfinished) {
        let (Some(name), Some(add)) = (inc.get("name").and_then(|v| v.as_str()), inc.get("add").and_then(|v| v.as_i64())) else { continue };
        let now = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default().iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| match g.value { crate::globals::GlobalValue::Number(n) => Some(n), _ => None }).unwrap_or(0);
        set_value(save, data, name, serde_json::json!((now - add).max(0)), &format!("{}.inc", key));
    }
    // the journal: the chapter's own quests leave it, or go back to the stage a chapter that
    // stays set
    let earlier_stage = |tag: &str| kept.iter().filter_map(|k| k.get("journal").and_then(|j| j.as_array()).and_then(|j| {
        j.iter().find(|e| e.get("tag").and_then(|t| t.as_str()).map_or(false, |t| t.eq_ignore_ascii_case(tag))).and_then(|e| e.get("stage").and_then(|s| s.as_i64()))
    })).max();
    let mut tags: Vec<String> = chapter_quests(save.game, &planet).into_iter().map(|(t, _)| t).collect();
    for j in m.get("journal").and_then(|j| j.as_array()).cloned().unwrap_or_default() {
        if let Some(t) = j.get("tag").and_then(|t| t.as_str()) {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                tags.push(t.to_string());
            }
        }
    }
    for t in &tags {
        match earlier_stage(t) {
            Some(st) => save.set_quest(t, st),
            None => save.remove_quest(t),
        }
    }
    // the party: who the chapter brought leaves (unless a chapter that stays brought them),
    // who it took away comes back
    let kept_slots: Vec<i64> = kept.iter().flat_map(|k| k.get("party").and_then(|p| p.get("recruit")).and_then(|r| r.as_array()).cloned().unwrap_or_default()).filter_map(|r| r.get("slot").and_then(|v| v.as_i64())).collect();
    for r in m.get("party").and_then(|p| p.get("recruit")).and_then(|r| r.as_array()).cloned().unwrap_or_default() {
        if let Some(slot) = r.get("slot").and_then(|v| v.as_i64()) {
            if !kept_slots.contains(&slot) {
                save.set_recruited(slot.max(0) as usize, false);
            }
        }
    }
    for r in m.get("party").and_then(|p| p.get("remove")).and_then(|r| r.as_array()).cloned().unwrap_or_default() {
        if let Some(slot) = r.as_i64().or_else(|| r.get("slot").and_then(|v| v.as_i64())) {
            if kept_slots.contains(&slot) {
                save.set_recruited(slot.max(0) as usize, true);
            }
        }
    }
    // the chapter's areas: forgotten, so the game builds them fresh
    let mut forgotten = 0;
    for (module, _, _) in cleared_of(m) {
        if save.remove_stored_module(&module) {
            forgotten += 1;
        }
    }
    // the form its master taught (KotOR 2): the reverse of the teaching, by the same rule, the
    // count one lower and the form it gave off the known list
    if m.get("teach_form").and_then(|v| v.as_bool()) == Some(true) && !unfinished {
        let all = save.globals.as_ref().map(|g| crate::globals::list(&g.root)).unwrap_or_default();
        let form = all.iter().find(|g| g.name.eq_ignore_ascii_case("G_PC_FORM")).and_then(|g| match g.value { crate::globals::GlobalValue::Number(n) => Some(n), _ => None }).unwrap_or(0);
        if form > 0 {
            if let Some(p) = save.player().cloned() {
                let classes = crate::creature::classes(&p);
                let base = classes.first().map(|c| c.0).unwrap_or(-1);
                let pre = classes.get(1).map(|c| c.0).unwrap_or(255);
                let known = crate::creature::powers(&p);
                // the rule saw the forms known before this teaching: without the one it gave
                for sp in 258..=268 {
                    let without: Vec<i64> = known.iter().copied().filter(|k| *k != sp).collect();
                    if known.contains(&sp) && form_rule(pre, base, form - 1, side == "dark", &without) == Some(sp) {
                        save.edit(Who::Player, &format!("{}.form", key), |c| {
                            crate::creature::remove_power(c, sp);
                        });
                        break;
                    }
                }
            }
            set_value(save, data, "G_PC_FORM", serde_json::json!(form - 1), &format!("{}.formcount", key));
        }
    }
    // Class levels are restored once, from their chronological records, in go_back.
    // the powers and feats its story granted (KotOR 2)
    for g in m.get("grants").and_then(|v| v.as_array()).cloned().unwrap_or_default().into_iter().filter(|_| !unfinished) {
        if g.get("side").and_then(|v| v.as_str()).map_or(false, |s| s != side) {
            continue;
        }
        let (spell, feat) = (g.get("spell").and_then(|v| v.as_i64()), g.get("feat").and_then(|v| v.as_i64()));
        save.edit(Who::Player, &format!("{}.grant", key), |c| {
            if let Some(sp) = spell {
                crate::creature::remove_power(c, sp);
            }
            if let Some(f) = feat {
                crate::creature::remove_feat(c, f);
            }
        });
    }
    // Credits and influence the chapter's conversations gave. Alignment is chosen
    // once for the return; estimated chapter gains cannot recover past choices.
    if let Some(c) = rewards(save.game, &planet).and_then(|r| r.get("credits").cloned()).filter(|_| !unfinished) {
        let empty = Before { quests: Vec::new(), recruited: before.recruited.clone(), items: Default::default() };
        let g = chapter_gains(&c, side, &planet, save.game, &empty);
        if !keep_items && g.credits != 0 {
            let gold = save.party_int("PT_GOLD").unwrap_or(0);
            save.set_party_int("PT_GOLD", (gold - g.credits).max(0));
        }
        for (slot, d) in g.influence {
            if let Some(now) = save.influence(slot).filter(|v| *v >= 0) {
                save.set_influence(slot, now - d);
            }
        }
    }
    // the items it gave (story, drops, containers), as far as the player still has them
    if !keep_items && !unfinished {
        let mut give: Vec<(String, i64)> = Vec::new();
        // the Jedi lightsaber, and the path's story value with it
        if let Some(jc) = m.get("jedi_class") {
            for choice in jc.get("choices").and_then(|c| c.as_array()).into_iter().flatten() {
                if let Some(saber) = choice.get("lightsaber").and_then(|v| v.as_str()) {
                    give.push((saber.to_ascii_lowercase(), 1));
                }
            }
        }
        for it in m.get("items").and_then(|i| i.get("held_at_end")).and_then(|h| h.as_array()).cloned().unwrap_or_default() {
            if let Some(r) = it.get("resref").and_then(|v| v.as_str()) {
                give.push((r.to_ascii_lowercase(), it.get("count").and_then(|v| v.as_i64()).unwrap_or(1)));
            }
        }
        if let Some(r) = rewards(save.game, &planet) {
            for d in r.get("drops").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
                if let Some(res) = d.get("resref").and_then(|v| v.as_str()) {
                    give.push((res.to_ascii_lowercase(), d.get("count").and_then(|v| v.as_i64()).unwrap_or(1)));
                }
            }
            for c in r.get("containers").and_then(|c| c.as_array()).cloned().unwrap_or_default() {
                for it in c.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default() {
                    if let Some(res) = it.get("resref").and_then(|v| v.as_str()) {
                        give.push((res.to_ascii_lowercase(), it.get("count").and_then(|v| v.as_i64()).unwrap_or(1)));
                    }
                }
            }
        }
        let mut taken = 0;
        save.edit_inventory(&format!("{}.items", key), |items| {
            for (res, mut n) in give {
                while n > 0 {
                    let Some(at) = items.iter().position(|it| it.string("TemplateResRef").map_or(false, |t| t.eq_ignore_ascii_case(&res)) || it.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&res))) else { break };
                    let stack = items[at].int("StackSize").unwrap_or(1);
                    if stack > n {
                        items[at].set_int("StackSize", stack - n, Value::Word(1));
                        n = 0;
                    } else {
                        n -= stack;
                        items.remove(at);
                    }
                    taken += 1;
                }
            }
        });
        if taken > 0 {
            notes.push(format!("{}: items it gave taken back where the inventory still has them", planet));
        }
    }
    notes.push(format!("{} undone: {} story values back, {} quests out of the journal, {} areas back to fresh", planet, values, tags.len(), forgotten));
    notes
}

