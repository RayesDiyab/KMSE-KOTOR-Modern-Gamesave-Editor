//! One save folder, read whole into memory, changed there and written back together.
//!
//! Where things live (measured on real saves, see save-editor/docs/save-editor.md):
//! - the player: `Mod_PlayerList` of the last module's Module.ifo inside SAVEGAME.sav,
//!   and KotOR 2 also keeps `pc.utc` in SAVEGAME.sav; both are changed together;
//! - party members: `AVAILNPCn.utc` in SAVEGAME.sav, and a second copy in the current
//!   area's creature list when they stand there (the Ebon Hawk); both are changed;
//! - credits, party XP, influence, components: PARTYTABLE.res;
//! - the party's items: INVENTORY in SAVEGAME.sav;
//! - story switches and counters: GLOBALVARS.res.
//!
//! Only the files that changed are written, each to a new file first and then moved
//! over the old one, after a copy of the whole save folder has been made.

use crate::formats::erf::{Erf, RES_TYPE_GIT, RES_TYPE_IFO, RES_TYPE_RES, RES_TYPE_SAV, RES_TYPE_UTC};
use crate::formats::gff::{Gff, Struct, Value};
use crate::formats::tga;
use crate::games::Game;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

fn file_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

fn read_gff(dir: &Path, name: &str) -> Result<(PathBuf, Gff)> {
    let path = file_ci(dir, name).ok_or_else(|| format!("{} is missing", name))?;
    let data = fs::read(&path).map_err(|e| format!("{}: {}", name, e))?;
    let gff = Gff::read(&data).map_err(|e| format!("{}: {}", name, e))?;
    Ok((path, gff))
}

/// What the save list shows, read from savenfo.res alone.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub name: String,
    pub area: String,
    pub pc_name: String,
    pub seconds: i64,
    pub portraits: Vec<String>,
    pub modified: Option<std::time::SystemTime>,
    pub game: Option<Game>,
}

pub fn summary(folder: &Path) -> Option<Summary> {
    let (path, nfo) = read_gff(folder, "savenfo.res").ok()?;
    let r = &nfo.root;
    let mut name = r.string("SAVEGAMENAME").unwrap_or_default();
    if name.trim().is_empty() {
        // Quick and auto saves have no name of their own: "000001 - AUTOSAVE".
        let folder_name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        name = folder_name.split_once(" - ").map(|(_, n)| n.to_string()).unwrap_or(folder_name);
        name = title_case(&name);
    }
    let portraits = (0..3).filter_map(|i| r.string(&format!("PORTRAIT{}", i))).filter(|p| !p.is_empty()).collect();
    Some(Summary {
        name,
        area: r.string("AREANAME").unwrap_or_default(),
        pc_name: r.string("PCNAME").unwrap_or_default(),
        seconds: r.int("TIMEPLAYED").unwrap_or(0),
        portraits,
        modified: fs::metadata(&path).and_then(|m| m.modified()).ok(),
        game: Some(game_of_nfo(r)),
    })
}

