//! Reading and changing one creature (the player or a party member) as the game stores
//! it in a save. Every change keeps the game's own field types and list shapes: a new
//! list entry is cloned from one already there, so KotOR 2's extra fields come along.

use crate::formats::gff::{LocString, Struct, Value};
use crate::games::Game;

pub const ATTRIBUTES: [&str; 6] = ["Str", "Dex", "Con", "Int", "Wis", "Cha"];

pub fn modifier(score: i64) -> i64 {
    (score - 10).div_euclid(2)
}

pub fn name(c: &Struct) -> String {
    let first = match c.get("FirstName") {
        Some(Value::LocString(l)) => l.strings.first().map(|(_, t)| crate::text::decode(t)),
        _ => None,
    };
    let last = match c.get("LastName") {
        Some(Value::LocString(l)) => l.strings.first().map(|(_, t)| crate::text::decode(t)),
        _ => None,
    };
    let mut n = first.unwrap_or_default();
    if let Some(l) = last.filter(|l| !l.is_empty()) {
        n = format!("{} {}", n, l);
    }
    crate::text::strip_codes(&n)
}

/// The player's name lives in FirstName (and Mod_FirstName); the last name is cleared,
/// as character creation leaves it.
pub fn set_name(c: &mut Struct, text: &str) {
    let bytes = crate::text::encode(text);
    for label in ["FirstName", "Mod_FirstName"] {
        if let Some(Value::LocString(l)) = c.get_mut(label) {
            l.strref = u32::MAX;
            if l.strings.is_empty() {
                l.strings.push((0, bytes.clone()));
            } else {
                for s in l.strings.iter_mut() {
                    s.1 = bytes.clone();
                }
            }
        }
    }
    for label in ["LastName", "Mod_LastName"] {
        if let Some(Value::LocString(l)) = c.get_mut(label) {
            *l = LocString { strref: u32::MAX, strings: Vec::new() };
        }
    }
}

pub fn attribute(c: &Struct, label: &str) -> i64 {
    c.int(label).unwrap_or(10)
}

pub fn set_attribute(c: &mut Struct, label: &str, value: i64) {
    c.set_int(label, value.clamp(1, 100), Value::Byte(0));
}

pub fn skill(c: &Struct, index: usize) -> i64 {
    c.list("SkillList").and_then(|l| l.get(index)).and_then(|s| s.int("Rank")).unwrap_or(0)
}

pub fn set_skill(c: &mut Struct, index: usize, rank: i64) {
    if let Some(list) = c.list_mut("SkillList") {
        if let Some(s) = list.get_mut(index) {
            s.set_int("Rank", rank.clamp(0, 99), Value::Byte(0));
        }
    }
}

pub fn classes(c: &Struct) -> Vec<(i64, i64)> {
    c.list("ClassList")
        .map(|l| l.iter().map(|e| (e.int("Class").unwrap_or(0), e.int("ClassLevel").unwrap_or(0))).collect())
        .unwrap_or_default()
}

pub fn level(c: &Struct) -> i64 {
    classes(c).iter().map(|(_, l)| l).sum()
}

pub fn feats(c: &Struct) -> Vec<i64> {
    c.list("FeatList").map(|l| l.iter().filter_map(|f| f.int("Feat")).collect()).unwrap_or_default()
}

/// Appends a list entry shaped like the entries already in the list (or `fallback`),
/// with `field` set to `value`.
fn push_like(list: &mut Vec<Struct>, field: &str, value: i64, fallback: Struct) {
    let mut entry = list.first().cloned().unwrap_or(fallback);
    entry.set_int(field, value, Value::Word(0));
    list.push(entry);
}

pub fn add_feat(c: &mut Struct, feat: i64) -> bool {
    if feats(c).contains(&feat) {
        return false;
    }
    if c.list("FeatList").is_none() {
        c.set("FeatList", Value::List(Vec::new()));
    }
    let mut fallback = Struct::new(1);
    fallback.set("Feat", Value::Word(0));
    push_like(c.list_mut("FeatList").unwrap(), "Feat", feat, fallback);
    true
}

/// Removes this one feat: from the creature's list and from the level record that gave
/// it (the player's LvlStatList), nothing else.
pub fn remove_feat(c: &mut Struct, feat: i64) -> bool {
    let mut removed = false;
    if let Some(list) = c.list_mut("FeatList") {
        let before = list.len();
        list.retain(|f| f.int("Feat") != Some(feat));
        removed = list.len() != before;
    }
    if let Some(levels) = c.list_mut("LvlStatList") {
        for l in levels.iter_mut() {
            if let Some(list) = l.list_mut("FeatList") {
                list.retain(|f| f.int("Feat") != Some(feat));
            }
        }
    }
    removed
}

/// The Force powers a creature knows, from every class's KnownList0.
pub fn powers(c: &Struct) -> Vec<i64> {
    let mut out = Vec::new();
    if let Some(classes) = c.list("ClassList") {
        for class in classes {
            if let Some(list) = class.list("KnownList0") {
                out.extend(list.iter().filter_map(|s| s.int("Spell")));
            }
        }
    }
    out
}

/// Which ClassList entry holds Force powers: the one that already has some, else the
/// first Force-using class (`is_force_class` from classes.2da).
fn power_class(c: &Struct, is_force_class: &dyn Fn(i64) -> bool) -> Option<usize> {
    let classes = c.list("ClassList")?;
    classes
        .iter()
        .position(|e| e.list("KnownList0").map_or(false, |l| !l.is_empty()))
        .or_else(|| classes.iter().position(|e| is_force_class(e.int("Class").unwrap_or(-1))))
}

pub fn can_have_powers(c: &Struct, is_force_class: &dyn Fn(i64) -> bool) -> bool {
    power_class(c, is_force_class).is_some()
}