fn title_case(s: &str) -> String {
    if s.chars().any(|c| c.is_lowercase()) {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// KotOR 2's savenfo carries fields KotOR 1's never has.
fn game_of_nfo(nfo: &Struct) -> Game {
    if nfo.get("PCNAME").is_some() || nfo.get("STORYHINT0").is_some() || nfo.get("TIMESTAMP").is_some() {
        Game::K2
    } else {
        Game::K1
    }
}

pub fn peek_game(folder: &Path) -> Option<Game> {
    summary(folder).and_then(|s| s.game)
}

pub fn screenshot(folder: &Path) -> Option<tga::Image> {
    tga::decode(&fs::read(file_ci(folder, "Screen.tga")?).ok()?)
}

/// A creature in the save the editor can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Player,
    Npc(usize),
}

#[derive(Clone)]
pub struct Save {
    pub game: Game,
    pub folder: PathBuf,
    pub nfo: Gff,
    pub party: Gff,
    pub globals: Option<Gff>,
    nfo_path: PathBuf,
    party_path: PathBuf,
    globals_path: Option<PathBuf>,
    archive_path: PathBuf,
    archive: Erf,
    /// The last module's nested save, with its Module.ifo and area .git read.
    module_name: String,
    module: Option<Erf>,
    ifo: Option<Gff>,
    git: Option<(String, Gff)>,
    /// GFF resources of SAVEGAME.sav that were read: AVAILNPCn, pc, INVENTORY.
    loaded: Vec<(String, u16, Gff)>,
    dirty: BTreeSet<&'static str>,
    dirty_archive: BTreeSet<(String, u16)>,
    /// SAVEGAME.sav changed in memory (a module flushed into it).
    archive_dirty: bool,
    /// What changed, by a key per value, so changing one value twice counts once.
    pub changes: BTreeSet<String>,
    /// The save's files as they were when read, to see that the game has not written since.
    on_disk: Vec<(String, u64, Option<std::time::SystemTime>)>,
    // the next object id new_object_id hands out where the module has no counter
    handed_out: u32,
    /// The backup made (or found the same) when the save was opened, see `back_up`.
    pub backup: Option<PathBuf>,
}

/// Every file of the save folder: name, size and time, in name order.
/// Why a save is refused when a write into it was cut off (interrupted_write).
pub const INTERRUPTED: &str = "The editor was stopped while it wrote this save, so its files may be part old and part new. Nothing was changed by opening it. Copy the files of its latest backup (Save Editor Backup, in the game's folder) over this save folder, then open it again. Files left from the write:";

/// The files a write leaves only while it runs: `*.kmrp-new`, a new file not yet in place, and
/// `*.kmrp-old`, an old one waiting to be removed. A write cut off part way (the program ended,
/// the power failed) leaves a new file not yet in place, or an old one whose file is gone. An
/// old one beside its file is only clean-up that failed after a finished write (a file held
/// by a virus scanner), which the next write replaces. Named, or None.
pub fn interrupted_write(folder: &Path) -> Option<String> {
    let names: Vec<String> = fs::read_dir(folder).ok()?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    let left: Vec<String> = names
        .iter()
        .filter(|n| {
            n.ends_with(".kmrp-new")
                || n.strip_suffix(".kmrp-old").map_or(false, |base| {
                    // compared as bytes: a name with letters of more than one byte cannot be cut
                    let b = base.as_bytes();
                    !names.iter().any(|x| {
                        let xb = x.as_bytes();
                        xb.len() > b.len() && xb[..b.len()].eq_ignore_ascii_case(b) && xb[b.len()] == b'.' && !x.ends_with(".kmrp-old") && !x.ends_with(".kmrp-new")
                    })
                })
        })
        .cloned()
        .collect();
    (!left.is_empty()).then(|| left.join(", "))
}

fn files_on_disk(folder: &Path) -> Vec<(String, u64, Option<std::time::SystemTime>)> {
    let mut files: Vec<_> = fs::read_dir(folder)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| Some((e.file_name().to_string_lossy().to_string(), e.metadata().ok()?)))
                .filter(|(n, m)| m.is_file() && !n.ends_with(".kmrp-new") && !n.ends_with(".kmrp-old"))
                .map(|(n, m)| (n, m.len(), m.modified().ok()))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// How an error starts when a write stopped part way and could not be undone.
pub const HALF_WRITTEN: &str = "The save may be half written";

/// Why a write was refused: the files are not the ones the changes were made on.
pub const CHANGED_ON_DISK: &str = "This save changed on disk after it was opened, so writing now would mix two points in the game. Press Revert to read it again, then make the changes again.";

impl Save {
    pub fn open(folder: &Path) -> Result<Save> {
        if let Some(left) = interrupted_write(folder) {
            return Err(format!("{} {}", INTERRUPTED, left));
        }
        // Before the read: a write that lands during it then counts as a change.
        let on_disk = files_on_disk(folder);
        let (nfo_path, nfo) = read_gff(folder, "savenfo.res")?;
        let (party_path, party) = read_gff(folder, "PARTYTABLE.res")?;
        let globals = read_gff(folder, "GLOBALVARS.res").ok();
        let archive_path = file_ci(folder, "SAVEGAME.sav").ok_or("SAVEGAME.sav is missing")?;
        let archive = Erf::read(&fs::read(&archive_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let game = game_of_nfo(&nfo.root);
        let module_name = nfo.root.string("LASTMODULE").unwrap_or_default();
        let mut save = Save {
            game,
            folder: folder.to_path_buf(),
            nfo,
            party,
            globals_path: globals.as_ref().map(|g| g.0.clone()),
            globals: globals.map(|g| g.1),
            nfo_path,
            party_path,
            archive_path,
            archive,
            module_name,
            module: None,
            ifo: None,
            git: None,
            loaded: Vec::new(),
            dirty: BTreeSet::new(),
            dirty_archive: BTreeSet::new(),
            archive_dirty: false,
            changes: BTreeSet::new(),
            on_disk,
            handed_out: 0,
            backup: None,
        };
        save.load_module();
        for r in save.archive.resources.iter() {
            let lower = r.name.to_ascii_lowercase();
            let wanted = (r.kind == RES_TYPE_UTC && (lower.starts_with("availnpc") || lower == "pc"))
                || (r.kind == RES_TYPE_RES && lower == "inventory");
            if wanted {
                if let Ok(g) = Gff::read(&r.data) {
                    save.loaded.push((r.name.clone(), r.kind, g));
                }
            }
        }
        if save.player().is_none() && file_ci(folder, "pifo.ifo").is_some() {
            save.ifo = Some(read_gff(folder, "pifo.ifo")?.1);
        }
        if save.player().is_none() {
            return Err("this save has no player character in it".into());
        }
        Ok(save)
    }

    fn load_module(&mut self) {
        let Some(r) = self.archive.find(&self.module_name, RES_TYPE_SAV) else { return };
        let Ok(module) = Erf::read(&r.data) else { return };
        self.ifo = module
            .resources
            .iter()
            .find(|r| r.kind == RES_TYPE_IFO)
            .and_then(|r| Gff::read(&r.data).ok());
        self.git = module
            .resources
            .iter()
            .find(|r| r.kind == RES_TYPE_GIT)
            .and_then(|r| Gff::read(&r.data).ok().map(|g| (r.name.clone(), g)));
        self.module = Some(module);
    }

    /// Puts the current module's changed .ifo and .git back into SAVEGAME.sav (in memory).
    fn flush_module(&mut self) {
        if !(self.dirty.contains("ifo") || self.dirty.contains("git")) {
            return;
        }
        if let Some(module) = self.module.as_mut() {
            if let (true, Some(ifo)) = (self.dirty.contains("ifo"), self.ifo.as_ref()) {
                if let Some(r) = module.resources.iter_mut().find(|r| r.kind == RES_TYPE_IFO) {
                    r.data = ifo.write();
                }
            }
            if let (true, Some((name, git))) = (self.dirty.contains("git"), self.git.as_ref()) {
                if let Some(r) = module.find_mut(name, RES_TYPE_GIT) {
                    r.data = git.write();
                }
            }
            let bytes = module.write();
            if let Some(r) = self.archive.find_mut(&self.module_name, RES_TYPE_SAV) {
                r.data = bytes;
            }
        }
        self.dirty.remove("ifo");
        self.dirty.remove("git");
        self.archive_dirty = true;
    }

    /// The area objects (.git) of another module the save has been to, as it left them.
    pub fn module_git(&self, module: &str) -> Option<Gff> {
        if module.eq_ignore_ascii_case(&self.module_name) {
            return self.git.as_ref().map(|(_, g)| g.clone());
        }
        let r = self.archive.resources.iter().find(|r| r.kind == RES_TYPE_SAV && r.name.eq_ignore_ascii_case(module))?;
        let erf = Erf::read(&r.data).ok()?;
        let g = erf.resources.iter().find(|r| r.kind == RES_TYPE_GIT)?;
        Gff::read(&g.data).ok()
    }

    /// The module the save resumes in.
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// The modules this save has been to: the module saves in SAVEGAME.sav.
    pub fn visited_modules(&self) -> Vec<String> {
        self.archive.resources.iter().filter(|r| r.kind == RES_TYPE_SAV).map(|r| r.name.to_ascii_lowercase()).collect()
    }

    /// Moves the player (every copy) to `pos` in the current area, facing `facing`.
    pub fn move_player(&mut self, pos: [f32; 3], facing: Option<f32>) {
        if pos.iter().any(|v| !v.is_finite()) {
            return;
        }
        self.edit(Who::Player, "position", |c| {
            crate::area::set_position(c, pos);
            if let Some(a) = facing {
                c.set_float("XOrientation", a.cos());
                c.set_float("YOrientation", a.sin());
            }
        });
    }

    /// Moves the player to `pos` in another module this save has been to: the player goes
    /// into that module's player list, and the save resumes there (savenfo LASTMODULE and
    /// AREANAME). The game reads the player from the module it resumes in.
    pub fn move_to_module(&mut self, module: &str, pos: [f32; 3], area_name: &str) -> Result<()> {
        if pos.iter().any(|v| !v.is_finite()) {
            return Err("that spot is not a position in the area".into());
        }
        if module.eq_ignore_ascii_case(&self.module_name) {
            self.move_player(pos, None);
            return Ok(());
        }
        let player = self.player().cloned().ok_or("this save has no player")?;
        let target = self
            .archive
            .resources
            .iter()
            .find(|r| r.kind == RES_TYPE_SAV && r.name.eq_ignore_ascii_case(module))
            .map(|r| r.name.clone())
            .ok_or("this save has not been to that area")?;
        // A trip set up here would wait in this module's queue for the player's return.
        if let Some(q) = self.ifo.as_mut().and_then(|i| i.root.list_mut("EventQueue")) {
            let before = q.len();
            q.retain(|e| e.int("EventId") != Some(26));
            if q.len() != before {
                self.dirty.insert("ifo");
            }
        }
        self.flush_module();
        // The target module, with the player put in.
        let data = self.archive.find(&target, RES_TYPE_SAV).unwrap().data.clone();
        let mut erf = Erf::read(&data).map_err(|e| e.to_string())?;
        let ifo_res = erf.resources.iter_mut().find(|r| r.kind == RES_TYPE_IFO).ok_or("that area's save has no module file")?;
        let mut ifo = Gff::read(&ifo_res.data).map_err(|e| e.to_string())?;
        let mut moved = player;
        crate::area::set_position(&mut moved, pos);
        // What it was doing and who it saw are ids of the old area.
        for list in ["PerceptionList", "ActionList"] {
            if moved.list(list).is_some() {
                moved.set(list, Value::List(Vec::new()));
            }
        }
        let old_self = ifo.root.list("Mod_PlayerList").and_then(|l| l.first()).cloned();
        if let Some(o) = old_self.as_ref() {
            moved.id = o.id;
        }
        // Object ids are each module's own: the player takes the id that module gave it,
        // and every item it carries a new one from the module's counter, so nothing clashes
        // with that area's objects; effects naming an old id are pointed at the new one.
        let mut ids: Vec<(u32, u32)> = Vec::new();
        if let (Some(old), Some(new)) = (moved.int("ObjectId"), old_self.as_ref().and_then(|o| o.int("ObjectId"))) {
            ids.push((old as u32, new as u32));
            moved.set("ObjectId", Value::Dword(new as u32));
        }
        if let Some(area) = old_self.as_ref().and_then(|o| o.int("AreaId")) {
            moved.set_int("AreaId", area, Value::Dword(0));
        }
        let mut next = ifo.root.int("Mod_NextObjId0").unwrap_or(0x10000) as u32;
        for list in ["Equip_ItemList", "ItemList"] {
            if let Some(items) = moved.list_mut(list) {
                for it in items.iter_mut() {
                    if let Some(old) = it.int("ObjectId") {
                        ids.push((old as u32, next));
                        it.set("ObjectId", Value::Dword(next));
                        next += 1;
                    }
                }
            }
        }
        ifo.root.set_int("Mod_NextObjId0", next as i64, Value::Dword(0));
        if let Some(effects) = moved.list_mut("EffectList") {
            for e in effects.iter_mut() {
                if let Some(c) = e.int("CreatorId") {
                    if let Some((_, n)) = ids.iter().find(|(o, _)| *o == c as u32) {
                        e.set("CreatorId", Value::Dword(*n));
                    }
                }
            }
        }
        ifo.root.set("Mod_PlayerList", Value::List(vec![moved]));
        ifo_res.data = ifo.write();
        let bytes = erf.write();
        self.archive.find_mut(&target, RES_TYPE_SAV).unwrap().data = bytes;
        self.archive_dirty = true;
        let name_bytes = crate::text::encode(area_name);
        let module_bytes = target.as_bytes().to_vec();
        self.edit_nfo(|n| {
            n.set("LASTMODULE", Value::String(module_bytes));
            n.set("AREANAME", Value::String(name_bytes));
        });
        self.module_name = target;
        self.load_module();
        self.changed("player.module");
        Ok(())
    }

    pub fn changed(&mut self, key: impl Into<String>) {
        self.changes.insert(key.into());
    }

    fn loaded(&self, name: &str, kind: u16) -> Option<&Gff> {
        self.loaded.iter().find(|(n, k, _)| *k == kind && n.eq_ignore_ascii_case(name)).map(|(_, _, g)| g)
    }

    fn loaded_mut(&mut self, name: &str, kind: u16) -> Option<&mut Gff> {
        let found = self.loaded.iter_mut().find(|(n, k, _)| *k == kind && n.eq_ignore_ascii_case(name))?;
        self.dirty_archive.insert((found.0.clone(), kind));
        Some(&mut found.2)
    }

    // ---- creatures ----

    /// Why edits to this save could miss what the game reads, found by reading it: the area it
    /// is in would not read, so its player would be edited in `pc.utc` alone (KotOR II), while
    /// the game reads the player from that area.
    pub fn module_problem(&self) -> Option<&'static str> {
        let held = self.archive.find(&self.module_name, RES_TYPE_SAV).is_some();
        (held && self.ifo.is_none()).then_some("The area this save is in could not be read, so its player could not be changed where the game reads it. It can be looked at but not changed.")
    }

    pub fn player(&self) -> Option<&Struct> {
        if let Some(p) = self.ifo.as_ref().and_then(|i| i.root.list("Mod_PlayerList")).and_then(|l| l.first()) {
            return Some(p);
        }
        self.loaded("pc", RES_TYPE_UTC).map(|g| &g.root)
    }

    pub fn creature(&self, who: Who) -> Option<&Struct> {
        match who {
            Who::Player => self.player(),
            Who::Npc(i) => self.loaded(&format!("availnpc{}", i), RES_TYPE_UTC).map(|g| &g.root),
        }
    }

    /// Applies `f` to every copy of the creature the save holds.
    pub fn edit(&mut self, who: Who, key: &str, f: impl Fn(&mut Struct)) {
        match who {
            Who::Player => {
                if let Some(p) = self.ifo.as_mut().and_then(|i| i.root.list_mut("Mod_PlayerList")).and_then(|l| l.first_mut()) {
                    f(p);
                    self.dirty.insert("ifo");
                }
                if let Some(pc) = self.loaded_mut("pc", RES_TYPE_UTC) {
                    f(&mut pc.root);
                }
            }
            Who::Npc(i) => {
                let tag = self.creature(who).and_then(|c| c.string("Tag")).unwrap_or_default();
                // The member's own copy in the area has their tag and their classes; another
                // creature can share the tag (the Endar Spire's Carth, where Skip Ahead's
                // saves resume, beside the Carth recruited from his template) and is left
                // alone.
                let classes = self.creature(who).map(crate::creature::classes).unwrap_or_default();
                if let Some(npc) = self.loaded_mut(&format!("availnpc{}", i), RES_TYPE_UTC) {
                    f(&mut npc.root);
                }
                if !tag.is_empty() {
                    if let Some((_, git)) = self.git.as_mut() {
                        let mut touched = false;
                        if let Some(list) = git.root.list_mut("Creature List") {
                            let same = |c: &Struct| c.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&tag)) && crate::creature::classes(c) == classes;
                            for c in list.iter_mut().filter(|c| same(c)) {
                                f(c);
                                touched = true;
                            }
                        }
                        if touched {
                            self.dirty.insert("git");
                        }
                    }
                }
            }
        }
        self.changed(format!("{:?}.{}", who, key));
    }

    /// The party members whose creature is in the save, by NPC index.
    pub fn npcs(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .loaded
            .iter()
            .filter_map(|(n, k, _)| {
                if *k != RES_TYPE_UTC {
                    return None;
                }
                n.to_ascii_lowercase().strip_prefix("availnpc")?.parse().ok()
            })
            .collect();
        out.sort();
        out
    }

    /// Brings party member `npc` over from another save of the same game: their
    /// creature replaces (or becomes) AVAILNPCn here, and they are listed as recruited.
    pub fn import_npc(&mut self, other: &Save, npc: usize) -> Result<()> {
        if other.game != self.game {
            return Err("that save is from the other game".into());
        }
        let mut creature = other
            .loaded(&format!("availnpc{}", npc), RES_TYPE_UTC)
            .ok_or("that save does not have this party member")?
            .clone();
        // Object ids are each save's own: every item gets a new one, and effects follow.
        let ids: Vec<u32> = (0..crate::area::creature_items_mut(&mut creature.root).len()).map(|_| self.new_object_id()).collect();
        let mut moved: Vec<(i64, u32)> = Vec::new();
        for (item, new) in crate::area::creature_items_mut(&mut creature.root).into_iter().zip(ids) {
            if let Some(old) = item.int("ObjectId") {
                moved.push((old, new));
            }
            item.set("ObjectId", Value::Dword(new));
        }
        for e in creature.root.list_mut("EffectList").into_iter().flatten() {
            if let Some((_, new)) = e.int("CreatorId").and_then(|c| moved.iter().find(|(o, _)| *o == c)).copied() {
                e.set("CreatorId", Value::Dword(new));
            }
        }
        // The copy standing in the area, by the tag and classes `edit` finds it by, becomes
        // the same person where they stand.
        let (tag, classes) = (self.creature(Who::Npc(npc)).and_then(|c| c.string("Tag")).unwrap_or_default(), self.creature(Who::Npc(npc)).map(crate::creature::classes).unwrap_or_default());
        const STAY: [&str; 9] = ["ObjectId", "AreaId", "XPosition", "YPosition", "ZPosition", "XOrientation", "YOrientation", "ZOrientation", "PerceptionList"];
        let new_body = creature.root.clone();
        let standing = self.area_git().and_then(|g| g.root.list(crate::area::CREATURES)).and_then(|l| l.iter().position(|c| !tag.is_empty() && c.string("Tag").map_or(false, |t| t.eq_ignore_ascii_case(&tag)) && crate::creature::classes(c) == classes));
        if let Some(at) = standing {
            self.edit_area(&format!("npc{}.import", npc), |git| {
                if let Some(c) = git.list_mut(crate::area::CREATURES).and_then(|l| l.get_mut(at)) {
                    let stay: Vec<(String, Value)> = c.fields.iter().filter(|(l, _)| STAY.contains(&l.as_str())).cloned().collect();
                    c.fields = new_body.fields.iter().filter(|(l, _)| !STAY.contains(&l.as_str())).cloned().collect();
                    c.fields.extend(stay);
                    // what they were doing in the other save names objects of that save's
                    // areas: an empty action list, as a move to another area leaves (#43)
                    if c.get("ActionList").is_some() {
                        c.set("ActionList", Value::List(Vec::new()));
                    }
                }
            });
        }
        self.put_npc(npc, creature);
        self.changed(format!("npc{}.import", npc));
        Ok(())
    }

    /// Recruits party member `npc` from the game's template, as the game's own scripts do
    /// (AddAvailableNPCByTemplate): the template made into a creature as the party table
    /// keeps it, the member available and selectable. A member already recruited is kept.
    pub fn recruit_from_template(&mut self, npc: usize, template: &Struct) -> Result<()> {
        // The same companion already there stays as the save has them; anyone else in the slot
        // is replaced, as the game's AddAvailableNPCByTemplate does (KotOR 1's Endar Spire
        // leaves its officer in slot 0, where Taris puts slot 0's companion).
        let same = self.creature(Who::Npc(npc)).and_then(|c| c.string("Tag")).zip(template.string("Tag")).map_or(false, |(a, b)| a.eq_ignore_ascii_case(&b));
        if same && self.recruited(npc) {
            return Ok(());
        }
        let mut c = crate::area::creature_from_template(template, None);
        // The stored maxima as the game would write them; it reads only the bases and the
        // current values, which stay the template's.
        crate::creature::sync_maxima(&mut c, self.game);
        // The party table's copy is a file of its own: its root, not a list element. Like the
        // game's (AVAILNPCn in a real save) it keeps position fields and has no ObjectId; its
        // spawn script has not run, so the game runs it when the member first appears.
        c.id = u32::MAX;
        self.put_npc(npc, Gff { file_type: *b"UTC ", root: c });
        self.changed(format!("npc{}.recruit", npc));
        Ok(())
    }

    fn put_npc(&mut self, npc: usize, creature: Gff) {
        let name = format!("availnpc{}", npc);
        let display = self
            .archive
            .resources
            .iter()
            .find(|r| r.name.to_ascii_lowercase().starts_with("availnpc"))
            .map(|r| if r.name.chars().any(|c| c.is_uppercase()) { name.to_uppercase() } else { name.clone() })
            .unwrap_or_else(|| name.clone());
        if let Some(slot) = self.loaded.iter_mut().find(|(n, k, _)| *k == RES_TYPE_UTC && n.eq_ignore_ascii_case(&name)) {
            slot.2 = creature;
        } else if let Some(existing) = self.archive.resources.iter().find(|r| r.kind == RES_TYPE_UTC && r.name.eq_ignore_ascii_case(&name)).map(|r| r.name.clone()) {
            // in the archive but not read (it would not parse): written over where it is, so
            // the archive never holds the member twice
            self.loaded.push((existing, RES_TYPE_UTC, creature));
        } else {
            // Keep the archive's order: the members come first, by index.
            let at = self
                .archive
                .resources
                .iter()
                .position(|r| {
                    r.kind != RES_TYPE_UTC
                        || r.name.to_ascii_lowercase().strip_prefix("availnpc").and_then(|n| n.parse::<usize>().ok()).map_or(true, |n| n > npc)
                })
                .unwrap_or(self.archive.resources.len());
            self.archive.resources.insert(
                at,
                crate::formats::erf::Resource { name: display.clone(), kind: RES_TYPE_UTC, id: 0, data: Vec::new() },
            );
            self.loaded.push((display, RES_TYPE_UTC, creature));
        }
        self.dirty_archive.insert((name, RES_TYPE_UTC));
        self.set_recruited(npc, true);
        self.set_selectable(npc, true);
    }

    // ---- the party table ----

    pub fn party_int(&self, label: &str) -> Option<i64> {
        self.party.root.int(label)
    }

    pub fn set_party_int(&mut self, label: &str, value: i64) {
        if self.party.root.set_int(label, value, Value::Dword(0)) {
            self.dirty.insert("party");
            self.changed(format!("party.{}", label));
        }
    }

    fn npc_list_int(&self, list: &str, index: usize, field: &str) -> Option<i64> {
        self.party.root.list(list)?.get(index)?.int(field)
    }

    fn set_npc_list_int(&mut self, list: &str, index: usize, field: &str, value: i64) {
        if let Some(entry) = self.party.root.list_mut(list).and_then(|l| l.get_mut(index)) {
            entry.set_int(field, value, Value::Byte(0));
            self.dirty.insert("party");
            self.changed(format!("party.{}.{}.{}", list, index, field));
        }
    }

    /// Influence (KotOR 2): 0 to 100, or -1 for someone not met yet.
    pub fn influence(&self, npc: usize) -> Option<i64> {
        self.npc_list_int("PT_INFLUENCE", npc, "PT_NPC_INFLUENCE")
    }

    pub fn set_influence(&mut self, npc: usize, value: i64) {
        self.set_npc_list_int("PT_INFLUENCE", npc, "PT_NPC_INFLUENCE", value.clamp(0, 100));
    }

    pub fn npc_count(&self) -> usize {
        self.party.root.list("PT_AVAIL_NPCS").map_or(0, |l| l.len())
    }

    /// Recruited: the party screen lists them.
    pub fn recruited(&self, npc: usize) -> bool {
        self.npc_list_int("PT_AVAIL_NPCS", npc, "PT_NPC_AVAIL") == Some(1)
    }

    /// Only a member whose creature the save holds can be listed; the game loads it
    /// from AVAILNPCn when they join.
    pub fn set_recruited(&mut self, npc: usize, on: bool) -> bool {
        if on && !self.npcs().contains(&npc) {
            return false;
        }
        self.set_npc_list_int("PT_AVAIL_NPCS", npc, "PT_NPC_AVAIL", on as i64);
        if !on {
            self.set_npc_list_int("PT_AVAIL_NPCS", npc, "PT_NPC_SELECT", 1);
            let controlled = self.party_int("PT_CONTROLLED_NP") == Some(npc as i64);
            if self.in_party(npc) || controlled {
                self.edit_party_table("members", |p| {
                    let count = if let Some(members) = p.list_mut("PT_MEMBERS") {
                        members.retain(|m| m.int("PT_MEMBER_ID") != Some(npc as i64));
                        if controlled {
                            for member in members.iter_mut() {
                                member.set_int("PT_IS_LEADER", 0, Value::Byte(0));
                            }
                        }
                        members.len()
                    } else { 0 };
                    p.set_int("PT_NUM_MEMBERS", count as i64, Value::Byte(0));
                    if controlled { p.set_int("PT_CONTROLLED_NP", -1, Value::Int(0)); }
                });
            }
        }
        true
    }

    /// Selectable: not locked out of the party screen by the story.
    pub fn selectable(&self, npc: usize) -> bool {
        self.npc_list_int("PT_AVAIL_NPCS", npc, "PT_NPC_SELECT") != Some(0)
    }

    pub fn set_selectable(&mut self, npc: usize, on: bool) {
        self.set_npc_list_int("PT_AVAIL_NPCS", npc, "PT_NPC_SELECT", on as i64);
    }

    pub fn in_party(&self, npc: usize) -> bool {
        self.party
            .root
            .list("PT_MEMBERS")
            .map_or(false, |l| l.iter().any(|m| m.int("PT_MEMBER_ID") == Some(npc as i64)))
    }

    // ---- inventory ----

    pub fn inventory(&self) -> Vec<Struct> {
        self.loaded("inventory", RES_TYPE_RES)
            .and_then(|g| g.root.list("ItemList"))
            .cloned()
            .unwrap_or_default()
    }

    pub fn edit_inventory(&mut self, key: &str, f: impl FnOnce(&mut Vec<Struct>)) {
        if let Some(inv) = self.loaded_mut("inventory", RES_TYPE_RES) {
            if inv.root.list("ItemList").is_none() {
                inv.root.set("ItemList", Value::List(Vec::new()));
            }
            f(inv.root.list_mut("ItemList").unwrap());
            self.changed(format!("inventory.{}", key));
        }
    }

    // ---- equipment ----

    /// A new object id from the module's counter (Mod_NextObjId0), which then moves on;
    /// the area's highest id + 1 when the module has no counter.
    pub fn new_object_id(&mut self) -> u32 {
        if let Some(ifo) = self.ifo.as_mut() {
            if let Some(next) = ifo.root.int("Mod_NextObjId0") {
                ifo.root.set_int("Mod_NextObjId0", next + 1, Value::Dword(0));
                self.dirty.insert("ifo");
                return next as u32;
            }
        }
        // no counter: the next above every id in the area, and above every id handed out
        // before (nothing is in the area yet when several are asked for at once)
        let next = self.git.as_ref().map(|(_, g)| crate::area::next_object_id(&g.root)).unwrap_or(0x10000).max(self.handed_out);
        self.handed_out = next + 1;
        next
    }

    /// Whether `slot` holds a template entry (only `EquippedRes`, as a creature made from its
    /// template wears items) and not a whole item, so nothing can go to the inventory yet.
    pub fn slot_is_template(&self, who: Who, slot: u32) -> Option<String> {
        let item = self.creature(who).and_then(|c| crate::creature::in_slot(c, slot))?;
        if item.get("BaseItem").is_some() {
            return None;
        }
        item.string("EquippedRes")
    }

    /// The template entry in `slot` replaced by the whole item made from its template, on
    /// every copy of the creature.
    pub fn fill_template_slot(&mut self, who: Who, slot: u32, item: Struct) {
        let id = self.new_object_id();
        self.edit(who, &format!("equip{}", slot), |c| crate::creature::equip(c, slot, item.clone(), id));
    }

    /// Equips one of inventory item `index` in `slot`, on every copy of the creature; what
    /// was in the slot goes back to the party inventory. False, with nothing changed, over a
    /// template entry: see `fill_template_slot`.
    pub fn equip(&mut self, who: Who, slot: u32, index: usize) -> bool {
        let inventory = self.inventory();
        let Some(mut item) = inventory.get(index).cloned() else { return false };
        if self.slot_is_template(who, slot).is_some() {
            return false;
        }
        let stack = item.int("StackSize").unwrap_or(1);
        item.set_int("StackSize", 1, Value::Word(1));
        let old = self.creature(who).and_then(|c| crate::creature::in_slot(c, slot)).cloned();
        let id = self.new_object_id();
        self.edit(who, &format!("equip{}", slot), |c| {
            crate::creature::unequip(c, slot);
            crate::creature::equip(c, slot, item.clone(), id);
        });
        self.edit_inventory(&format!("equip{}", slot), |items| {
            if stack > 1 {
                items[index].set_int("StackSize", stack - 1, Value::Word(1));
            } else {
                items.remove(index);
            }
            if let Some(mut o) = old {
                o.id = 0;
                o.fields.retain(|(l, _)| l != "ObjectId");
                items.push(o);
            }
        });
        true
    }

    /// Takes the item out of `slot` on every copy and puts it in the party inventory. False,
    /// with nothing changed, for a template entry: see `fill_template_slot`.
    pub fn unequip(&mut self, who: Who, slot: u32) -> bool {
        if self.slot_is_template(who, slot).is_some() {
            return false;
        }
        let Some(mut item) = self.creature(who).and_then(|c| crate::creature::in_slot(c, slot)).cloned() else { return false };
        self.edit(who, &format!("equip{}", slot), |c| {
            crate::creature::unequip(c, slot);
        });
        item.id = 0;
        item.fields.retain(|(l, _)| l != "ObjectId");
        self.edit_inventory(&format!("unequip{}", slot), |items| items.push(item));
        true
    }

    /// The party inventory replaced by another save's of the same game.
    pub fn copy_inventory(&mut self, other: &Save) -> Result<()> {
        if other.game != self.game {
            return Err("that save is from the other game".into());
        }
        let items = other.inventory();
        self.edit_inventory("copied", |list| *list = items);
        Ok(())
    }

    // ---- the active party ----

    pub fn party_members(&self) -> Vec<usize> {
        self.party
            .root
            .list("PT_MEMBERS")
            .map(|l| l.iter().filter_map(|m| m.int("PT_MEMBER_ID")).map(|i| i as usize).collect())
            .unwrap_or_default()
    }

    /// Who travels with the player: two at most, each recruited and held in the save. The
    /// area is left alone: on the Ebon Hawk the game stands every member around the ship
    /// itself (a member taken out of the Hawk's area went missing in a test, 2026-10-22).
    pub fn set_party_members(&mut self, members: &[usize]) -> Result<()> {
        if members.len() > 2 {
            return Err("two party members at most".into());
        }
        for &m in members {
            if self.creature(Who::Npc(m)).is_none() {
                return Err("that member is not in this save".into());
            }
        }
        let shape = self.party.root.list("PT_MEMBERS").and_then(|l| l.first().cloned()).unwrap_or_else(|| {
            let mut s = Struct::new(0);
            s.set("PT_MEMBER_ID", Value::Int(0));
            s.set("PT_IS_LEADER", Value::Byte(0));
            s
        });
        let before = self.party_members();
        let list: Vec<Struct> = members
            .iter()
            .map(|&m| {
                let mut e = shape.clone();
                e.set_int("PT_MEMBER_ID", m as i64, Value::Int(0));
                e.set_int("PT_IS_LEADER", 0, Value::Byte(0));
                e
            })
            .collect();
        let controlled = self.party.root.int("PT_CONTROLLED_NP").unwrap_or(-1);
        self.edit_party_table("members", |p| {
            p.set("PT_MEMBERS", Value::List(list));
            p.set_int("PT_NUM_MEMBERS", members.len() as i64, Value::Byte(0));
            if controlled >= 0 && !members.contains(&(controlled as usize)) {
                p.set_int("PT_CONTROLLED_NP", -1, Value::Int(0));
            }
        });
        for &m in members.iter().filter(|m| !before.contains(m)) {
            self.set_recruited(m, true);
        }
        Ok(())
    }

    // ---- quests ----

    /// The journal entries as (quest tag, stage).
    pub fn quests(&self) -> Vec<(String, i64)> {
        self.party
            .root
            .list("JNL_Entries")
            .map(|l| l.iter().map(|e| (e.string("JNL_PlotID").unwrap_or_default(), e.int("JNL_State").unwrap_or(0))).collect())
            .unwrap_or_default()
    }

    /// Puts quest `tag` at `stage`, adding it to the journal when it is not there yet;
    /// a new entry takes the date and time of the newest entry already written.
    pub fn set_quest(&mut self, tag: &str, stage: i64) {
        let tag = tag.to_string();
        self.edit_party_table(&format!("quest.{}", tag), |p| {
            if p.list("JNL_Entries").is_none() {
                p.set("JNL_Entries", Value::List(Vec::new()));
            }
            let list = p.list_mut("JNL_Entries").unwrap();
            if let Some(e) = list.iter_mut().find(|e| e.string("JNL_PlotID").map_or(false, |t| t.eq_ignore_ascii_case(&tag))) {
                e.set_int("JNL_State", stage, Value::Int(0));
                return;
            }
            let mut e = list.last().cloned().unwrap_or_else(|| {
                let mut s = Struct::new(0);
                s.set("JNL_PlotID", Value::String(Vec::new()));
                s.set("JNL_State", Value::Int(0));
                s.set("JNL_Date", Value::Dword(0));
                s.set("JNL_Time", Value::Dword(0));
                s
            });
            e.set("JNL_PlotID", Value::String(crate::text::encode(&tag)));
            e.set_int("JNL_State", stage, Value::Int(0));
            list.push(e);
        });
    }

    pub fn remove_quest(&mut self, tag: &str) {
        let tag = tag.to_string();
        self.edit_party_table(&format!("quest.{}", tag), |p| {
            if let Some(list) = p.list_mut("JNL_Entries") {
                list.retain(|e| !e.string("JNL_PlotID").map_or(false, |t| t.eq_ignore_ascii_case(&tag)));
            }
        });
    }

    // ---- globals ----

    pub fn edit_globals(&mut self, key: &str, f: impl FnOnce(&mut Struct)) {
        if let Some(g) = self.globals.as_mut() {
            f(&mut g.root);
            self.dirty.insert("globals");
            self.changed(format!("globals.{}", key));
        }
    }

    pub fn area_git(&self) -> Option<&Gff> {
        self.git.as_ref().map(|(_, g)| g)
    }

    pub fn edit_area(&mut self, key: &str, f: impl FnOnce(&mut Struct)) {
        if let Some((_, g)) = self.git.as_mut() {
            f(&mut g.root);
            self.dirty.insert("git");
            self.changed(format!("area.{}", key));
        }
    }

    /// Adds a creature to the current area with ids nothing else uses: its own from the
    /// module's counter, and one for every full item it carries. Returns its id.
    pub fn spawn_creature(&mut self, mut c: Struct) -> u32 {
        let id = self.new_object_id();
        c.set("ObjectId", Value::Dword(id));
        // Copied from a creature of the area when it has one; the player stands there too.
        if c.get("AreaId").is_none() {
            if let Some(area) = self.player().and_then(|p| p.int("AreaId")) {
                c.set_int("AreaId", area, Value::Dword(0));
            }
        }
        let items = crate::area::creature_items_mut(&mut c).len();
        let ids: Vec<u32> = (0..items).map(|_| self.new_object_id()).collect();
        for (item, new) in crate::area::creature_items_mut(&mut c).into_iter().zip(ids) {
            item.set("ObjectId", Value::Dword(new));
        }
        self.edit_area(&format!("spawn{}", id), move |git| crate::area::add_creature(git, c));
        id
    }

    /// Sends the player to `module` the way a door between areas does: an area-transition
    /// event (EVENT_AREA_TRANSITION, 26) queued in the current module, which the game runs
    /// right after the save loads. The game then makes the trip itself: it stores this area,
    /// loads the target fresh from its own files when the save has not been there (as on a
    /// first arrival, with the area's arrival scripts), and puts the player and the party at
    /// `waypoint`. The event's layout is the game's (struct 0xABCD holding a script event,
    /// 0xEEEE, whose eight strings are the module, the waypoint and six movies); from the
    /// maintainer's own reverse engineering of KotOR 1 (area event handler 0x0050D6C0).
    /// `movies` (up to six) play before the area loads, as a script's StartNewModule plays
    /// them (the escape pod's 01c before the vision, for one).
    pub fn queue_transition(&mut self, module: &str, waypoint: &str, movies: &[&str]) -> Result<()> {
        let ifo = self.ifo.as_mut().ok_or("this save has no module file")?;
        let day = ifo.root.int("Mod_PauseDay").unwrap_or(0);
        let time = ifo.root.int("Mod_PauseTime").unwrap_or(0);
        let area = ifo.root.list("Mod_Area_list").and_then(|l| l.first()).and_then(|a| a.int("ObjectId")).unwrap_or(1);
        let text = |t: &str| {
            let mut p = Struct::new(0x69);
            p.set("Parameter", Value::String(t.as_bytes().to_vec()));
            p
        };
        let mut data = Struct::new(0xEEEE);
        data.set("EventType", Value::Word(0));
        data.set("IntList", Value::List(Vec::new()));
        data.set("FloatList", Value::List(Vec::new()));
        let mut strings = vec![text(module), text(waypoint)];
        strings.extend((0..6).map(|i| text(movies.get(i).copied().unwrap_or(""))));
        data.set("StringList", Value::List(strings));
        data.set("ObjectList", Value::List(Vec::new()));
        let mut event = Struct::new(0xABCD);
        event.set("Day", Value::Dword(day as u32));
        // Due at the moment the save was made, so it runs as soon as the game does (a second
        // later waited, on the Endar Spire, until the player first did something).
        event.set("Time", Value::Dword(time as u32));
        event.set("ObjectId", Value::Dword(area as u32));
        event.set("CallerId", Value::Dword(area as u32));
        event.set("EventId", Value::Dword(26));
        event.set("EventData", Value::Struct(data));
        if ifo.root.list("EventQueue").is_none() {
            ifo.root.set("EventQueue", Value::List(Vec::new()));
        }
        // One trip at a time: the new one replaces any waiting in the queue.
        ifo.root.list_mut("EventQueue").unwrap().retain(|e| e.int("EventId") != Some(26));
        // First in the queue: the game looks at its queue in order, and behind the save's own
        // delayed events (Trask's opening, on the Endar Spire) the trip waited until those
        // ran or the player did something (the maintainer's test, 2026-10-23).
        ifo.root.list_mut("EventQueue").unwrap().insert(0, event);
        self.dirty.insert("ifo");
        self.changed(format!("travel.{}", module.to_ascii_lowercase()));
        Ok(())
    }

    /// The area a trip waiting in the current module's queue leads to (Skip Ahead, Go Back
    /// and Travel Here queue one; the game makes it as the save loads), by module name.
    pub fn queued_trip(&self) -> Option<String> {
        let q = self.ifo.as_ref()?.root.list("EventQueue")?;
        let e = q.iter().find(|e| e.int("EventId") == Some(26))?;
        let first = e.child("EventData")?.list("StringList")?.first()?;
        let name = first.string("Parameter")?;
        (!name.is_empty()).then(|| name.to_ascii_lowercase())
    }

    /// Where the save will be once the game runs it: a queued trip's area, else the one it is in.
    pub fn place(&self) -> String {
        self.queued_trip().unwrap_or_else(|| self.module_name.to_ascii_lowercase())
    }

    /// Whether SAVEGAME.sav holds a stored copy of `module` (the player has been there).
    pub fn has_module(&self, module: &str) -> bool {
        self.archive.resources.iter().any(|r| r.kind == RES_TYPE_SAV && r.name.eq_ignore_ascii_case(module))
    }

    /// The current module's module.ifo and nested archive: the live counters and the
    /// archive layout a new stored copy takes after (stored::played_through).
    pub fn current_module_parts(&self) -> Option<(Struct, Erf)> {
        Some((self.ifo.as_ref()?.root.clone(), self.module.as_ref()?.clone()))
    }

    /// Adds a stored copy of an area to SAVEGAME.sav (lower case, as the game names them).
    /// Never over a copy that is there: the player has been to that area.
    pub fn add_stored_module(&mut self, module: &str, data: Vec<u8>) -> bool {
        if self.has_module(module) {
            return false;
        }
        self.archive.resources.push(crate::formats::erf::Resource { name: module.to_ascii_lowercase(), kind: RES_TYPE_SAV, id: 0, data });
        self.archive_dirty = true;
        self.changed(format!("stored.{}", module.to_ascii_lowercase()));
        true
    }

    /// Forgets an area the save has been to (its stored copy), so the game builds it fresh from
    /// its own files the next time the player arrives. Never the area the save is in.
    pub fn remove_stored_module(&mut self, module: &str) -> bool {
        if module.eq_ignore_ascii_case(&self.module_name) || !self.has_module(module) {
            return false;
        }
        self.archive.resources.retain(|r| !(r.kind == RES_TYPE_SAV && r.name.eq_ignore_ascii_case(module)));
        self.archive_dirty = true;
        self.changed(format!("stored.{}.removed", module.to_ascii_lowercase()));
        true
    }

    /// Changes the saved objects (.git) of `module`: the current area, or another area this
    /// save has been to. Returns false when the save has not been there (the game then
    /// builds that area fresh from its own files when the player arrives).
    pub fn edit_module_git(&mut self, module: &str, key: &str, f: impl FnOnce(&mut Struct)) -> bool {
        if module.eq_ignore_ascii_case(&self.module_name) {
            if self.git.is_none() {
                return false;
            }
            self.edit_area(key, f);
            return true;
        }
        let Some(name) = self
            .archive
            .resources
            .iter()
            .find(|r| r.kind == RES_TYPE_SAV && r.name.eq_ignore_ascii_case(module))
            .map(|r| r.name.clone())
        else {
            return false;
        };
        let data = self.archive.find(&name, RES_TYPE_SAV).unwrap().data.clone();
        let Ok(mut erf) = Erf::read(&data) else { return false };
        let Some(res) = erf.resources.iter_mut().find(|r| r.kind == RES_TYPE_GIT) else { return false };
        let Ok(mut git) = Gff::read(&res.data) else { return false };
        f(&mut git.root);
        res.data = git.write();
        let bytes = erf.write();
        self.archive.find_mut(&name, RES_TYPE_SAV).unwrap().data = bytes;
        self.archive_dirty = true;
        self.changed(format!("{}.{}", module.to_ascii_lowercase(), key));
        true
    }

    pub fn edit_nfo(&mut self, f: impl FnOnce(&mut Struct)) {
        f(&mut self.nfo.root);
        self.dirty.insert("nfo");
    }

    /// The name the game's Load screen shows.
    pub fn rename(&mut self, name: &str) {
        let bytes = crate::text::encode(name);
        self.edit_nfo(|n| n.set("SAVEGAMENAME", Value::String(bytes)));
        self.changed("nfo.name");
    }

    pub fn edit_party_table(&mut self, key: &str, f: impl FnOnce(&mut Struct)) {
        f(&mut self.party.root);
        self.dirty.insert("party");
        self.changed(format!("party.{}", key));
    }

    // ---- writing ----

    /// Backs the save up as it is on disk, into `backups`, when it is opened (2026-11-07, the
    /// maintainer: the backup is made on opening, and saving checks it). The copy is read back
    /// and compared with the save. Where the newest backup of this save already holds exactly
    /// these files, that one is kept instead of a second copy. Returns the backup folder.
    pub fn back_up(&mut self, backups: &Path) -> Result<PathBuf> {
        let newest = backup_list(backups, &self.folder).pop();
        let backup = match newest {
            Some(b) if same_files(&self.folder, &b) => b,
            _ => make_backup(backups, self.game, &self.folder)?,
        };
        self.backup = Some(backup.clone());
        Ok(backup)
    }

    /// Writes every changed file, never without a backup of exactly the files it replaces:
    /// first a read-only health check of the backup made on opening (every file there, byte
    /// for byte the save on disk), and where it fails (no backup, a damaged one, or the save
    /// already written since) a new backup, made and checked the same way, before anything is
    /// written. Stops if that copy fails. Returns the backup folder.
    pub fn write(&mut self, backups: &Path) -> Result<PathBuf> {
        if player_outside_archive(&self.folder) {
            return Err(PLAYER_OUTSIDE_ARCHIVE.into());
        }
        if files_on_disk(&self.folder) != self.on_disk {
            return Err(CHANGED_ON_DISK.into());
        }
        let backup = match self.backup.clone() {
            Some(b) if same_files(&self.folder, &b) => b,
            _ => make_backup(backups, self.game, &self.folder)?,
        };
        self.backup = Some(backup.clone());
        self.write_files()?;
        Ok(backup)
    }

    /// Writes the save, its changes and all, as a new save in `target` (a folder beside it
    /// that does not exist yet) named `name` on the game's Load screen, and leaves the save it
    /// was read from exactly as it was. From then on this is the new save; if it fails, this
    /// stays the save it was and the new folder is gone.
    pub fn write_as(&mut self, target: &Path, name: &str) -> Result<()> {
        if player_outside_archive(&self.folder) {
            return Err(PLAYER_OUTSIDE_ARCHIVE.into());
        }
        if files_on_disk(&self.folder) != self.on_disk {
            return Err(CHANGED_ON_DISK.into());
        }
        if target.exists() {
            return Err(format!("{} already exists", target.display()));
        }
        if let Err(e) = copy_dir(&self.folder, target) {
            let _ = fs::remove_dir_all(target);
            return Err(format!("the save could not be copied: {}", e));
        }
        let before = (self.folder.clone(), self.nfo_path.clone(), self.party_path.clone(), self.archive_path.clone(), self.globals_path.clone());
        let (nfo, changes) = (self.nfo.clone(), self.changes.clone());
        let moved = |p: &Path| target.join(p.file_name().unwrap_or_default());
        self.nfo_path = moved(&self.nfo_path);
        self.party_path = moved(&self.party_path);
        self.archive_path = moved(&self.archive_path);
        self.globals_path = self.globals_path.as_deref().map(moved);
        self.folder = target.to_path_buf();
        self.rename(name);
        let result = self.write_files();
        if result.is_err() {
            (self.folder, self.nfo_path, self.party_path, self.archive_path, self.globals_path) = before;
            self.nfo = nfo;
            self.changes = changes;
            let _ = fs::remove_dir_all(target);
        }
        result
    }

    /// Copies the save folder `folder` as it is on disk into `target` (a folder beside it that
    /// does not exist yet) and names the copy `name` on the game's Load screen; nothing else
    /// in it changes.
    pub fn copy_save(folder: &Path, target: &Path, name: &str) -> Result<()> {
        if target.exists() {
            return Err(format!("{} already exists", target.display()));
        }
        let result = copy_dir(folder, target).map_err(|e| format!("the save could not be copied: {}", e)).and_then(|()| {
            let mut copy = Save::open(target)?;
            copy.rename(name);
            copy.write_files()
        });
        if result.is_err() {
            let _ = fs::remove_dir_all(target);
        }
        result
    }

    /// Writes every changed file into the save's folder, each new file in full before any old
    /// one is replaced. If a file cannot be replaced, the ones already replaced go back, so a
    /// failed write leaves the save as it was.
    fn write_files(&mut self) -> Result<()> {
        self.flush_module();
        let module_dirty = self.archive_dirty;
        for (name, kind) in self.dirty_archive.clone() {
            if let Some(g) = self.loaded(&name, kind) {
                let bytes = g.write();
                if let Some(r) = self.archive.find_mut(&name, kind) {
                    r.data = bytes;
                }
            }
        }
        let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        if module_dirty || !self.dirty_archive.is_empty() {
            files.push((self.archive_path.clone(), self.archive.write()));
        }
        // Keep the load screen's copies in step with the party table.
        if self.dirty.contains("party") {
            let seconds = self.party.root.int("PT_PLAYEDSECONDS");
            let cheat = self.party.root.int("PT_CHEAT_USED");
            let name = self.party.root.string("PT_PCNAME");
            let nfo = &mut self.nfo.root;
            if let Some(s) = seconds {
                nfo.set_int("TIMEPLAYED", s, Value::Dword(0));
            }
            if let Some(c) = cheat {
                nfo.set_int("CHEATUSED", c, Value::Byte(0));
            }
            if let (Some(n), true) = (name, nfo.get("PCNAME").is_some()) {
                nfo.set("PCNAME", Value::String(crate::text::encode(&n)));
            }
            self.dirty.insert("nfo");
        }
        if self.dirty.contains("party") {
            files.push((self.party_path.clone(), self.party.write()));
        }
        if self.dirty.contains("nfo") {
            files.push((self.nfo_path.clone(), self.nfo.write()));
        }
        if let (true, Some(g), Some(p)) = (self.dirty.contains("globals"), self.globals.as_ref(), self.globals_path.as_ref()) {
            files.push((p.clone(), g.write()));
        }
        // Every new file is written in full before any old one is replaced.
        let mut staged = Vec::new();
        for (path, bytes) in &files {
            let tmp = path.with_extension("kmrp-new");
            // flushed to the disk before any old file is replaced: after a power failure the
            // renames alone could survive with the new file's data lost
            let written = fs::File::create(&tmp).and_then(|mut f| {
                use std::io::Write;
                f.write_all(bytes)?;
                f.sync_all()
            });
            if let Err(e) = written {
                let _ = fs::remove_file(&tmp);
                staged.iter().for_each(|(t, _): &(PathBuf, PathBuf)| drop(fs::remove_file(t)));
                return Err(format!("{}: {}", path.display(), e));
            }
            staged.push((tmp, path.clone()));
        }
        replace_all(&staged)?;
        self.dirty.clear();
        self.dirty_archive.clear();
        self.archive_dirty = false;
        self.changes.clear();
        self.on_disk = files_on_disk(&self.folder);
        Ok(())
    }
}