pub fn add_power(c: &mut Struct, spell: i64, is_force_class: &dyn Fn(i64) -> bool) -> bool {
    if powers(c).contains(&spell) {
        return false;
    }
    let Some(index) = power_class(c, is_force_class) else { return false };
    let class = &mut c.list_mut("ClassList").unwrap()[index];
    if class.list("KnownList0").is_none() {
        class.set("KnownList0", Value::List(Vec::new()));
    }
    let mut fallback = Struct::new(3);
    fallback.set("Spell", Value::Word(0));
    push_like(class.list_mut("KnownList0").unwrap(), "Spell", spell, fallback);
    true
}

/// Removes exactly this power (KSE removed every power at once in KotOR 2).
pub fn remove_power(c: &mut Struct, spell: i64) -> bool {
    let mut removed = false;
    if let Some(classes) = c.list_mut("ClassList") {
        for class in classes.iter_mut() {
            if let Some(list) = class.list_mut("KnownList0") {
                let before = list.len();
                list.retain(|s| s.int("Spell") != Some(spell));
                removed |= list.len() != before;
            }
        }
    }
    if let Some(levels) = c.list_mut("LvlStatList") {
        for l in levels.iter_mut() {
            if let Some(list) = l.list_mut("KnownList0") {
                list.retain(|s| s.int("Spell") != Some(spell));
            }
        }
    }
    removed
}

// ---- Health and Force: the engine's own rules ----
//
// What the game does with these fields (save-editor-format.md section 3.4 has the whole
// account, with the engine addresses): it never reads MaxHitPoints or MaxForcePoints. When
// a creature loads it computes both maxima from the classes, the level records (the
// player's), the attributes, a few feats and the ability effects, then turns the stored
// current values into the real ones, by one rule for the player and another for anyone
// else. So every value below is computed the way the engine computes it, and the stored
// maxima are only rewritten to match, as the game does at every save.

/// Classes the engine counts as Jedi: KotOR 1's three (classes 3 to 5), and KotOR 2's
/// nine (3 to 5 and the prestige classes 11 to 16).
fn is_jedi(game: Game, class: i64) -> bool {
    match game {
        Game::K1 => (3..=5).contains(&class),
        Game::K2 => (3..=5).contains(&class) || (11..=16).contains(&class),
    }
}

/// Classes whose Force maximum gains 50 under effect type 109 (KotOR 1: 4; KotOR 2: 4, 12,
/// 15).
fn is_fifty_class(game: Game, class: i64) -> bool {
    match game {
        Game::K1 => class == 4,
        Game::K2 => [4, 12, 15].contains(&class),
    }
}

fn has_feat(c: &Struct, feat: i64) -> bool {
    c.list("FeatList").map_or(false, |l| l.iter().any(|f| f.int("Feat") == Some(feat)))
}

fn has_effect(c: &Struct, kind: i64) -> bool {
    c.list("EffectList").map_or(false, |l| l.iter().any(|e| e.int("Type") == Some(kind)))
}

/// An attribute with what the creature's ability effects (type 36: the attribute's number,
/// then the amount, as items and powers put them in the save) add to it. The engine's
/// modifiers are taken from this value, not the bare score.
pub fn effective_attribute(c: &Struct, label: &str) -> i64 {
    let index = ATTRIBUTES.iter().position(|a| *a == label).unwrap_or(0) as i64;
    let mut v = attribute(c, label);
    for e in c.list("EffectList").into_iter().flatten().filter(|e| e.int("Type") == Some(36)) {
        let ints: Vec<i64> = e.list("IntList").map(|l| l.iter().filter_map(|i| i.int("Value")).collect()).unwrap_or_default();
        if ints.first() == Some(&index) {
            v += ints.get(1).copied().unwrap_or(0);
        }
    }
    v.max(3)
}

fn effective_modifier(c: &Struct, label: &str) -> i64 {
    modifier(effective_attribute(c, label))
}

/// The modifier the Force maximum adds per level: Wisdom and Charisma in KotOR 1, Wisdom
/// alone in KotOR 2.
fn force_modifier(c: &Struct, game: Game) -> i64 {
    match game {
        Game::K1 => effective_modifier(c, "Wis") + effective_modifier(c, "Cha"),
        Game::K2 => effective_modifier(c, "Wis"),
    }
}

/// The engine sums the player's maxima from the level records; anyone else's (party
/// members, area creatures) from the stored base and the level. It takes the player's way
/// when IsPC is set, which only the player has, along with the records.
pub fn uses_level_records(c: &Struct) -> bool {
    c.int("IsPC").map_or(false, |v| v != 0) && c.list("LvlStatList").is_some()
}

/// Whether the player's level records agree with the class list: one record per level, and
/// as many naming each class as it has levels. The game's own saves always do; a save
/// another editor changed may not (one on this machine has twenty records with a hit die of
/// 0 under a 20/20 multiclass). The engine sums Health and Force from the records as they
/// are, so a class change on such a save cannot be undone exactly. True for anyone
/// without records.
pub fn level_records_match(c: &Struct) -> bool {
    let Some(records) = c.list("LvlStatList") else { return true };
    if !uses_level_records(c) {
        return true;
    }
    let classes = classes(c);
    records.len() as i64 == level(c)
        && classes.iter().all(|(id, l)| records.iter().filter(|r| r.int("LvlStatClass") == Some(*id)).count() as i64 == *l)
}

/// The Toughness bonus per level (feat 124: 2, else feat 84: 1).
fn toughness(c: &Struct) -> i64 {
    if has_feat(c, 124) {
        2
    } else if has_feat(c, 84) {
        1
    } else {
        0
    }
}

/// The maximum Health the engine computes (GetMaxHitPoints(1); KotOR 1 004ED310, KotOR 2
/// 0057E8C0). The same in both games: feats 206, 224 and 225 exist only in KotOR 2.
pub fn max_health(c: &Struct) -> i64 {
    let l = level(c);
    let con = effective_modifier(c, "Con");
    let tough = toughness(c);
    let v = if uses_level_records(c) {
        let records = c.list("LvlStatList").unwrap();
        let mut sum: i64 = records.iter().take(l.max(0) as usize).map(|r| (con + r.int("LvlStatHitDie").unwrap_or(0)).max(1) + tough).sum();
        if has_feat(c, 206) {
            sum += 25;
        }
        sum
    } else {
        let mut per = tough;
        if has_feat(c, 206) {
            per += 25;
        }
        per += if has_feat(c, 225) {
            4
        } else if has_feat(c, 224) {
            3
        } else if has_feat(c, 95) {
            2
        } else {
            0
        };
        let base = c.int("HitPoints").unwrap_or(0);
        if base + l * con < l {
            l + l * per
        } else {
            base + l * (con + per)
        }
    };
    v.clamp(i16::MIN as i64, i16::MAX as i64)
}

/// The maximum Force the engine computes (GetMaxForcePoints; KotOR 1 004FD490, KotOR 2
/// 0057ECA0).
pub fn max_force(c: &Struct, game: Game) -> i64 {
    if c.int("Race") == Some(5) {
        return 0; // droids
    }
    let classes = classes(c);
    let l = level(c);
    let m = force_modifier(c, game);
    let mut extra = if has_feat(c, 116) { 40 } else { 0 };
    extra += c.int("BonusForcePoints").unwrap_or(0);
    if has_effect(c, 109) && classes.iter().any(|(k, _)| is_fifty_class(game, *k)) {
        extra += 50;
    }
    let v = if uses_level_records(c) {
        // Only when the newest class is a Jedi class.
        if !classes.last().map_or(false, |(k, _)| is_jedi(game, *k)) {
            return 0;
        }
        let records = c.list("LvlStatList").unwrap();
        let sum: i64 = records
            .iter()
            .take(l.max(0) as usize)
            .filter_map(|r| r.int("LvlStatForce").filter(|d| *d != 0))
            .map(|d| (m + d).max(1))
            .sum();
        (sum + extra).max(0)
    } else {
        if !classes.iter().any(|(k, _)| is_jedi(game, *k)) {
            return 0;
        }
        (c.int("ForcePoints").unwrap_or(0) + l * m + extra).max(l)
    };
    v.clamp(i16::MIN as i64, i16::MAX as i64)
}

/// How the engine's load (KotOR 1 005B1254, KotOR 2 006AFED0) turns a stored current value
/// into the real one for anyone but the player: the stored value plus the level times the
/// modifier, unless base plus that is no more than the level.
fn loaded_current(base: i64, stored: i64, level: i64, modifier: i64) -> i64 {
    if base + level * modifier > level {
        stored + level * modifier
    } else if stored < base {
        level + stored - base
    } else {
        level
    }
}

/// The stored value `loaded_current` turns into `wanted` (or as near as the rule allows).
fn stored_for(base: i64, wanted: i64, level: i64, modifier: i64) -> i64 {
    if base + level * modifier > level {
        wanted - level * modifier
    } else if wanted < level {
        base + wanted - level
    } else {
        base
    }
}

/// Health as the game shows it once the save loads: (current, maximum). The player's
/// stored current value is relative to the base: current = CurrentHitPoints + max -
/// HitPoints. Anyone else's: CurrentHitPoints + level x Con modifier.
pub fn health(c: &Struct) -> (i64, i64) {
    let max = max_health(c);
    let base = c.int("HitPoints").unwrap_or(0);
    let stored = c.int("CurrentHitPoints").unwrap_or(base);
    let current = if uses_level_records(c) {
        stored + max - base
    } else {
        loaded_current(base, stored, level(c), effective_modifier(c, "Con"))
    };
    (current, max)
}

/// Force as the game shows it once the save loads: (current, maximum), by the same two
/// rules as Health, with the Force modifier.
pub fn force(c: &Struct, game: Game) -> (i64, i64) {
    let max = max_force(c, game);
    let base = c.int("ForcePoints").unwrap_or(0);
    let stored = c.int("CurrentForce").unwrap_or(base);
    let current = if uses_level_records(c) {
        stored + max - base
    } else {
        loaded_current(base, stored, level(c), force_modifier(c, game))
    };
    (current, max)
}

fn put_short(c: &mut Struct, label: &str, v: i64) {
    c.set_int(label, v.clamp(i16::MIN as i64, i16::MAX as i64), Value::Short(0));
}

/// Stores `current` Health so the game loads exactly it, with the maximum (and
/// PregameCurrent, the current value) written as the game writes them at a save.
fn put_health(c: &mut Struct, current: i64) {
    let max = max_health(c);
    let base = c.int("HitPoints").unwrap_or(0);
    let stored = if uses_level_records(c) {
        current - max + base
    } else {
        stored_for(base, current, level(c), effective_modifier(c, "Con"))
    };
    put_short(c, "CurrentHitPoints", stored);
    put_short(c, "MaxHitPoints", max);
    let shown = health(c).0;
    if c.get("PregameCurrent").is_some() {
        put_short(c, "PregameCurrent", shown);
    }
}

fn put_force(c: &mut Struct, game: Game, current: i64) {
    let max = max_force(c, game);
    let base = c.int("ForcePoints").unwrap_or(0);
    let stored = if uses_level_records(c) {
        current - max + base
    } else {
        stored_for(base, current, level(c), force_modifier(c, game))
    };
    put_short(c, "CurrentForce", stored);
    put_short(c, "MaxForcePoints", max);
}

/// Writes the stored maxima (and PregameCurrent) as the game writes them at a save, the
/// current values untouched: for a creature made from a template, whose own fields are
/// whatever its author left (the game ignores them; another editor may not).
pub fn sync_maxima(c: &mut Struct, game: Game) {
    let max = max_health(c);
    put_short(c, "MaxHitPoints", max);
    if c.get("PregameCurrent").is_some() {
        let shown = health(c).0;
        put_short(c, "PregameCurrent", shown);
    }
    let fmax = max_force(c, game);
    put_short(c, "MaxForcePoints", fmax);
}

/// Health and Force before a change, to carry the current values across it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pools {
    hp: (i64, i64),
    fp: (i64, i64),
}

fn pools(c: &Struct, game: Game) -> Pools {
    Pools { hp: health(c), fp: force(c, game) }
}