/// Puts each new file in place of its old one. Each old file waits under a second name until
/// all are done, so a failure part way can put every old file back. A file whose old version
/// cannot be put back is named in the error, with HALF_WRITTEN, never "nothing was changed".
fn replace_all(staged: &[(PathBuf, PathBuf)]) -> Result<()> {
    let old_of = |path: &PathBuf| path.with_extension("kmrp-old");
    let mut failed = None;
    let mut stuck: Vec<String> = Vec::new();
    let mut done = 0;
    for (tmp, path) in staged {
        if let Err(e) = fs::rename(path, old_of(path)) {
            failed = Some(format!("{}: {}", path.display(), e));
            break;
        }
        if let Err(e) = fs::rename(tmp, path) {
            // this one's old file goes straight back
            if let Err(back) = fs::rename(old_of(path), path) {
                stuck.push(format!("{}: {}", path.display(), back));
            }
            failed = Some(format!("{}: {}", path.display(), e));
            break;
        }
        done += 1;
    }
    let Some(error) = failed else {
        for (_, path) in staged {
            let _ = fs::remove_file(old_of(path));
        }
        return Ok(());
    };
    // the old files of those already swapped go back over the new ones (a rename replaces)
    for (_, path) in staged[..done].iter().rev() {
        if let Err(e) = fs::rename(old_of(path), path) {
            stuck.push(format!("{}: {}", path.display(), e));
        }
    }
    for (tmp, _) in staged {
        let _ = fs::remove_file(tmp);
    }
    if stuck.is_empty() {
        Err(error)
    } else {
        Err(format!("{}. Restore it from the backup. {}; {}", HALF_WRITTEN, error, stuck.join("; ")))
    }
}

/// Whether the save has an external player-state file the editor does not write.
/// KotOR 1's measured arrival autosave has no archived player; Save::open falls back
/// to pifo.ifo. KotOR 2's measured autosave has both pifo.ifo and archived copies.
/// Both stay read only until writing all player-state files is supported and checked.
pub fn player_outside_archive(folder: &Path) -> bool {
    file_ci(folder, "pifo.ifo").is_some()
}

/// Why an external player-state file makes a save read only.
pub const PLAYER_OUTSIDE_ARCHIVE: &str = "This save has player state in pifo.ifo, which the editor does not write. It can be looked at but not changed. Save again in a normal slot in the game, then open that save.";

/// The folder for a new save beside `folder`: the next number after the highest in its saves
/// folder, in the game's own pattern ("000135 - Game134"). The game reads only the number, but
/// when it overwrites a save it writes to "<number> - Game<number - 1>", so a folder named any
/// other way is left in place and the overwrite becomes a second save of the same name. From
/// 2026-10-24 to 2026-10-29 the folder carried the save's name instead ("000913 - Game26 -
/// Copy"), which is how "000920 - KMRPPazaak" came to have a twin "000920 - Game919".
pub fn next_save_folder(folder: &Path) -> Option<PathBuf> {
    let parent = folder.parent()?;
    let highest = fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().split(" - ").next().and_then(|n| n.trim().parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    let n = highest.checked_add(1)?;
    let target = parent.join(format!("{:06} - Game{}", n, n - 1));
    (!target.exists()).then_some(target)
}

/// The name of a save's folder of backups, inside the backups folder: the save's folder name.
/// KotOR II's local and Steam Cloud saves share their folder names ("000001 - QuickSave") and
/// the backups folder, so a cloud save's is told apart by name, not by time alone.
pub fn backup_name(folder: &Path) -> String {
    let mut name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "save".into());
    if folder.ancestors().any(|a| a.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("cloudsaves"))) {
        name.push_str(" (Steam Cloud)");
    }
    name
}