/// After a change to levels, classes or dice: the current values follow the maxima as the
/// engine's level-up moves them (CSWSCreatureStats::LevelUp, KotOR 1 005AABF0, KotOR 2
/// 006B9870): a level gained heals Health fully, and Force rises by what its maximum rose.
/// Anything else (a level taken away, a class changed) keeps what was missing missing.
/// Health stays at 1 or more, Force at 0 or more.
fn settle(c: &mut Struct, game: Game, before: Pools, levelled_up: bool) {
    let (hp_max, fp_max) = (max_health(c), max_force(c, game));
    let hp = if levelled_up { hp_max } else { before.hp.0 + hp_max - before.hp.1 };
    put_health(c, hp.max(1.min(hp_max)));
    let fp = before.fp.0 + fp_max - before.fp.1;
    put_force(c, game, fp.max(0));
}

/// Full Health and Force, as the game shows them after the save loads.
pub fn heal(c: &mut Struct, game: Game) {
    let hp = max_health(c);
    put_health(c, hp);
    let fp = max_force(c, game);
    put_force(c, game, fp);
}

pub fn experience(c: &Struct) -> i64 {
    c.int("Experience").unwrap_or(0)
}

pub fn set_experience(c: &mut Struct, xp: i64) {
    c.set_int("Experience", xp.max(0), Value::Dword(0));
}

pub fn alignment(c: &Struct) -> i64 {
    c.int("GoodEvil").unwrap_or(50)
}

pub fn set_alignment(c: &mut Struct, value: i64) {
    c.set_int("GoodEvil", value.clamp(0, 100), Value::Byte(0));
}

/// A second class at level 1, the way a level-up into it would leave the creature: the
/// class entry, its hit die (and Force die) added to the base Health (and Force), the feats
/// its first level grants on its own and its skill points banked (`raise_one`), and
/// for the player a level record, without which the game's Health sum runs past the
/// end of the list. Health and Force move as a level-up moves them (`settle`). The game
/// recognises two classes at most.
pub fn add_class(c: &mut Struct, class: &crate::catalog::Class, xp_needed: Option<i64>) -> bool {
    let Some(entry) = second_class_entry(c, class, 0) else { return false };
    let before = pools(c, class.game);
    c.list_mut("ClassList").unwrap().push(entry);
    let index = c.list("ClassList").unwrap().len() - 1;
    raise_one(c, index, class);
    settle(c, class.game, before, true);
    if let Some(need) = xp_needed {
        if experience(c) < need {
            set_experience(c, need);
        }
    }
    true
}

/// The ClassList entry for a second class at `level`, shaped like the first; None when the
/// creature has no class list, already has two classes, or already has this one.
fn second_class_entry(c: &Struct, class: &crate::catalog::Class, level: i64) -> Option<Struct> {
    let list = c.list("ClassList")?;
    if list.is_empty() || list.len() >= 2 || list.iter().any(|e| e.int("Class") == Some(class.id)) {
        return None;
    }
    let force_template = list.iter().find(|e| e.list("KnownList0").is_some()).cloned();
    let mut entry = list[0].clone();
    entry.fields.retain(|(l, _)| l != "KnownList0" && l != "SpellsPerDayList");
    entry.set_int("Class", class.id, Value::Int(0));
    entry.set_int("ClassLevel", level, Value::Short(0));
    if class.force_die > 0 {
        match force_template {
            Some(t) => {
                if let Some(v) = t.get("SpellsPerDayList") {
                    entry.set("SpellsPerDayList", v.clone());
                }
                entry.set("KnownList0", Value::List(Vec::new()));
            }
            None => {
                let mut left = Struct::new(17767);
                left.set("NumSpellsLeft", Value::Byte(0));
                entry.set("SpellsPerDayList", Value::List(vec![left]));
                entry.set("KnownList0", Value::List(Vec::new()));
            }
        }
    }
    Some(entry)
}

/// What KotOR 2's script command AddMultiClass (routine 389, swkotor2.exe 0066C7A0) leaves
/// on a party member, as a save written straight after it would hold it. Used for the
/// companions' Jedi training (a_makejedi). The engine:
///
/// - takes the XP the next level needs (CSWSCreatureStats 006B72E0: the exptable entry for
///   the current total level) less the current XP;
/// - adds the class at level 0 (006BDCD0, the level argument is 0): no hit die, no Force
///   die, no feats or powers, the total level unchanged;
/// - for anyone but the player, adds that difference to this creature's XP alone (006B7060,
///   whose multiclass penalty is 0 here: no race in KotOR 2's racialtypes.2da has a
///   favoured class, and a level 0 class is never more than one level behind) and to its
///   JoiningXP, so the catch-up XP a party member gets on joining does not take it back;
///   the engine refuses to lower XP (006B6BD0) but adds the difference to JoiningXP even
///   when it is negative.
///
/// The level-up into the new class is then the player's, in the game: the save writes
/// MClassLevUpIn as the class count less one, the new class. Health is untouched; Force
/// keeps its current value while its maximum now counts the Jedi class (the engine's Force
/// maximum looks for a Jedi class, whatever its level). `xp_next` is the XP the next level
/// needs (Catalog::xp_for(level + 1)); None leaves XP alone.
pub fn add_multiclass(c: &mut Struct, class: &crate::catalog::Class, xp_next: Option<i64>) -> bool {
    let Some(entry) = second_class_entry(c, class, 0) else { return false };
    let game = class.game;
    let before = pools(c, game);
    c.list_mut("ClassList").unwrap().push(entry);
    let count = c.list("ClassList").map_or(0, |l| l.len()) as i64;
    c.set_int("MClassLevUpIn", count - 1, Value::Byte(0));
    if let Some(need) = xp_next {
        let xp = experience(c);
        let delta = need - xp;
        if delta > 0 {
            set_experience(c, need);
        }
        let joining = c.int("JoiningXP").unwrap_or(0);
        c.set_int("JoiningXP", joining + delta, Value::Int(0));
    }
    put_force(c, game, before.fp.0.max(0));
    sync_maxima(c, game);
    true
}

/// A new portrait, with the body that goes with it in the same build (small, medium,
/// large) as the current one, and the matching gender.
pub fn set_look(c: &mut Struct, portrait: &crate::catalog::Portrait, current: Option<&crate::catalog::Portrait>) {
    let appearance = c.int("Appearance_Type").unwrap_or(0);
    let build = current.and_then(|p| p.bodies.iter().position(|&b| b == appearance)).unwrap_or(1);
    c.set_int("PortraitId", portrait.id, Value::Word(0));
    c.set_int("Appearance_Type", portrait.bodies[build], Value::Word(0));
    c.set_int("Gender", portrait.female as i64, Value::Byte(0));
}

pub fn equipped(c: &Struct) -> Vec<&Struct> {
    c.list("Equip_ItemList").map(|l| l.iter().collect()).unwrap_or_default()
}

// ---- what KSE also edits: look, voice, health, classes and levels, equipment ----

pub fn set_portrait(c: &mut Struct, id: i64) {
    c.set_int("PortraitId", id, Value::Word(0));
}

pub fn set_appearance(c: &mut Struct, id: i64) {
    c.set_int("Appearance_Type", id, Value::Word(0));
}

pub fn set_gender(c: &mut Struct, id: i64) {
    c.set_int("Gender", id, Value::Byte(0));
}

pub fn set_soundset(c: &mut Struct, id: i64) {
    c.set_int("SoundSetFile", id, Value::Word(0));
}

/// Health as the game shows it, set (1 up to the maximum): stored so the engine's load
/// gives exactly this value back.
pub fn set_current_health(c: &mut Struct, shown: i64) {
    let max = max_health(c);
    put_health(c, shown.min(max).max(1.min(max)));
}

/// Force as the game shows it, set (0 up to the maximum).
pub fn set_current_force(c: &mut Struct, game: Game, shown: i64) {
    let max = max_force(c, game);
    put_force(c, game, shown.min(max).max(0));
}

/// Moves the dice of the player's level records (`field`, newest record first) by `delta`
/// in total, each kept within `low..=255`; returns what was applied.
fn move_record_dice(c: &mut Struct, field: &str, delta: i64, low: i64, only_nonzero: bool) -> i64 {
    let l = level(c).max(0) as usize;
    let mut left = delta;
    if let Some(records) = c.list_mut("LvlStatList") {
        let n = records.len().min(l);
        for r in records[..n].iter_mut().rev() {
            if left == 0 {
                break;
            }
            let die = r.int(field).unwrap_or(0);
            if only_nonzero && die == 0 {
                continue;
            }
            let new = (die + left).clamp(low, 255);
            left -= new - die;
            r.set_int(field, new, Value::Byte(0));
        }
    }
    delta - left
}

/// The maximum Health, set. The engine computes it, so what changes is what it is computed
/// from: the player's level-record hit dice (newest first, each 1 to 255), anyone else's
/// base (HitPoints). The player's base moves by the same amount, as a level-up moves it
/// with the die. The current value keeps what was missing, as far as it can; the result is
/// what the engine computes, which a value outside its reach (a record's die floor, the
/// floor of 1 per level) can stop short of.
pub fn set_max_health(c: &mut Struct, max: i64) {
    let (cur, now) = health(c);
    let want = max.max(1);
    let mut delta = want - now;
    if delta == 0 {
        return;
    }
    if uses_level_records(c) {
        // A record's term is max(1, Con + die): only dice above that floor count, so step
        // until the computed maximum gets there or stops moving.
        for _ in 0..8 {
            let applied = move_record_dice(c, "LvlStatHitDie", delta, 1, false);
            add_to_short(c, "HitPoints", applied);
            delta = want - max_health(c);
            if delta == 0 || applied == 0 {
                break;
            }
        }
    } else {
        add_to_short(c, "HitPoints", delta);
    }
    let new = max_health(c);
    put_health(c, (cur + new - now).min(new).max(1.min(new)));
}

/// The maximum Force, set: through the Force dice of the player's records (those that have
/// one, newest first, kept at 1 or more) or anyone else's base (ForcePoints). A creature
/// the engine gives no Force (no Jedi class, a droid, the player's newest class not a Jedi
/// class) keeps 0. The current value keeps what was missing.
pub fn set_max_force(c: &mut Struct, game: Game, max: i64) {
    let (cur, now) = force(c, game);
    let want = max.max(0);
    let mut delta = want - now;
    if delta == 0 || !max_force_possible(c, game) {
        return;
    }
    if uses_level_records(c) {
        for _ in 0..8 {
            let applied = move_record_dice(c, "LvlStatForce", delta, 1, true);
            add_to_short(c, "ForcePoints", applied);
            delta = want - max_force(c, game);
            if delta == 0 || applied == 0 {
                break;
            }
        }
    } else {
        add_to_short(c, "ForcePoints", delta);
    }
    let new = max_force(c, game);
    put_force(c, game, (cur + new - now).min(new).max(0));
}

/// Whether the engine gives this creature any Force at all.
fn max_force_possible(c: &Struct, game: Game) -> bool {
    if c.int("Race") == Some(5) {
        return false;
    }
    let classes = classes(c);
    if uses_level_records(c) {
        classes.last().map_or(false, |(k, _)| is_jedi(game, *k))
            && c.list("LvlStatList").map_or(false, |l| l.iter().any(|r| r.int("LvlStatForce").unwrap_or(0) != 0))
    } else {
        classes.iter().any(|(k, _)| is_jedi(game, *k))
    }
}

pub fn min1hp(c: &Struct) -> bool {
    c.int("Min1HP") == Some(1)
}

pub fn set_min1hp(c: &mut Struct, on: bool) {
    c.set_int("Min1HP", on as i64, Value::Byte(0));
}