fn backup_folder(backups: &Path, _game: Game, folder: &Path) -> PathBuf {
    let name = backup_name(folder);
    let stamp = timestamp();
    let mut path = backups.join(&name).join(&stamp);
    let mut n = 2;
    while path.exists() {
        path = backups.join(&name).join(format!("{} ({})", stamp, n));
        n += 1;
    }
    path
}

/// Where a save's backups go: "Save Editor Backup" in the game's folder, beside the saves,
/// since 2026-11-02. Steam Cloud syncs KotOR 1's `Saves` with everything in it and KotOR 2's
/// `cloudsaves`, so a backup inside either used up the quota and moved between PCs. A save
/// outside the game (a port, VirtualStore, one added by hand) keeps its backups beside it.
pub fn backup_root(folder: &Path, game_root: Option<&Path>) -> PathBuf {
    let plain = |p: &Path| {
        let p = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        p.to_string_lossy().trim_start_matches("\\\\?\\").to_ascii_lowercase()
    };
    if let Some(root) = game_root {
        if folder.ancestors().any(|a| plain(a) == plain(root)) {
            return root.join("Save Editor Backup");
        }
    }
    // Not inside the game found (none found, or a folder added by hand): never inside a
    // `saves` or `cloudsaves` folder, which Steam Cloud syncs, but beside it.
    let parent = folder.parent().unwrap_or(folder);
    let synced = |p: &Path| p.file_name().map_or(false, |n| {
        let n = n.to_string_lossy().to_ascii_lowercase();
        n == "saves" || n == "cloudsaves"
    });
    let mut at = parent;
    // KotOR II's cloud saves sit one level deeper: cloudsaves/<account>/<save>
    for a in parent.ancestors().take(3) {
        if synced(a) {
            at = a.parent().unwrap_or(a);
            break;
        }
    }
    at.join("Save Editor Backup")
}

/// "2026-10-22 14-05-09", in UTC (no time zone database needed).
pub fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", y, m, d, t / 3600, t % 3600 / 60, t % 60)
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
pub fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// What a restore did: the backup of the save made just before it was replaced.
pub struct Restored {
    pub before: PathBuf,
}

/// Puts the save folder `folder` back to its backup `backup` (2026-11-07, the maintainer:
/// History's Revert to This Backup, "a safe feature ... impossible to break").
///
/// Refuses, with nothing changed, unless: no write into the save was cut off; `backup` is one
/// of this save's own backups in `backups`, not the save itself; the backup opens as a save of
/// the same game; neither folder has subfolders or a write's left-over files; no two of the
/// files share a stem (the staged names are `<stem>.kmrp-new`); the save is not already the
/// same as the backup. Then:
/// 1. the save as it is now is backed up and that copy checked, so the restore can be undone;
/// 2. every file of the backup is written beside the save as `<stem>.kmrp-new`, flushed to
///    the disk and read back the same, before anything is replaced (a failure removes them
///    and leaves the save as it was);
/// 3. each save file is swapped for its new one, the old waiting as `<stem>.kmrp-old`; a file
///    the backup does not have goes the same way; a failure part way puts every old file back;
/// 4. the old files are removed and the save folder is compared with the backup, byte for
///    byte, and opened as a save.
/// A file that cannot be put back is named in the error, with HALF_WRITTEN and where the
/// backup made in step 1 is.
pub fn restore(folder: &Path, backup: &Path, backups: &Path, game: Game) -> Result<Restored> {
    if let Some(left) = interrupted_write(folder) {
        return Err(format!("{} {}", INTERRUPTED, left));
    }
    let canon = |p: &Path| fs::canonicalize(p).map_err(|e| format!("{}: {}", p.display(), e));
    let (cf, cb) = (canon(folder)?, canon(backup)?);
    let own = canon(&backups.join(backup_name(folder)))?;
    if cb.parent() != Some(own.as_path()) || cb == cf {
        return Err("that folder is not one of this save's backups".into());
    }
    let listing = |p: &Path| -> Result<Vec<(String, PathBuf)>> {
        let mut v = Vec::new();
        for e in fs::read_dir(p).map_err(|e| format!("{}: {}", p.display(), e))? {
            let e = e.map_err(|e| e.to_string())?;
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                return Err(format!("{} has a folder inside it ({}), which a save does not", p.display(), name));
            }
            if name.ends_with(".kmrp-new") || name.ends_with(".kmrp-old") {
                return Err(format!("{} holds {}, left from a write", p.display(), name));
            }
            v.push((name, e.path()));
        }
        v.sort();
        Ok(v)
    };
    let now = listing(folder)?;
    let then = listing(backup)?;
    let opened = Save::open(backup).map_err(|e| format!("the backup does not open as a save: {}", e))?;
    if opened.game != game {
        return Err("the backup is a save of the other game".into());
    }
    drop(opened);
    let stem = |n: &str| Path::new(n).file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let mut stems: Vec<String> = now.iter().chain(then.iter()).map(|(n, _)| n.to_ascii_lowercase()).collect();
    stems.sort();
    stems.dedup();
    let mut seen = BTreeSet::new();
    for n in &stems {
        if !seen.insert(stem(n)) {
            return Err(format!("two files share the name {} before their extension", stem(n)));
        }
    }
    if same_files(folder, backup) {
        return Err("the save is already the same as this backup".into());
    }

    // 1. the save as it is now, kept and checked
    let before = make_backup(backups, game, folder)?;
    let undo = |e: String| format!("{} (the save as it was is kept in {})", e, before.display());

    // 2. every file of the backup staged beside the save, flushed and read back
    let mut staged: Vec<PathBuf> = Vec::new();
    let unstage = |staged: &[PathBuf]| staged.iter().for_each(|t| drop(fs::remove_file(t)));
    let mut wanted: Vec<(String, PathBuf, PathBuf)> = Vec::new(); // name, its place, staged copy
    for (name, src) in &then {
        let bytes = match fs::read(src) {
            Ok(b) => b,
            Err(e) => {
                unstage(&staged);
                return Err(undo(format!("the backup's {} could not be read: {}", name, e)));
            }
        };
        let place = folder.join(name);
        let tmp = place.with_extension("kmrp-new");
        let written = fs::File::create(&tmp).and_then(|mut f| {
            use std::io::Write;
            f.write_all(&bytes)?;
            f.sync_all()
        });
        staged.push(tmp.clone());
        if let Err(e) = written {
            unstage(&staged);
            return Err(undo(format!("{}: {}", tmp.display(), e)));
        }
        if fs::read(&tmp).map_or(true, |b| b != bytes) {
            unstage(&staged);
            return Err(undo(format!("{} did not read back the same as the backup's file", tmp.display())));
        }
        wanted.push((name.clone(), place, tmp));
    }

    // 3. the swap; every step remembered so a failure can be undone in reverse
    enum Step {
        Aside(PathBuf), // a save file moved to its .kmrp-old name
        In(PathBuf),    // a staged file moved into its place
    }
    let old_of = |p: &Path| p.with_extension("kmrp-old");
    let mut steps: Vec<Step> = Vec::new();
    let mut failed: Option<String> = None;
    let have = |name: &str| now.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, p)| p.clone());
    'swap: {
        for (name, place, tmp) in &wanted {
            if let Some(cur) = have(name) {
                if let Err(e) = fs::rename(&cur, old_of(&cur)) {
                    failed = Some(format!("{}: {}", cur.display(), e));
                    break 'swap;
                }
                steps.push(Step::Aside(cur));
            }
            if let Err(e) = fs::rename(tmp, place) {
                failed = Some(format!("{}: {}", place.display(), e));
                break 'swap;
            }
            steps.push(Step::In(place.clone()));
        }
        // a file the backup does not have goes aside too, removed with the old ones
        for (name, cur) in &now {
            if wanted.iter().any(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
                continue;
            }
            if let Err(e) = fs::rename(cur, old_of(cur)) {
                failed = Some(format!("{}: {}", cur.display(), e));
                break 'swap;
            }
            steps.push(Step::Aside(cur.clone()));
        }
    }
    if let Some(error) = failed {
        let mut stuck = Vec::new();
        for step in steps.iter().rev() {
            match step {
                Step::In(place) => {
                    if let Err(e) = fs::remove_file(place) {
                        stuck.push(format!("{}: {}", place.display(), e));
                    }
                }
                Step::Aside(cur) => {
                    if let Err(e) = fs::rename(old_of(cur), cur) {
                        stuck.push(format!("{}: {}", cur.display(), e));
                    }
                }
            }
        }
        unstage(&staged);
        return Err(if stuck.is_empty() {
            undo(error)
        } else {
            format!("{}. Copy the files of {} over the save folder. {}; {}", HALF_WRITTEN, before.display(), error, stuck.join("; "))
        });
    }

    // 4. the old files removed, the result checked
    for step in &steps {
        if let Step::Aside(cur) = step {
            let _ = fs::remove_file(old_of(cur));
        }
    }
    if !same_files(folder, backup) || Save::open(folder).is_err() {
        return Err(format!("{}. The restored save does not read back as the backup. Copy the files of {} over the save folder.", HALF_WRITTEN, before.display()));
    }
    Ok(Restored { before })
}