/// A level record for one level of `class`, shaped like the records already there.
fn level_record(c: &Struct, class: &crate::catalog::Class) -> Struct {
    let mut record = c.list("LvlStatList").and_then(|l| l.last().cloned()).unwrap_or_else(|| Struct::new(0));
    record.fields.retain(|(l, _)| l != "LvlStatAbility");
    record.set_int("LvlStatHitDie", class.hit_die, Value::Byte(0));
    record.set_int("LvlStatForce", class.force_die, Value::Byte(0));
    record.set_int("LvlStatClass", class.id, Value::Byte(0));
    record.set_int("SkillPoints", 0, Value::Word(0));
    if let Some(skills) = record.list_mut("SkillList") {
        for s in skills.iter_mut() {
            s.set_int("Rank", 0, Value::Byte(0));
        }
    }
    for l in ["FeatList", "KnownList0"] {
        if record.get(l).is_some() {
            record.set(l, Value::List(Vec::new()));
        }
    }
    record
}

/// Adds `delta` to a Short field (the base Health or Force), without a floor: lowering a
/// level takes back exactly what raising it gave.
fn add_to_short(c: &mut Struct, label: &str, delta: i64) {
    if delta == 0 {
        return;
    }
    let v = c.int(label).unwrap_or(0);
    put_short(c, label, v + delta);
}

/// What `levels` levels of `class` add to the base Health and Force, as the engine's
/// level-up adds a record's hit die and Force die to them.
fn add_base(c: &mut Struct, class: &crate::catalog::Class, levels: i64) {
    add_to_short(c, "HitPoints", class.hit_die * levels);
    add_to_short(c, "ForcePoints", class.force_die * levels);
}

/// Sets class entry `index` to `level`. Going up adds, per level, what the game's level-up
/// gives before any choice (`raise_one`: the dice on the bases, the feats the level grants
/// on its own, the skill points banked unspent, and for the player a level record); Health is then full and
/// Force rises by what its maximum rose, as the game's level-up leaves them. Going down
/// takes the player's last records of that class away together with what they gave
/// (feats, powers, skill ranks, the raised attribute), as the game would never have
/// granted them, and keeps what was missing of Health and Force missing. Experience moves
/// to the first XP of the new total level when it no longer fits.
pub fn set_class_level(c: &mut Struct, index: usize, level_wanted: i64, class: &crate::catalog::Class, xp_for: &dyn Fn(i64) -> Option<i64>) {
    let before = pools(c, class.game);
    let Some(delta) = change_levels(c, index, level_wanted, class) else { return };
    settle(c, class.game, before, delta > 0);
    fit_experience(c, xp_for);
}

/// The class entry, bases and records of `set_class_level`, without Health and Force;
/// returns the levels added (negative: taken away).
fn change_levels(c: &mut Struct, index: usize, level_wanted: i64, class: &crate::catalog::Class) -> Option<i64> {
    let now = c.list("ClassList").and_then(|l| l.get(index)).and_then(|e| e.int("ClassLevel"))?;
    let wanted = level_wanted.max(1);
    let delta = wanted - now;
    if delta == 0 {
        return None;
    }
    for _ in 0..delta.abs() {
        if delta > 0 {
            raise_one(c, index, class);
        } else {
            lower_one(c, index, class);
        }
    }
    Some(delta)
}

fn set_class_entry_level(c: &mut Struct, index: usize, level: i64) {
    if let Some(e) = c.list_mut("ClassList").and_then(|l| l.get_mut(index)) {
        e.set_int("ClassLevel", level, Value::Short(0));
    }
}

/// The banked skill points (the creature's SkillPoints: what the last level-up left
/// unspent, which the next one starts from).
pub fn skill_points(c: &Struct) -> i64 {
    c.int("SkillPoints").unwrap_or(0)
}

fn set_skill_points(c: &mut Struct, v: i64) {
    c.set_int("SkillPoints", v.clamp(0, u16::MAX as i64), Value::Word(0));
}

/// The skill points a level-up gives, the creature's levels already counting the new one.
/// KotOR 2 (AutoLevelup 006BEDF0, and the player's level records): SkillPointBase plus the
/// Intelligence modifier, at least 1, four times at the first character level. KotOR 1
/// (AutoLevelup 005B27E0, and the player's level records): half of that sum, at least 1;
/// the first character level (half the base plus the modifier, at least 1) times four.
/// The modifier is the base score's; no race adds any.
pub fn level_skill_points(c: &Struct, class: &crate::catalog::Class) -> i64 {
    let base = class.grants.skill_point_base;
    let m = modifier(attribute(c, "Int"));
    let first = level(c) == 1;
    match class.game {
        Game::K2 => {
            let v = (base + m).max(1);
            if first { v * 4 } else { v }
        }
        Game::K1 => {
            if first {
                (base / 2 + m).max(1) * 4
            } else {
                ((base + m) / 2).max(1)
            }
        }
    }
}