/// A new backup of the save folder `folder` in `backups`, read back and compared with the
/// save; an error if the copy fails or does not read back the same.
fn make_backup(backups: &Path, game: Game, folder: &Path) -> Result<PathBuf> {
    let backup = backup_folder(backups, game, folder);
    copy_dir(folder, &backup).map_err(|e| format!("the backup could not be made: {}", e))?;
    if !same_files(folder, &backup) {
        return Err(format!("the backup could not be made: the copy in {} does not read back the same as the save", backup.display()));
    }
    Ok(backup)
}

/// The backups of the save folder `folder` in `backups`, oldest first (their folder names are
/// the times they were made).
pub fn backup_list(backups: &Path, folder: &Path) -> Vec<PathBuf> {
    let dir = backups.join(backup_name(folder));
    let mut list: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    // "2026-11-07 10-00-00", then "... (2)", "... (3)" made in the same second, "(10)" after "(9)"
    list.sort_by_key(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match name.rsplit_once(" (").and_then(|(base, n)| n.strip_suffix(')').and_then(|n| n.parse::<u32>().ok()).map(|n| (base.to_string(), n))) {
            Some((base, n)) => (base, n),
            None => (name, 1),
        }
    });
    list
}

/// Whether folder `b` holds exactly the files of folder `a`, the same names and the same bytes,
/// subfolders too. A write's left-over `.kmrp-old` and `.kmrp-new` files are not the save's and
/// are passed over on both sides. Only reads.
pub fn same_files(a: &Path, b: &Path) -> bool {
    let names = |p: &Path| -> Option<Vec<(std::ffi::OsString, bool)>> {
        let mut v: Vec<_> = fs::read_dir(p)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                !n.ends_with(".kmrp-old") && !n.ends_with(".kmrp-new")
            })
            .map(|e| (e.file_name(), e.path().is_dir()))
            .collect();
        v.sort();
        Some(v)
    };
    let (Some(na), Some(nb)) = (names(a), names(b)) else { return false };
    if na != nb {
        return false;
    }
    na.iter().all(|(n, dir)| {
        let (pa, pb) = (a.join(n), b.join(n));
        if *dir {
            same_files(&pa, &pb)
        } else {
            match (fs::read(&pa), fs::read(&pb)) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            }
        }
    })
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}