/// The feats the game's level-up grants on its own for the level class entry `index` has
/// just reached (its other levels and classes as they stand), in feat order; with
/// `skip_held`, those the creature already has are left out, as the game leaves them.
///
/// KotOR 1 (AutoLevelup 005B27E0; the player's level records agree): the class's granted
/// feats whose level is the new class level.
///
/// KotOR 2 (AutoLevelup 006BEDF0; the player's level records agree), for every feat:
/// - not granted by the class: granted by a companion column (by the creature's tag:
///   handmaiden, baodur, hanharr, hk47, g0t0, atton) whose value is the new total level;
/// - granted by the class and not 212 to 219: when its level is the new class level, unless
///   the creature is Bao-Dur or Kreia and their column says 255 (never);
/// - 212 to 219 (Unarmed Specialist I to VIII) granted by the class: when the class is a
///   Jedi class and its level is the creature's levels in Jedi classes.
pub fn level_feats(c: &Struct, index: usize, class: &crate::catalog::Class, skip_held: bool) -> Vec<i64> {
    let cls = classes(c);
    let class_level = cls.get(index).map_or(0, |e| e.1);
    let total = level(c);
    let mut out: Vec<i64> = Vec::new();
    match class.game {
        Game::K1 => {
            for (f, l) in &class.grants.granted {
                if *l == class_level {
                    out.push(*f);
                }
            }
        }
        Game::K2 => {
            let jedi: i64 = cls.iter().filter(|(k, _)| is_jedi(Game::K2, *k)).map(|(_, l)| l).sum();
            let tag = c.string("Tag").unwrap_or_default().to_ascii_lowercase();
            let column = |f: i64, name: &str| -> i64 {
                let i = crate::catalog::COMPANION_COLUMNS.iter().position(|n| *n == name).unwrap();
                class.grants.companion.iter().find(|(k, _)| *k == f).map_or(0, |(_, v)| v[i])
            };
            let never = |v: i64| v == 255 || v == -1;
            let mut all: Vec<i64> = class.grants.granted.iter().map(|(f, _)| *f).chain(class.grants.companion.iter().map(|(f, _)| *f)).collect();
            all.sort();
            all.dedup();
            for f in all {
                match class.grants.granted.iter().find(|(k, _)| *k == f).map(|(_, l)| *l) {
                    None => {
                        let own = crate::catalog::COMPANION_COLUMNS[..6].iter().find(|n| **n == tag);
                        if let Some(n) = own {
                            let v = column(f, n);
                            if v != 0 && !never(v) && v == total {
                                out.push(f);
                            }
                        }
                    }
                    Some(l) if !(212..=219).contains(&f) => {
                        let barred = (tag == "baodur" && never(column(f, "baodur"))) || (tag == "kreia" && never(column(f, "kreia")));
                        if l == class_level && !barred {
                            out.push(f);
                        }
                    }
                    Some(l) => {
                        if is_jedi(Game::K2, class.id) && l == jedi {
                            out.push(f);
                        }
                    }
                }
            }
        }
    }
    if skip_held {
        let have = feats(c);
        out.retain(|f| !have.contains(f));
    }
    out
}

/// One level up in class entry `index` (which may be at level 0), as the game's level-up
/// leaves it before any choice: the class level, the hit and Force dice on the bases, the
/// feats the level grants on its own, the level's skill points banked unspent, and for the
/// player a level record holding those feats and the banked points. Feats and powers the
/// player picks, skill ranks and the attribute raise every fourth level are choices the
/// game makes no record of before they are made.
fn raise_one(c: &mut Struct, index: usize, class: &crate::catalog::Class) {
    let now = c.list("ClassList").and_then(|l| l.get(index)).and_then(|e| e.int("ClassLevel")).unwrap_or(0);
    set_class_entry_level(c, index, now + 1);
    add_base(c, class, 1);
    let granted = level_feats(c, index, class, true);
    for f in &granted {
        add_feat(c, *f);
    }
    let banked = skill_points(c) + level_skill_points(c, class);
    set_skill_points(c, banked);
    if c.list("LvlStatList").is_some() {
        let mut record = level_record(c, class);
        let shape = c.list("FeatList").and_then(|l| l.first().cloned());
        let list: Vec<Struct> = granted
            .iter()
            .map(|f| {
                let mut e = shape.clone().unwrap_or_else(|| Struct::new(1));
                e.set_int("Feat", *f, Value::Word(0));
                e
            })
            .collect();
        if !list.is_empty() || record.get("FeatList").is_some() {
            record.set("FeatList", Value::List(list));
        }
        record.set_int("SkillPoints", banked, Value::Word(0));
        c.list_mut("LvlStatList").unwrap().push(record);
    }
}

/// One level down in class entry `index`, taking back what `raise_one` (or the game's
/// level-up) gave: the player's last record of that class with its feats, powers, ranks,
/// attribute and the skill points it banked; for anyone else, who keeps no records, the
/// feats that level grants on its own and the points it gives.
fn lower_one(c: &mut Struct, index: usize, class: &crate::catalog::Class) {
    let now = c.list("ClassList").and_then(|l| l.get(index)).and_then(|e| e.int("ClassLevel")).unwrap_or(0);
    if now <= 0 {
        return;
    }
    if c.list("LvlStatList").is_some() {
        let levels = c.list("LvlStatList").unwrap();
        // The last record of that class; the last record when none names it (a save
        // another editor left with records that do not match its classes).
        if let Some(pos) = levels.iter().rposition(|r| r.int("LvlStatClass") == Some(class.id)).or(levels.len().checked_sub(1)) {
            let previous = pos.checked_sub(1).and_then(|p| levels.get(p)).and_then(|r| r.int("SkillPoints")).unwrap_or(0);
            let record = c.list_mut("LvlStatList").unwrap().remove(pos);
            undo_record(c, &record);
            let gave = record.int("SkillPoints").unwrap_or(0) - previous;
            if gave > 0 {
                let sp = skill_points(c);
                set_skill_points(c, sp - gave);
            }
        }
    } else {
        for f in level_feats(c, index, class, false) {
            if let Some(list) = c.list_mut("FeatList") {
                list.retain(|x| x.int("Feat") != Some(f));
            }
        }
        let points = level_skill_points(c, class);
        let sp = skill_points(c);
        set_skill_points(c, sp - points);
    }
    set_class_entry_level(c, index, now - 1);
    add_base(c, class, -1);
}

/// Experience within the current total level: raised to its first XP, or lowered when it
/// would already reach the next level.
fn fit_experience(c: &mut Struct, xp_for: &dyn Fn(i64) -> Option<i64>) {
    let total = level(c);
    let xp = experience(c);
    let low = xp_for(total).unwrap_or(0);
    let high = xp_for(total + 1);
    if xp < low || high.map_or(false, |h| xp >= h) {
        set_experience(c, low);
    }
}

/// Takes back what one level record gave.
fn undo_record(c: &mut Struct, record: &Struct) {
    let feats: Vec<i64> = record.list("FeatList").map(|l| l.iter().filter_map(|f| f.int("Feat")).collect()).unwrap_or_default();
    for f in feats {
        if let Some(list) = c.list_mut("FeatList") {
            list.retain(|x| x.int("Feat") != Some(f));
        }
    }
    let powers: Vec<i64> = record.list("KnownList0").map(|l| l.iter().filter_map(|s| s.int("Spell")).collect()).unwrap_or_default();
    for p in powers {
        if let Some(classes) = c.list_mut("ClassList") {
            for e in classes.iter_mut() {
                if let Some(list) = e.list_mut("KnownList0") {
                    list.retain(|s| s.int("Spell") != Some(p));
                }
            }
        }
    }
    let ranks: Vec<i64> = record.list("SkillList").map(|l| l.iter().map(|s| s.int("Rank").unwrap_or(0)).collect()).unwrap_or_default();
    for (i, r) in ranks.into_iter().enumerate() {
        if r > 0 {
            let now = skill(c, i);
            set_skill(c, i, now - r);
        }
    }
    if let Some(a) = record.int("LvlStatAbility") {
        if let Some(label) = ATTRIBUTES.get(a as usize) {
            let v = attribute(c, label);
            set_attribute(c, label, v - 1);
        }
    }
}

/// Changes class entry `index` to another class at the same level: the hit and Force
/// dice of its levels change with it (the first level's record keeps having no Force die:
/// KotOR 2's has none even for a Jedi); powers go when the new class has no Force, and a
/// Force class gets an empty power list. What was missing of Health and Force stays
/// missing.
pub fn change_class(c: &mut Struct, index: usize, old: &crate::catalog::Class, new: &crate::catalog::Class) {
    let Some(entry) = c.list("ClassList").and_then(|l| l.get(index)).cloned() else { return };
    if c.list("ClassList").unwrap().iter().any(|e| e.int("Class") == Some(new.id)) {
        return;
    }
    let before = pools(c, new.game);
    let levels = entry.int("ClassLevel").unwrap_or(1);
    {
        let e = &mut c.list_mut("ClassList").unwrap()[index];
        e.set_int("Class", new.id, Value::Int(0));
        if new.force_die > 0 {
            if e.get("KnownList0").is_none() {
                e.set("KnownList0", Value::List(Vec::new()));
            }
            if e.get("SpellsPerDayList").is_none() {
                let mut left = Struct::new(17767);
                left.set("NumSpellsLeft", Value::Byte(0));
                e.set("SpellsPerDayList", Value::List(vec![left]));
            }
        } else {
            e.fields.retain(|(l, _)| l != "KnownList0" && l != "SpellsPerDayList");
        }
    }
    add_to_short(c, "HitPoints", (new.hit_die - old.hit_die) * levels);
    add_to_short(c, "ForcePoints", (new.force_die - old.force_die) * levels);
    if let Some(records) = c.list_mut("LvlStatList") {
        for (i, r) in records.iter_mut().enumerate().filter(|(_, r)| r.int("LvlStatClass") == Some(old.id)) {
            r.set_int("LvlStatClass", new.id, Value::Byte(0));
            r.set_int("LvlStatHitDie", new.hit_die, Value::Byte(0));
            // The first record (character creation) has no Force die in either game's
            // saves, KotOR 2's Jedi included; it keeps none.
            let had = r.int("LvlStatForce").unwrap_or(0);
            if i > 0 || had != 0 {
                r.set_int("LvlStatForce", new.force_die, Value::Byte(0));
            }
            if new.force_die == 0 && r.get("KnownList0").is_some() {
                r.set("KnownList0", Value::List(Vec::new()));
            }
        }
    }
    settle(c, new.game, before, false);
}

/// Removes a class entry when there are two: its levels are taken back first, as if
/// levelled down, then the entry goes with its powers. What was missing of Health and
/// Force stays missing.
pub fn remove_class(c: &mut Struct, index: usize, class: &crate::catalog::Class, xp_for: &dyn Fn(i64) -> Option<i64>) {
    if c.list("ClassList").map_or(true, |l| l.len() < 2 || index >= l.len()) {
        return;
    }
    let before = pools(c, class.game);
    change_levels(c, index, 1, class);
    lower_one(c, index, class);
    c.list_mut("ClassList").unwrap().remove(index);
    settle(c, class.game, before, false);
    fit_experience(c, xp_for);
}

// ---- equipment ----

/// The equipment slots, by the struct id the game gives an equipped item (measured in
/// both games' saves), and their names. The second weapon set is KotOR 2's.
pub const SLOTS: [(u32, &str); 11] = [
    (0x1, "Head"),
    (0x2, "Body"),
    (0x8, "Hands"),
    (0x10, "Right weapon"),
    (0x20, "Left weapon"),
    (0x80, "Left arm"),
    (0x100, "Right arm"),
    (0x200, "Implant"),
    (0x400, "Belt"),
    (0x40000, "Right weapon, set 2"),
    (0x80000, "Left weapon, set 2"),
];

pub fn in_slot(c: &Struct, slot: u32) -> Option<&Struct> {
    c.list("Equip_ItemList")?.iter().find(|i| i.id == slot)
}

/// Takes the item out of `slot` with the effects it gave (each names the item's object id
/// as its creator). Returns it without the object id, as the party inventory keeps items.
pub fn unequip(c: &mut Struct, slot: u32) -> Option<Struct> {
    let list = c.list_mut("Equip_ItemList")?;
    let pos = list.iter().position(|i| i.id == slot)?;
    let mut item = list.remove(pos);
    let id = item.int("ObjectId");
    if let (Some(id), Some(effects)) = (id, c.list_mut("EffectList")) {
        effects.retain(|e| e.int("CreatorId") != Some(id));
    }
    item.id = 0;
    item.fields.retain(|(l, _)| l != "ObjectId");
    Some(item)
}

/// Puts `item` into `slot` with a new object id, in slot order.
pub fn equip(c: &mut Struct, slot: u32, mut item: Struct, object_id: u32) {
    if c.list("Equip_ItemList").is_none() {
        c.set("Equip_ItemList", Value::List(Vec::new()));
    }
    item.id = slot;
    item.fields.retain(|(l, _)| l != "ObjectId");
    item.fields.insert(0, ("ObjectId".into(), Value::Dword(object_id)));
    let list = c.list_mut("Equip_ItemList").unwrap();
    list.retain(|i| i.id != slot);
    let at = list.iter().position(|i| i.id > slot).unwrap_or(list.len());
    list.insert(at, item);
}
