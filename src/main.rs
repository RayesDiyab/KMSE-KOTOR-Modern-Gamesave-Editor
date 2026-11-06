//! KOTOR Modern Gamesave Editor: the window and everything it does. The save itself is read, changed
//! and written by the library (lib.rs); this file turns it into what the screens show
//! (ui/state.slint) and answers the screens' actions.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

use kse_core::area;
use kse_core::catalog::{self, Catalog, ItemTemplate};
use kse_core::creature;
use kse_core::formats::erf::{RES_TYPE_UTC, RES_TYPE_UTI};
use kse_core::formats::gff::{Struct, Value};
use kse_core::formats::{tga, tpc};
use kse_core::gamedata::GameData;
use kse_core::games::{self, Game, Install, SaveFolder, SaveKind};
use kse_core::globals::{self, Global, GlobalValue, Kind};
use kse_core::save::{self, Save, Summary, Who};
use kse_core::story_preview::{self, Plan as StoryPlan, Prepared as PreparedStory};
use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

fn image(img: &tga::Image) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&img.rgba, img.width, img.height);
    Image::from_rgba8(buffer)
}

fn s(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

fn thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{}", out)
    } else {
        out
    }
}

fn played(seconds: i64) -> String {
    format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A file's time as "22 Oct 2026, 01:23", in local time where the platform says what that
/// is (TZ offset is not available without a time zone database, so UTC elsewhere).
fn when(t: Option<std::time::SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + local_offset();
    let (y, m, d) = save::civil(secs.div_euclid(86_400));
    let r = secs.rem_euclid(86_400);
    format!("{} {} {}, {:02}:{:02}", d, MONTHS[(m - 1) as usize], y, r / 3600, r % 3600 / 60)
}

#[cfg(windows)]
fn local_offset() -> i64 {
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: [u16; 8],
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: [u16; 8],
        daylight_bias: i32,
    }
    extern "system" {
        fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    }
    let mut info: TimeZoneInformation = unsafe { std::mem::zeroed() };
    let mode = unsafe { GetTimeZoneInformation(&mut info) };
    let bias = info.bias + if mode == 2 { info.daylight_bias } else { info.standard_bias };
    -(bias as i64) * 60
}

#[cfg(not(windows))]
fn local_offset() -> i64 {
    // `date +%z` gives "+0200"; read once.
    thread_local!(static OFFSET: i64 = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|t| {
            let t = t.trim();
            let sign = if t.starts_with('-') { -1 } else { 1 };
            let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
            let v: i64 = digits.parse().ok()?;
            Some(sign * (v / 100 * 3600 + v % 100 * 60))
        })
        .unwrap_or(0));
    OFFSET.with(|o| *o)
}

fn open_path(path: &Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

/// Whether the game is running, so a save is not changed under it.
fn game_running(game: Game) -> bool {
    let names: &[&str] = match game {
        Game::K1 => &["swkotor.exe", "KOTOR", "swkotor"],
        Game::K2 => &["swkotor2.exe", "KOTOR2", "swkotor2"],
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up.
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {}", names[0]), "/NH"])
            .creation_flags(0x0800_0000)
            .output();
        // A check that failed says nothing, so it counts as running: nothing is written.
        return out.map_or(true, |o| !o.status.success() || String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains(names[0]));
    }
    #[cfg(not(windows))]
    {
        // Under Proton the process is swkotor.exe. pgrep exits 1 for no match, above that it failed.
        return names.iter().any(|n| {
            std::process::Command::new("pgrep").args(["-x", n]).output().map_or(true, |o| o.status.code() != Some(1))
        });
    }
}

// ---- settings, kept beside the backups ----

#[derive(Default)]
struct Settings {
    added: Vec<PathBuf>,
    /// The install to use for each game, when the player picked one.
    k1_dir: Option<PathBuf>,
    k2_dir: Option<PathBuf>,
    k2: bool,
    light: bool,
    /// 0 Small, 1 Medium (the default since 2026-10-22), 2 Large.
    text_size: i32,
}

impl Settings {
    fn path() -> PathBuf {
        games::data_dir().join("settings.txt")
    }

    fn load() -> Settings {
        let mut s = Settings { text_size: 1, ..Settings::default() };
        // Bytes, not a string: one bad byte in a path must not lose every other setting.
        if let Ok(bytes) = std::fs::read(Self::path()) {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Some(p) = line.strip_prefix("added=") {
                    s.added.push(PathBuf::from(p));
                } else if let Some(p) = line.strip_prefix("k1dir=") {
                    s.k1_dir = Some(PathBuf::from(p));
                } else if let Some(p) = line.strip_prefix("k2dir=") {
                    s.k2_dir = Some(PathBuf::from(p));
                } else if line == "game=k2" {
                    s.k2 = true;
                } else if line == "theme=light" {
                    s.light = true;
                } else if let Some(n) = line.strip_prefix("textsize=") {
                    s.text_size = n.parse::<i32>().unwrap_or(1).clamp(0, 2);
                }
            }
        }
        s
    }

    fn store(&self) {
        let mut text = String::new();
        for a in &self.added {
            text.push_str(&format!("added={}\n", a.display()));
        }
        for (key, dir) in [("k1dir", &self.k1_dir), ("k2dir", &self.k2_dir)] {
            if let Some(d) = dir {
                text.push_str(&format!("{}={}\n", key, d.display()));
            }
        }
        text.push_str(if self.k2 { "game=k2\n" } else { "game=k1\n" });
        text.push_str(if self.light { "theme=light\n" } else { "theme=dark\n" });
        text.push_str(&format!("textsize={}\n", self.text_size));
        let _ = std::fs::create_dir_all(games::data_dir());
        let _ = std::fs::write(Self::path(), text);
    }
}

/// Where a picked item or save goes.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    None,
    Inventory,
    Container,
    Class,
    Compare,
    ImportNpc(usize),
    CopyInventory,
    Appearance,
    Soundset,
    Gender,
    ChangeClass(usize),
    Spawn,
}

/// A game's tables, names and (when asked) item list. Made in the background at start
/// for the game not on screen, so switching to it is instant.
fn build_game(game: Game, path: &Path, with_items: bool) -> Game_ {
    let data = Arc::new(GameData::open(game, path));
    let catalog = Arc::new(Catalog::build(&data));
    let items = with_items.then(|| Arc::new(catalog::item_templates(&data)));
    Game_ { data, catalog, items, journal: Default::default() }
}

#[derive(Clone)]
enum MapPick {
    Object(usize),
    Floor([f32; 3]),
}

struct Game_ {
    data: Arc<GameData>,
    catalog: Arc<Catalog>,
    items: Option<Arc<Vec<ItemTemplate>>>,
    /// The game's journal (global.jrl), read on first use: every quest's name and stages.
    journal: std::sync::OnceLock<Arc<Vec<kse_core::quests::Quest>>>,
}

struct State {
    settings: Settings,
    installs: Vec<Install>,
    game: Game,
    games: HashMap<Game, Game_>,
    ready: Arc<Mutex<HashMap<Game, Game_>>>,
    ready_items: Arc<Mutex<HashMap<Game, Arc<Vec<ItemTemplate>>>>>,
    picker_tab: usize,
    inventory_tab: usize,
    /// The rows of a list pop-up that is not the item list, for its search box.
    picker_all: Vec<Named>,
    /// Quest tags in the order the Quests list shows them.
    quest_tags: Vec<String>,
    /// The quest tracker: its tab (0 active, 1 completed, 2 not started) and the quest shown.
    quest_tab: usize,
    quest_pick: Option<String>,
    /// Branch steps of the picked quest's chain (questdb::path's choices, all endings in
    /// order) and whether each is chosen for "as if played".
    chain_choices: Vec<bool>,
    /// The area shown on the Map tab, and what is picked on it.
    map: Option<kse_core::map::Area>,
    map_pick: Option<MapPick>,
    saves: Vec<(SaveFolder, Summary)>,
    shots: Arc<Mutex<HashMap<PathBuf, Option<Arc<tga::Image>>>>>,
    portraits: HashMap<String, Option<Image>>,
    selected: usize,
    save: Option<Save>,
    original: Option<Save>,
    read_only: bool,
    whos: Vec<Who>,
    who: usize,
    tab: i32,
    story: i32,
    feat_query: String,
    power_query: String,
    picker_query: String,
    picker_ids: Vec<usize>,
    target: Target,
    other_saves: Vec<SaveFolder>,
    compare: Option<(String, Vec<Global>)>,
    global_filter: (String, bool),
    selected_box: Option<(&'static str, usize)>,
    /// What can be spawned at the picked spot, and where it goes and which way it faces.
    spawnable: Vec<Spawnable>,
    spawn_at: Option<([f32; 3], f32)>,
    /// The slot picked on the equip grid, whose fitting items the inventory list shows.
    equip_slot: Option<u32>,
    // KotOR II's weapon set the Equipped grid shows: 0 the first, 1 the second
    weapon_set: usize,
    /// Where each module can be arrived at (map::arrivals), read on first use, by game; and
    /// the ways into the area on the map, as Travel Here lists them.
    arrivals: HashMap<Game, Arc<HashMap<String, Vec<(String, String)>>>>,
    travel_ways: Vec<(String, String)>,
    /// Every module's display name ("Dantooine - Jedi Enclave"), read once a game: opening
    /// each module's files for ~150 names made every map refresh take seconds.
    module_titles: HashMap<Game, HashMap<String, String>>,
    // The two above, read away from the window when a save of the game opens (about 0.4 s
    // in all), so the Map tab does not wait for them: taken over by adopt_map_lists.
    map_lists: Arc<Mutex<HashMap<Game, MapLists>>>,
    map_lists_asked: std::collections::HashSet<Game>,
    // the Galaxy Map page's art, made away from the window right after start (preload_art)
    art: Arc<Mutex<HashMap<String, Option<Pixels>>>>,
    art_asked: std::collections::HashSet<Game>,
    /// The planets open in the Areas list.
    open_planets: std::collections::HashSet<String>,
    // Skip Ahead's open chapters and quests (SkipRow keys), and the quest stage asked about
    skip_open: std::collections::HashSet<String>,
    skip_stage: Option<(String, i64)>,
    // the timeline's other saves of this game, worked out once per save, away from the
    // window (reading every save takes a while): the result lands in timeline_load
    timeline_saves: Option<Vec<TimelineOther>>,
    timeline_load: Arc<Mutex<Option<Vec<TimelineOther>>>>,
    timeline_loading: bool,
    // the Quests page: whether the picked quest's stages are open under it, and the stage
    // picked among them (its id)
    quest_open: bool,
    stage_pick: Option<i64>,
    /// spawned.txt lines of spawns not saved yet.
    pending_spawns: Vec<String>,
    // the chapter whose pop-up is open on the timeline (a milestone index), and the row
    // picked in it: (depth, chapter index, quest tag, stage), as SkipRow has them
    timeline_pick: Option<i32>,
    // the XP each quest's stage clicks have given since the save was opened or last saved,
    // by quest tag: a stage's XP is given once, and taken back when the quest is set back
    quest_xp: HashMap<String, i64>,
    // the order picked on the galaxy map for the free planets (planet names), for this save
    route_plan: Vec<String>,
    // a save clicked on the timeline, waiting for Open in its warning (an index into saves)
    open_ask: Option<usize>,
    pop_pick: Option<(i32, i32, String, i32)>,
    // a most Health or Force typed out of reach at the creature's level (overlay 13)
    pool_ask: Option<PoolAsk>,
    // the point going back asks about: the chapter, a quest and stage (None: its start)
    back_to: Option<(i32, Option<(String, i64)>)>,
    story_plan: Option<StoryPlan>,
    story_prepared: Option<PreparedStory>,
    story_result: Arc<Mutex<Option<Result<PreparedStory, String>>>>,
    status: (String, bool),
}

type Shared = Rc<RefCell<State>>;

impl State {
    fn current(&self) -> Option<&Game_> {
        self.games.get(&self.game)
    }

    fn data(&self) -> Option<Arc<GameData>> {
        self.current().map(|g| g.data.clone())
    }

    fn catalog(&self) -> Option<Arc<Catalog>> {
        self.current().map(|g| g.catalog.clone())
    }

    fn ensure_game(&mut self, game: Game) {
        if self.games.contains_key(&game) {
            return;
        }
        if let Some(ready) = self.ready.lock().unwrap().remove(&game) {
            self.games.insert(game, ready);
            return;
        }
        if let Some(install) = self.installs.iter().find(|i| i.game == game) {
            self.games.insert(game, build_game(game, &install.path, false));
        } else {
            // a save opened without the game on this PC: what the save holds is still shown
            let data = Arc::new(GameData::none(game));
            let catalog = Arc::new(Catalog::build(&data));
            self.games.insert(game, Game_ { data, catalog, items: None, journal: Default::default() });
        }
    }

    /// The game's journal, read once (it was read again and every stage's text decoded on
    /// each refresh of the story).
    fn journal(&self) -> Arc<Vec<kse_core::quests::Quest>> {
        match self.current() {
            Some(g) => g.journal.get_or_init(|| Arc::new(kse_core::quests::all(&g.data))).clone(),
            None => Arc::new(Vec::new()),
        }
    }

    /// Whether the game's own files are here: names, icons, maps and the class rules.
    fn has_game_files(&self) -> bool {
        self.data().map_or(false, |d| d.is_usable())
    }

    fn portrait(&mut self, resref: &str) -> Option<Image> {
        let key = resref.to_ascii_lowercase();
        if key.is_empty() {
            return None;
        }
        if let Some(p) = self.portraits.get(&key) {
            return p.clone();
        }
        let img = self.data().and_then(|d| d.texture(&key)).and_then(|(bytes, is_tpc)| {
            if is_tpc {
                tpc::decode(&bytes)
            } else {
                tga::decode(&bytes)
            }
        });
        let img = img.map(|i| image(&i));
        self.portraits.insert(key, img.clone());
        img
    }

    /// An icon texture, or the fallback when the game has no such texture.
    fn icon(&mut self, name: &str, fallback: &str) -> Option<Image> {
        self.portrait(name).or_else(|| self.portrait(fallback))
    }

    fn item_icon(&mut self, item: &Struct) -> Option<Image> {
        let base = self.data()?.table("baseitems");
        let row = item.int("BaseItem").unwrap_or(0) as usize;
        let (name, fallback) = catalog::item_icon(&base, row, item.int("ModelVariation").unwrap_or(1));
        self.icon(&name, &fallback)
    }

    fn portrait_of(&mut self, c: &Struct) -> Option<Image> {
        let id = c.int("PortraitId")?;
        let resref = self.data()?.table("portraits").cell(id as usize, "baseresref")?.to_string();
        self.portrait(&resref)
    }

    fn creature_name(&self, who: Who) -> String {
        let Some(save) = self.save.as_ref() else { return String::new() };
        let Some(c) = save.creature(who) else { return String::new() };
        let mut name = creature::name(c);
        if name.is_empty() {
            if let (Some(Value::LocString(l)), Some(d)) = (c.get("FirstName"), self.data()) {
                name = d.loc(l);
            }
        }
        if name.is_empty() {
            if let (Who::Npc(i), Some(cat)) = (who, self.catalog()) {
                name = cat.npc_names.get(i).cloned().unwrap_or_default();
            }
        }
        name
    }

    fn npc_name(&self, index: usize) -> String {
        if self.save.as_ref().map_or(false, |s| s.creature(Who::Npc(index)).is_some()) {
            let n = self.creature_name(Who::Npc(index));
            if !n.is_empty() {
                return n;
            }
        }
        self.catalog().and_then(|c| c.npc_names.get(index).cloned()).unwrap_or_else(|| format!("Member {}", index + 1))
    }

    fn who(&self) -> Who {
        self.whos.get(self.who).copied().unwrap_or(Who::Player)
    }

    fn item_templates(&mut self) -> Arc<Vec<ItemTemplate>> {
        let game = self.game;
        if let Some(g) = self.games.get_mut(&game) {
            if g.items.is_none() {
                let ready = self.ready_items.lock().unwrap().remove(&game);
                g.items = Some(ready.unwrap_or_else(|| Arc::new(catalog::item_templates(&g.data))));
            }
            return g.items.clone().unwrap();
        }
        Arc::new(Vec::new())
    }

    fn item_name(&self, item: &Struct) -> String {
        let Some(data) = self.data() else { return item.string("Tag").or_else(|| item.string("EquippedRes")).unwrap_or_default() };
        // A creature made from its template wears template entries (EquippedRes alone): the
        // name is the template's (the story preview listed a recruit's gear with no names)
        let template = match (item.get("LocalizedName"), item.string("EquippedRes")) {
            (None, Some(res)) if !res.is_empty() => data.gff(&res, "uti", kse_core::formats::erf::RES_TYPE_UTI),
            _ => None,
        };
        let item = template.as_ref().map_or(item, |t| &t.root);
        let name = match item.get("LocalizedName") {
            Some(Value::LocString(l)) => data.loc(l),
            _ => String::new(),
        };
        if name.trim().is_empty() {
            let tag = item.string("Tag").unwrap_or_default();
            if tag.is_empty() {
                item.string("TemplateResRef").unwrap_or_default()
            } else {
                tag
            }
        } else {
            name.replace('\n', " ")
        }
    }

    fn item_kind(&self, item: &Struct) -> (String, i64) {
        let Some(data) = self.data() else { return (String::new(), 1) };
        let base = item.int("BaseItem").unwrap_or(0) as usize;
        let t = data.table("baseitems");
        let kind = t.int(base, "name").and_then(|r| data.string(r as u32)).unwrap_or_default();
        (kind, t.int(base, "stacking").unwrap_or(1).max(1))
    }
}

/// Every install found, with the one the player picked for each game first.
fn installs_for(settings: &Settings) -> Vec<Install> {
    let mut extra = settings.added.clone();
    extra.extend(settings.k1_dir.iter().cloned());
    extra.extend(settings.k2_dir.iter().cloned());
    let mut installs = games::find_installs(&extra);
    let picked = |i: &Install| {
        let want = match i.game {
            Game::K1 => &settings.k1_dir,
            Game::K2 => &settings.k2_dir,
        };
        want.as_ref().map_or(false, |w| games::clean(w) == i.path)
    };
    installs.sort_by_key(|i| !picked(i));
    installs
}

// ---- filling the screens ----

/// The folders shown in Settings: each game's install as found or picked, and the
/// save folders the player added.
fn fill_settings(ui: &AppWindow, st: &State) {
    let app = ui.global::<App>();
    let folder = |game: Game| {
        st.installs
            .iter()
            .find(|i| i.game == game)
            .map(|i| format!("{}: {}", i.source, i.path.display()))
            .unwrap_or_default()
    };
    app.set_k1_folder(s(folder(Game::K1)));
    app.set_k2_folder(s(folder(Game::K2)));
    app.set_added_folders(model(st.settings.added.iter().map(|p| s(p.display().to_string())).collect()));
}

fn fill_saves(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let theme = ui.global::<Theme>();
    theme.set_k2(st.game == Game::K2);
    // a game shows on the switch when it is installed or has saves added in Settings: saves of
    // the other game added on a PC with one game could not be left again (2026-11-06 audit)
    let has = |g: Game| st.installs.iter().any(|i| i.game == g) || !games::find_saves(&st.installs, g, &st.settings.added).is_empty();
    app.set_has_k1(has(Game::K1));
    app.set_has_k2(has(Game::K2));
    app.set_showing_k2(st.game == Game::K2);
    st.ensure_game(st.game);
    preload_art(st);
    fill_settings(ui, st);

    let folders = games::find_saves(&st.installs, st.game, &st.settings.added);
    let mut saves: Vec<(SaveFolder, Summary)> =
        folders.into_iter().filter_map(|f| save::summary(&f.path).map(|s| (f, s))).collect();
    saves.sort_by(|a, b| b.1.modified.cmp(&a.1.modified));
    // the save picked stays picked by its folder, wherever the new order puts it (a save just
    // written moves to the top; the row number would then pick another)
    let picked = st.saves.get(st.selected).map(|(f, _)| f.path.clone());
    if let Some(k) = picked.and_then(|p| saves.iter().position(|(f, _)| f.path == p)) {
        st.selected = k;
    }
    st.saves = saves;
    let shots = st.shots.lock().unwrap().clone();
    let rows: Vec<SaveRow> = st
        .saves
        .iter()
        .map(|(f, sm)| {
            let shot = shots.get(&f.path).cloned().flatten();
            SaveRow {
                name: s(&sm.name),
                area: s(&sm.area),
                time: s(played(sm.seconds)),
                date: s(when(sm.modified)),
                tag: s(match f.kind {
                    SaveKind::Cloud => "CLOUD",
                    SaveKind::Added => "ADDED",
                    SaveKind::Local => "",
                }),
                has_shot: shot.is_some(),
                shot: shot.map(|i| image(&i)).unwrap_or_default(),
            }
        })
        .collect();
    app.set_saves(model(rows));
    let n = st.saves.len();
    app.set_saves_note(s(match n {
        0 => String::new(),
        1 => "1 save found".to_string(),
        _ => format!("{} saves found", n),
    }));
    let installed = st.installs.iter().any(|i| i.game == st.game);
    if !installed && st.installs.is_empty() {
        app.set_empty_title(s("No KotOR game found"));
        app.set_empty_note(s(
            "The editor looks in every Steam library, GOG and disc install. If the game is somewhere else, use Add a Save Folder below and pick the game's folder or a saves folder.",
        ));
    } else {
        app.set_empty_title(s(format!("No {} saves found", st.game.title())));
        app.set_empty_note(s(
            "Saves made in the game appear here. Saves kept somewhere else (copied from a phone, a Switch or another PC) can be added with Add a Save Folder below.",
        ));
    }
    if st.selected >= n {
        st.selected = 0;
    }
    app.set_selected_save(st.selected as i32);
    fill_preview(ui, st);
    load_shots(ui, st);
}

/// Counts the save lists filled, so a picture knows when its row number has gone out of date.
static SHOT_LIST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Decodes the save pictures away from the window and puts each in its row as it comes.
fn load_shots(ui: &AppWindow, st: &State) {
    let paths: Vec<PathBuf> = st.saves.iter().map(|(f, _)| f.path.clone()).collect();
    let cache = st.shots.clone();
    let weak = ui.as_weak();
    let list = SHOT_LIST.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        for (i, path) in paths.into_iter().enumerate() {
            if cache.lock().unwrap().contains_key(&path) {
                continue;
            }
            let img = save::screenshot(&path).map(Arc::new);
            cache.lock().unwrap().insert(path.clone(), img.clone());
            let Some(img) = img else { continue };
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return };
                // the list was filled again since: `i` is another save's row now
                if SHOT_LIST.load(std::sync::atomic::Ordering::SeqCst) != list {
                    return;
                }
                let app = ui.global::<App>();
                let rows = app.get_saves();
                if let Some(mut row) = rows.row_data(i) {
                    row.shot = image(&img);
                    row.has_shot = true;
                    rows.set_row_data(i, row);
                }
                if app.get_selected_save() as usize == i && app.get_page() == 0 {
                    app.set_preview_shot(image(&img));
                    app.set_preview_has_shot(true);
                }
            });
        }
    });
}

fn is_xbox_save(path: &Path) -> bool {
    std::fs::read_dir(path).map_or(false, |d| {
        d.filter_map(|e| e.ok()).any(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            n == "savemeta.xbx" || n == "saveimage.xbx"
        })
    })
}

/// Why the save in `path` can be looked at but not changed, if it cannot.
fn read_only_reason(path: &Path) -> Option<&'static str> {
    if is_xbox_save(path) {
        Some("It is an original Xbox save; the Xbox would call it damaged.")
    } else if save::player_outside_archive(path) {
        Some(save::PLAYER_OUTSIDE_ARCHIVE)
    } else {
        None
    }
}

/// A save added on its own: its copy sits in a folder the list does not scan, so it is added
/// the same way, or the new save would not be listed.
fn add_beside(st: &mut State, source: &Path, target: &Path) {
    if st.settings.added.iter().any(|a| a == source) && !st.settings.added.iter().any(|a| a == target) {
        st.settings.added.push(target.to_path_buf());
        st.settings.store();
    }
}

/// Stops a step that would drop changes not saved, and says why.
fn unsaved(ui: &AppWindow, st: &State) -> bool {
    let any = st.save.as_ref().map_or(false, |s| !s.changes.is_empty());
    if any {
        message(ui, "There are changes not saved", "Save them or press Revert first, so nothing is lost by accident.");
    }
    any
}

/// Undoes an edit that reached a read-only save (a path with no check of its own), and says
/// why. Nothing there can be saved, so keeping it would only be work lost later.
fn undo_read_only(ui: &AppWindow, st: &mut State) -> bool {
    if !st.read_only || st.save.as_ref().map_or(true, |s| s.changes.is_empty()) {
        return false;
    }
    let folder = st.save.as_ref().unwrap().folder.clone();
    if let Ok(fresh) = Save::open(&folder) {
        st.save = Some(fresh);
    }
    let why = read_only_reason(&folder).or_else(|| st.save.as_ref().and_then(|s| s.module_problem())).unwrap_or("It is read only.");
    message(ui, "This save cannot be changed", why);
    true
}

fn fill_preview(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some((folder, sm)) = st.saves.get(st.selected).cloned() else { return };
    app.set_preview_name(s(&sm.name));
    app.set_preview_area(s(&sm.area));
    app.set_preview_time(s(played(sm.seconds)));
    app.set_preview_date(s(when(sm.modified)));
    app.set_preview_tag(s(if folder.kind == SaveKind::Cloud { "Steam Cloud" } else { "" }));
    let shot = st.shots.lock().unwrap().get(&folder.path).cloned().flatten();
    app.set_preview_has_shot(shot.is_some());
    app.set_preview_shot(shot.map(|i| image(&i)).unwrap_or_default());
    let party: Vec<Image> = sm.portraits.iter().filter_map(|p| st.portrait(p)).collect();
    app.set_preview_party(model(party));
    let mut pc = sm.pc_name.clone();
    if pc.is_empty() {
        // KotOR 1 keeps the name only in the player creature.
        pc = Save::open(&folder.path).ok().and_then(|sv| sv.player().map(creature::name)).unwrap_or_default();
    }
    app.set_preview_pc(s(pc));
    let xbox = is_xbox_save(&folder.path);
    let outside = save::player_outside_archive(&folder.path);
    app.set_preview_read_only(xbox || outside);
    app.set_preview_warning(s(if xbox {
        "An original Xbox save: it can be looked at but not changed, as the Xbox checks a signature on every save."
    } else if outside {
        "Its player is kept in pifo.ifo, outside the save's archive, which the editor does not write: it can be looked at but not changed. Save again in a normal slot in the game."
    } else if folder.kind == SaveKind::Cloud {
        "A Steam Cloud save: close the game and let Steam finish syncing before changing it. Steam sends the changed save to the cloud the next time the game starts."
    } else {
        ""
    }));
}

fn open_selected(ui: &AppWindow, st: &mut State) {
    let Some((folder, _)) = st.saves.get(st.selected).cloned() else { return };
    st.ensure_game(folder.game);
    match (Save::open(&folder.path), Save::open(&folder.path)) {
        (Ok(sv), Ok(orig)) => {
            st.read_only = read_only_reason(&folder.path).is_some() || sv.module_problem().is_some();
            st.save = Some(sv);
            st.original = Some(orig);
            st.quest_xp.clear();
            st.who = 0;
            st.tab = 0;
            st.stage_pick = None;
            st.equip_slot = None;
            forget_pending_spawns(st);
            // Story opens on its quests; the timeline has its own tab (2026-11-06)
            st.story = 3;
            st.compare = None;
            st.selected_box = None;
            st.status = (String::new(), false);
            st.global_filter = (String::new(), false);
            st.map = None;
            st.map_pick = None;
            st.route_plan.clear();
            forget_timeline_saves(st);
            prefetch_map_lists(st);
            preload_art(st);
            ui.global::<App>().set_page(1);
            fill_editor(ui, st);
            // the Timeline filled now, and the other saves read for it in the background, so
            // its tab opens at once (2026-11-06, the maintainer: "preload it")
            fill_skip(ui, st);
        }
        (Err(e), _) | (_, Err(e)) => message(ui, "This save could not be opened", &e),
    }
}

fn message(ui: &AppWindow, title: &str, body: &str) {
    let app = ui.global::<App>();
    app.set_message_title(s(title));
    app.set_message_body(s(body));
    app.set_overlay(4);
}

fn fill_editor(ui: &AppWindow, st: &mut State) {
    undo_read_only(ui, st);
    let app = ui.global::<App>();
    // A box typed in stops following its value; this has every box read it again.
    app.set_pool_epoch(app.get_pool_epoch().wrapping_add(1));
    let Some(save) = st.save.as_ref() else { return };
    // The player, then who is in the party, then who is recruited, then the rest
    // (2026-10-24, the maintainer: the ones available first); the one picked stays picked.
    let picked = st.whos.get(st.who).copied();
    let mut npcs = save.npcs();
    npcs.sort_by_key(|&i| if save.in_party(i) { 0 } else if save.recruited(i) { 1 } else { 2 });
    let mut whos = vec![Who::Player];
    whos.extend(npcs.into_iter().map(Who::Npc));
    st.who = picked.and_then(|p| whos.iter().position(|w| *w == p)).unwrap_or(0);
    st.whos = whos.clone();
    let nfo = &save.nfo.root;
    app.set_context_line(s(format!(
        "{} · {} · {}",
        save::summary(&save.folder).map(|x| x.name).unwrap_or_default(),
        nfo.string("AREANAME").unwrap_or_default(),
        save.game.title()
    )));
    let backups = backups_of(st).join(save::backup_name(&save.folder));
    app.set_backup_path(s(backups.display().to_string()));

    let cat = st.catalog();
    let mut members = Vec::new();
    for w in whos.clone() {
        let name = st.creature_name(w);
        let c = st.save.as_ref().unwrap().creature(w).cloned();
        let sub = match w {
            Who::Player => format!("Player · level {}", c.as_ref().map(creature::level).unwrap_or(0)),
            Who::Npc(i) => {
                let sv = st.save.as_ref().unwrap();
                if sv.in_party(i) {
                    "In the party".to_string()
                } else if sv.recruited(i) {
                    format!("Recruited · level {}", c.as_ref().map(creature::level).unwrap_or(0))
                } else {
                    "Not recruited".to_string()
                }
            }
        };
        let portrait = c.as_ref().and_then(|c| st.portrait_of(c));
        let prefix = format!("{:?}.", w);
        let changed = st.save.as_ref().map_or(false, |sv| sv.changes.iter().any(|c| c.starts_with(&prefix)));
        members.push(Member {
            changed,
            name: s(if name.is_empty() { "Player".to_string() } else { name }),
            sub: s(sub),
            has_portrait: portrait.is_some(),
            portrait: portrait.unwrap_or_default(),
        });
    }
    app.set_members(model(members));
    app.set_who(st.who as i32);
    app.set_tab(st.tab);
    app.set_is_player(st.who() == Who::Player);
    let _ = cat;
    fill_character(ui, st);
    fill_abilities(ui, st);
    fill_inventory(ui, st);
    fill_party(ui, st);
    fill_story(ui, st);
    if st.tab == 5 {
        fill_map(ui, st);
    }
    fill_bar(ui, st);
}

/// Which tab holds each change, for the orange marks beside the tab names.
fn tab_of(change: &str) -> usize {
    // the player's name as the party table keeps it, and the party's members
    if change == "party.name" {
        return 0;
    }
    if change == "party.members" {
        return 3;
    }
    let field = change.rsplit_once('.').map(|(_, f)| f).unwrap_or(change);
    if change.starts_with("Player.") || change.starts_with("Npc(") {
        if field.starts_with("skill") || field.starts_with("feat") || field.starts_with("power") {
            1
        } else if field == "position" {
            5
        } else {
            0
        }
    } else if change.starts_with("inventory.") || change.contains("PT_GOLD") || change.contains("PT_ITEM_") {
        2
    } else if change.starts_with("npc") || change.contains("PT_AVAIL_NPCS") || change.contains("PT_INFLUENCE") || change.contains("PT_XP_POOL") {
        3
    } else if change.starts_with("globals") || change.starts_with("area.") || change.starts_with("door") || change.starts_with("box") || change.starts_with("creature") {
        // story values, area objects, creatures: the Advanced tab
        6
    } else {
        4
    }
}

fn fill_bar(ui: &AppWindow, st: &State) {
    let app = ui.global::<App>();
    // Timeline is tab 7 (it shows no changes of its own: a skip's are Story's)
    let mut tabs = vec![false; 8];
    if let Some(sv) = st.save.as_ref() {
        for c in &sv.changes {
            tabs[tab_of(c)] = true;
        }
    }
    app.set_tab_changed(model(tabs));
    app.set_change_count(st.save.as_ref().map_or(0, |s| s.changes.len() as i32));
    app.set_status(s(&st.status.0));
    app.set_status_bad(st.status.1);
}

fn fill_character(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let who = st.who();
    let Some(c) = st.save.as_ref().and_then(|s| s.creature(who)).cloned() else { return };
    let o = st.original.as_ref().and_then(|s| s.creature(who)).cloned().unwrap_or_else(|| c.clone());
    let Some(cat) = st.catalog() else { return };

    // the box edits the name as stored; a companion known only by its line in dialog.tlk
    // shows that name
    let stored = creature::stored_name(&c);
    app.set_name(s(if stored.trim().is_empty() { st.creature_name(st.who()) } else { stored }));
    app.set_name_changed(creature::name(&c) != creature::name(&o));
    let classes: Vec<String> =
        creature::classes(&c).iter().map(|(id, lv)| format!("{} {}", cat.class_name(*id), lv)).collect();
    app.set_classes(s(classes.join(" · ")));
    app.set_can_add_class(creature::classes(&c).len() < 2 && creature::level(&c) < cat.max_level());
    let xp = creature::experience(&c);
    app.set_xp(xp as i32);
    app.set_xp_changed(xp != creature::experience(&o));
    let level = creature::level(&c);
    let max = cat.max_level();
    let next = cat.xp_for(level + 1);
    let note = match next {
        Some(need) if level < max && xp >= need => format!(
            "Level {}. This is enough XP for level {}: the game offers the level-up itself, with its own choices of skills, feats and powers.",
            level,
            cat.level_for(xp).min(max)
        ),
        Some(need) if level < max => format!(
            "Level {}. Level {} needs {} XP. Give more XP and the game offers the level-up itself, with its own choices of skills, feats and powers.",
            level,
            level + 1,
            thousands(need)
        ),
        _ => format!("Level {}, the highest the game allows.", level),
    };
    app.set_level_note(s(note));
    app.set_max_xp(cat.xp_for(max).unwrap_or(0) as i32);
    app.set_max_level(s(max.to_string()));
    app.set_alignment(creature::alignment(&c) as i32);
    app.set_alignment_changed(creature::alignment(&c) != creature::alignment(&o));

    const NAMES: [(&str, &str); 6] = [
        ("Strength", "Melee hits and damage"),
        ("Dexterity", "Defence, ranged hits"),
        ("Constitution", "Health"),
        ("Intelligence", "Skill points"),
        ("Wisdom", "Force, power saves"),
        ("Charisma", "Force, persuasion"),
    ];
    let attrs: Vec<Stat> = creature::ATTRIBUTES
        .iter()
        .zip(NAMES)
        .map(|(label, (name, note))| {
            let v = creature::attribute(&c, label);
            let m = creature::modifier(v);
            Stat {
                label: s(name),
                note: s(note),
                value: v as i32,
                minimum: 1,
                maximum: 100,
                r#mod: s(if m >= 0 { format!("+{}", m) } else { m.to_string() }),
                changed: v != creature::attribute(&o, label),
                ..Default::default()
            }
        })
        .collect();
    app.set_attributes(model(attrs));

    let (hp, hp_max) = creature::health(&c);
    let (fp, fp_max) = creature::force(&c, cat.game);
    app.set_health(s(format!("{} / {}", hp, hp_max)));
    app.set_hp_current(hp as i32);
    app.set_hp_max(hp_max as i32);
    app.set_fp_current(fp as i32);
    app.set_fp_max(fp_max as i32);
    app.set_hp_changed(creature::health(&c) != creature::health(&o));
    app.set_fp_changed(creature::force(&c, cat.game) != creature::force(&o, cat.game));
    app.set_min1hp(creature::min1hp(&c));
    app.set_min1hp_changed(creature::min1hp(&c) != creature::min1hp(&o));
    let pick = |list: &[kse_core::catalog::Entry], id: Option<i64>| {
        id.and_then(|i| list.iter().find(|e| e.id == i))
            .map(|e| e.name.clone())
            .unwrap_or_else(|| id.map(|i| if list.is_empty() { format!("Number {}", i) } else { format!("{} (from a mod)", i) }).unwrap_or_default())
    };
    app.set_body_name(s(pick(&cat.appearances, c.int("Appearance_Type"))));
    app.set_voice_name(s(pick(&cat.soundsets, c.int("SoundSetFile"))));
    app.set_gender_name(s(pick(&cat.genders, c.int("Gender"))));
    let class_list = creature::classes(&c);
    let rows: Vec<ClassRow> = class_list
        .iter()
        .enumerate()
        .map(|(i, (id, lv))| ClassRow {
            index: i as i32,
            name: s(cat.class_name(*id)),
            level: *lv as i32,
            // the game's cap is on the total of all classes
            maximum: (cat.max_level() - (class_list.iter().map(|(_, l)| *l).sum::<i64>() - *lv)).max(*lv) as i32,
            can_remove: class_list.len() > 1,
        })
        .collect();
    app.set_class_rows(model(rows));
    app.set_force(s(format!("{} / {}", fp, fp_max)));
    // What the game works the most out from (creature.rs): the player's level records, or
    // anyone else's base plus a bonus per level. The Force bonus is Wis and Cha in KotOR,
    // Wis alone in KotOR II.
    let force_from = if cat.game == Game::K2 { "Wis" } else { "Wis, Cha" };
    let records = creature::uses_level_records(&c);
    app.set_health_note(s(if records {
        "Now, most · level dice + Con".to_string()
    } else {
        format!("Now, most · base {} + Con per level", c.int("HitPoints").unwrap_or(0))
    }));
    app.set_force_note(s(if fp_max <= 0 {
        "No Force class".to_string()
    } else if records {
        format!("Now, most · level dice + {}", force_from)
    } else {
        format!("Now, most · base {} + {} per level", c.int("ForcePoints").unwrap_or(0), force_from)
    }));
    app.set_pool_epoch(app.get_pool_epoch().wrapping_add(1));
    app.set_records_warning(s(if creature::level_records_match(&c) {
        ""
    } else {
        "Another editor left the level records out of step with the classes: a class change may not undo exactly."
    }));

    let portrait = st.portrait_of(&c);
    app.set_has_portrait(portrait.is_some());
    app.set_portrait(portrait.unwrap_or_default());
    let female = c.int("Gender") == Some(1);
    app.set_look_note(s(format!(
        "{}, portrait {}, body {}. A new portrait brings the body that goes with it.",
        if female { "Female" } else { "Male" },
        c.int("PortraitId").unwrap_or(0),
        c.int("Appearance_Type").unwrap_or(0)
    )));
    app.set_look_changed(c.int("PortraitId") != o.int("PortraitId"));
    let p = area::position(&c);
    let po = area::position(&o);
    app.set_position_x(s(format!("{:.2}", p[0])));
    app.set_position_y(s(format!("{:.2}", p[1])));
    app.set_position_z(s(format!("{:.2}", p[2])));
    app.set_position_changed(p != po);
}

fn row(id: i64, name: impl AsRef<str>, note: impl AsRef<str>) -> Named {
    Named { id: id as i32, name: s(name), note: s(note), ..Default::default() }
}

fn named(rows: impl Iterator<Item = (i64, String, String)>) -> ModelRc<Named> {
    model(rows.map(|(id, name, note)| row(id, name, note)).collect())
}

/// Every entry of a catalogue list, under its group's heading, with its icon: what the
/// creature has is lit. Entries it has that the game's list lacks (a removed mod) come
/// last, so they can still be removed.
fn ability_rows(st: &mut State, list: &[kse_core::catalog::Entry], have: &[i64], before: &[i64], query: &str, fallback_name: &str) -> Vec<Named> {
    let game_files = st.has_game_files();
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    let mut group = String::new();
    for e in list {
        if !q.is_empty() && !e.name.to_lowercase().contains(&q) {
            continue;
        }
        if e.group != group {
            group = e.group.clone();
            out.push(Named { header: true, ..row(-1, group.to_uppercase(), "") });
        }
        let owned = have.contains(&e.id);
        let icon = st.portrait(&e.icon);
        out.push(Named {
            owned,
            changed: owned != before.contains(&e.id),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            ..row(e.id, &e.name, short(&e.description))
        });
    }
    let unknown: Vec<i64> = have.iter().copied().filter(|id| !list.iter().any(|e| e.id == *id)).collect();
    if !unknown.is_empty() && q.is_empty() {
        out.push(Named { header: true, ..row(-1, if game_files { "NOT IN THE GAME'S LIST" } else { "IN THE SAVE (NAMES NEED THE GAME)" }, "") });
        for id in unknown {
            out.push(Named { owned: true, ..row(id, format!("{} {}", fallback_name, id), "From a mod that is no longer installed, or hidden") });
        }
    }
    out
}

fn short(text: &str) -> String {
    let first = text.split(['\n', '.']).next().unwrap_or("").trim();
    if first.chars().count() > 90 {
        format!("{}…", first.chars().take(88).collect::<String>())
    } else {
        first.to_string()
    }
}

fn fill_abilities(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let who = st.who();
    let Some(c) = st.save.as_ref().and_then(|s| s.creature(who)).cloned() else { return };
    let o = st.original.as_ref().and_then(|s| s.creature(who)).cloned().unwrap_or_else(|| c.clone());
    let Some(cat) = st.catalog() else { return };
    let skill_icons: Vec<Option<Image>> = cat.skills.iter().map(|sk| st.portrait(&sk.icon)).collect();
    let skills: Vec<Stat> = cat
        .skills
        .iter()
        .enumerate()
        .map(|(i, sk)| {
            let v = creature::skill(&c, i);
            let icon = skill_icons[i].clone();
            Stat {
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
                label: s(&sk.name),
                note: s(""),
                value: v as i32,
                minimum: 0,
                maximum: 99,
                r#mod: s(""),
                changed: v != creature::skill(&o, i),
            }
        })
        .collect();
    app.set_skills(model(skills));
    let (feats_now, feats_before) = (creature::feats(&c), creature::feats(&o));
    let (powers_now, powers_before) = (creature::powers(&c), creature::powers(&o));
    let (fq, pq) = (st.feat_query.clone(), st.power_query.clone());
    let feat_rows = ability_rows(st, &cat.feats, &feats_now, &feats_before, &fq, "Feat");
    let power_rows = ability_rows(st, &cat.powers, &powers_now, &powers_before, &pq, "Power");
    app.set_feats(model(feat_rows));
    app.set_powers(model(power_rows));
    app.set_feat_count(feats_now.len() as i32);
    app.set_power_count(powers_now.len() as i32);
    let force = |id: i64| cat.is_force_class(id);
    let can = creature::can_have_powers(&c, &force);
    app.set_can_powers(can);
    let jedi: Vec<String> = creature::classes(&c)
        .iter()
        .filter(|(id, _)| cat.is_force_class(*id))
        .map(|(id, _)| cat.class_name(*id))
        .collect();
    app.set_power_note(s(jedi.join(", ")));
}

fn item_rows(st: &mut State, items: &[Struct], original: &[Struct]) -> Vec<ItemRow> {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let icon = st.item_icon(it);
            let (kind, max) = st.item_kind(it);
            let stack = it.int("StackSize").unwrap_or(1);
            // the same item in the save as opened, by its object id (by place where it has
            // none): removing one row no longer marks every row under it
            let id = it.int("ObjectId");
            let same_id = |o: &&Struct| o.int("ObjectId") == id;
            let was = match id {
                Some(_) if original.iter().filter(same_id).count() == 1 => original.iter().find(same_id),
                _ => original.get(i),
            };
            let changed = was.map_or(true, |o| o != it);
            ItemRow {
                index: i as i32,
                name: s(st.item_name(it)),
                kind: s(kind),
                stack: stack as i32,
                max_stack: max.max(stack) as i32,
                tag: s(it.string("Tag").unwrap_or_default()),
                changed,
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
            }
        })
        .collect()
}

fn fill_inventory(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let orig = st.original.as_ref();
    let get = |s: &Save, l: &str| s.party_int(l).unwrap_or(0);
    let credits = get(save, "PT_GOLD");
    app.set_credits(credits as i32);
    app.set_credits_changed(orig.map_or(false, |o| get(o, "PT_GOLD") != credits));
    let crafting = save.party_int("PT_ITEM_COMPONEN").is_some();
    app.set_has_crafting(crafting);
    let comp = get(save, "PT_ITEM_COMPONEN");
    let chem = get(save, "PT_ITEM_CHEMICAL");
    app.set_components(comp as i32);
    app.set_chemicals(chem as i32);
    app.set_components_changed(orig.map_or(false, |o| get(o, "PT_ITEM_COMPONEN") != comp));
    app.set_chemicals_changed(orig.map_or(false, |o| get(o, "PT_ITEM_CHEMICAL") != chem));
    let items = save.inventory();
    let original = orig.map(|o| o.inventory()).unwrap_or_default();
    let base = st.data().map(|d| d.table("baseitems"));
    let tab = st.inventory_tab;
    let shown = |it: &Struct| {
        tab == 0
            || base.as_ref().map_or(true, |b| catalog::item_category(b, it.int("BaseItem").unwrap_or(0) as usize).unwrap_or(7) == tab)
    };
    let rows: Vec<ItemRow> = item_rows(st, &items, &original).into_iter().filter(|r| shown(&items[r.index as usize])).collect();
    app.set_inventory_tab(tab as i32);
    let who = st.who();
    let worn = st.save.as_ref().and_then(|s| s.creature(who)).cloned();
    let worn_before = st.original.as_ref().and_then(|s| s.creature(who)).cloned();
    let k2 = st.game == Game::K2;
    let mut slot_rows = Vec::new();
    // In the order of the game's equip screen (equip.gui, equip_p.gui): three across, three
    // down, in both games. KotOR II's second weapon set takes the weapons' places when it is
    // picked under the grid, as the game swaps them (2026-11-06, the maintainer: "make it same
    // layout as kotor 1"; it was a fourth row, which made every slot smaller).
    let set2 = k2 && st.weapon_set == 1;
    let grid: [u32; 9] = [0x200, 0x1, 0x8, 0x80, 0x2, 0x100, if set2 { 0x80000 } else { 0x20 }, 0x400, if set2 { 0x40000 } else { 0x10 }];
    app.set_has_weapon_sets(k2);
    app.set_weapon_set(if k2 { st.weapon_set as i32 } else { 0 });
    for slot in grid {
        if slot == 0 {
            slot_rows.push(SlotRow::default());
            continue;
        }
        let name = creature::SLOTS.iter().find(|(s, _)| *s == slot).map(|(_, n)| *n).unwrap_or("Slot");
        // The picture the game shows in an empty slot (swpc_tex_gui).
        let art_name = match slot {
            0x200 => "iimplant",
            0x1 => "ihead",
            0x8 => "ihands",
            0x80 => "iforearm_l",
            0x2 => "iarmor",
            0x100 => "iforearm_r",
            0x20 | 0x80000 => "iweap_l",
            0x400 => "ibelt",
            _ => "iweap_r",
        };
        let art = st.icon(art_name, "");
        let item = worn.as_ref().and_then(|c| creature::in_slot(c, slot)).cloned();
        let before = worn_before.as_ref().and_then(|c| creature::in_slot(c, slot)).map(|i| i.string("Tag"));
        let icon = item.as_ref().and_then(|i| st.item_icon(i));
        slot_rows.push(SlotRow {
            slot: slot as i32,
            name: s(name),
            item: s(item.as_ref().map(|i| st.item_name(i)).unwrap_or_default()),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            empty: item.is_none(),
            changed: before != item.as_ref().map(|i| i.string("Tag")),
            has_art: art.is_some(),
            art: art.unwrap_or_default(),
        });
    }
    app.set_slots(model(slot_rows));

    // A picked slot: the list on the right shows what fits it, as in the game.
    match st.equip_slot {
        Some(slot) => {
            let rows = slot_items(st, slot);
            let worn = st.save.as_ref().and_then(|s| s.creature(who)).and_then(|c| creature::in_slot(c, slot)).cloned();
            let name = creature::SLOTS.iter().find(|(s, _)| *s == slot).map(|(_, n)| *n).unwrap_or("Slot");
            app.set_equip_slot(slot as i32);
            app.set_equip_slot_name(s(name));
            app.set_equip_slot_item(s(worn.as_ref().map(|i| st.item_name(i)).unwrap_or_default()));
            app.set_slot_items(model(rows));
        }
        None => {
            app.set_equip_slot(0);
            app.set_slot_items(model(Vec::new()));
        }
    }
    app.set_picker_tabs(model(catalog::ITEM_CATEGORIES.iter().map(|c| s(c)).collect()));
    app.set_picker_tabs_a(model(catalog::ITEM_CATEGORIES[..4].iter().map(|c| s(c)).collect()));
    app.set_picker_tabs_b(model(catalog::ITEM_CATEGORIES[4..].iter().map(|c| s(c)).collect()));
    app.set_items(model(rows));
    app.set_inventory_note(s("A stack can be as large as the game allows for that kind of item."));
}

fn fill_party(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let count = save.npc_count();
    let mut rows = Vec::new();
    for i in 0..count {
        let sv = st.save.as_ref().unwrap();
        let has = sv.creature(Who::Npc(i)).is_some();
        let influence = sv.influence(i);
        let changed = st.original.as_ref().map_or(false, |o| {
            o.recruited(i) != sv.recruited(i) || o.selectable(i) != sv.selectable(i) || o.influence(i) != influence
        }) || sv.changes.iter().any(|c| c.starts_with(&format!("npc{}.", i)));
        let (recruited, selectable, in_party) = (sv.recruited(i), sv.selectable(i), sv.in_party(i));
        let portrait = sv.creature(Who::Npc(i)).cloned().and_then(|c| st.portrait_of(&c));
        rows.push(NpcRow {
            index: i as i32,
            name: s(st.npc_name(i)),
            has_portrait: portrait.is_some(),
            portrait: portrait.unwrap_or_default(),
            has_creature: has,
            recruited,
            selectable,
            in_party,
            influence: influence.unwrap_or(0).max(0) as i32,
            has_influence: influence.map_or(false, |v| v >= 0),
            changed,
        });
    }
    app.set_npcs(model(rows));
    let sv = st.save.as_ref().unwrap();
    app.set_party_size(sv.party_members().len() as i32);
    let xp = sv.party_int("PT_XP_POOL").unwrap_or(0);
    app.set_party_xp(xp as i32);
    app.set_party_xp_changed(st.original.as_ref().map_or(false, |o| o.party_int("PT_XP_POOL").unwrap_or(0) != xp));
}

/// A chapter Skip Ahead offers, in story order.
struct Chapter {
    index: i32,
    order: i64,
    // A side chapter (the ship's own quests, a trader's station) comes only when picked itself.
    side: bool,
    planet: String,
    title: String,
    summary: String,
    played: bool,
    // not played, but the save has been there (begun_chapters)
    begun: bool,
}

/// The game's chapters the editor has mapped (questdb::milestones), in story order, each
/// with what skipping to its end does and whether this save has played it (its journal
/// entries reached).
fn skip_chapters(st: &State) -> Vec<Chapter> {
    let Some(save) = st.save.as_ref() else { return Vec::new() };
    let done = kse_core::questdb::chapters_played_in(save);
    let titles = st.module_titles.get(&st.game);
    let mut out: Vec<Chapter> = kse_core::questdb::milestones_ref(st.game)
        .iter()
        .enumerate()
        .map(|(i, (planet, m))| {
            let order = m.get("order").and_then(|v| v.as_i64()).unwrap_or(if planet.eq_ignore_ascii_case("Endar Spire") { 1 } else { 99 });
            let played = done.get(i).copied().unwrap_or(false);
            let next = m.get("next_chapter").and_then(|n| n.get("module")).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let place = titles
                .and_then(|t| t.get(&next))
                .cloned()
                .or_else(|| st.data().map(|d| kse_core::map::module_title(&d, &next)))
                .unwrap_or(next);
            let rewards = kse_core::questdb::rewards_of(st.game, planet);
            let xp = rewards.as_ref().and_then(|r| r.get("xp_at_end")).and_then(|v| v.as_i64());
            let items = rewards.as_ref().map_or(0, |r| {
                r.get("drops").and_then(|d| d.as_array()).map_or(0, |d| d.len())
                    + r.get("containers").and_then(|c| c.as_array()).map_or(0, |c| c.len())
            });
            let title = m
                .get("chapter_title")
                .and_then(|v| v.as_str())
                .map(|t| t.to_string())
                .unwrap_or_else(|| format!("The end of the {}", planet));
            let mut parts = vec!["its quests finished".to_string()];
            if xp.is_some() {
                // filled in below, once the route's order is known
                parts.push("{XP} XP in all".to_string());
            }
            if items > 0 {
                parts.push("the loot of a full run".to_string());
            }
            let credits = |side: &str| rewards.as_ref().and_then(|r| r.get("credits")).and_then(|c| c.get(side)).and_then(|c| c.get("credits")).and_then(|v| v.as_i64());
            match (credits("light"), credits("dark")) {
                (Some(l), Some(d)) if l == d && l != 0 => parts.push(format!("{} credits", l)),
                (Some(l), Some(d)) if l != d => parts.push(format!("{} credits on the light side, {} on the dark", l, d)),
                _ => {}
            }
            let summary = format!(
                "The save becomes as if {} was played to its end: {}. Then, when the save loads, the game takes you on to {} the way the story does, with its cutscenes.",
                planet,
                parts.join(", "),
                place
            );
            let side = m.get("side_chapter").and_then(|v| v.as_bool()).unwrap_or(false);
            Chapter { index: i as i32, order, side, planet: planet.to_string(), title, summary, played, begun: false }
        })
        .collect();
    // the main chapters in the order this save plays them: the free planets as the save went
    // to them, then as picked on the galaxy map (side chapters keep their places after them)
    let route = story_route(st);
    let begun = begun_chapters(st);
    for c in out.iter_mut() {
        c.begun = begun.contains(&c.index);
    }
    for c in out.iter_mut().filter(|c| !c.side) {
        if let Some(k) = route.order.iter().position(|i| *i as i32 == c.index) {
            c.order = k as i64 + 1;
        }
    }
    out.sort_by_key(|c| c.order);
    // the XP a skip to each chapter ends with: the chapters played and those the skip plays
    // on the route, each counted once (the files' totals follow the timeline's order)
    let states = chapter_states(&out);
    let all = kse_core::questdb::milestones_ref(st.game);
    let played: Vec<String> = out.iter().filter(|c| c.played).map(|c| c.planet.clone()).collect();
    for k in 0..out.len() {
        if !out[k].summary.contains("{XP}") {
            continue;
        }
        let mut run = played.clone();
        run.extend(kse_core::questdb::skip_path(&states, out[k].index).into_iter().filter_map(|i| all.get(i as usize).map(|(p, _)| p.to_string())));
        let xp = kse_core::questdb::xp_after(st.game, &run);
        out[k].summary = out[k].summary.replace("{XP}", &thousands(xp));
    }
    out
}

/// The chapters (milestone indices) the open save has begun and not played: it has been to one
/// of their areas (questdb::been_to), or, for a free planet, the game's flag says it is done.
fn begun_chapters(st: &State) -> Vec<i32> {
    let Some(save) = st.save.as_ref() else { return Vec::new() };
    let played = kse_core::questdb::chapters_played_in(save);
    let route = story_route(st);
    let areas = been_areas(save);
    (0..kse_core::questdb::milestones_ref(st.game).len())
        .filter(|i| !played.get(*i).copied().unwrap_or(false))
        .filter(|i| route.groups.iter().position(|g| g.contains(i)).map_or(false, |g| route.kinds[g] < 2) || kse_core::questdb::been_to(st.game, *i, &areas))
        .map(|i| i as i32)
        .collect()
}

/// The areas a save has been to: those it keeps (Save::visited_modules) and the one it is in.
fn been_areas(save: &Save) -> Vec<String> {
    let mut v = save.visited_modules();
    v.push(save.module_name().to_ascii_lowercase());
    v
}

/// The open save's route through the story (questdb::story_route) with the order picked.
fn story_route(st: &State) -> kse_core::questdb::Route {
    let Some(save) = st.save.as_ref() else { return Default::default() };
    let globals = save.globals.as_ref().map(|g| kse_core::globals::list(&g.root)).unwrap_or_default();
    kse_core::questdb::story_route(st.game, &save.quests(), &kse_core::questdb::chapters_played_in(save), &globals, &been_areas(save), &st.route_plan)
}

/// The Skip Ahead confirmation for chapter `i` (or, with st.skip_stage, a stage in it).
fn ask_skip(u: &AppWindow, st: &mut State, i: i32) {
    let chapters = skip_chapters(st);
    let Some(c) = chapters.iter().find(|c| c.index == i) else { return };
    let path = skip_path(&chapters, i);
    let all = kse_core::questdb::milestones_ref(st.game); // not copied: only the chapters used are
    let jedi = path.iter().any(|k| all.get(*k as usize).map_or(false, |(_, m)| m.get("jedi_class").is_some()));
    let picked: Vec<serde_json::Value> = path.iter().filter_map(|k| all.get(*k as usize).map(|(_, m)| (*m).clone())).collect();
    let side = kse_core::questdb::side_matters(st.game, &picked);
    // The side the player leans to now comes first.
    let leans_dark = st.save.as_ref().and_then(|s| s.player()).map_or(false, |p| kse_core::creature::alignment(p) < 50);
    let mut summary = c.summary.clone();
    let mut title = format!("Skip to: {}", c.title);
    // a quest's stage inside the chapter: the story up to it, that quest as if played
    if let Some((tag, stage)) = st.skip_stage.clone() {
        let q = st.journal().iter().find(|q| q.tag.eq_ignore_ascii_case(&tag)).cloned();
        let name = q.as_ref().map(|q| q.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or(tag.clone());
        let text = q.as_ref().and_then(|q| q.stage(stage)).map(|x| x.text.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
        title = format!("Skip to: {}", name);
        summary = format!(
            "{}\n\nIn {}: this quest up to here as if played (its story values, journal, the people's own flags, its XP, items and credits), the main-story steps it needs, and the companions who join on the way. The chapter's other quests stay open; then the game takes you to where this happens.",
            if text.is_empty() { format!("Stage {}", stage) } else { text },
            c.planet
        );
    }
    summary.push_str("\n\nWorked out from the game's files. None of this data has been checked by playing it.");
    if path.len() > 1 {
        let names: Vec<String> = path.iter().filter(|k| st.skip_stage.is_none() || **k != i).filter_map(|k| chapters.iter().find(|x| x.index == *k)).map(|x| x.planet.clone()).collect();
        summary.push_str(&format!("\n\nEvery chapter before it on your route that this save has not played comes first, in the route's order: {}.", names.join(", ")));
    }
    let app = u.global::<App>();
    app.set_skip_asked(i);
    app.set_skip_title(s(title));
    app.set_skip_summary(s(summary));
    app.set_skip_jedi(jedi);
    app.set_skip_side(side);
    app.set_skip_dark(leans_dark as i32);
    // KotOR 2: the prestige class Kreia offers from level 15, by the XP the skip ends with;
    // the one matching the base class first (Guardian, Sentinel, Consular)
    // the XP the skip ends with on the route: the chapters played and those it plays
    let end_xp = {
        let mut run: Vec<String> = chapters.iter().filter(|c| c.played).map(|c| c.planet.clone()).collect();
        run.extend(picked.iter().filter_map(|m| m.get("planet").and_then(|p| p.as_str()).map(str::to_string)));
        Some(kse_core::questdb::xp_after(st.game, &run))
    };
    let prestige = match (st.save.as_ref(), st.data(), end_xp) {
        (Some(save), Some(data), Some(xp)) => kse_core::questdb::prestige_due(save, &data, xp.max(save.player().map(kse_core::creature::experience).unwrap_or(0))),
        _ => false,
    };
    let base = st.save.as_ref().and_then(|s| s.player()).and_then(|p| kse_core::creature::classes(p).first().map(|c| c.0));
    app.set_skip_prestige(prestige);
    // KotOR 2: offered once the skip reaches the masters' meeting, by which point a full
    // run can have finished the companions' conversation chains
    let train = st.game == kse_core::games::Game::K2 && picked.iter().any(|m| m.get("order").and_then(|v| v.as_i64()).map_or(false, |o| o >= 8));
    app.set_skip_train(train);
    app.set_skip_train_pick(0);
    app.set_skip_prestige_pick(match base { Some(3) => 0, Some(5) => 1, Some(4) => 2, _ => 3 });
    app.set_overlay(10);
    request_story_preview(u, st, false);
}

/// The Go Back confirmation for chapter `i` (a quest's tag and stage in it: the chapter's
/// start when the tag is empty): what is undone, with the story's items and credits.
fn ask_back(u: &AppWindow, st: &mut State, i: i32, tag: &str, stage: i32) {
    let chapters = skip_chapters(st);
    let Some(c) = chapters.iter().find(|c| c.index == i) else { return };
    let stage = (!tag.is_empty() && stage > 0).then(|| (tag.to_string(), stage as i64));
    let mut title = format!("Go back to: {}", c.title);
    let mut stage_text = String::new();
    if let Some((t, sid)) = &stage {
        let q = st.journal().iter().find(|q| q.tag.eq_ignore_ascii_case(t)).cloned();
        let name = q.as_ref().map(|q| q.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or(t.clone());
        let text = q.as_ref().and_then(|q| q.stage(*sid)).map(|x| x.text.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
        title = format!("Go back to: {}", name);
        if !text.is_empty() {
            title = format!("{} ({})", title, text.chars().take(80).collect::<String>());
            stage_text = format!("{}\n\n", text);
        }
    }
    let plan = kse_core::questdb::return_to(&chapter_states(&chapters), i);
    let later: Vec<String> = plan.undo.iter().filter_map(|k| chapters.iter().find(|x| x.index == *k)).map(|x| x.planet.clone()).collect();
    let summary = format!(
        "{}These chapters are taken back out of the save, newest first: {}.\n\nTheir story values return to what the chapters before left, their quests leave the journal, the companions they brought leave and the ones they took away return, their areas are built fresh by the game, and the XP returns to the end of the chapter before, the levels gained since taken off newest first.{}\n\nWhat the save cannot tell: things you did outside the mapped quests, what you bought, sold or used since.",
        stage_text,
        later.join(", "),
        if stage.is_some() { " Then the chapter plays up to the picked stage again, as Skip to Here does." } else { "" }
    );
    st.back_to = Some((i, stage));
    let app = u.global::<App>();
    app.set_back_title(s(title));
    app.set_back_summary(s(summary));
    app.set_overlay(11);
    request_story_preview(u, st, true);
}

fn current_story_plan(ui: &AppWindow, st: &State, back: bool) -> Option<StoryPlan> {
    let chapters = skip_chapters(st);
    let all = kse_core::questdb::milestones_ref(st.game);
    let milestone = |i: i32| all.get(i as usize).map(|(_, m)| (*m).clone());
    if back {
        let (i, stage) = st.back_to.as_ref()?;
        let plan = kse_core::questdb::return_to(&chapter_states(&chapters), *i);
        Some(StoryPlan::Back {
            alignment: ui.global::<App>().get_back_alignment() as i64,
            kept: plan.kept.iter().filter_map(|k| milestone(*k)).collect(),
            undo: plan.undo.iter().filter_map(|k| milestone(*k)).collect(),
            stage: stage.clone(),
            unfinished: plan.unfinished.iter().filter_map(|k| chapters.iter().find(|c| c.index == *k)).map(|c| c.planet.clone()).collect(),
        })
    } else {
        let app = ui.global::<App>();
        let dark = app.get_skip_side() && app.get_skip_dark() == 1;
        let prestige = app.get_skip_prestige().then(|| app.get_skip_prestige_pick()).filter(|k| (0..3).contains(k)).map(|k| {
            (if dark { kse_core::questdb::PRESTIGE_DARK } else { kse_core::questdb::PRESTIGE_LIGHT })[k as usize]
        });
        Some(StoryPlan::Skip {
            chapters: skip_path(&chapters, app.get_skip_asked()).into_iter().filter_map(milestone).collect(),
            stage: st.skip_stage.clone(),
            class: app.get_skip_jedi().then(|| app.get_skip_class().clamp(0, 2) as usize),
            dark,
            prestige,
            train: app.get_skip_train() && app.get_skip_train_pick() == 0,
        })
    }
}

fn clear_story_preview(ui: &AppWindow, st: &mut State) {
    // A fresh slot also invalidates a worker still finishing an older selection.
    st.story_result = Arc::new(Mutex::new(None));
    st.story_plan = None;
    st.story_prepared = None;
    let app = ui.global::<App>();
    app.set_story_preview_ready(false);
    app.set_story_preview_status(s("Choose a subchapter to see what will change."));
    app.set_story_values(model(Vec::new()));
    app.set_story_items(model(Vec::new()));
    app.set_story_grants(model(Vec::new()));
}

fn request_story_preview(ui: &AppWindow, st: &mut State, back: bool) {
    clear_story_preview(ui, st);
    let (Some(plan), Some(before), Some(data)) = (current_story_plan(ui, st, back), st.save.as_ref().cloned(), st.data()) else { return };
    st.story_plan = Some(plan.clone());
    ui.global::<App>().set_story_preview_status(s("Calculating the changes..."));
    let result = st.story_result.clone();
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        *result.lock().unwrap() = Some(plan.prepare(&before, &data));
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<App>().invoke_story_preview_finished();
            }
        });
    });
}

fn number_preview(rows: &mut Vec<StoryValue>, label: &str, before: i64, after: i64, always: bool) {
    if !always && before == after { return; }
    let difference = after - before;
    rows.push(StoryValue {
        label: s(label), before: s(thousands(before)), after: s(thousands(after)),
        difference: s(if difference == 0 { String::new() } else { format!("{}{}", if difference > 0 { "+" } else { "" }, thousands(difference)) }),
        change: difference.signum() as i32, changed: difference != 0,
    });
}

fn story_preview_rows(st: &State, after: &Save) -> (Vec<StoryValue>, Vec<Named>) {
    let Some(before) = st.save.as_ref() else { return (Vec::new(), Vec::new()) };
    let mut rows = Vec::new();
    let mut grants = Vec::new();
    let pc = |s: &Save, f: fn(&Struct) -> i64| s.player().map(f).unwrap_or(0);
    number_preview(&mut rows, "Experience", pc(before, creature::experience), pc(after, creature::experience), true);
    number_preview(&mut rows, "Credits", before.party_int("PT_GOLD").unwrap_or(0), after.party_int("PT_GOLD").unwrap_or(0), true);
    number_preview(&mut rows, "Level", pc(before, creature::level), pc(after, creature::level), true);
    let recruited = |s: &Save| (0..s.npc_count()).filter(|i| s.recruited(*i)).count() as i64;
    number_preview(&mut rows, "Companions", recruited(before), recruited(after), true);
    number_preview(&mut rows, "Alignment", pc(before, creature::alignment), pc(after, creature::alignment), false);
    for (field, label) in [("PT_ITEM_COMPONEN", "Components"), ("PT_ITEM_CHEMICAL", "Chemicals"), ("PT_XP_POOL", "Party experience")] {
        number_preview(&mut rows, label, before.party_int(field).unwrap_or(0), after.party_int(field).unwrap_or(0), false);
    }
    let (Some(cat), Some(data)) = (st.catalog(), st.data()) else { return (rows, grants) };
    let npc_name = |save: &Save, i: usize| story_preview::companion_name(save, &data, i, cat.npc_names.get(i).map(String::as_str).unwrap_or(""));
    let same_npc = |i: usize| {
        let tag = |save: &Save| save.creature(Who::Npc(i)).and_then(|c| c.string("Tag")).map(|t| t.to_ascii_lowercase());
        tag(before) == tag(after)
    };
    for i in 0..before.npc_count().max(after.npc_count()) {
        if before.recruited(i) && (!after.recruited(i) || !same_npc(i)) {
            grants.push(Named { id: -1, name: s(npc_name(before, i)), note: s("Companion"), ..Default::default() });
        }
        if after.recruited(i) && (!before.recruited(i) || !same_npc(i)) {
            grants.push(Named { id: 1, name: s(npc_name(after, i)), note: s("Companion"), ..Default::default() });
        }
    }
    // Class changes and the abilities granted by them matter as much as the level.
    let whos = std::iter::once(Who::Player).chain(after.npcs().into_iter().filter(|i| before.recruited(*i) && after.recruited(*i) && same_npc(*i)).map(Who::Npc));
    for who in whos {
        let (Some(a), Some(b)) = (before.creature(who), after.creature(who)) else { continue };
        let prefix = match who { Who::Player => String::new(), Who::Npc(i) => format!("{}: ", npc_name(after, i)) };
        if who != Who::Player {
            number_preview(&mut rows, &format!("{}experience", prefix), creature::experience(a), creature::experience(b), false);
            number_preview(&mut rows, &format!("{}level", prefix), creature::level(a), creature::level(b), false);
            if let Who::Npc(i) = who {
                if let (Some(old), Some(new)) = (before.influence(i), after.influence(i)) {
                    if old >= 0 && new >= 0 { number_preview(&mut rows, &format!("{}influence", prefix), old, new, false); }
                }
            }
        }
        let (health_a, max_health_a) = creature::health(a);
        let (health_b, max_health_b) = creature::health(b);
        let (force_a, max_force_a) = creature::force(a, before.game);
        let (force_b, max_force_b) = creature::force(b, after.game);
        for (label, old, new) in [("Health", health_a, health_b), ("Max Health", max_health_a, max_health_b), ("Force", force_a, force_b), ("Max Force", max_force_a, max_force_b)] {
            number_preview(&mut rows, &format!("{}{}", prefix, label), old, new, false);
        }
        let class_names = |c: &Struct| creature::classes(c).into_iter().map(|(id, _)| cat.class_name(id)).collect::<Vec<_>>().join(" / ");
        let (old_class, new_class) = (class_names(a), class_names(b));
        if old_class != new_class {
            rows.push(StoryValue { label: s(format!("{}Class", prefix)), before: s(old_class), after: s(new_class), changed: true, ..Default::default() });
        }
        for (label, field) in [("Strength", "Str"), ("Dexterity", "Dex"), ("Constitution", "Con"), ("Intelligence", "Int"), ("Wisdom", "Wis"), ("Charisma", "Cha")] {
            number_preview(&mut rows, &format!("{}{}", prefix, label), creature::attribute(a, field), creature::attribute(b, field), false);
        }
        for (i, skill) in cat.skills.iter().enumerate() {
            number_preview(&mut rows, &format!("{}{}", prefix, skill.name), creature::skill(a, i), creature::skill(b, i), false);
        }
        for (old, new, power) in [(creature::feats(a), creature::feats(b), false), (creature::powers(a), creature::powers(b), true)] {
            for (ids, other, sign) in [(&new, &old, 1), (&old, &new, -1)] {
                for id in ids.iter().filter(|id| !other.contains(id)) {
                    let name = if power { cat.power_name(*id) } else { cat.feat_name(*id) };
                    grants.push(Named { id: sign, name: s(format!("{}{}", prefix, name)), note: s(if power { "Force power" } else { "Feat" }), ..Default::default() });
                }
            }
        }
    }
    (rows, grants)
}

fn finish_story_preview(ui: &AppWindow, st: &mut State) {
    let result = st.story_result.lock().unwrap().take();
    if st.story_plan.is_none() || !matches!(ui.global::<App>().get_overlay(), 10 | 11 | 12) { return; }
    let Some(result) = result else { return };
    let prepared = match result {
        Ok(p) => p,
        Err(error) => {
            ui.global::<App>().set_story_preview_status(s(format!("This point could not be prepared: {}", error)));
            return;
        }
    };
    let (values, grants) = story_preview_rows(st, &prepared.save);
    let old = st.save.as_ref().map(story_preview::accessible_items).unwrap_or_default();
    let mut changes = story_preview::inventory_changes(&old, &story_preview::accessible_items(&prepared.save));
    changes.sort_by_cached_key(|r| st.item_name(&r.item).to_lowercase());
    let items = changes.into_iter().map(|r| {
        let name = st.item_name(&r.item);
        let icon = st.item_icon(&r.item);
        StoryItem { name: s(name), has_icon: icon.is_some(), icon: icon.unwrap_or_default(), gained: r.difference > 0,
            difference: s(format!("{}{}", if r.difference > 0 { "+" } else { "" }, thousands(r.difference))) }
    }).collect();
    let app = ui.global::<App>();
    app.set_story_values(model(values));
    app.set_story_grants(model(grants));
    app.set_story_items(model(items));
    app.set_story_preview_status(s(""));
    app.set_story_preview_ready(true);
    st.story_prepared = Some(prepared);
}

fn take_story_preview(ui: &AppWindow, st: &mut State, back: bool) -> Option<PreparedStory> {
    if st.story_plan != current_story_plan(ui, st, back) || !ui.global::<App>().get_story_preview_ready() {
        request_story_preview(ui, st, back);
        return None;
    }
    let prepared = st.story_prepared.take()?;
    clear_story_preview(ui, st);
    Some(prepared)
}

fn chapter_states(chapters: &[Chapter]) -> Vec<kse_core::questdb::ChapterState> {
    chapters.iter().map(|c| kse_core::questdb::ChapterState { index: c.index, order: c.order, side: c.side, played: c.played, begun: c.begun }).collect()
}

fn skip_path(chapters: &[Chapter], index: i32) -> Vec<i32> {
    kse_core::questdb::skip_path(&chapter_states(chapters), index)
}

/// What a skip or a return left undone, for the player to know. Reports of what went well are
/// not shown (2026-10-25, the maintainer: no success screen).
fn story_problems(ui: &AppWindow, notes: &[String]) {
    const FAILED: [&str; 7] = ["not found", "not recruited", "could not", "No form taught", "no story switch", "left as they are", "is undone too"];
    let problems: Vec<&str> = notes.iter().map(String::as_str).filter(|n| FAILED.iter().any(|f| n.contains(f))).collect();
    if !problems.is_empty() {
        message(ui, "Done, except for this", &problems.join("\n"));
    }
}

/// Another save of the open save's game, for the timeline.
#[derive(Clone)]
struct TimelineOther {
    // index into the save list
    index: usize,
    // the furthest main chapter it has played on its route, a milestone index (-1: none yet)
    reached: i32,
    seconds: i64,
    // the player character's name
    who: String,
    name: String,
    area: String,
    // "Revan · Level 11 · 55,480 XP"
    level: String,
    // "After Manaan"
    story: String,
}

/// The furthest main chapter a save has played on its own route, as a milestone index, and
/// its name. A free planet earlier on the route may still be open.
fn furthest_played(save: &Save) -> Option<(usize, String)> {
    let quests = save.quests();
    let done = kse_core::questdb::chapters_played_in(save);
    let globals = save.globals.as_ref().map(|g| kse_core::globals::list(&g.root)).unwrap_or_default();
    let route = kse_core::questdb::story_route(save.game, &quests, &done, &globals, &been_areas(save), &[]);
    let all = kse_core::questdb::milestones_ref(save.game);
    route.order.iter().filter(|i| done.get(**i).copied().unwrap_or(false)).last().map(|i| (*i, all[*i].0.to_string()))
}

/// Every other save of the open save's game (any character), each with where it is in the
/// story. Read away from the window the first time the timeline shows (empty until then);
/// when done, timeline-ready fills the timeline again.
fn timeline_saves(ui: &AppWindow, st: &mut State) -> Vec<TimelineOther> {
    if let Some(t) = &st.timeline_saves {
        return t.clone();
    }
    if let Some(t) = st.timeline_load.lock().unwrap().take() {
        st.timeline_loading = false;
        st.timeline_saves = Some(t.clone());
        return t;
    }
    if !st.timeline_loading {
        st.timeline_loading = true;
        let here = st.saves.get(st.selected).map(|(f, _)| f.path.clone());
        let game = st.game;
        let list: Vec<(usize, PathBuf, Summary)> = st
            .saves
            .iter()
            .enumerate()
            .filter(|(_, (f, sm))| Some(&f.path) != here.as_ref() && sm.game.map_or(true, |g| g == game))
            .map(|(i, (f, sm))| (i, f.path.clone(), sm.clone()))
            .collect();
        let slot = st.timeline_load.clone();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let mut out = Vec::new();
            for (i, path, sm) in list {
                if is_xbox_save(&path) {
                    continue;
                }
                let Ok(other) = Save::open(&path) else { continue };
                let Some(pc) = other.player() else { continue };
                if other.game != game {
                    continue;
                }
                let furthest = furthest_played(&other);
                out.push(TimelineOther {
                    index: i,
                    reached: furthest.as_ref().map_or(-1, |(k, _)| *k as i32),
                    seconds: sm.seconds,
                    who: creature::name(pc),
                    name: sm.name.clone(),
                    area: sm.area.clone(),
                    level: format!("{}  ·  Level {}  ·  {} XP", creature::name(pc), creature::level(pc), thousands(creature::experience(pc))),
                    story: furthest.map_or("At the start of the story".to_string(), |(_, p)| format!("Story: after {}", p)),
                });
            }
            *slot.lock().unwrap() = Some(out);
            let _ = slot;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.global::<App>().invoke_timeline_ready();
                }
            });
        });
    }
    Vec::new()
}

/// Forgets the other saves read for the timeline (another save opened, the save changed);
/// a read still running finishes into a slot no one reads.
fn forget_timeline_saves(st: &mut State) {
    st.timeline_saves = None;
    st.timeline_load = Arc::new(Mutex::new(None));
    st.timeline_loading = false;
}

/// A lane of the galaxy map from `a` to `b` (shares of the map): a quadratic curve bending a
/// little to one side, as its own box (the curve's bounds) and its three points as shares of
/// that box. Slint fits a path's view into its element keeping the view's shape, so the map
/// draws each lane in a view the size of its box, in pixels.
fn lane(a: (f32, f32), b: (f32, f32), lit: bool) -> GalaxyLane {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let bend = 0.12;
    let c = ((a.0 + b.0) / 2.0 - dy * bend, (a.1 + b.1) / 2.0 + dx * bend);
    // the curve's reach on each axis: its ends, and its turning point when it has one
    let reach = |p0: f32, p1: f32, p2: f32| {
        let mut lo = p0.min(p2);
        let mut hi = p0.max(p2);
        let den = p0 - 2.0 * p1 + p2;
        if den.abs() > 1e-6 {
            let t = (p0 - p1) / den;
            if t > 0.0 && t < 1.0 {
                let v = (1.0 - t) * (1.0 - t) * p0 + 2.0 * (1.0 - t) * t * p1 + t * t * p2;
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo, (hi - lo).max(0.002))
    };
    let (x, w) = reach(a.0, c.0, b.0);
    let (y, h) = reach(a.1, c.1, b.1);
    let at = |p: (f32, f32)| ((p.0 - x) / w, (p.1 - y) / h);
    let (p0, p1, p2) = (at(a), at(c), at(b));
    GalaxyLane { x, y, w, h, ax: p0.0, ay: p0.1, cx: p1.0, cy: p1.1, bx: p2.0, by: p2.1, lit }
}

/// A soft white glow, brightest in the middle and gone at the edge: the galaxy map tints it
/// for the halos round its stops and the light at the galaxy's core (Slint's software renderer
/// draws a radial gradient as a square, so the glow is a picture).
fn glow_image() -> Image {
    const N: usize = 128;
    let mut px = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let u = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = if r >= 1.0 { 0.0 } else { (1.0 - r).powf(2.2) };
            let o = (y * N + x) * 4;
            px[o] = 255;
            px[o + 1] = 255;
            px[o + 2] = 255;
            px[o + 3] = (a * 255.0) as u8;
        }
    }
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&px, N as u32, N as u32))
}

/// The Timeline tab's galaxy map: the main chapters as stops on the map in the order this save
/// plays them, lit lanes along the part of the route behind the save and faint ones ahead, the
/// open save's place (the chapter whose areas it is in, or the last one played when it is on
/// the Ebon Hawk), the game's other saves at the furthest stop each has played, and the rows
/// of the chapter whose pop-up is open (st.timeline_pick). The saves at one stop are one
/// marker: a point for one save, a larger point with their number for more, which opens into
/// a ring of them, grouped by character and in the order of their time played.
fn fill_skip(ui: &AppWindow, st: &mut State) {
    // the other saves: read away from the window once per save opened (empty until ready)
    let others = timeline_saves(ui, st);
    let chapters = skip_chapters(st);
    let main: Vec<&Chapter> = chapters.iter().filter(|c| !c.side).collect();
    let route = story_route(st);
    let next = main.iter().position(|c| !c.played);
    let all = kse_core::questdb::milestones_ref(st.game);
    // where the save is: the chapter whose areas hold its area; on the ship (or anywhere no
    // chapter names), at the last stop played
    let module = st.save.as_ref().map(|s| s.place()).unwrap_or_default();
    let in_chapter = kse_core::questdb::chapter_of_module(st.game, &module);
    let here_k = in_chapter.and_then(|i| main.iter().position(|c| c.index == i as i32));
    // a side chapter's own place (Yavin Station) is not the ship
    let at_side = in_chapter.filter(|i| chapters.iter().any(|c| c.side && c.index == *i as i32)).map(|i| all[i].0.to_string()).filter(|p| !p.eq_ignore_ascii_case("Ebon Hawk"));
    let aboard = here_k.is_none() && at_side.is_none();
    let here_k = if at_side.is_some() { None } else { here_k.or_else(|| main.iter().rposition(|c| c.played)).or(if main.is_empty() { None } else { Some(0) }) };
    // the chapter the save is in: the one it stands in, unplayed. Begun: a chapter not played
    // that the save has been to (one of its areas, or KotOR II's marker for it); a
    // free planet left half done is begun, not the save's place.
    let begun = begun_chapters(st);
    let current_k = here_k.filter(|k| !main[*k].played);
    let game = st.game;
    let place = |planet: &str| MAP_STOPS.iter().find(|m| m.0 == game && m.1.eq_ignore_ascii_case(planet));
    let data = st.data();
    // the free planets by group, named as the order row shows them
    let free: Vec<String> = route.groups.iter().map(|g| g.iter().map(|i| all[*i].0.to_string()).collect::<Vec<_>>().join(" and ")).collect();
    let mut stops: Vec<GalaxyStop> = Vec::new();
    for (k, c) in main.iter().enumerate() {
        let spot = place(&c.planet);
        let (x, y) = spot.map_or(((k as f32 + 0.5) / main.len().max(1) as f32, 0.5), |m| (m.2, m.3));
        let globe = match (spot.filter(|m| !m.4.is_empty()), data.as_ref()) {
            (Some(m), Some(d)) => picture(&mut st.portraits, &st.art, globe_key(m.4, false), || globe_pixels(d, m.4, false)),
            _ => None,
        };
        // a free planet moves among the free planets of its own kind (done, begun, not visited)
        let g = route.groups.iter().position(|g| g.first() == Some(&(c.index as usize)));
        let kind = g.map_or(-1, |g| route.kinds[g] as i32);
        let earlier = g.map_or(false, |g| g > 0 && (route.kinds[g - 1] == 0) == (kind == 0));
        let later = g.map_or(false, |g| g + 1 < route.groups.len() && (route.kinds[g + 1] == 0) == (kind == 0));
        stops.push(GalaxyStop {
            index: c.index,
            stop: s((k + 1).to_string()),
            planet: s(&c.planet),
            title: s(&c.title),
            state: if c.played { 0 } else if Some(k) == current_k { 1 } else if begun.contains(&c.index) { 4 } else if Some(k) == next { 3 } else { 2 },
            x,
            y,
            has_globe: globe.is_some(),
            globe: globe.unwrap_or_default(),
            look: spot.map_or(0, |m| m.5 as i32),
            free: g.is_some(),
            chip: s(g.map_or(String::new(), |g| free[g].clone())),
            earlier,
            later,
            kind,
            here: Some(k) == here_k,
        });
    }
    // the lanes: from each stop to the next, curving a little to one side as hyperspace routes
    // are drawn; lit where the save has gone (to a stop played, or to the one it stands in)
    let mut lanes: Vec<GalaxyLane> = Vec::new();
    for k in 1..stops.len() {
        let (a, b) = (&stops[k - 1], &stops[k]);
        let lit = b.state == 0 || b.state == 4 || (Some(k) == here_k && !aboard);
        lanes.push(lane((a.x, a.y), (b.x, b.y), lit));
    }
    // the other saves, at the furthest stop each has played (none yet: at the first)
    let shots = st.shots.lock().unwrap().clone();
    let mut saves: Vec<TimelineSave> = Vec::new();
    let mut groups: Vec<TimelineGroup> = Vec::new();
    for (k, stop) in stops.iter().enumerate() {
        let mut at: Vec<&TimelineOther> = others.iter().filter(|o| if o.reached < 0 { k == 0 } else { o.reached == stop.index }).collect();
        if at.is_empty() {
            continue;
        }
        at.sort_by(|a, b| a.who.to_lowercase().cmp(&b.who.to_lowercase()).then(a.seconds.cmp(&b.seconds)));
        groups.push(TimelineGroup { x: stop.x, y: stop.y, count: at.len() as i32, start: saves.len() as i32 });
        let group = groups.len() as i32 - 1;
        for (slot, o) in at.into_iter().enumerate() {
            let shot = st.saves.get(o.index).and_then(|(f, _)| shots.get(&f.path).cloned().flatten());
            saves.push(TimelineSave {
                index: o.index as i32,
                group,
                slot: slot as i32,
                name: s(&o.name),
                area: s(&o.area),
                time: s(played(o.seconds)),
                level: s(&o.level),
                story: s(&o.story),
                has_shot: shot.is_some(),
                shot: shot.map(|i| image(&i)).unwrap_or_default(),
            });
        }
    }
    let free_note = if free.is_empty() {
        String::new()
    } else {
        format!(
            "These can be played in any order: set yours and Skip Ahead follows it.{}",
            if st.game == Game::K1 { " The Leviathan comes after the third, as in the game." } else { "" }
        )
    };
    let here_text = match here_k.map(|k| &stops[k]) {
        _ if at_side.is_some() => format!("At {}", at_side.clone().unwrap_or_default()),
        Some(h) if aboard => format!("Aboard the Ebon Hawk, last at {}", h.planet),
        Some(h) => format!("On {}", h.planet),
        None => String::new(),
    };

    // the side chapters (the Ebon Hawk's own quests, Yavin Station): off the route, as they
    // come only when picked themselves. The Ebon Hawk waits beside the save's stop.
    // the Ebon Hawk beside the save's stop, or beside the side chapter's place it is at
    let side_spot = at_side.as_deref().and_then(|p| place(p)).map(|m| (m.2, m.3));
    let (hx, hy) = side_spot.or(here_k.map(|k| (stops[k].x, stops[k].y))).unwrap_or((0.5, 0.5));
    let mut sides: Vec<GalaxyStop> = Vec::new();
    for c in chapters.iter().filter(|c| c.side) {
        let spot = place(&c.planet);
        let hawk = spot.map_or(false, |m| m.5 == 1);
        let (x, y) = if hawk { (hx, hy) } else { spot.map_or((0.95, 0.9), |m| (m.2, m.3)) };
        let globe = match (spot.filter(|m| !m.4.is_empty()), data.as_ref()) {
            (Some(m), Some(d)) => picture(&mut st.portraits, &st.art, globe_key(m.4, false), || globe_pixels(d, m.4, false)),
            _ => None,
        };
        sides.push(GalaxyStop {
            index: c.index,
            stop: s(""),
            planet: s(&c.planet),
            title: s(&c.title),
            state: if c.played { 0 } else { 2 },
            x,
            y,
            has_globe: globe.is_some(),
            globe: globe.unwrap_or_default(),
            look: if hawk { 1 } else { 0 },
            free: false,
            chip: s(""),
            earlier: false,
            later: false,
            kind: if hawk && hx < 0.2 { 1 } else { -1 },
            here: at_side.as_deref().map_or(false, |p| p.eq_ignore_ascii_case(&c.planet)),
        });
    }

    // the open pop-up's rows
    let mut rows: Vec<SkipRow> = Vec::new();
    let mut height = 0.0f32;
    let mut kicker = String::new();
    let mut title = String::new();
    let mut note = String::new();
    if let Some(c) = st.timeline_pick.and_then(|i| chapters.iter().find(|c| c.index == i)) {
        let k = main.iter().position(|x| x.index == c.index);
        let current = k.is_some() && (k == current_k || begun.contains(&c.index));
        let row = |depth: i32, title: String, note: &str, state: i32, index: i32, key: String, tag: &str, stage: i32, open: bool| SkipRow {
            index,
            order: 0,
            planet: s(""),
            title: s(title),
            note: s(note),
            state,
            depth,
            open,
            key: s(key),
            tag: s(tag),
            stage,
        };
        // Its beginning: gone back to when the save has played it or is in it; otherwise
        // skipped to by playing the stop before it on the route (none when that one is
        // played: the save is at its beginning already).
        let begin = if c.played || current {
            Some((0, c.index))
        } else {
            k.and_then(|k| k.checked_sub(1)).filter(|p| !main[*p].played).map(|p| (2, main[p].index))
        };
        if let Some((state, index)) = begin {
            rows.push(row(5, "The beginning of the chapter".into(), "", state, index, String::new(), "", 0, false));
            height += 44.0;
        }
        let journal = st.journal();
        let have = st.save.as_ref().map(|s| s.quests()).unwrap_or_default();
        let stage_of = |tag: &str| have.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map_or(-1, |(_, v)| *v);
        // The chapter's own quests: the main story first (its milestone's journal), then the
        // side quests by name; their names and stage texts from the player's own game.
        for (tag, main_story) in kse_core::questdb::chapter_quests(st.game, &c.planet) {
            let Some(q) = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(&tag)) else { continue };
            let now = stage_of(&tag);
            let qkey = format!("q{}:{}", c.index, tag);
            let qopen = st.skip_open.contains(&qkey);
            let name = if q.name.trim().is_empty() { tag.clone() } else { q.name.clone() };
            rows.push(row(1, name, if main_story { "main" } else { "" }, 3, c.index, qkey, &tag, 0, qopen));
            height += if main_story { 56.0 } else { 44.0 };
            if !qopen {
                continue;
            }
            for stg in &q.stages {
                let text = stg.text.split_whitespace().collect::<Vec<_>>().join(" ");
                let state = if c.played || now >= stg.id { 0 } else { 2 };
                let short = text.split(". ").next().unwrap_or(&text);
                let short = if short.chars().count() > 110 { format!("{}...", short.chars().take(107).collect::<String>()) } else { short.to_string() };
                rows.push(row(2, if short.is_empty() { format!("Stage {}", stg.id) } else { short }, "", state, c.index, String::new(), &tag, stg.id as i32, false));
                height += 64.0;
            }
        }
        // Its end: skipped to when ahead; when played, gone back to by taking the chapter
        // after it back out (if the save has played that one or is in it).
        let end = if !c.played {
            Some((2, c.index))
        } else {
            k.and_then(|k| main.get(k + 1).filter(|f| f.played || f.begun)).map(|f| (0, f.index))
        };
        if let Some((state, index)) = end {
            rows.push(row(6, "The whole chapter, to its end".into(), "", state, index, String::new(), "", 0, false));
            height += 44.0;
        }
        kicker = match k {
            Some(k) => format!("STOP {}  ·  {}", k + 1, if c.played { "BEHIND YOU" } else if Some(k) == current_k { "YOU ARE IN IT" } else if current { "BEGUN, NOT FINISHED" } else if Some(k) == next { "YOUR NEXT STOP" } else { "AHEAD OF YOU" }),
            None => format!("SIDE CHAPTER  ·  {}", if c.played { "PLAYED" } else { "NOT PLAYED" }),
        };
        title = c.planet.clone();
        note = if c.played {
            "Choose where to go back to, from the chapter's beginning to its end. What came after it is taken back out of the save.".into()
        } else if current {
            "Choose where to go: back to a point you have passed, or ahead to one you have not. Skipping ahead plays the story up to it for you: quests, XP, loot and companions.".into()
        } else {
            "Choose where to skip to, from the chapter's beginning to its end. Everything before it on your route is played for you: quests, XP, loot and companions.".into()
        };
    }
    let app = ui.global::<App>();
    app.set_gm_stops(model(stops));
    app.set_gm_lanes(model(lanes));
    app.set_gm_aboard(aboard);
    app.set_gm_here_text(s(here_text));
    app.set_gm_free_note(s(free_note));
    app.set_gm_reordered(!st.route_plan.is_empty());
    app.set_tl_sides(model(sides));
    app.set_tl_saves(model(saves));
    app.set_tl_groups(model(groups));
    // the row picked, found again by what it is (opening a quest moves the rows)
    let picked = st.pop_pick.as_ref().and_then(|(d, i, tag, stage)| rows.iter().position(|r| r.depth == *d && r.index == *i && r.tag.as_str() == tag && r.stage == *stage));
    app.set_pop_picked(picked.map_or(-1, |k| k as i32));
    app.set_pop_action(picked.map_or(0, |i| if rows[i].state == 0 { 2 } else { 1 }));
    app.set_pop_place(s(&title));
    app.set_pop_kicker(s(kicker));
    app.set_pop_title(s(title));
    app.set_pop_note(s(note));
    app.set_skip_height(height);
    app.set_skip_rows(model(rows));
}

fn fill_story(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    app.set_story_section(st.story);
    // only the section on screen (2026-10-24): every edit filled all six, the timeline and
    // the globals list the heaviest; switching sections comes back here for the new one
    match st.story {
        0 => {
            fill_globals(ui, st);
            fill_more(ui, st); // the save's name, time played and cheat mark sit above the globals
        }
        1 => fill_area(ui, st),
        5 => fill_skip(ui, st),
        _ => fill_more(ui, st), // creatures, quests, galaxy map and Pazaak
    }
}

fn fill_globals(ui: &AppWindow, st: &State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let Some(g) = save.globals.as_ref() else {
        app.set_variables(model(Vec::new()));
        app.set_globals_note(s("This save has no global variables file."));
        return;
    };
    let all = globals::list(&g.root);
    let orig = st.original.as_ref().and_then(|o| o.globals.as_ref()).map(|g| globals::list(&g.root)).unwrap_or_default();
    let other = st.compare.as_ref().map(|(_, l)| l);
    let (query, only) = (&st.global_filter.0, st.global_filter.1);
    ui.global::<App>().set_globals_query(s(query));
    ui.global::<App>().set_globals_changed_only(only);
    let mut rows = Vec::new();
    for (pos, v) in all.iter().enumerate() {
        let changed = orig.get(pos).map_or(false, |o| o.value != v.value);
        let theirs = other.and_then(|l| l.iter().find(|o| o.kind == v.kind && o.name == v.name));
        let differs = theirs.map_or(false, |t| t.value != v.value);
        if only && !(if other.is_some() { differs } else { changed }) {
            continue;
        }
        if !query.trim().is_empty() && !v.name.to_lowercase().contains(&query.trim().to_lowercase()) {
            continue;
        }
        if rows.len() >= 200 {
            break;
        }
        let (on, number, text) = match &v.value {
            GlobalValue::Bool(b) => (*b, 0, String::new()),
            GlobalValue::Number(n) => (false, *n as i32, String::new()),
            GlobalValue::Text(t) => (false, 0, t.clone()),
        };
        rows.push(GlobalRow {
            index: pos as i32,
            name: s(&v.name),
            kind: match v.kind {
                Kind::Boolean => 0,
                Kind::Number => 1,
                Kind::String => 2,
            },
            on,
            number,
            text: s(text),
            changed,
            other: s(theirs.map(|t| if t.value != v.value { format!("theirs: {}", t.value.display()) } else { "same".into() }).unwrap_or_default()),
        });
    }
    let shown = rows.len();
    app.set_variables(model(rows));
    let total = all.len();
    app.set_compare_name(s(st.compare.as_ref().map(|c| c.0.clone()).unwrap_or_default()));
    let mut note = format!(
        "{} switches, {} numbers and {} texts that the game's scripts read. {} shown.",
        all.iter().filter(|g| g.kind == Kind::Boolean).count(),
        all.iter().filter(|g| g.kind == Kind::Number).count(),
        all.iter().filter(|g| g.kind == Kind::String).count(),
        if shown >= 200 && shown < total { "The first 200".to_string() } else { shown.to_string() }
    );
    if let Some((name, _)) = &st.compare {
        note.push_str(&format!(" Compared with {}.", name));
    }
    app.set_globals_note(s(note));
}

fn fill_area(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let Some(git) = save.area_git().map(|g| g.root.clone()) else {
        app.set_doors(model(Vec::new()));
        app.set_boxes(model(Vec::new()));
        app.set_area_note(s("The area of this save could not be read."));
        return;
    };
    // the same area as opened (after a move the save is in another, which nothing marks)
    let same_area = |o: &&Save| st.save.as_ref().map_or(false, |s| s.module_name().eq_ignore_ascii_case(o.module_name()));
    let orig_doors = st.original.as_ref().filter(same_area).and_then(|o| o.area_git()).map(|g| area::doors(&g.root));
    let doors: Vec<DoorRow> = area::doors(&git)
        .into_iter()
        .map(|d| {
            let o = orig_doors.as_ref().and_then(|v| v.get(d.index));
            let mut note = Vec::new();
            if d.key_required {
                note.push(format!("needs the key {}", d.key));
            } else if d.locked {
                note.push(format!("Security {}", d.lock_dc));
            }
            if d.plot {
                note.push("story door".into());
            }
            DoorRow {
                index: d.index as i32,
                tag: s(if d.tag.is_empty() { format!("Door {}", d.index + 1) } else { d.tag.clone() }),
                note: s(note.join(", ")),
                locked: d.locked,
                open: d.open,
                changed: o.map_or(false, |o| o.locked != d.locked || o.open != d.open),
            }
        })
        .collect();
    app.set_doors(model(doors));
    let data = st.data();
    let namer = |p: &Struct| -> String {
        let n = match (p.get("LocName"), &data) {
            (Some(Value::LocString(l)), Some(d)) => d.loc(l),
            _ => String::new(),
        };
        if n.trim().is_empty() {
            p.string("Tag").unwrap_or_default()
        } else {
            n
        }
    };
    let boxes = area::containers(&git, &namer);
    let rows: Vec<BoxRow> = boxes
        .iter()
        .enumerate()
        .map(|(_, b)| BoxRow {
            index: b.index as i32,
            name: s(&b.name),
            note: s(format!(
                "{}{} item{}{}",
                if b.list == area::STORES { "store, " } else { "" },
                b.items,
                if b.items == 1 { "" } else { "s" },
                if b.locked { ", locked" } else { "" }
            )),
        })
        .collect();
    app.set_boxes(model(rows));
    let selected = st.selected_box.and_then(|(l, i)| boxes.iter().position(|b| b.list == l && b.index == i));
    app.set_selected_box(selected.map_or(-1, |p| p as i32));
    let items: Vec<Struct> = st
        .selected_box
        .and_then(|(l, i)| area::container(&git, l, i))
        .and_then(|c| c.list("ItemList").cloned())
        .unwrap_or_default();
    let orig_items: Vec<Struct> = st
        .selected_box
        .and_then(|(l, i)| st.original.as_ref().filter(same_area).and_then(|o| o.area_git()).and_then(|g| area::container(&g.root, l, i).cloned()))
        .and_then(|c| c.list("ItemList").cloned())
        .unwrap_or_default();
    let module = save.nfo.root.string("AREANAME").unwrap_or_default();
    let item_rows = item_rows(st, &items, &orig_items);
    app.set_box_items(model(item_rows));
    app.set_area_note(s(format!(
        "{}: what the game finds when the save loads. Story scripts may lock a door again.",
        module
    )));
}

/// The galaxy map's planets: the game's own planet texture (what its galaxy map wraps on
/// each planet model) by planetary.2da label, and where the planet stands on the editor's
/// map, as a share of its width and height, spread as the game's own map spreads them.
const GLOBES: &[(Game, &str, &str, f32, f32)] = &[
    (Game::K1, "Korriban", "Gui_korriban_1", 0.55, 0.04),
    (Game::K1, "Dantooine", "Gui_Dantooine_1", 0.27, 0.22),
    (Game::K1, "Manaan", "Gui_Manaan_1", 0.86, 0.20),
    (Game::K1, "Taris", "Gui_Taris_1", 0.04, 0.50),
    (Game::K1, "Unknown_World", "Gui_unknown_1", 0.50, 0.46),
    (Game::K1, "Live_Planet_01", "GUI_yavin_1", 0.96, 0.60),
    (Game::K1, "Tatooine", "Gui_Tatooine_1", 0.72, 0.74),
    (Game::K1, "Kashyyyk", "Gui_Kashyyyk_1", 0.26, 0.78),
    (Game::K2, "Peragus", "Gui_peragus", 0.12, 0.30),
    (Game::K2, "Telos", "Gui_telos", 0.24, 0.62),
    (Game::K2, "Citadel_Station", "Gui_telos_st", 0.36, 0.80),
    (Game::K2, "Nar_Shaddaa", "Gui_narshada", 0.44, 0.20),
    (Game::K2, "Malachor_V", "Gui_malachor", 0.40, 0.50),
    (Game::K2, "Dantooine", "Gui_Dantooine_1", 0.62, 0.40),
    (Game::K2, "Dxun", "Gui_dxun", 0.62, 0.74),
    (Game::K2, "Onderon", "Gui_onderon", 0.76, 0.60),
    (Game::K2, "Korriban", "Gui_korriban_1", 0.82, 0.18),
    (Game::K2, "M4_78", "Gui_M4-78", 0.90, 0.82),
];

/// The story's chapters on the Timeline tab's galaxy map: where each stands (a share of the map's
/// width and height; the planets where GLOBES puts them, as the game's own map spreads them),
/// the game's planet texture for its globe ("" for none), and what is drawn without one: 0 a
/// planet, 1 a ship, 2 a station. The ships and stations are placed near the planet the story
/// meets them at (the Endar Spire over Taris, the Star Forge over the Unknown World, the
/// Ravager over Malachor V); the masters' meeting is at Dantooine's enclave.
const MAP_STOPS: &[(Game, &str, f32, f32, &str, u8)] = &[
    // a little above where Taris's save tag reaches (at 1280 px it covered the Endar Spire's name)
    (Game::K1, "Endar Spire", 0.11, 0.03, "", 1),
    (Game::K1, "Taris", 0.03, 0.60, "Gui_Taris_1", 0),
    (Game::K1, "Dantooine", 0.25, 0.25, "Gui_Dantooine_1", 0),
    (Game::K1, "Tatooine", 0.76, 0.86, "Gui_Tatooine_1", 0),
    (Game::K1, "Kashyyyk", 0.24, 0.85, "Gui_Kashyyyk_1", 0),
    (Game::K1, "Manaan", 0.86, 0.22, "Gui_Manaan_1", 0),
    (Game::K1, "Korriban", 0.55, 0.08, "Gui_korriban_1", 0),
    // a little above Tatooine's number (at 1280 px its name ran into it)
    (Game::K1, "Leviathan", 0.65, 0.41, "", 1),
    (Game::K1, "Unknown World", 0.47, 0.56, "Gui_unknown_1", 0),
    (Game::K1, "Star Forge", 0.37, 0.38, "", 2),
    (Game::K1, "Yavin Station", 0.97, 0.62, "GUI_yavin_1", 0),
    (Game::K1, "Ebon Hawk", 0.0, 0.0, "", 1),
    (Game::K2, "Peragus", 0.04, 0.20, "Gui_peragus", 0),
    (Game::K2, "Telos", 0.16, 0.75, "Gui_telos", 0),
    (Game::K2, "Nar Shaddaa", 0.36, 0.12, "Gui_narshada", 0),
    (Game::K2, "Dxun", 0.80, 0.88, "Gui_dxun", 0),
    (Game::K2, "Onderon", 0.90, 0.50, "Gui_onderon", 0),
    (Game::K2, "Dantooine", 0.60, 0.25, "Gui_Dantooine_1", 0),
    (Game::K2, "Korriban", 0.84, 0.10, "Gui_korriban_1", 0),
    (Game::K2, "The Jedi Masters", 0.62, 0.75, "", 2),
    // right of where the Ebon Hawk waits beside Telos (it covered the Ravager's name), below
    // Nar Shaddaa's
    (Game::K2, "Ravager", 0.36, 0.53, "", 1),
    (Game::K2, "Malachor V", 0.47, 0.86, "Gui_malachor", 0),
    (Game::K2, "Ebon Hawk", 0.0, 0.0, "", 1),
];

/// planetary.2da rows that are not places the galaxy map flies to: k_sup_galaxymap (the trip)
/// has no take-off or landing for them, so picking one would leave the player on the Ebon Hawk
/// with nowhere to land (2026-10-24, the maintainer: "remove them if we cannot travel there").
/// The Star Forge is reached by the story's own cutscene, not the map.
const NOT_DESTINATIONS: &[(Game, &str)] = &[
    (Game::K1, "Endar_Spire"),
    (Game::K1, "Ebon_Hawk"),
    (Game::K1, "Leviathan"),
    (Game::K1, "Star_Forge"),
    (Game::K2, "Ebon_Hawk"),
    (Game::K2, "Peragus_Tutorial"),
];

/// A Pazaak side card's look: its value as the card shows it, the game's card face, and
/// the middle of the face's black window, where the value goes. KotOR: +1..+6, -1..-6, the flip cards +/-1..+/-6;
/// KotOR II adds +/-1/2, Double, Tiebreaker and the 2&4 and 3&6 flips (g_i_pazcard_019..023).
/// How many Pazaak side cards the game has: KotOR's 18 (+1 to +6, -1 to -6, the flip cards),
/// KotOR II's 23 (five more). The save's list holds one entry more than that in both
/// (docs/save-editor-format.md, PT_PAZAAKCARDS), which is no card.
fn pazaak_cards(game: Game) -> usize {
    match game {
        Game::K1 => 18,
        Game::K2 => 23,
    }
}

fn card_look(game: Game, i: usize) -> (String, &'static str, f32) {
    let n = i % 6 + 1;
    let label = match i {
        0..=5 => format!("+{}", n),
        6..=11 => format!("-{}", n),
        12..=17 => format!("\u{b1}{}", n),
        18 => "\u{b1}1/2".to_string(),
        19 => "D".to_string(),
        20 => "T".to_string(),
        21 => "2&4".to_string(),
        22 => "3&6".to_string(),
        _ => format!("{}", i + 1),
    };
    match game {
        Game::K1 => (label, ["lbl_cardmpos", "lbl_cardmneg", "lbl_cardraref"][(i / 6).min(2)], 0.40),
        Game::K2 => {
            let face = match i {
                0..=5 => "pcards_pos_p",
                6..=11 => "pcards_neg_p",
                12..=17 => "pcards_dblpos_p",
                18 => "pcards_dblneg_p",
                _ => "pcards_gold_p",
            };
            (label, face, 0.37)
        }
    }
}

/// Pixels ready for the window: RGBA, width, height. Made away from the window (Slint's
/// pictures can only be made on it) and turned into pictures when shown.
type Pixels = (Vec<u8>, u32, u32);

/// The Galaxy Map page's art, made right after start for the game shown (2026-10-24, the
/// maintainer: preload it, snappy): every planet globe, open and closed, and every card face.
fn preload_art(st: &mut State) {
    let game = st.game;
    if !st.art_asked.insert(game) {
        return;
    }
    let Some(data) = st.data() else { return };
    let slot = st.art.clone();
    std::thread::spawn(move || {
        let mut names: Vec<(String, Option<Pixels>)> = Vec::new();
        for g in GLOBES.iter().filter(|g| g.0 == game) {
            for grey in [false, true] {
                names.push((globe_key(g.2, grey), globe_pixels(&data, g.2, grey)));
            }
        }
        let faces: std::collections::BTreeSet<&str> = (0..23).map(|i| card_look(game, i).1).collect();
        for f in faces {
            names.push((texture_key(f), texture_pixels(&data, f)));
        }
        let mut art = slot.lock().unwrap();
        for (k, v) in names {
            art.insert(k, v);
        }
    });
}

fn texture_key(name: &str) -> String {
    format!("tex:{}", name.to_ascii_lowercase())
}

fn globe_key(name: &str, grey: bool) -> String {
    format!("globe:{}:{}", name.to_ascii_lowercase(), grey)
}

/// A picture from the cache, else from the preloaded pixels, else made now.
fn picture(cache: &mut HashMap<String, Option<Image>>, art: &Mutex<HashMap<String, Option<Pixels>>>, key: String, make: impl FnOnce() -> Option<Pixels>) -> Option<Image> {
    if let Some(v) = cache.get(&key) {
        return v.clone();
    }
    let pixels = art.lock().unwrap().remove(&key).unwrap_or_else(make);
    let img = pixels.map(|(rgba, w, h)| Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, w, h)));
    cache.insert(key, img.clone());
    img
}

/// A game texture's pixels.
fn texture_pixels(data: &GameData, name: &str) -> Option<Pixels> {
    data.texture(name).and_then(|(b, is_tpc)| if is_tpc { tpc::decode(&b) } else { tga::decode(&b) }).map(|i| (i.rgba.clone(), i.width, i.height))
}

/// A planet as a lit globe, drawn from the game's planet texture (wrapped round a sphere,
/// lit from the upper left, a thin bright rim); `grey` gives the same globe without colour
/// and dimmed, for a planet the Ebon Hawk cannot fly to.
fn globe_pixels(data: &GameData, name: &str, grey: bool) -> Option<Pixels> {
    let t = data.texture(name).and_then(|(b, is_tpc)| if is_tpc { tpc::decode(&b) } else { tga::decode(&b) })?;
    const N: usize = 192;
    let (tw, th) = (t.width as usize, t.height as usize);
    // KotOR II's planet textures end in a saw-tooth at top and bottom: the band between
    // is what wraps round the globe
    let band = if th > 0 && tw > 0 && name.to_ascii_lowercase().starts_with("gui_") && !name.ends_with("_1") { 0.12 } else { 0.0 };
    let mut out = vec![0u8; N * N * 4];
    let light = {
        let (x, y, z) = (-0.55f32, -0.45f32, 0.70f32);
        let l = (x * x + y * y + z * z).sqrt();
        (x / l, y / l, z / l)
    };
    for py in 0..N {
        for px in 0..N {
            let u = (px as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let v = (py as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r2 = u * u + v * v;
            if r2 > 1.0 {
                continue;
            }
            let z = (1.0 - r2).max(0.0).sqrt();
            let lon = u.atan2(z) + 0.6;
            let lat = v.clamp(-1.0, 1.0).asin();
            let fx = (lon / std::f32::consts::TAU).rem_euclid(1.0);
            let fy = band + (lat / std::f32::consts::PI + 0.5) * (1.0 - 2.0 * band);
            let sx = ((fx * (tw - 1) as f32) as usize).min(tw - 1);
            let sy = ((fy * (th - 1) as f32) as usize).min(th - 1);
            let o = (sy * tw + sx) * 4;
            let lam = (u * light.0 + v * light.1 + z * light.2).max(0.0);
            let shade = 0.12 + 0.95 * lam.powf(0.85);
            let rim = (1.0 - z).powi(3) * 0.35;
            let mut c = [0f32; 3];
            for k in 0..3 {
                let tint = [0.55, 0.75, 1.0][k];
                c[k] = (t.rgba[o + k] as f32 / 255.0 * shade + rim * tint).clamp(0.0, 1.0);
            }
            if grey {
                let l = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
                c = [l * 0.5; 3];
            }
            let edge = ((1.0 - r2.sqrt()) * N as f32 / 2.0).clamp(0.0, 1.0);
            let d = (py * N + px) * 4;
            out[d] = (c[0] * 255.0) as u8;
            out[d + 1] = (c[1] * 255.0) as u8;
            out[d + 2] = (c[2] * 255.0) as u8;
            out[d + 3] = (edge * 255.0) as u8;
        }
    }
    Some((out, N as u32, N as u32))
}

fn fill_more(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let data = st.data();
    let map = save.party.root.child("GlxyMap");
    let orig_map = st.original.as_ref().and_then(|o| o.party.root.child("GlxyMap").cloned());
    let count = map.and_then(|m| m.int("GlxyMapNumPnts")).unwrap_or(0).min(16);
    let mask = map.and_then(|m| m.int("GlxyMapPlntMsk")).unwrap_or(0);
    let omask = orig_map.as_ref().and_then(|m| m.int("GlxyMapPlntMsk")).unwrap_or(mask);
    let game = save.game;
    let mut planets = Vec::new();
    for i in 0..count {
        let name = data
            .as_ref()
            .and_then(|d| d.row_name("planetary", i as usize))
            .unwrap_or_else(|| format!("Planet {}", i + 1));
        if name.starts_with("Live Planet") {
            continue;
        }
        let label = data.as_ref().and_then(|d| d.table("planetary").cell(i as usize, "label").map(|x| x.to_string())).unwrap_or_default();
        if NOT_DESTINATIONS.iter().any(|g| g.0 == game && g.1.eq_ignore_ascii_case(&label)) {
            continue;
        }
        let on = mask & (1 << i) != 0;
        let place = GLOBES.iter().find(|g| g.0 == game && g.1.eq_ignore_ascii_case(&label));
        let (x, y) = place.map_or((0.5, 0.95), |g| (g.3, g.4));
        let (globe, off) = match (place, data.as_ref()) {
            (Some(g), Some(d)) => (
                picture(&mut st.portraits, &st.art, globe_key(g.2, false), || globe_pixels(d, g.2, false)),
                picture(&mut st.portraits, &st.art, globe_key(g.2, true), || globe_pixels(d, g.2, true)),
            ),
            _ => (None, None),
        };
        planets.push(PlanetRow {
            index: i as i32,
            name: s(name),
            on,
            changed: on != (omask & (1 << i) != 0),
            x,
            y,
            has_globe: globe.is_some(),
            globe: globe.unwrap_or_default(),
            globe_off: off.unwrap_or_default(),
        });
    }
    app.set_planets(model(planets));
    let cards = save.party.root.list("PT_PAZAAKCARDS").cloned().unwrap_or_default();
    let ocards = st.original.as_ref().and_then(|o| o.party.root.list("PT_PAZAAKCARDS").cloned()).unwrap_or_default();
    let mut pazaak: Vec<CardRow> = Vec::new();
    // the list is longer than the cards: KotOR keeps 19 entries for 18 cards, KotOR II 24 for 23
    for (i, c) in cards.iter().enumerate().take(pazaak_cards(game)) {
        let v = c.int("PT_PAZAAKCOUNT").unwrap_or(0);
        let (label, face, label_y) = card_look(game, i);
        let face = data.as_ref().and_then(|d| picture(&mut st.portraits, &st.art, texture_key(face), || texture_pixels(d, face)));
        pazaak.push(CardRow {
            index: i as i32,
            label: s(label),
            value: v as i32,
            changed: ocards.get(i).and_then(|o| o.int("PT_PAZAAKCOUNT")) != Some(v),
            has_face: face.is_some(),
            face: face.unwrap_or_default(),
            label_y,
        });
    }
    let carried: i32 = pazaak.iter().map(|c| c.value).sum();
    app.set_pazaak(model(pazaak));
    app.set_pazaak_total(s(format!("{} {} carried", carried, if carried == 1 { "card" } else { "cards" })));
    let cheat = save.party.root.int("PT_CHEAT_USED").unwrap_or(0) != 0 || save.nfo.root.int("CHEATUSED").unwrap_or(0) != 0;
    let ocheat = st
        .original
        .as_ref()
        .map_or(cheat, |o| o.party.root.int("PT_CHEAT_USED").unwrap_or(0) != 0 || o.nfo.root.int("CHEATUSED").unwrap_or(0) != 0);
    app.set_cheat_used(cheat);
    app.set_cheat_changed(cheat != ocheat);
    let secs = save.party_int("PT_PLAYEDSECONDS").unwrap_or(0);
    app.set_played_hours(s((secs / 3600).to_string()));
    app.set_played_minutes(s((secs % 3600 / 60).to_string()));
    app.set_played_changed(st.original.as_ref().map_or(false, |o| o.party_int("PT_PLAYEDSECONDS") != Some(secs)));
    let names = |c: &Struct| -> String {
        let n = creature::name(c);
        if !n.is_empty() {
            return n;
        }
        match (c.get("FirstName"), &data) {
            (Some(Value::LocString(l)), Some(d)) => d.loc(l),
            _ => c.string("Tag").unwrap_or_default(),
        }
    };
    let rows: Vec<CreatureRow> = save
        .area_git()
        .map(|g| area::creatures(&g.root, &names))
        .unwrap_or_default()
        .into_iter()
        .map(|c| CreatureRow { index: c.index as i32, name: s(&c.name), note: s(&c.tag), hp: c.hp as i32, max_hp: c.max_hp as i32 })
        .collect();
    app.set_creatures(model(rows));
    let name = save.nfo.root.string("SAVEGAMENAME").unwrap_or_default();
    app.set_save_name(s(&name));
    app.set_save_name_changed(st.original.as_ref().map_or(false, |o| o.nfo.root.string("SAVEGAMENAME").unwrap_or_default() != name));
    fill_journal(ui, st);
}

/// The quests as the game's journal knows them (global.jrl): those in this save's journal
/// first, at their stage, then the rest. Picking one sets its stage.
fn fill_journal(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    let Some(save) = st.save.as_ref() else { return };
    let Some(data) = st.data() else { return };
    let have = save.quests();
    let mut all = (*st.journal()).clone();
    if !data.is_usable() {
        // the journal's names and texts are the game's: the save keeps a tag and a number
        all = have
            .iter()
            .map(|(tag, v)| kse_core::quests::Quest {
                tag: tag.clone(),
                name: tag.clone(),
                stages: vec![kse_core::quests::Stage { id: *v, end: false, text: format!("Stage {}", v), xp: 0 }],
            })
            .collect();
    }
    let before = st.original.as_ref().map(|o| o.quests()).unwrap_or_default();
    let stage_of = |tag: &str| have.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, v)| *v);
    // A quest is done when its journal entry is one of its endings.
    let status = |q: &kse_core::quests::Quest| match stage_of(&q.tag) {
        Some(v) if q.stage(v).map_or(false, |x| x.end) => 1,
        Some(_) => 0,
        None => 2,
    };
    let counts = [0, 1, 2].map(|k| all.iter().filter(|q| status(q) == k).count() as i32);
    app.set_quest_counts(model(counts.to_vec()));
    // The quest picked follows itself to its new tab (started: Active; finished: Completed),
    // so the stage picked stays with the quest it belongs to. Until 2026-11-06 the pick fell
    // to the first quest left on the tab, which then showed the other quest's stage picked.
    if let Some(q) = st.quest_pick.as_deref().and_then(|t| all.iter().find(|q| q.tag.eq_ignore_ascii_case(t))) {
        let k = status(q);
        if k != st.quest_tab as i32 {
            st.quest_tab = k as usize;
        }
    }
    app.set_quest_tab(st.quest_tab as i32);
    let mut tags = Vec::new();
    let mut rows = Vec::new();
    for q in all.iter().filter(|q| status(q) == st.quest_tab as i32) {
        let note = match stage_of(&q.tag) {
            Some(v) => q.stage(v).map(|x| short(&x.text)).unwrap_or_else(|| format!("Stage {}", v)),
            None => "Not started".to_string(),
        };
        let changed = stage_of(&q.tag) != before.iter().find(|(t, _)| t.eq_ignore_ascii_case(&q.tag)).map(|(_, v)| *v);
        let picked = st.quest_pick.as_deref().map_or(false, |t| t.eq_ignore_ascii_case(&q.tag));
        rows.push(Named { owned: picked, changed, ..row(tags.len() as i64, &q.name, note) });
        tags.push(q.tag.clone());
    }
    // The first quest of the tab is shown until another is picked.
    if st.quest_pick.as_ref().map_or(true, |t| !tags.iter().any(|x| x.eq_ignore_ascii_case(t))) {
        st.quest_pick = tags.first().cloned();
        st.stage_pick = None;
        if let Some(r) = rows.first_mut() {
            r.owned = true;
        }
    }
    st.quest_tags = tags;
    app.set_journal(model(rows));

    // The quest shown: where it stands and every stage.
    let picked = st.quest_pick.clone().and_then(|t| all.iter().find(|q| q.tag.eq_ignore_ascii_case(&t)).cloned());
    match picked {
        Some(q) => {
            let current = stage_of(&q.tag);
            let state = status(&q);
            app.set_quest_title(s(&q.name));
            app.set_quest_state(state);
            app.set_quest_text(s(current.and_then(|v| q.stage(v)).map(|x| x.text.trim().to_string()).unwrap_or_else(|| {
                "This quest is not in the journal yet. Start it, or put it at any stage below.".to_string()
            })));
            let stages: Vec<StageRow> = q
                .stages
                .iter()
                .map(|x| StageRow {
                    id: x.id as i32,
                    text: s(short(&x.text)),
                    xp: x.xp as i32,
                    end: x.end,
                    // 0 behind, 1 where the quest is, 2 ahead (the journal keeps only the
                    // current stage; an earlier number is taken as passed).
                    place: match current {
                        Some(c) if x.id == c => 1,
                        Some(c) if x.id < c && !x.end => 0,
                        _ => 2,
                    },
                })
                .collect();
            app.set_quest_stages(model(stages));
            let endings: Vec<StageRow> = q
                .endings()
                .iter()
                .map(|x| StageRow { id: x.id as i32, text: s(x.text.trim()), xp: x.xp as i32, end: true, place: 2 })
                .collect();
            // the stage picked in the list, in full, and what its button does
            let pick = st.stage_pick.and_then(|id| q.stages.iter().position(|x| x.id == id));
            app.set_stage_pick(pick.map_or(-1, |k| k as i32));
            if let Some(k) = pick {
                let x = &q.stages[k];
                app.set_stage_text(s(x.text.trim()));
                app.set_stage_action(if current == Some(x.id) {
                    3
                } else if x.end {
                    2
                } else if k == 0 {
                    0
                } else {
                    1
                });
                app.set_stage_ending(q.endings().iter().position(|e| e.id == x.id).map_or(-1, |e| e as i32));
            }
            app.set_quest_open(st.quest_open);
            app.set_quest_endings(model(endings));
            app.set_quest_shown(true);
        }
        None => app.set_quest_shown(false),
    }
}

/// Puts a quest at a stage, giving that stage's experience as the game does when the journal
/// reaches it (to the player and the party pool), once.
fn set_quest_stage(st: &mut State, tag: &str, id: i64) {
    let journal = st.journal();
    let quest = journal.iter().find(|q| q.tag.eq_ignore_ascii_case(tag)).cloned();
    let xp_of = |stage: i64| quest.as_ref().and_then(|q| q.stage(stage).map(|x| x.xp)).unwrap_or(0);
    // The XP is the stage's when it lies past where the save was opened, given once: what
    // this session gave the quest before is taken off first, so clicking stages back and forth
    // gives it once, and a quest set back to where it was gives it back. Until 2026-11-06 every
    // click on another stage gave its XP again.
    let opened = st.original.as_ref().and_then(|o| o.quests().into_iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, v)| v));
    let due = if opened.map_or(true, |o| id > o) { xp_of(id) } else { 0 };
    let key = tag.to_ascii_lowercase();
    let given = st.quest_xp.get(&key).copied().unwrap_or(0);
    let delta = due - given;
    if let Some(save) = st.save.as_mut() {
        save.set_quest(tag, id);
        if delta != 0 {
            save.edit(Who::Player, "questxp", |c| {
                let now = creature::experience(c);
                creature::set_experience(c, (now + delta).max(0));
            });
            let pool = save.party_int("PT_XP_POOL").unwrap_or(0);
            save.set_party_int("PT_XP_POOL", (pool + delta).max(0));
        }
    }
    st.quest_xp.insert(key, due);
}

// ---- the map ----

fn kind_code(k: kse_core::map::Kind) -> i32 {
    use kse_core::map::Kind::*;
    match k {
        Door => 0,
        Exit => 1,
        Container => 2,
        Placeable => 3,
        Creature | Merchant => 4,
        Note => 5,
    }
}

/// The game's own icon for each marker kind, in kind_code order. A way out is KotOR II's
/// walking figure; KotOR 1 never shipped one, so there it is the door cursor with its arrow.
const MAP_ICONS: [&[&str]; 6] = [
    &["i_opendoor"],
    &["ig_areatrans", "gui_mp_doorU"],
    &["i_openplace"],
    &["i_useplace"],
    &["i_dialog"],
    &["whitetarget"],
];

/// A round map marker drawn from a game picture: a face fills the disc from the top of its
/// picture, an icon sits inside a dark disc. Drawn here, edge smoothed, so it is round
/// whatever the renderer can clip.
fn round_badge(img: &tga::Image, face: bool) -> Image {
    const N: usize = 64;
    let r = N as f32 / 2.0;
    let side = img.width.min(img.height) as f32;
    let left = (img.width as f32 - side) / 2.0;
    let inner = if face { N as f32 } else { N as f32 * 0.72 };
    let off = (N as f32 - inner) / 2.0;
    // Straight-alpha bilinear sample of the source at (x, y) in its pixels.
    let sample = |x: f32, y: f32| -> [f32; 4] {
        let (w, h) = (img.width as i32, img.height as i32);
        let (x, y) = (x - 0.5, y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let mut acc = [0.0f32; 4];
        for (dx, dy, wgt) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            let px = (x0 as i32 + dx).clamp(0, w - 1) as usize;
            let py = (y0 as i32 + dy).clamp(0, h - 1) as usize;
            let p = &img.rgba[(py * img.width as usize + px) * 4..][..4];
            let a = p[3] as f32 / 255.0 * wgt;
            acc[0] += p[0] as f32 * a;
            acc[1] += p[1] as f32 * a;
            acc[2] += p[2] as f32 * a;
            acc[3] += a;
        }
        if acc[3] > 0.0 {
            [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3]]
        } else {
            [0.0; 4]
        }
    };
    let mut out = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let (cx, cy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let cover = (r - (cx * cx + cy * cy).sqrt()).clamp(0.0, 1.0);
            if cover == 0.0 {
                continue;
            }
            let mut c = [9.0f32, 14.0, 22.0];
            let (u, v) = ((x as f32 + 0.5 - off) / inner, (y as f32 + 0.5 - off) / inner);
            if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                let p = sample(left + u * side, v * side);
                for k in 0..3 {
                    c[k] = c[k] * (1.0 - p[3]) + p[k] * p[3];
                }
            }
            let o = &mut out[(y * N + x) * 4..][..4];
            o.copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, (cover * 255.0) as u8]);
        }
    }
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&out, N as u32, N as u32))
}

/// A round badge of a game texture, kept for the session (key "badge:<name>").
fn badge(cache: &mut HashMap<String, Option<Image>>, data: &GameData, names: &[&str], face: bool) -> Option<Image> {
    let key = format!("badge:{}", names[0]);
    if let Some(b) = cache.get(&key) {
        return b.clone();
    }
    let b = names
        .iter()
        .find_map(|n| data.texture(n).and_then(|(b, t)| if t { tpc::decode(&b) } else { tga::decode(&b) }))
        .map(|i| round_badge(&i, face));
    cache.insert(key, b.clone());
    b
}

/// Builds the map of `module` (the current one when None) from the save's copy of it.
/// Every area's title and every area's ways in, for one game (the Map tab's area list).
type MapLists = (HashMap<String, String>, Arc<HashMap<String, Vec<(String, String)>>>);

/// Starts reading the game's area titles and ways in, away from the window, once per game.
fn prefetch_map_lists(st: &mut State) {
    let game = st.game;
    if st.module_titles.contains_key(&game) || !st.map_lists_asked.insert(game) {
        return;
    }
    let Some(data) = st.data() else { return };
    let Some(root) = st.installs.iter().find(|i| i.game == game).map(|i| i.path.clone()) else { return };
    let slot = st.map_lists.clone();
    std::thread::spawn(move || {
        let titles: HashMap<String, String> = kse_core::map::game_modules(&root)
            .into_iter()
            .map(|m| {
                let t = kse_core::map::module_title(&data, &m);
                (m, t)
            })
            .collect();
        let arrivals = Arc::new(kse_core::map::arrivals(&data));
        slot.lock().unwrap().insert(game, (titles, arrivals));
    });
}

/// Takes over the area lists read away from the window, where they are ready.
fn adopt_map_lists(st: &mut State) {
    let ready: Vec<(Game, MapLists)> = st.map_lists.lock().unwrap().drain().collect();
    for (game, (titles, arrivals)) in ready {
        st.module_titles.entry(game).or_insert(titles);
        st.arrivals.entry(game).or_insert(arrivals);
    }
}

fn load_map(st: &mut State, module: Option<String>) {
    let Some(save) = st.save.as_ref() else { return };
    let module = module.unwrap_or_else(|| save.module_name().to_string());
    let Some(data) = st.data() else { return };
    let Some(git) = save.module_git(&module).or_else(|| kse_core::map::game_git(&data, &module)) else { return };
    let namer = |c: &Struct| -> String {
        let n = creature::name(c);
        if !n.is_empty() {
            return n;
        }
        for l in ["LocName", "FirstName", "LocalizedName"] {
            if let Some(Value::LocString(x)) = c.get(l) {
                let t = kse_core::text::strip_codes(&data.loc(x));
                if !t.trim().is_empty() {
                    return t;
                }
            }
        }
        c.string("Tag").unwrap_or_default()
    };
    st.map = Some(kse_core::map::Area::build(&data, &module, &git.root, &namer));
    st.map_pick = None;
}

fn fill_map(ui: &AppWindow, st: &mut State) {
    let app = ui.global::<App>();
    // the area lists, if read in the background by now
    adopt_map_lists(st);
    let Some(save) = st.save.as_ref() else { return };
    let current = save.module_name().to_ascii_lowercase();
    if st.map.is_none() {
        load_map(st, None);
    }
    let arrow = st.portrait("mm_barrow");
    let Some(area) = st.map.as_ref() else { return };
    let Some(data) = st.data() else { return };
    let save = st.save.as_ref().unwrap();
    let viewing = area.module.to_ascii_lowercase();
    let here = viewing == current;
    // The picture: the map part of the game's texture (440 of 512 in both games).
    match area.picture.as_ref() {
        Some(img) => {
            app.set_map_picture(image(img));
            app.set_has_map(true);
            let clip = 440.0 / 512.0;
            app.set_map_clip(clip);
            app.set_map_aspect(img.width as f32 * clip / img.height as f32);
        }
        None => app.set_has_map(false),
    }
    let to = |x: f32, y: f32| area.to_map(x, y).map(|(u, v)| (u, v));
    let mut marks = Vec::new();
    let mut faces: Vec<(usize, String)> = Vec::new();
    for (i, o) in area.objects.iter().enumerate() {
        let Some((u, v)) = to(o.pos[0], o.pos[1]) else { continue };
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            continue;
        }
        let selected = matches!(st.map_pick, Some(MapPick::Object(k)) if k == i);
        let face = o.portrait.and_then(|p| data.table("portraits").cell(p as usize, "baseresref").map(|r| r.to_string()));
        marks.push(MapMark { index: i as i32, x: u, y: v, kind: kind_code(o.kind), name: s(&o.name), selected, face: Default::default(), has_face: false });
        if let Some(r) = face {
            faces.push((marks.len() - 1, r));
        }
    }
    app.set_map_you(false);
    if here {
        if let Some(p) = save.player() {
            let pos = area::position(p);
            if let Some((u, v)) = to(pos[0], pos[1]) {
                // The way the player faces, as a turn on the map (the map's y runs down).
                let (fx, fy) = (p.float("XOrientation").unwrap_or(1.0), p.float("YOrientation").unwrap_or(0.0));
                let ahead = to(pos[0] + fx, pos[1] + fy).unwrap_or((u + 0.01, v));
                // The arrow points up in its picture; the angle turns it clockwise.
                let angle = (ahead.0 - u).atan2(-(ahead.1 - v)).to_degrees();
                app.set_map_you(true);
                app.set_map_you_x(u);
                app.set_map_you_y(v);
                app.set_map_you_angle(angle);
            }
        }
    }
    // the "you are here" arrow, through the texture cache (it was read and decoded again on
    // every map click: looked up there but never stored)
    match arrow {
        Some(img) => {
            app.set_map_arrow(img);
            app.set_has_arrow(true);
        }
        None => app.set_has_arrow(false),
    }
    if let Some(MapPick::Floor(p)) = st.map_pick {
        if let Some((u, v)) = to(p[0], p[1]) {
            marks.push(MapMark { index: -2, x: u, y: v, kind: 7, name: s("Picked spot"), selected: true, face: Default::default(), has_face: false });
        }
    }
    // Every marker is a round badge: a person's portrait, else the game's icon for its kind.
    let icons: Vec<Option<Image>> = MAP_ICONS.iter().map(|n| badge(&mut st.portraits, &data, n, false)).collect();
    for m in marks.iter_mut() {
        if let Some(Some(icon)) = icons.get(m.kind as usize) {
            m.face = icon.clone();
            m.has_face = true;
        }
    }
    for (k, r) in faces {
        if let Some(img) = badge(&mut st.portraits, &data, &[r.as_str()], true) {
            marks[k].face = img;
            marks[k].has_face = true;
        }
    }
    app.set_map_icons(model(icons.into_iter().map(|i| i.unwrap_or_default()).collect()));
    app.set_map_marks(model(marks));
    app.set_map_title(s(&area.name));
    let been = save.visited_modules().contains(&viewing);
    app.set_map_note(s(if here {
        "Where the save leaves you. Click a marker to see it, or the floor to pick a spot."
    } else if been {
        "An area this save has been to, as it was left. Travel Here takes you there."
    } else {
        "Not been here yet: shown as the game first builds it. Travel Here takes you there."
    }));

    // Areas: this one, where the viewed area's ways out lead, then the rest visited.
    let game = st.game;
    if !st.module_titles.contains_key(&game) {
        let root = st.installs.iter().find(|i| i.game == game).map(|i| i.path.clone());
        let titles: HashMap<String, String> = root
            .map(|r| kse_core::map::game_modules(&r))
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let t = kse_core::map::module_title(&data, &m);
                (m, t)
            })
            .collect();
        st.module_titles.insert(game, titles);
    }
    let titles = st.module_titles.get(&game).cloned().unwrap_or_default();
    let save = st.save.as_ref().unwrap();
    let visited = save.visited_modules();
    // The doors and transitions of every area, read once: they tell real places from the
    // game's cutscene stages, and which of two areas with one name the game leads into.
    if !st.arrivals.contains_key(&game) {
        st.arrivals.insert(game, Arc::new(kse_core::map::arrivals(&data)));
    }
    let arrivals = st.arrivals.get(&game).cloned().unwrap_or_default();
    // Planets, each a group that opens and closes (the current planet open at first), with
    // its areas under it: the ones visited, led into by a door, or with a name of their own.
    // Cutscene stages (stunt_ modules, the game's movie sets) and nameless modules are not
    // places to go; of two areas with one name the visited one, else the one a door leads
    // into, stays.
    let split = |m: &str| -> (String, String) {
        let t = titles.get(m).cloned().unwrap_or_else(|| m.to_string());
        match t.rsplit_once(" - ") {
            Some((planet, place)) => (planet.trim().to_string(), place.trim().to_string()),
            None => (t.trim().to_string(), t.trim().to_string()),
        }
    };
    let mut groups: std::collections::BTreeMap<String, Vec<(String, String)>> = std::collections::BTreeMap::new();
    for m in titles.keys() {
        let (planet, place) = split(m);
        let real = visited.contains(m) || *m == current || arrivals.contains_key(m);
        if m.starts_with("stunt_") || place.eq_ignore_ascii_case(m) || !real {
            continue;
        }
        let list = groups.entry(planet).or_default();
        let rank = |x: &str| (visited.contains(&x.to_string()) as u8 + (x == current) as u8 * 2, arrivals.get(x).map_or(0, |a| a.len()));
        match list.iter().position(|(_, p)| p.eq_ignore_ascii_case(&place)) {
            Some(k) if rank(m) > rank(&list[k].0) => list[k] = (m.clone(), place),
            Some(_) => {}
            None => list.push((m.clone(), place)),
        }
    }
    let (here_planet, _) = split(&current);
    if st.open_planets.is_empty() {
        st.open_planets.insert(here_planet.clone());
    }
    let mut order: Vec<String> = groups.keys().cloned().collect();
    order.sort_by_key(|p| (*p != here_planet, p.clone()));
    let mut rows = Vec::new();
    for planet in order {
        let mut areas = groups.remove(&planet).unwrap_or_default();
        areas.sort_by(|x, y| x.1.cmp(&y.1));
        let open = st.open_planets.contains(&planet);
        let been = areas.iter().filter(|(m, _)| visited.contains(m)).count();
        rows.push(AreaRow {
            module: s(""),
            name: s(&planet),
            note: s(format!("{} areas · {} visited", areas.len(), been)),
            current: planet == here_planet,
            viewing: false,
            visited: true,
            header: true,
            open,
            planet: s(&planet),
        });
        if !open {
            continue;
        }
        for (m, place) in areas {
            let note = if m == current { "You are here" } else if m == viewing { "Looking at" } else if visited.contains(&m) { "Been here" } else { "Not been here" };
            rows.push(AreaRow {
                module: s(&m),
                name: s(place),
                note: s(note),
                current: m == current,
                viewing: m == viewing,
                visited: visited.contains(&m),
                header: false,
                open: true,
                planet: s(&planet),
            });
        }
    }
    app.set_map_areas(model(rows));

    // Travel Here: the ways the game leads into the area on the map.
    st.travel_ways.clear();
    if !here {
        let ways = st.arrivals.get(&game).and_then(|a| a.get(&viewing)).cloned().unwrap_or_default();
        st.travel_ways = ways;
        if st.travel_ways.is_empty() {
            if let Some(w) = kse_core::map::any_waypoint(&data, &viewing) {
                st.travel_ways.push((w, String::new()));
            }
        }
    }
    let ways: Vec<Named> = st
        .travel_ways
        .iter()
        .enumerate()
        .map(|(i, (tag, from))| {
            let place = if from.is_empty() {
                "At the area's own marker".to_string()
            } else {
                let t = st.module_titles.get(&st.game).and_then(|t| t.get(from)).cloned().unwrap_or_else(|| from.clone());
                format!("From {}", t.rsplit_once(" - ").map(|(_, p)| p.to_string()).unwrap_or(t))
            };
            row(i as i64, place, tag)
        })
        .collect();
    // Two doors from the same place are numbered.
    let mut ways = ways;
    for i in 0..ways.len() {
        let same: Vec<usize> = (0..ways.len()).filter(|&k| ways[k].name == ways[i].name).collect();
        if same.len() > 1 && same[0] == i {
            for (n, &k) in same.iter().enumerate() {
                ways[k].name = s(format!("{} ({})", ways[k].name, n + 1));
            }
        }
    }
    app.set_travel_ways(model(ways));

    // The pick.
    app.set_pick_can_move(false);
    app.set_pick_has_second(false);
    app.set_pick_can_unlock(false);
    app.set_pick_is_exit(false);
    app.set_pick_can_spawn(false);
    app.set_pick_can_remove(false);
    app.set_map_here(here);
    match st.map_pick.clone() {
        // Another area: what is offered first is going there.
        None if !here => {
            app.set_pick_title(s(format!("Go to {}", area.name)));
            app.set_pick_note(s("Pick where to arrive below, or a marker or spot on the map."));
        }
        None => {
            app.set_pick_title(s("Nothing yet"));
            app.set_pick_note(s("Click a marker on the map, or any spot on the floor."));
        }
        Some(MapPick::Floor(_)) => {
            app.set_pick_title(s("A spot on the floor"));
            app.set_pick_note(s(if here {
                "Checked: walkable floor with room around it."
            } else {
                "Checked: walkable floor with room around it. Moving straight there takes the player alone; Travel Here makes the trip the game's way, with the party."
            }));
            // Straight to a spot needs the area's saved copy; elsewhere Travel Here goes.
            app.set_pick_can_move(here || been);
            app.set_pick_move_label(s(if here { "Move Here" } else { "Move to This Area, Here" }));
            app.set_pick_can_spawn(here);
        }
        Some(MapPick::Object(i)) => {
            let o = &area.objects[i];
            use kse_core::map::Kind::*;
            let what = match o.kind {
                Door => "Door",
                Exit => "Way out",
                Container => "Container",
                Placeable => "Something to use",
                Creature | Merchant => "Person",
                Note => "Map note",
            };
            app.set_pick_title(s(if o.name.is_empty() { what.to_string() } else { o.name.clone() }));
            let mut note = what.to_string();
            if o.locked {
                note.push_str(", locked");
            }
            if !o.leads_to.is_empty() {
                note.push_str(&format!(". Leads to {}", kse_core::map::module_title(&data, &o.leads_to)));
            }
            if o.spots.is_empty() {
                note.push_str(". No free floor next to it to stand on.");
            } else if !here && been {
                note.push_str(". Moving straight there takes the player alone; Travel Here makes the trip the game's way, with the party.");
            }
            app.set_pick_note(s(note));
            let door = matches!(o.kind, Door | Exit) && o.spots.len() == 2;
            app.set_pick_can_move(!o.spots.is_empty() && (here || been));
            app.set_pick_has_second(door);
            app.set_pick_move_label(s(if door { "Move to This Side" } else if here { "Move Next to It" } else { "Move to This Area, Next to It" }));
            app.set_pick_second_label(s("Move to the Other Side"));
            app.set_pick_can_unlock(o.locked && matches!(o.kind, Door | Exit | Container) && here);
            // Only a creature the editor added can be taken out here; the area's own stay.
            let added = here
                && o.list == area::CREATURES
                && save.area_git().and_then(|g| g.root.list(area::CREATURES)).and_then(|l| l.get(o.index)).map_or(false, |c| was_spawned(st.game, &current, c));
            app.set_pick_can_remove(added);
            app.set_pick_is_exit(!o.leads_to.is_empty() && visited.contains(&o.leads_to));
        }
    }
}

/// Items in the party inventory the picked member can wear in `slot` (baseitems.2da
/// `equipableslots`, and `droidorhuman`: 1 people only, 2 droids only).
fn slot_items(st: &mut State, slot: u32) -> Vec<Named> {
    let Some(data) = st.data() else { return Vec::new() };
    let base = data.table("baseitems");
    let items = st.save.as_ref().map(|s| s.inventory()).unwrap_or_default();
    let droid = match (st.catalog(), st.save.as_ref().and_then(|s| s.creature(st.who()))) {
        (Some(cat), Some(c)) => creature::classes(c).iter().any(|(id, _)| cat.class(*id).map_or(false, |k| k.droid)),
        _ => false,
    };
    // A weapon for the right hand alone takes both hands, so the left is taken.
    if let Some(right) = right_hand_of(slot) {
        if st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| creature::in_slot(c, right)).map_or(false, |w| two_handed(&base, w)) {
            return Vec::new();
        }
    }
    let mut rows = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let b = it.int("BaseItem").unwrap_or(0) as usize;
        // KotOR 2's second weapon set takes what the first does.
        let wanted = match slot { 0x40000 => 0x10, 0x80000 => 0x20, other => other };
        let fits = base.int(b, "equipableslots").map_or(false, |m| (m as u32) & wanted != 0);
        let user = base.int(b, "droidorhuman").unwrap_or(0);
        if !fits || (user == 1 && droid) || (user == 2 && !droid) {
            continue;
        }
        let icon = st.item_icon(it);
        let (kind, _) = st.item_kind(it);
        rows.push(Named { has_icon: icon.is_some(), icon: icon.unwrap_or_default(), ..row(i as i64, st.item_name(it), kind) });
    }
    rows
}

/// The right hand that goes with left hand `slot`, in either weapon set.
fn right_hand_of(slot: u32) -> Option<u32> {
    match slot {
        0x20 => Some(0x10),
        0x80000 => Some(0x40000),
        _ => None,
    }
}

/// Measured in both games' baseitems.2da: staves, rifles and heavy weapons (weaponwield 3, 5,
/// 6) list the right hand alone, weapons that can pair list both. KotOR 1's Gamorrean
/// Battleaxe also lists the right hand alone but is wielded in one hand (2).
fn two_handed(base: &kse_core::formats::tables::TwoDa, item: &Struct) -> bool {
    let row = item.int("BaseItem").unwrap_or(0) as usize;
    let slots = base.int(row, "equipableslots").unwrap_or(0);
    slots & 0x10 != 0 && slots & 0x20 == 0 && matches!(base.int(row, "weaponwield"), Some(3 | 5 | 6))
}

/// The creatures the editor spawned, so the map offers to remove those and only those:
/// one line per creature, "game|module|object id|tag", in spawned.txt beside the settings.
/// The game keeps a creature's object id when it saves, so the record still finds it in
/// later saves of the same game.
fn spawned_path() -> PathBuf {
    games::data_dir().join("spawned.txt")
}

fn spawned_key(game: Game, module: &str, id: u32, tag: &str) -> String {
    format!("{}|{}|{}|{}", game.title(), module.to_ascii_lowercase(), id, tag.to_ascii_lowercase())
}

fn was_spawned(game: Game, module: &str, c: &Struct) -> bool {
    let (Some(id), tag) = (c.int("ObjectId"), c.string("Tag").unwrap_or_default()) else { return false };
    let key = spawned_key(game, module, id as u32, &tag);
    std::fs::read_to_string(spawned_path()).map_or(false, |t| t.lines().any(|l| l == key))
}

/// A spawn that was reverted before it was saved is not in any save, so its line goes: the id
/// would otherwise offer Remove on a later creature that gets it.
fn forget_pending_spawns(st: &mut State) {
    let pending = std::mem::take(&mut st.pending_spawns);
    if pending.is_empty() {
        return;
    }
    if let Ok(text) = std::fs::read_to_string(spawned_path()) {
        let left: String = text.lines().filter(|l| !pending.iter().any(|p| p == l)).map(|l| format!("{}\n", l)).collect();
        let _ = std::fs::write(spawned_path(), left);
    }
}

fn record_spawned(game: Game, module: &str, id: u32, tag: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(games::data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(spawned_path()) {
        let _ = writeln!(f, "{}", spawned_key(game, module, id, tag));
    }
}

/// The Load screen's pictures of the party: savenfo PORTRAIT1 and PORTRAIT2. Left as they
/// are when a member's picture cannot be told, so a row never shows the wrong companion.
fn update_party_portraits(st: &mut State) {
    let Some(save) = st.save.as_ref() else { return };
    let portraits: Option<Vec<String>> = save
        .party_members()
        .iter()
        .map(|m| {
            let id = save.creature(Who::Npc(*m))?.int("PortraitId")?;
            st.data()?.table("portraits").cell(id as usize, "baseresref").map(|r| r.to_string())
        })
        .collect();
    let Some(portraits) = portraits else { return };
    if let Some(save) = st.save.as_mut() {
        save.edit_nfo(|n| {
            for slot in 1..=2 {
                let label = format!("PORTRAIT{}", slot);
                match portraits.get(slot - 1) {
                    Some(r) => n.set(&label, Value::ResRef(r.as_bytes().to_vec())),
                    None => n.fields.retain(|(l, _)| *l != label),
                }
            }
        });
    }
}

/// The open save's backups folder (save::backup_root).
fn backups_of(st: &State) -> PathBuf {
    let root = st.installs.iter().find(|i| i.game == st.game).map(|i| i.path.clone());
    match st.save.as_ref() {
        Some(save) => save::backup_root(&save.folder, root.as_deref()),
        None => games::data_dir().join("backups"),
    }
}

// ---- spawning ----

/// Where a creature to spawn comes from: one standing in the area (copied), a template
/// any area can use (the game's templates and Override), or one of this module's own.
#[derive(Clone)]
enum SpawnSource {
    Area(usize),
    Template(String),
    Module(String),
}

#[derive(Clone)]
struct Spawnable {
    source: SpawnSource,
    name: String,
    note: String,
    /// The picker tab it belongs to besides All: 1 in this area, 2 hostile, 3 friendly, 4 other.
    group: usize,
    portrait: Option<String>,
}

const SPAWN_TABS: [&str; 5] = ["All", "In This Area", "Hostile", "Friendly", "Other"];

/// Hostile, Friendly or Other, from the faction's label in repute.2da.
fn faction_group(data: &GameData, faction: i64) -> (usize, String) {
    let label = data.table("repute").cell(faction.max(0) as usize, "label").map(|l| l.to_string()).unwrap_or_default();
    let lower = label.to_ascii_lowercase();
    let group = if lower.contains("hostile") { 2 } else if lower.contains("friendly") { 3 } else { 4 };
    let shown = if label.is_empty() { format!("faction {}", faction) } else { label.replace('_', " ") };
    (group, shown)
}

fn creature_label(data: &GameData, c: &Struct) -> String {
    let mut parts = Vec::new();
    for l in ["FirstName", "LastName"] {
        if let Some(Value::LocString(x)) = c.get(l) {
            let t = kse_core::text::strip_codes(&data.loc(x)).trim().to_string();
            if !t.is_empty() && t != "Bad StrRef" {
                parts.push(t);
            }
        }
    }
    parts.join(" ")
}

fn spawn_portrait(data: &GameData, c: &Struct) -> Option<String> {
    let id = c.int("PortraitId").filter(|p| *p > 0 && *p != 0xFFFF)?;
    data.table("portraits").cell(id as usize, "baseresref").map(|r| r.to_string())
}

/// Everything that can be spawned in the save's current area.
fn spawnables(st: &State) -> Vec<Spawnable> {
    let Some(data) = st.data() else { return Vec::new() };
    let Some(save) = st.save.as_ref() else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(git) = save.area_git() {
        for (i, c) in git.root.list(area::CREATURES).map(|l| l.iter().collect::<Vec<_>>()).unwrap_or_default().into_iter().enumerate() {
            let name = creature_label(&data, c);
            let tag = c.string("Tag").unwrap_or_default();
            let (_, faction) = faction_group(&data, c.int("FactionID").unwrap_or(0));
            out.push(Spawnable {
                source: SpawnSource::Area(i),
                name: if name.is_empty() { tag.clone() } else { name },
                note: format!("Copy of one in this area · {} · {}", faction, tag),
                group: 1,
                portrait: spawn_portrait(&data, c),
            });
        }
    }
    let mut add = |source: SpawnSource, resref: &str, utc: &Struct, from: &str| {
        let name = creature_label(&data, utc);
        let (group, faction) = faction_group(&data, utc.int("FactionID").unwrap_or(0));
        out.push(Spawnable {
            source,
            name: if name.is_empty() { resref.to_string() } else { name },
            note: format!("{} · {} · {}", faction, from, resref),
            group,
            portrait: spawn_portrait(&data, utc),
        });
    };
    let root = st.installs.iter().find(|i| i.game == st.game).map(|i| i.path.clone());
    let mut module_names = Vec::new();
    if let Some(root) = root.as_ref() {
        let files = kse_core::map::ModuleFiles::open(root, save.module_name());
        for resref in files.names(RES_TYPE_UTC) {
            let Some(utc) = files.read(&resref, RES_TYPE_UTC).and_then(|d| kse_core::formats::gff::Gff::read(&d).ok()) else { continue };
            add(SpawnSource::Module(resref.clone()), &resref, &utc.root, "this area's own");
            module_names.push(resref);
        }
    }
    for resref in data.resource_names("utc", RES_TYPE_UTC) {
        if module_names.contains(&resref) {
            continue;
        }
        let Some(utc) = data.gff(&resref, "utc", RES_TYPE_UTC) else { continue };
        add(SpawnSource::Template(resref.clone()), &resref, &utc.root, "any area");
    }
    out
}

fn search_spawn(ui: &AppWindow, st: &mut State) {
    let q = st.picker_query.trim().to_lowercase();
    let tab = st.picker_tab;
    let data = st.data();
    let mut rows = Vec::new();
    for (i, sp) in st.spawnable.iter().enumerate() {
        if tab != 0 && sp.group != tab {
            continue;
        }
        if !q.is_empty() && !sp.name.to_lowercase().contains(&q) && !sp.note.to_lowercase().contains(&q) {
            continue;
        }
        rows.push((i, sp.clone()));
        if rows.len() >= 200 {
            break;
        }
    }
    let rows: Vec<Named> = rows
        .into_iter()
        .map(|(i, sp)| {
            let icon = match (sp.portrait.as_deref(), data.as_ref()) {
                (Some(p), Some(d)) => badge(&mut st.portraits, d, &[p], true),
                _ => None,
            };
            Named { has_icon: icon.is_some(), icon: icon.unwrap_or_default(), ..row(i as i64, &sp.name, &sp.note) }
        })
        .collect();
    ui.global::<App>().set_picker(model(rows));
}

/// The creature for a pick in the spawn list, as the area will hold it.
fn spawn_creature_of(st: &State, sp: &Spawnable) -> Option<Struct> {
    let save = st.save.as_ref()?;
    let git = save.area_git()?;
    let like = git.root.list(area::CREATURES).and_then(|l| l.first()).cloned();
    match &sp.source {
        SpawnSource::Area(i) => git.root.list(area::CREATURES)?.get(*i).map(area::copy_creature),
        SpawnSource::Template(resref) => {
            let utc = st.data()?.gff(resref, "utc", RES_TYPE_UTC)?;
            Some(area::creature_from_template(&utc.root, like.as_ref()))
        }
        SpawnSource::Module(resref) => {
            let root = st.installs.iter().find(|i| i.game == st.game)?.path.clone();
            let files = kse_core::map::ModuleFiles::open(&root, save.module_name());
            let utc = kse_core::formats::gff::Gff::read(&files.read(resref, RES_TYPE_UTC)?).ok()?;
            Some(area::creature_from_template(&utc.root, like.as_ref()))
        }
    }
}

// ---- actions ----

fn with<F: Fn(&AppWindow, &mut State) + 'static>(ui: &AppWindow, state: &Shared, f: F) -> impl Fn() + 'static {
    let weak = ui.as_weak();
    let state = state.clone();
    move || {
        if let Some(ui) = weak.upgrade() {
            f(&ui, &mut state.borrow_mut());
        }
    }
}

macro_rules! on {
    ($ui:expr, $state:expr, $name:ident, |$u:ident, $st:ident $(, $arg:ident : $ty:ty)*| $body:block) => {{
        let weak = $ui.as_weak();
        let state = $state.clone();
        #[allow(unused_variables, unused_mut)]
        let handler = Rc::new(move |$u: AppWindow, $st: &mut State $(, $arg: $ty)*| $body);
        $ui.global::<App>().$name(move |$($arg: $ty),*| {
            let h = handler.clone();
            run_handler(weak.clone(), state.clone(), Rc::new(move |u: AppWindow, st: &mut State| h(u, st $(, $arg.clone())*)));
        });
    }};
}

/// A folder picked in the system's dialog, which belongs to the window: the window takes no
/// clicks while it is open (a click on a save row could otherwise wait and land on a list the
/// dialog's result has changed).
fn pick_folder(ui: &AppWindow, title: &str) -> Option<PathBuf> {
    use slint::winit_030::WinitWindowAccessor;
    let owned = ui.window().with_winit_window(|w| rfd::FileDialog::new().set_parent(w).set_title(title).pick_folder());
    match owned {
        Some(picked) => picked,
        None => rfd::FileDialog::new().set_title(title).pick_folder(),
    }
}

/// Runs a handler with the state. A handler can come while another still holds the state:
/// a result from a background thread delivered while a folder dialog is open, whose modal loop
/// runs the window's events. Borrowing it then ended the program (2026-11-06 audit); the
/// handler now waits its turn.
fn run_handler(weak: slint::Weak<AppWindow>, state: Shared, f: Rc<dyn Fn(AppWindow, &mut State)>) {
    let Some(u) = weak.upgrade() else { return };
    let busy = match state.try_borrow_mut() {
        Ok(mut st) => {
            f(u, &mut st);
            false
        }
        Err(_) => true,
    };
    if busy {
        slint::Timer::single_shot(std::time::Duration::from_millis(40), move || run_handler(weak, state, f));
    }
}

/// A most Health (or Force) typed that the creature cannot have at its level: the two values
/// typed, the class levels at which it can (class index, level; None: at no level), and the
/// nearest it can have without a level change.
struct PoolAsk {
    // the member it was asked for: a click on another member can land as the pop-up opens
    who: Who,
    force: bool,
    now: i64,
    most: i64,
    levels: Option<Vec<(usize, i64)>>,
    nearest: i64,
}

/// Picks member `who` again (an answer to a pop-up asked for one member acts on that one).
fn pick_who(st: &mut State, who: Who) {
    if let Some(k) = st.whos.iter().position(|w| *w == who) {
        st.who = k;
    }
}

/// The lowest and highest most Health (force: most Force) the creature can have at its
/// level: each end set on a copy, then read back as the engine computes it.
fn pool_reach(c: &Struct, game: Game, force: bool) -> (i64, i64) {
    let at = |v: i64| {
        let mut t = c.clone();
        if force {
            creature::set_max_force(&mut t, game, v);
            creature::max_force(&t, game)
        } else {
            creature::set_max_health(&mut t, v);
            creature::max_health(&t)
        }
    };
    (at(i16::MIN as i64), at(i16::MAX as i64))
}

/// The class levels at which a most of `want` comes within reach, tried on a copy, with the
/// lowest and highest most at them: up, the newest class raised a level at a time to the
/// game's highest level; down, the newest class lowered to 1, then the one before. None when
/// no level reaches it.
fn levels_for_pool(c: &Struct, cat: &Catalog, force: bool, want: i64) -> Option<(Vec<(usize, i64)>, i64, i64)> {
    let classes = creature::classes(c);
    if classes.is_empty() {
        return None;
    }
    let up = want > pool_reach(c, cat.game, force).1;
    let order: Vec<usize> = if up { vec![classes.len() - 1] } else { (0..classes.len()).rev().collect() };
    let mut t = c.clone();
    for idx in order {
        let class = cat.class(classes[idx].0)?.clone();
        loop {
            let now = creature::classes(&t)[idx].1;
            let next = if up { now + 1 } else { now - 1 };
            if next < 1 || (up && creature::level(&t) >= cat.max_level()) {
                break;
            }
            let keep = creature::other_class_feats(&t, idx, &|id| cat.class(id).cloned());
            creature::set_class_level_keeping(&mut t, idx, next, &class, &|l| cat.xp_for(l), &keep);
            let (lo, hi) = pool_reach(&t, cat.game, force);
            if (lo..=hi).contains(&want) {
                return Some((creature::classes(&t).iter().enumerate().map(|(i, (_, l))| (i, *l)).collect(), lo, hi));
            }
        }
    }
    None
}

/// A most Health or Force out of reach at the creature's level: asks (overlay 13) whether to
/// change the level, with a chart of what this level allows, what the new one allows and
/// where the typed value falls. False when it is in reach.
fn ask_pool(ui: &AppWindow, st: &mut State, force: bool, now: i64, most: i64) -> bool {
    let Some(cat) = st.catalog() else { return false };
    let Some(c) = st.save.as_ref().and_then(|s| s.creature(st.who())).cloned() else { return false };
    let (lo, hi) = pool_reach(&c, cat.game, force);
    if (lo..=hi).contains(&most) {
        return false;
    }
    let what = if force { "Max Force" } else { "Max Health" };
    let level = creature::level(&c);
    let up = most > hi;
    let nearest = if up { hi } else { lo };
    let found = levels_for_pool(&c, &cat, force, most);
    let app = ui.global::<App>();
    app.set_pool_what(s(what));
    app.set_pool_want(most as i32);
    app.set_pool_low(lo as i32);
    app.set_pool_high(hi as i32);
    app.set_pool_level_now(level as i32);
    if force && hi <= 0 {
        app.set_pool_title(s("No Force"));
        app.set_pool_line(s("This character can't have Force: the game gives Force only with a Jedi class."));
        app.set_pool_line2(s("Add or change a class under Classes first."));
        app.set_pool_new_level(0);
        app.set_pool_chart(false);
        app.set_pool_set_label(s(""));
        app.set_pool_level_label(s(""));
        app.set_overlay(13);
        st.pool_ask = None;
        return true;
    }
    app.set_pool_chart(true);
    let limit = if up {
        format!("At level {}, {} is {} at most.", level, what, thousands(hi))
    } else {
        format!("At level {}, {} is {} at least.", level, what, thousands(lo))
    };
    let levels = match &found {
        Some((l, nlo, nhi)) => {
            let total: i64 = l.iter().map(|(_, v)| *v).sum();
            app.set_pool_title(s(if up { "Raise the level?" } else { "Lower the level?" }));
            app.set_pool_line(s(format!("{} {} needs a {} level.", what, thousands(most), if up { "higher" } else { "lower" })));
            app.set_pool_line2(s(format!("{} At level {} it fits.", limit, total)));
            app.set_pool_new_level(total as i32);
            app.set_pool_new_low(*nlo as i32);
            app.set_pool_new_high(*nhi as i32);
            app.set_pool_level_label(s(format!("{} to Level {}", if up { "Raise" } else { "Lower" }, total)));
            Some(l.clone())
        }
        None => {
            app.set_pool_title(s("Out of reach"));
            app.set_pool_line(s(format!("{} {} can't be reached at any level.", what, thousands(most))));
            app.set_pool_line2(s(limit));
            app.set_pool_new_level(0);
            app.set_pool_level_label(s(""));
            None
        }
    };
    app.set_pool_set_label(s(format!("Set to {}", thousands(nearest))));
    st.pool_ask = Some(PoolAsk { who: st.who(), force, now, most, levels, nearest });
    app.set_overlay(13);
    true
}

/// Sets the most and the current Health (or Force), as typed.
fn set_pool(ui: &AppWindow, st: &mut State, force: bool, now: i64, most: i64) {
    let Some(game) = st.save.as_ref().map(|s| s.game) else { return };
    if force {
        edit_who(ui, st, "fp", move |c| {
            creature::set_max_force(c, game, most);
            creature::set_current_force(c, game, now);
        });
    } else {
        edit_who(ui, st, "hp", move |c| {
            creature::set_max_health(c, most);
            creature::set_current_health(c, now);
        });
    }
}

fn edit_who(ui: &AppWindow, st: &mut State, key: &str, f: impl Fn(&mut Struct)) {
    let who = st.who();
    if let Some(save) = st.save.as_mut() {
        save.edit(who, key, f);
    }
    st.status = (String::new(), false);
    refresh_after_edit(ui, st);
}

/// Without the game's files (a save opened on a PC without the game), the edits that need the
/// game's rules or its files are refused with a word why: class and level changes and the most
/// Health and Force (the classes' dice), new items (the game's templates), Skip Ahead, Go Back
/// and travel (its quests' items and areas), spawning and companions brought in (its
/// creatures). Everything the save holds can still be seen and its numbers changed
/// (2026-10-24, the maintainer).
fn needs_game(ui: &AppWindow, st: &State) -> bool {
    if st.read_only {
        let why = st.save.as_ref().and_then(|s| read_only_reason(&s.folder).or_else(|| s.module_problem())).unwrap_or("It is read only.");
        message(ui, "This save cannot be changed", why);
        return true;
    }
    if st.has_game_files() {
        return false;
    }
    let title = st.save.as_ref().map_or(st.game, |s| s.game).title();
    message(
        ui,
        &format!("This needs {} on this PC", title),
        &format!(
            "It uses the game's own rules and files, which are not here. Pick the game's folder in Settings, or open this save on a PC with {} installed.",
            title
        ),
    );
    true
}

/// Whether a typed Health or Force pair changes the most, not only the current value.
fn most_changes(st: &State, force: bool, most: i64) -> bool {
    let who = st.who();
    let Some(c) = st.save.as_ref().and_then(|s| s.creature(who)) else { return false };
    let now_most = if force { creature::force(c, st.game).1 } else { creature::health(c).1 };
    most != now_most
}

/// The save Copy copies: the one open in the editor, or on the saves page the one picked.
fn copy_source(ui: &AppWindow, st: &State) -> Option<(Game, PathBuf)> {
    if ui.global::<App>().get_page() == 1 {
        st.save.as_ref().map(|s| (s.game, s.folder.clone()))
    } else {
        st.saves.get(st.selected).map(|(f, _)| (st.game, f.path.clone()))
    }
}

/// Picks the save in `folder` in the saves list and shows it beside the list.
fn pick_save(ui: &AppWindow, st: &mut State, folder: &Path) {
    if let Some(i) = st.saves.iter().position(|(f, _)| f.path == folder) {
        st.selected = i;
        ui.global::<App>().set_selected_save(i as i32);
        fill_preview(ui, st);
    }
}

/// A template entry in `slot` made into the whole item, so it can go to the inventory. False,
/// and says so, when the game's template for it cannot be read.
fn whole_item_in_slot(ui: &AppWindow, st: &mut State, who: Who, slot: u32) -> bool {
    let Some(save) = st.save.as_ref() else { return false };
    let Some(resref) = save.slot_is_template(who, slot) else { return true };
    let like = save.inventory().first().cloned();
    let item = st.data().and_then(|d| d.gff(&resref, "uti", kse_core::formats::erf::RES_TYPE_UTI)).map(|g| catalog::item_from_template(&g.root, like.as_ref(), 1));
    match item {
        Some(item) => {
            st.save.as_mut().unwrap().fill_template_slot(who, slot, item);
            true
        }
        None => {
            message(ui, "That item was left where it is", &format!("The game's template {} could not be read, so the item cannot go to the inventory.", resref));
            false
        }
    }
}

fn refresh_after_edit(ui: &AppWindow, st: &mut State) {
    undo_read_only(ui, st);
    // the whole window drawn again after the edit (app.slint says why)
    let app = ui.global::<App>();
    app.set_repaint(!app.get_repaint());
    match st.tab {
        0 => fill_character(ui, st),
        1 => fill_abilities(ui, st),
        2 => fill_inventory(ui, st),
        3 => fill_party(ui, st),
        5 => fill_map(ui, st),
        _ => fill_story(ui, st),
    }
    fill_bar(ui, st);
}

fn open_items_picker(ui: &AppWindow, st: &mut State, target: Target, title: &str) {
    st.target = target;
    st.picker_query.clear();
    let app = ui.global::<App>();
    app.set_picker_placeholder(s("Type part of a name: medpac, lightsaber, g_w_ ..."));
    app.set_overlay_title(s(title));
    app.set_overlay_note(s(
        "Every item the game has, mods included. Type to narrow the list; click an item to add one.",
    ));
    app.set_overlay(1);
    st.picker_tab = 0;
    app.set_picker_tab(0);
    app.set_picker_tabs(model(catalog::ITEM_CATEGORIES.iter().map(|c| s(c)).collect()));
    let _ = st.item_templates();
    search_items(ui, st);
}

fn search_items(ui: &AppWindow, st: &mut State) {
    let templates = st.item_templates();
    let q = st.picker_query.trim().to_lowercase();
    let mut ids = Vec::new();
    let mut rows = Vec::new();
    let tab = st.picker_tab;
    for (i, t) in templates.iter().enumerate() {
        if tab != 0 && t.category != tab {
            continue;
        }
        if !q.is_empty() && !t.name.to_lowercase().contains(&q) && !t.resref.contains(&q) && !t.kind.to_lowercase().contains(&q) {
            continue;
        }
        ids.push(i);
        let base = st.data().map(|d| d.table("baseitems"));
        let fallback = base.as_ref().map(|b| catalog::item_icon(b, t.base_item as usize, 1).1).unwrap_or_default();
        let icon = st.icon(&t.icon, &fallback);
        rows.push(Named {
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            ..row(i as i64, &t.name, format!("{} · {}", t.kind, t.resref))
        });
        if rows.len() >= 200 {
            break;
        }
    }
    st.picker_ids = ids;
    // the list stops at 200 rows: say so, or most items look missing
    let more = rows.len() >= 200;
    ui.global::<App>().set_overlay_note(s(if more {
        "Every item the game has, mods included. The first 200 that match are shown: type part of a name to narrow the list; click an item to add one."
    } else {
        "Every item the game has, mods included. Type to narrow the list; click an item to add one."
    }));
    ui.global::<App>().set_picker(model(rows));
}

/// A searchable list pop-up for anything but items.
fn open_list(ui: &AppWindow, st: &mut State, target: Target, title: &str, note: &str, rows: Vec<Named>) {
    st.target = target;
    st.picker_query.clear();
    st.picker_all = rows.clone();
    let app = ui.global::<App>();
    app.set_picker_tabs(model(Vec::new()));
    app.set_picker_placeholder(s("Type part of a name"));
    app.set_overlay_title(s(title));
    app.set_overlay_note(s(note));
    app.set_picker(model(rows));
    app.set_overlay(1);
}

fn search_list(ui: &AppWindow, st: &State) {
    let q = st.picker_query.trim().to_lowercase();
    let rows: Vec<Named> = st
        .picker_all
        .iter()
        .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q) || r.note.to_lowercase().contains(&q))
        .cloned()
        .collect();
    ui.global::<App>().set_picker(model(rows));
}

fn catalogue_rows(list: &[kse_core::catalog::Entry], current: Option<i64>) -> Vec<Named> {
    list.iter()
        .map(|e| Named { owned: Some(e.id) == current, ..row(e.id, &e.name, &e.description) })
        .collect()
}

fn new_item(st: &mut State, index: usize, like: Option<&Struct>) -> Option<Struct> {
    let templates = st.item_templates();
    let t = templates.get(index)?;
    let data = st.data()?;
    let g = data.gff(&t.resref, "uti", RES_TYPE_UTI)?;
    Some(catalog::item_from_template(&g.root, like, 1))
}

fn open_save_picker(ui: &AppWindow, st: &mut State, target: Target, title: &str, note: &str) {
    let current = st.save.as_ref().map(|s| s.folder.clone());
    let npc = if let Target::ImportNpc(i) = target { Some(i) } else { None };
    let mut others: Vec<SaveFolder> = st
        .saves
        .iter()
        .map(|(f, _)| f.clone())
        .filter(|f| Some(&f.path) != current.as_ref())
        .collect();
    if let Some(i) = npc {
        others.retain(|f| {
            std::fs::read_dir(&f.path).is_ok()
                && Save::open(&f.path).map_or(false, |sv| sv.creature(Who::Npc(i)).is_some())
        });
    }
    // A save gone from the disk has no row, and rows and `others` must stay in step.
    others.retain(|f| save::summary(&f.path).is_some());
    let shots = st.shots.lock().unwrap().clone();
    let rows: Vec<SaveRow> = others
        .iter()
        .filter_map(|f| {
            let sm = save::summary(&f.path)?;
            let shot = shots.get(&f.path).cloned().flatten();
            Some(SaveRow {
                name: s(sm.name),
                area: s(sm.area),
                time: s(played(sm.seconds)),
                date: s(when(sm.modified)),
                tag: s(""),
                has_shot: shot.is_some(),
                shot: shot.map(|i| image(&i)).unwrap_or_default(),
            })
        })
        .collect();
    st.other_saves = others;
    st.target = target;
    let app = ui.global::<App>();
    app.set_other_saves(model(rows));
    app.set_overlay_title(s(title));
    app.set_overlay_note(s(if st.other_saves.is_empty() { "No other save of this game has what is needed." } else { note }));
    app.set_overlay(3);
}

/// A panic ends the release build at once, so it is told here: to a file beside the settings,
/// and on Windows, where the program has no console, in a box.
fn report_panics() {
    std::panic::set_hook(Box::new(|info| {
        let path = games::data_dir().join("crash.txt");
        let _ = std::fs::create_dir_all(games::data_dir());
        let _ = std::fs::write(&path, format!("{}\n{}\n", save::timestamp(), info));
        let text = format!(
            "The editor hit an error it did not expect and has to close. Changes not saved are lost; saves on disk keep what was last saved.\n\n{}\n\nThe details are in {}",
            info,
            path.display()
        );
        #[cfg(windows)]
        {
            #[link(name = "user32")]
            extern "system" {
                fn MessageBoxW(window: *mut core::ffi::c_void, text: *const u16, title: *const u16, kind: u32) -> i32;
            }
            let wide = |t: &str| t.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
            // 0x10: the error icon. Safe: both strings end in a zero and outlive the call.
            unsafe { MessageBoxW(std::ptr::null_mut(), wide(&text).as_ptr(), wide("KOTOR Modern Gamesave Editor").as_ptr(), 0x10) };
        }
        #[cfg(not(windows))]
        eprintln!("{}", text);
    }));
}

fn main() {
    report_panics();
    let ui = AppWindow::new().expect("the window could not be made");
    ui.global::<App>().set_gm_glow(glow_image());
    {
        // A mouse press anywhere first takes the focus out of a text box, so a number typed
        // and then left counts as entered (the boxes treat losing focus as Enter).
        use slint::winit_030::{winit::event::{ElementState, WindowEvent}, EventResult, WinitWindowAccessor};
        let weak = ui.as_weak();
        ui.window().on_winit_window_event(move |_, event| {
            if let WindowEvent::MouseInput { state: ElementState::Pressed, .. } = event {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_drop_focus();
                }
            }
            EventResult::Propagate
        });
    }
    let settings = Settings::load();
    let installs = installs_for(&settings);
    let k2_only = installs.iter().all(|i| i.game == Game::K2) && !installs.is_empty();
    let k1_only = installs.iter().all(|i| i.game == Game::K1) && !installs.is_empty();
    let game = if k2_only || (settings.k2 && !k1_only) { Game::K2 } else { Game::K1 };
    ui.global::<Theme>().set_dark(!settings.light);
    ui.global::<Theme>().set_text_size(settings.text_size);
    let state: Shared = Rc::new(RefCell::new(State {
        settings,
        installs,
        game,
        games: HashMap::new(),
        ready: Arc::new(Mutex::new(HashMap::new())),
        ready_items: Arc::new(Mutex::new(HashMap::new())),
        picker_tab: 0,
        inventory_tab: 0,
        picker_all: Vec::new(),
        quest_tags: Vec::new(),
        quest_tab: 0,
        quest_pick: None,
        chain_choices: Vec::new(),
        map: None,
        map_pick: None,
        saves: Vec::new(),
        shots: Arc::new(Mutex::new(HashMap::new())),
        portraits: HashMap::new(),
        selected: 0,
        save: None,
        original: None,
        read_only: false,
        whos: Vec::new(),
        who: 0,
        tab: 0,
        story: 3,
        feat_query: String::new(),
        power_query: String::new(),
        picker_query: String::new(),
        picker_ids: Vec::new(),
        target: Target::None,
        other_saves: Vec::new(),
        compare: None,
        global_filter: (String::new(), false),
        selected_box: None,
        spawnable: Vec::new(),
        spawn_at: None,
        equip_slot: None,
        weapon_set: 0,
        arrivals: HashMap::new(),
        travel_ways: Vec::new(),
        module_titles: HashMap::new(),
        map_lists: Arc::new(Mutex::new(HashMap::new())),
        map_lists_asked: std::collections::HashSet::new(),
        art: Arc::new(Mutex::new(HashMap::new())),
        art_asked: std::collections::HashSet::new(),
        open_planets: std::collections::HashSet::new(),
        skip_open: std::collections::HashSet::new(),
        skip_stage: None,
        timeline_saves: None,
        timeline_load: Arc::new(Mutex::new(None)),
        timeline_loading: false,
        timeline_pick: None,
        quest_xp: HashMap::new(),
        route_plan: Vec::new(),
        open_ask: None,
        pop_pick: None,
        quest_open: false,
        stage_pick: None,
        pending_spawns: Vec::new(),
        pool_ask: None,
        back_to: None,
        story_plan: None,
        story_prepared: None,
        story_result: Arc::new(Mutex::new(None)),
        status: (String::new(), false),
    }));
    fill_saves(&ui, &mut state.borrow_mut());
    {
        // The other game's data and both games' item lists, made while the player looks
        // at the list; the first switch and the first Add an Item then cost nothing.
        let st = state.borrow();
        let (ready, ready_items, current) = (st.ready.clone(), st.ready_items.clone(), st.game);
        let jobs: Vec<(Game, PathBuf)> = [Game::K1, Game::K2]
            .iter()
            .filter_map(|g| st.installs.iter().find(|i| i.game == *g).map(|i| (*g, i.path.clone())))
            .collect();
        std::thread::spawn(move || {
            for (game, path) in jobs {
                if game == current {
                    let data = GameData::open(game, &path);
                    ready_items.lock().unwrap().insert(game, Arc::new(catalog::item_templates(&data)));
                } else {
                    let built = build_game(game, &path, true);
                    ready.lock().unwrap().insert(game, built);
                }
            }
        });
    }

    // ---- the save list ----
    on!(ui, state, on_switch_game, |u, st, k2: bool| {
        u.global::<App>().set_renaming_save(false);
        st.game = if k2 { Game::K2 } else { Game::K1 };
        st.settings.k2 = k2;
        st.selected = 0;
        st.portraits.clear();
        fill_saves(&u, st);
    });
    on!(ui, state, on_select_save, |u, st, i: i32| {
        u.global::<App>().set_renaming_save(false);
        st.selected = i.max(0) as usize;
        u.global::<App>().set_selected_save(i);
        fill_preview(&u, st);
    });
    on!(ui, state, on_open_save, |u, st| { open_selected(&u, st); });
    on!(ui, state, on_show_folder, |_u, st| {
        if let Some((f, _)) = st.saves.get(st.selected) {
            open_path(&f.path);
        }
    });
    on!(ui, state, on_open_saves_folder, |_u, st| {
        let folder = st.saves.get(st.selected).and_then(|(f, _)| f.path.parent().map(|p| p.to_path_buf())).or_else(|| {
            st.installs.iter().find(|i| i.game == st.game).map(|i| i.path.clone())
        });
        if let Some(f) = folder {
            open_path(&f);
        }
    });
    on!(ui, state, on_add_save_folder, |u, st| {
        if let Some(folder) = pick_folder(&u, "Pick a save, a saves folder or a game folder") {
            if !st.settings.added.contains(&folder) {
                st.settings.added.push(folder.clone());
                st.settings.store();
            }
            st.installs = installs_for(&st.settings);
            if let Some(g) = games::identify(&folder).or_else(|| save::peek_game(&folder)) {
                st.game = g;
            }
            fill_saves(&u, st);
        }
    });

    on!(ui, state, on_choose_game_folder, |u, st| {
        let Some(folder) = pick_folder(&u, "Pick the game's folder (the one with chitin.key)") else { return };
        match games::identify(&folder) {
            Some(game) => {
                match game {
                    Game::K1 => st.settings.k1_dir = Some(folder),
                    Game::K2 => st.settings.k2_dir = Some(folder),
                }
                st.settings.store();
                st.installs = installs_for(&st.settings);
                st.games.remove(&game);
                st.ready.lock().unwrap().remove(&game);
                st.ready_items.lock().unwrap().remove(&game);
                st.portraits.clear();
                st.game = game;
                st.selected = 0;
                fill_saves(&u, st);
            }
            None => message(&u, "That is not a KotOR folder", "Pick the folder that holds chitin.key and the game's program."),
        }
    });

    // ---- settings ----
    on!(ui, state, on_open_settings, |u, st| {
        fill_settings(&u, st);
        u.global::<App>().set_overlay(7);
    });
    on!(ui, state, on_settings_changed, |u, st| {
        let theme = u.global::<Theme>();
        st.settings.light = !theme.get_dark();
        st.settings.text_size = theme.get_text_size();
        st.settings.store();
    });
    on!(ui, state, on_remove_save_folder, |u, st, i: i32| {
        if (i as usize) < st.settings.added.len() {
            st.settings.added.remove(i as usize);
            st.settings.store();
            st.installs = installs_for(&st.settings);
            st.selected = 0;
            fill_saves(&u, st);
        }
    });

    // ---- the editor ----
    on!(ui, state, on_back, |u, st| {
        if unsaved(&u, st) {
            return;
        }
        st.save = None;
        st.original = None;
        u.global::<App>().set_page(0);
        fill_saves(&u, st);
    });
    on!(ui, state, on_select_who, |u, st, i: i32| {
        st.who = i.max(0) as usize;
        st.feat_query.clear();
        st.power_query.clear();
        u.global::<App>().set_ability_reset(u.global::<App>().get_ability_reset().wrapping_add(1));
        st.power_query.clear();
        u.global::<App>().set_ability_reset(u.global::<App>().get_ability_reset().wrapping_add(1));
        fill_editor(&u, st);
    });
    on!(ui, state, on_select_tab, |u, st, i: i32| {
        st.tab = i;
        // Timeline is section 5 alone, Story holds 3 and 4, Advanced 0 to 2: each opens on
        // its first
        if i == 7 {
            st.story = 5;
        } else if i == 4 && st.story != 3 && st.story != 4 {
            st.story = 3;
        } else if i == 6 && st.story >= 3 {
            st.story = 0;
        }
        u.global::<App>().set_tab(i);
        refresh_after_edit(&u, st);
    });

    // Character
    on!(ui, state, on_set_name, |u, st, text: SharedString| {
        if text.trim().is_empty() {
            return;
        }
        let t = text.to_string();
        let who = st.who();
        if let Some(save) = st.save.as_mut() {
            save.edit(who, "name", |c| creature::set_name(c, &t));
            let bytes = kse_core::text::encode(&t);
            if who == Who::Player && save.party.root.get("PT_PCNAME").is_some() {
                save.edit_party_table("name", |p| p.set("PT_PCNAME", Value::String(bytes.clone())));
            }
        }
        if undo_read_only(&u, st) {
            fill_editor(&u, st);
            return;
        }
        // the member list shows the new name (only the name: the box keeps its caret)
        let app = u.global::<App>();
        let name = st.creature_name(st.who());
        if let Some(mut row) = app.get_members().row_data(st.who) {
            row.name = s(if name.is_empty() { "Player".to_string() } else { name });
            app.get_members().set_row_data(st.who, row);
        }
        fill_bar(&u, st);
        app.set_name_changed(true);
    });
    on!(ui, state, on_set_xp, |u, st, xp: i32| {
        let xp = xp.max(0) as i64;
        edit_who(&u, st, "xp", |c| creature::set_experience(c, xp));
    });
    on!(ui, state, on_xp_to_next, |u, st| {
        let Some(cat) = st.catalog() else { return };
        let level = st.save.as_ref().and_then(|s| s.creature(st.who())).map(creature::level).unwrap_or(1);
        let current = st.save.as_ref().and_then(|s| s.creature(st.who())).map(creature::experience).unwrap_or(0);
        let mut target = cat.xp_for(level + 1).unwrap_or(current);
        if current >= target {
            target = cat.xp_for(cat.level_for(current) + 1).unwrap_or(current);
        }
        if target > current {
            edit_who(&u, st, "xp", |c| creature::set_experience(c, target));
        }
    });
    on!(ui, state, on_xp_to_max, |u, st| {
        let Some(cat) = st.catalog() else { return };
        if let Some(xp) = cat.xp_for(cat.max_level()) {
            edit_who(&u, st, "xp", |c| creature::set_experience(c, xp));
        }
    });
    on!(ui, state, on_set_alignment, |u, st, v: i32| {
        edit_who(&u, st, "alignment", |c| creature::set_alignment(c, v as i64));
    });
    on!(ui, state, on_set_attribute, |u, st, i: i32, v: i32| {
        let label = creature::ATTRIBUTES[i.clamp(0, 5) as usize];
        edit_who(&u, st, label, |c| creature::set_attribute(c, label, v as i64));
    });
    on!(ui, state, on_heal_all, |u, st| {
        let whos = st.whos.clone();
        if let Some(save) = st.save.as_mut() {
            let game = save.game;
            for w in whos {
                save.edit(w, "heal", |c| creature::heal(c, game));
            }
        }
        refresh_after_edit(&u, st);
        fill_editor(&u, st);
    });
    on!(ui, state, on_open_look, |u, st| {
        let Some(cat) = st.catalog() else { return };
        let current = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| c.int("PortraitId")).unwrap_or(-1);
        let mut rows = Vec::new();
        let mut list = cat.portraits.clone();
        list.sort_by_key(|p| !p.for_pc);
        for p in list {
            if let Some(img) = st.portrait(&p.resref) {
                rows.push(PortraitRow { id: p.id as i32, image: img, female: p.female, current: p.id == current });
            }
        }
        let app = u.global::<App>();
        app.set_looks(model(rows));
        app.set_overlay_title(s("Change Look"));
        app.set_overlay_note(s(
            "Every portrait the game has, the ones character creation offers first. Picking one of those also brings its body in the same build as now; any other portrait changes the picture only.",
        ));
        app.set_overlay(2);
    });
    on!(ui, state, on_pick_look, |u, st, id: i32| {
        let Some(cat) = st.catalog() else { return };
        let Some(p) = cat.portrait_for(id as i64).cloned() else { return };
        let current_id = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| c.int("PortraitId")).unwrap_or(-1);
        let current = cat.portrait_for(current_id).cloned();
        u.global::<App>().set_overlay(0);
        if p.for_pc && p.bodies[1] > 0 {
            edit_who(&u, st, "look", |c| creature::set_look(c, &p, current.as_ref()));
        } else {
            edit_who(&u, st, "look", |c| creature::set_portrait(c, p.id));
        }
        fill_editor(&u, st);
    });
    on!(ui, state, on_open_add_class, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        let Some(cat) = st.catalog() else { return };
        let Some(c) = st.save.as_ref().and_then(|s| s.creature(st.who())).cloned() else { return };
        let have: Vec<i64> = creature::classes(&c).iter().map(|(id, _)| *id).collect();
        let droid = have.iter().any(|id| cat.class(*id).map_or(false, |k| k.droid));
        let rows = cat
            .classes
            .iter()
            .filter(|k| !have.contains(&k.id) && k.droid == droid && !k.name.is_empty())
            .map(|k| {
                let note = if k.force_die > 0 {
                    format!("Jedi class: {} Health and {} Force a level", k.hit_die, k.force_die)
                } else {
                    format!("{} Health a level", k.hit_die)
                };
                (k.id, k.name.clone(), note)
            });
        st.target = Target::Class;
        let app = u.global::<App>();
        app.set_picker(named(rows));
        app.set_overlay_title(s("Add a Class"));
        app.set_overlay_note(s(
            "A second class at level 1, as a level-up into it would leave the character: its Health and Force are added and the XP is raised to the new level if needed. The game knows two classes at most.",
        ));
        app.set_overlay(5);
    });
    on!(ui, state, on_set_position, |u, st, x: SharedString, y: SharedString, z: SharedString| {
        // "nan", "inf" and "1e30" parse as numbers too, and would be written as the player's
        // place (2026-11-06 audit); no area of either game reaches 100,000 units
        let number = |t: &SharedString| t.trim().parse::<f32>().ok().filter(|v| v.is_finite() && v.abs() < 100_000.0);
        let (Some(x), Some(y), Some(z)) = (number(&x), number(&y), number(&z)) else {
            message(&u, "That is not a position", "Type three numbers, for example 12.5, -3 and 0.");
            fill_editor(&u, st);
            return;
        };
        if st.who() != Who::Player {
            message(&u, "Only the player can be moved here", "This moves the player. A companion's place is where the game puts the party when the save loads.");
            return;
        }
        edit_who(&u, st, "position", |c| area::set_position(c, [x, y, z]));
    });

    // Abilities
    on!(ui, state, on_set_skill, |u, st, i: i32, v: i32| {
        let i = i.max(0) as usize;
        edit_who(&u, st, &format!("skill{}", i), |c| creature::set_skill(c, i, v as i64));
    });
    on!(ui, state, on_search_feats, |u, st, q: SharedString| {
        st.feat_query = q.to_string();
        fill_abilities(&u, st);
    });
    on!(ui, state, on_search_powers, |u, st, q: SharedString| {
        st.power_query = q.to_string();
        fill_abilities(&u, st);
    });
    on!(ui, state, on_add_feat, |u, st, id: i32| {
        st.feat_query.clear();
        u.global::<App>().set_ability_reset(u.global::<App>().get_ability_reset().wrapping_add(1));
        edit_who(&u, st, &format!("feat{}", id), |c| {
            creature::add_feat(c, id as i64);
        });
    });
    on!(ui, state, on_remove_feat, |u, st, id: i32| {
        edit_who(&u, st, &format!("feat{}", id), |c| {
            creature::remove_feat(c, id as i64);
        });
    });
    on!(ui, state, on_add_power, |u, st, id: i32| {
        st.power_query.clear();
        u.global::<App>().set_ability_reset(u.global::<App>().get_ability_reset().wrapping_add(1));
        let Some(cat) = st.catalog() else { return };
        edit_who(&u, st, &format!("power{}", id), move |c| {
            creature::add_power(c, id as i64, &|k| cat.is_force_class(k));
        });
    });
    on!(ui, state, on_remove_power, |u, st, id: i32| {
        edit_who(&u, st, &format!("power{}", id), |c| {
            creature::remove_power(c, id as i64);
        });
    });

    // Inventory
    on!(ui, state, on_set_credits, |u, st, v: i32| {
        if let Some(save) = st.save.as_mut() {
            save.set_party_int("PT_GOLD", v.max(0) as i64);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_components, |u, st, v: i32| {
        if let Some(save) = st.save.as_mut() {
            save.set_party_int("PT_ITEM_COMPONEN", v.max(0) as i64);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_chemicals, |u, st, v: i32| {
        if let Some(save) = st.save.as_mut() {
            save.set_party_int("PT_ITEM_CHEMICAL", v.max(0) as i64);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_stack, |u, st, row: i32, v: i32| {
        let row = row.max(0) as usize;
        if let Some(save) = st.save.as_mut() {
            save.edit_inventory(&format!("stack{}", row), |items| {
                if let Some(it) = items.get_mut(row) {
                    it.set_int("StackSize", v.max(1) as i64, Value::Word(1));
                }
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_remove_item, |u, st, row: i32| {
        let row = row.max(0) as usize;
        if let Some(save) = st.save.as_mut() {
            save.edit_inventory(&format!("remove{}", row), |items| {
                if row < items.len() {
                    items.remove(row);
                }
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_select_inventory_tab, |u, st, i: i32| {
        st.inventory_tab = i.max(0) as usize;
        fill_inventory(&u, st);
    });
    on!(ui, state, on_open_add_item, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        open_items_picker(&u, st, Target::Inventory, "Add an Item");
    });

    // Party
    on!(ui, state, on_set_recruited, |u, st, i: i32, on: bool| {
        let party = st.save.as_ref().map(|s| s.party_members());
        if let Some(save) = st.save.as_mut() {
            save.set_recruited(i.max(0) as usize, on);
        }
        if st.save.as_ref().map(|s| s.party_members()) != party {
            update_party_portraits(st);
        }
        // the member list says who is recruited and in the party
        fill_editor(&u, st);
    });
    on!(ui, state, on_set_selectable, |u, st, i: i32, on: bool| {
        if let Some(save) = st.save.as_mut() {
            save.set_selectable(i.max(0) as usize, on);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_influence, |u, st, i: i32, v: i32| {
        if let Some(save) = st.save.as_mut() {
            save.set_influence(i.max(0) as usize, v as i64);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_party_xp, |u, st, v: i32| {
        if let Some(save) = st.save.as_mut() {
            save.set_party_int("PT_XP_POOL", v.max(0) as i64);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_import_npc, |u, st, i: i32| {
        if needs_game(&u, st) {
            return;
        }
        let name = st.npc_name(i.max(0) as usize);
        open_save_picker(
            &u,
            st,
            Target::ImportNpc(i.max(0) as usize),
            &format!("Bring {} from Another Save", name),
            &format!("{} comes over as they are in the save you pick: level, items worn, feats and powers. They are listed as recruited here.", name),
        );
    });

    // Story
    on!(ui, state, on_select_story, |u, st, i: i32| {
        st.story = i;
        fill_story(&u, st);
    });
    on!(ui, state, on_filter_globals, |u, st, q: SharedString, only: bool| {
        st.global_filter = (q.to_string(), only);
        fill_globals(&u, st);
    });
    on!(ui, state, on_set_global_bool, |u, st, index: i32, on: bool| {
        let index = index.max(0) as usize;
        if let Some(save) = st.save.as_mut() {
            let all = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            if let Some(g) = all.get(index).cloned() {
                save.edit_globals(&format!("b{}", g.index), |root| globals::set(root, &g, &GlobalValue::Bool(on)));
            }
        }
        undo_read_only(&u, st);
        fill_globals(&u, st);
        fill_bar(&u, st);
    });
    on!(ui, state, on_set_global_number, |u, st, index: i32, v: i32| {
        let index = index.max(0) as usize;
        if let Some(save) = st.save.as_mut() {
            let all = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            if let Some(g) = all.get(index).cloned() {
                save.edit_globals(&format!("n{}", g.index), |root| globals::set(root, &g, &GlobalValue::Number(v as i64)));
            }
        }
        undo_read_only(&u, st);
        fill_globals(&u, st);
        fill_bar(&u, st);
    });
    on!(ui, state, on_set_global_text, |u, st, index: i32, t: SharedString| {
        let index = index.max(0) as usize;
        if let Some(save) = st.save.as_mut() {
            let all = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            if let Some(g) = all.get(index).cloned() {
                save.edit_globals(&format!("s{}", g.index), |root| globals::set(root, &g, &GlobalValue::Text(t.to_string())));
            }
        }
        undo_read_only(&u, st);
        fill_globals(&u, st);
        fill_bar(&u, st);
    });
    on!(ui, state, on_open_compare, |u, st| {
        open_save_picker(
            &u,
            st,
            Target::Compare,
            "Compare with Another Save",
            "Pick a save from before or after a quest. Every global variable that differs is marked, with the other save's value beside it.",
        );
    });
    on!(ui, state, on_use_other, |u, st, index: i32| {
        let index = index.max(0) as usize;
        let Some(theirs) = st.compare.as_ref().map(|c| c.1.clone()) else { return };
        if let Some(save) = st.save.as_mut() {
            let all = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            if let Some(g) = all.get(index).cloned() {
                if let Some(t) = theirs.iter().find(|t| t.kind == g.kind && t.name == g.name) {
                    let v = t.value.clone();
                    save.edit_globals(&format!("{:?}{}", g.kind, g.index), |root| globals::set(root, &g, &v));
                }
            }
        }
        undo_read_only(&u, st);
        fill_globals(&u, st);
        fill_bar(&u, st);
    });
    on!(ui, state, on_use_all_other, |u, st| {
        let Some(theirs) = st.compare.as_ref().map(|c| c.1.clone()) else { return };
        if let Some(save) = st.save.as_mut() {
            let all = save.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
            for (g, v) in globals::compare(&all, &theirs) {
                save.edit_globals(&format!("{:?}{}", g.kind, g.index), |root| globals::set(root, &g, &v));
            }
        }
        undo_read_only(&u, st);
        fill_globals(&u, st);
        fill_bar(&u, st);
    });
    on!(ui, state, on_set_door_locked, |u, st, i: i32, on: bool| {
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("door{}", i), |git| area::set_door_locked(git, i.max(0) as usize, on));
        }
        st.map = None;
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_door_open, |u, st, i: i32, on: bool| {
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("door{}", i), |git| area::set_door_open(git, i.max(0) as usize, on));
        }
        st.map = None;
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_select_box, |u, st, row: i32| {
        let Some(save) = st.save.as_ref() else { return };
        let Some(git) = save.area_git() else { return };
        let boxes = area::containers(&git.root, &|p: &Struct| p.string("Tag").unwrap_or_default());
        st.selected_box = boxes.get(row.max(0) as usize).map(|b| (b.list, b.index));
        fill_area(&u, st);
    });
    on!(ui, state, on_set_box_stack, |u, st, row: i32, stack: i32| {
        let Some((list, index)) = st.selected_box else { return };
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("box{}{}.{}", list, index, row), |git| {
                if let Some(item) = area::container_mut(git, list, index).and_then(|c| c.list_mut("ItemList")).and_then(|l| l.get_mut(row.max(0) as usize)) {
                    item.set_int("StackSize", stack.max(1) as i64, Value::Word(1));
                }
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_remove_box_item, |u, st, row: i32| {
        let Some((list, index)) = st.selected_box else { return };
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("box{}{}", list, index), |git| {
                if let Some(items) = area::container_mut(git, list, index).and_then(|c| c.list_mut("ItemList")) {
                    let row = row.max(0) as usize;
                    if row < items.len() {
                        items.remove(row);
                    }
                    if list == area::STORES {
                        for (i, it) in items.iter_mut().enumerate() {
                            it.id = i as u32;
                        }
                    }
                }
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_add_box_item, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        open_items_picker(&u, st, Target::Container, "Add an Item Here");
    });
    on!(ui, state, on_set_planet, |u, st, i: i32, on: bool| {
        // without the game's planetary.2da the Endar Spire, the Leviathan and the Star Forge
        // cannot be told from places the galaxy map flies to, and opening one strands the ship
        if needs_game(&u, st) {
            return;
        }
        if let Some(save) = st.save.as_mut() {
            save.edit_party_table(&format!("planet{}", i), |p| {
                if let Some(map) = p.child_mut("GlxyMap") {
                    let mask = map.int("GlxyMapPlntMsk").unwrap_or(0);
                    // The game keeps two 16-bit halves and sets a planet's bit in both
                    // (measured on real saves: Dantooine to Korriban and Yavin, 3..7 and
                    // 11, and the same again from 19).
                    let i = i.clamp(0, 15);
                    let bit = (1i64 << i) | (1i64 << (i + 16));
                    let mask = if on { mask | bit } else { mask & !bit };
                    map.set_int("GlxyMapPlntMsk", mask, Value::Dword(0));
                }
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_cheat, |u, st, on: bool| {
        if let Some(save) = st.save.as_mut() {
            save.edit_party_table("cheat", |p| {
                p.set_int("PT_CHEAT_USED", on as i64, Value::Byte(0));
            });
            save.edit_nfo(|n| {
                n.set_int("CHEATUSED", on as i64, Value::Byte(0));
            });
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_pazaak, |u, st, i: i32, v: i32| {
        if i < 0 || i as usize >= pazaak_cards(st.game) {
            return;
        }
        if let Some(save) = st.save.as_mut() {
            save.edit_party_table(&format!("pazaak{}", i), |p| {
                if let Some(card) = p.list_mut("PT_PAZAAKCARDS").and_then(|l| l.get_mut(i.max(0) as usize)) {
                    card.set_int("PT_PAZAAKCOUNT", v.clamp(0, 9) as i64, Value::Int(0));
                }
            });
        }
        refresh_after_edit(&u, st);
    });

    // ---- the quest tracker ----
    on!(ui, state, on_select_quest_tab, |u, st, i: i32| {
        st.quest_tab = i.clamp(0, 2) as usize;
        st.quest_pick = None;
        fill_journal(&u, st);
    });
    on!(ui, state, on_pick_quest, |u, st, i: i32| {
        // a click on the open quest closes it; on another, opens that one
        let tag = st.quest_tags.get(i.max(0) as usize).cloned();
        let same = tag.is_some() && tag.as_deref().map(str::to_ascii_lowercase) == st.quest_pick.as_deref().map(str::to_ascii_lowercase);
        st.quest_open = !(same && st.quest_open);
        st.quest_pick = tag;
        st.stage_pick = None;
        fill_journal(&u, st);
    });
    on!(ui, state, on_pick_stage, |u, st, id: i32| {
        st.stage_pick = Some(id as i64);
        fill_journal(&u, st);
    });
    on!(ui, state, on_set_quest_stage, |u, st, id: i32| {
        let Some(tag) = st.quest_pick.clone() else { return };
        set_quest_stage(st, &tag, id as i64);
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_finish_quest, |u, st, id: i32| {
        // refused at once on a save that cannot be changed (it said "Finished" before)
        if st.read_only {
            needs_game(&u, st);
            return;
        }
        let Some(tag) = st.quest_pick.clone() else { return };
        // As if played: every step on the way to this ending first, then the ending itself.
        let mut skipped = Vec::new();
        if u.global::<App>().get_quest_as_played() {
            if let (Some((q, _)), Some(data)) = (kse_core::questdb::quest(st.game, &tag), st.data()) {
                // The choices are numbered over every ending in order; find this ending's.
                let mut first = 0;
                for e in q.stages.iter().filter(|x| x.end).map(|x| x.stage) {
                    let (_, c) = kse_core::questdb::path(&q, e);
                    if e == id as i64 {
                        break;
                    }
                    first += c.iter().filter(|x| x.does.iter().any(|d| !kse_core::questdb::describe(d, &data).is_empty())).count();
                }
                let (sure, choices) = kse_core::questdb::path(&q, id as i64);
                let mut chosen = Vec::new();
                let mut n = first;
                for c in choices {
                    if c.does.iter().all(|d| kse_core::questdb::describe(d, &data).is_empty()) {
                        continue;
                    }
                    if st.chain_choices.get(n).copied().unwrap_or(false) {
                        chosen.push(c);
                    }
                    n += 1;
                }
                if let Some(save) = st.save.as_mut() {
                    for (k, step) in sure.iter().chain(chosen.iter()).enumerate() {
                        skipped.extend(kse_core::questdb::apply(save, &data, step, &format!("chain.{}.{}", tag, k)));
                    }
                }
            }
        }
        set_quest_stage(st, &tag, id as i64);
        u.global::<App>().set_overlay(0);
        // The quest moves to Completed; the tracker follows it there.
        st.quest_tab = 1;
        refresh_after_edit(&u, st);
        if !skipped.is_empty() {
            skipped.dedup();
            message(&u, "Finished, with some steps left to the game", &skipped.join("\n"));
        }
    });
    on!(ui, state, on_open_finish_quest, |u, st| {
        // The quest database's steps to each ending, in plain words.
        let app = u.global::<App>();
        let (Some(tag), Some(data)) = (st.quest_pick.clone(), st.data()) else { return };
        let mut lines = Vec::new();
        st.chain_choices.clear();
        match kse_core::questdb::quest(st.game, &tag) {
            Some((q, checked)) => {
                let endings: Vec<i64> = q.stages.iter().filter(|x| x.end).map(|x| x.stage).collect();
                for e in endings {
                    let (sure, choices) = kse_core::questdb::path(&q, e);
                    for step in &sure {
                        for d in &step.does {
                            let text = kse_core::questdb::describe(d, &data);
                            if !text.is_empty() {
                                lines.push(ChainLine { ending: e as i32, text: s(text), can: kse_core::questdb::can_apply(d), branch: s(""), choice: -1, on: true });
                            }
                        }
                    }
                    for step in &choices {
                        let text: Vec<String> = step.does.iter().map(|d| kse_core::questdb::describe(d, &data)).filter(|t| !t.is_empty()).collect();
                        if text.is_empty() {
                            continue;
                        }
                        lines.push(ChainLine {
                            ending: e as i32,
                            text: s(text.join("; ")),
                            can: step.does.iter().any(kse_core::questdb::can_apply),
                            branch: s(step.branch.clone().unwrap_or_default()),
                            choice: st.chain_choices.len() as i32,
                            on: false,
                        });
                        st.chain_choices.push(false);
                    }
                }
                app.set_quest_has_chain(true);
                app.set_quest_chain_checked(checked);
            }
            None => {
                app.set_quest_has_chain(false);
                app.set_quest_chain_checked(false);
            }
        }
        app.set_quest_chain(model(lines));
        app.set_overlay(8);
    });
    on!(ui, state, on_toggle_chain_choice, |u, st, i: i32, on: bool| {
        if let Some(c) = st.chain_choices.get_mut(i.max(0) as usize) {
            *c = on;
        }
        let app = u.global::<App>();
        let rows: Vec<ChainLine> = app.get_quest_chain().iter().map(|mut r| {
            if r.choice == i {
                r.on = on;
            }
            r
        }).collect();
        app.set_quest_chain(model(rows));
    });
    on!(ui, state, on_remove_quest, |u, st| {
        let Some(tag) = st.quest_pick.clone() else { return };
        if let Some(save) = st.save.as_mut() {
            save.remove_quest(&tag);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_save_name, |u, st, t: SharedString| {
        if t.trim().is_empty() {
            return;
        }
        if let Some(save) = st.save.as_mut() {
            save.rename(t.trim());
        }
        refresh_after_edit(&u, st);
    });

    on!(ui, state, on_refresh_saves, |u, st| {
        st.installs = installs_for(&st.settings);
        st.shots.lock().unwrap().clear();
        fill_saves(&u, st);
    });
    on!(ui, state, on_set_hp, |u, st, now: i32, most: i32| {
        if most_changes(st, false, most as i64) && needs_game(&u, st) {
            fill_character(&u, st);
            return;
        }
        if !ask_pool(&u, st, false, now as i64, most as i64) {
            set_pool(&u, st, false, now as i64, most as i64);
        }
    });
    on!(ui, state, on_set_fp, |u, st, now: i32, most: i32| {
        if most_changes(st, true, most as i64) && needs_game(&u, st) {
            fill_character(&u, st);
            return;
        }
        if !ask_pool(&u, st, true, now as i64, most as i64) {
            set_pool(&u, st, true, now as i64, most as i64);
        }
    });
    // the out-of-reach pop-up: the nearest value at this level, or the level change
    on!(ui, state, on_pool_set, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        u.global::<App>().set_overlay(0);
        let Some(a) = st.pool_ask.take() else { return };
        pick_who(st, a.who);
        set_pool(&u, st, a.force, a.now, a.nearest);
    });
    on!(ui, state, on_pool_level, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        u.global::<App>().set_overlay(0);
        let Some(a) = st.pool_ask.take() else { return };
        pick_who(st, a.who);
        let Some(levels) = a.levels else { return };
        let Some(cat) = st.catalog() else { return };
        let Some(have) = st.save.as_ref().and_then(|s| s.creature(st.who())).map(creature::classes) else { return };
        // lowered newest first, raised as planned
        for (i, level) in levels.into_iter().rev() {
            let Some(class) = have.get(i).and_then(|k| cat.class(k.0)).cloned() else { continue };
            if have[i].1 == level {
                continue;
            }
            let xp_cat = cat.clone();
            edit_who(&u, st, &format!("class{}", i), move |c| {
                let keep = creature::other_class_feats(c, i, &|id| xp_cat.class(id).cloned());
                creature::set_class_level_keeping(c, i, level, &class, &|l| xp_cat.xp_for(l), &keep);
            });
        }
        set_pool(&u, st, a.force, a.now, a.most);
        fill_editor(&u, st);
    });
    on!(ui, state, on_pool_cancel, |u, st| {
        st.pool_ask = None;
        u.global::<App>().set_overlay(0);
        fill_character(&u, st);
    });
    on!(ui, state, on_set_min1hp, |u, st, on: bool| {
        edit_who(&u, st, "min1hp", move |c| creature::set_min1hp(c, on));
    });
    on!(ui, state, on_open_body, |u, st| {
        let Some(cat) = st.catalog() else { return };
        let current = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| c.int("Appearance_Type"));
        let rows = catalogue_rows(&cat.appearances, current);
        open_list(&u, st, Target::Appearance, "Body", "Every body in appearance.2da, mods included. A party member's body on the player can take their head from them (KSE's readme describes it); choose with care.", rows);
    });
    on!(ui, state, on_open_voice, |u, st| {
        let Some(cat) = st.catalog() else { return };
        let current = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| c.int("SoundSetFile"));
        let rows = catalogue_rows(&cat.soundsets, current);
        open_list(&u, st, Target::Soundset, "Voice", "The sounds a character makes in combat (soundset.2da).", rows);
    });
    on!(ui, state, on_open_gender, |u, st| {
        let Some(cat) = st.catalog() else { return };
        let current = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| c.int("Gender"));
        let rows = catalogue_rows(&cat.genders, current);
        open_list(&u, st, Target::Gender, "Gender", "Conversations and romances read this. Anything but male or female can stop scenes that expect one of them.", rows);
    });
    on!(ui, state, on_open_classes, |u, st| {
        fill_character(&u, st);
        u.global::<App>().set_overlay(6);
    });
    on!(ui, state, on_set_class_level, |u, st, index: i32, level: i32| {
        if needs_game(&u, st) {
            return;
        }
        let Some(cat) = st.catalog() else { return };
        let Some((id, _)) = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| creature::classes(c).get(index.max(0) as usize).copied()) else { return };
        let Some(class) = cat.class(id).cloned() else { return };
        let index = index.max(0) as usize;
        let xp_cat = cat.clone();
        edit_who(&u, st, &format!("class{}", index), move |c| {
            let keep = creature::other_class_feats(c, index, &|id| xp_cat.class(id).cloned());
            creature::set_class_level_keeping(c, index, level as i64, &class, &|l| xp_cat.xp_for(l), &keep);
        });
        fill_editor(&u, st);
        u.global::<App>().set_overlay(6);
    });
    on!(ui, state, on_change_class, |u, st, index: i32| {
        if needs_game(&u, st) {
            return;
        }
        let Some(cat) = st.catalog() else { return };
        let Some(c) = st.save.as_ref().and_then(|s| s.creature(st.who())).cloned() else { return };
        let have: Vec<i64> = creature::classes(&c).iter().map(|(id, _)| *id).collect();
        let droid = have.iter().any(|id| cat.class(*id).map_or(false, |k| k.droid));
        let rows: Vec<Named> = cat
            .classes
            .iter()
            .filter(|k| !have.contains(&k.id) && k.droid == droid && !k.name.is_empty())
            .map(|k| row(k.id, &k.name, format!("{} Health{} a level", k.hit_die, if k.force_die > 0 { format!(" and {} Force", k.force_die) } else { String::new() })))
            .collect();
        open_list(&u, st, Target::ChangeClass(index.max(0) as usize), "Change Class", "The class takes the old one's place at the same level; Health and Force follow the new class's dice. Leaving a Force class takes its powers away.", rows);
    });
    on!(ui, state, on_remove_class, |u, st, index: i32| {
        if needs_game(&u, st) {
            return;
        }
        let Some(cat) = st.catalog() else { return };
        let Some((id, _)) = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| creature::classes(c).get(index.max(0) as usize).copied()) else { return };
        let Some(class) = cat.class(id).cloned() else { return };
        let index = index.max(0) as usize;
        let xp_cat = cat.clone();
        edit_who(&u, st, &format!("class{}", index), move |c| {
            let keep = creature::other_class_feats(c, index, &|id| xp_cat.class(id).cloned());
            creature::remove_class_keeping(c, index, &class, &|l| xp_cat.xp_for(l), &keep);
        });
        fill_editor(&u, st);
        u.global::<App>().set_overlay(6);
    });
    // A slot picked on the equip grid; picked again, the list shows everything.
    // KotOR II's weapon set shown in the Equipped grid; a weapon slot of the other set that
    // was picked is let go.
    on!(ui, state, on_pick_weapon_set, |u, st, set: i32| {
        st.weapon_set = set.clamp(0, 1) as usize;
        if matches!(st.equip_slot, Some(0x10 | 0x20 | 0x40000 | 0x80000)) {
            st.equip_slot = None;
        }
        fill_inventory(&u, st);
    });
    on!(ui, state, on_open_slot, |u, st, slot: i32| {
        let slot = slot as u32;
        st.equip_slot = if st.equip_slot == Some(slot) || slot == 0 { None } else { Some(slot) };
        fill_inventory(&u, st);
    });
    on!(ui, state, on_equip_item, |u, st, index: i32| {
        if needs_game(&u, st) {
            return;
        }
        let (Some(slot), who) = (st.equip_slot, st.who()) else { return };
        if !whole_item_in_slot(&u, st, who, slot) {
            return;
        }
        // Two hands on one weapon: what the left hand holds goes to the inventory first.
        let left = match slot { 0x10 => Some(0x20), 0x40000 => Some(0x80000), _ => None };
        let both = st.data().map(|d| d.table("baseitems")).zip(st.save.as_ref().and_then(|s| s.inventory().get(index.max(0) as usize).cloned())).map_or(false, |(b, it)| two_handed(&b, &it));
        if let (Some(left), true) = (left, both) {
            if !whole_item_in_slot(&u, st, who, left) {
                return;
            }
            if let Some(save) = st.save.as_mut() {
                save.unequip(who, left);
            }
        }
        if let Some(save) = st.save.as_mut() {
            save.equip(who, slot, index.max(0) as usize);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_unequip_slot, |u, st| {
        let (Some(slot), who) = (st.equip_slot, st.who()) else { return };
        if !whole_item_in_slot(&u, st, who, slot) {
            return;
        }
        if let Some(save) = st.save.as_mut() {
            save.unequip(who, slot);
        }
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_copy_inventory, |u, st| {
        open_save_picker(&u, st, Target::CopyInventory, "Copy Inventory from Another Save", "The party inventory of the save you pick replaces this one's. Equipped items stay as they are.");
    });
    on!(ui, state, on_set_in_party, |u, st, npc: i32, on: bool| {
        let Some(save) = st.save.as_mut() else { return };
        let mut members = save.party_members();
        let npc = npc.max(0) as usize;
        if on && !members.contains(&npc) {
            members.push(npc);
        } else if !on {
            members.retain(|m| *m != npc);
        }
        if let Err(e) = save.set_party_members(&members) {
            message(&u, "The party was not changed", &e);
            return;
        }
        update_party_portraits(st);
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_remove_creature, |u, st, index: i32| {
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("creature{}", index), |git| area::remove_creature(git, index.max(0) as usize));
        }
        // the map's people are numbered as the area lists them: read it again
        st.map = None;
        st.map_pick = None;
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_set_played, |u, st, h: SharedString, m: SharedString| {
        let (Ok(h), Ok(m)) = (h.trim().parse::<i64>(), m.trim().parse::<i64>()) else {
            message(&u, "That is not a time", "Type whole hours and minutes, for example 12 and 30.");
            return;
        };
        if let Some(save) = st.save.as_mut() {
            save.set_party_int("PT_PLAYEDSECONDS", h.max(0) * 3600 + m.clamp(0, 59) * 60);
        }
        refresh_after_edit(&u, st);
    });

    on!(ui, state, on_toggle_planet, |u, st, planet: SharedString| {
        let p = planet.to_string();
        if !st.open_planets.remove(&p) {
            st.open_planets.insert(p);
        }
        fill_map(&u, st);
    });
    on!(ui, state, on_view_area, |u, st, module: SharedString| {
        load_map(st, Some(module.to_string()));
        fill_map(&u, st);
    });
    on!(ui, state, on_pick_mark, |u, st, i: i32| {
        st.map_pick = Some(MapPick::Object(i.max(0) as usize));
        fill_map(&u, st);
    });
    on!(ui, state, on_map_click, |u, st, fx: f32, fy: f32| {
        let Some(area) = st.map.as_ref() else { return };
        let Some(cal) = area.calibration.as_ref() else { return };
        let (x, y) = cal.to_world(fx, fy);
        if std::env::var("KMRP_MAP_DEBUG").is_ok() {
            eprintln!("map click {} {} -> {} {}", fx, fy, x, y);
        }
        let z = st.save.as_ref().and_then(|s| s.player()).map(|p| area::position(p)[2]).unwrap_or(0.0);
        match area.floor.nearest(x, y, z, 1.5) {
            Some(spot) => st.map_pick = Some(MapPick::Floor(spot)),
            None => {
                st.map_pick = None;
                let app = u.global::<App>();
                fill_map(&u, st);
                app.set_pick_title(s("Not a place to stand"));
                app.set_pick_note(s("That is not walkable floor (a wall, a drop, or outside the area). Click nearer a room or corridor."));
                return;
            }
        }
        fill_map(&u, st);
    });
    on!(ui, state, on_move_here, |u, st, which: i32| {
        let Some(area) = st.map.as_ref() else { return };
        let (spot, look_at) = match st.map_pick.clone() {
            Some(MapPick::Floor(p)) => (p, None),
            Some(MapPick::Object(i)) => {
                let o = &area.objects[i];
                let Some(p) = o.spots.get(which.max(0) as usize).copied() else { return };
                (p, Some(o.pos))
            }
            None => return,
        };
        let facing = look_at.map(|t| (t[1] - spot[1]).atan2(t[0] - spot[0]));
        let module = area.module.clone();
        let title = area.name.clone();
        let Some(save) = st.save.as_mut() else { return };
        if module.eq_ignore_ascii_case(save.module_name()) {
            save.move_player(spot, facing);
        } else if let Err(e) = save.move_to_module(&module, spot, &title) {
            message(&u, "Could not move there", &e);
            return;
        } else {
            save.move_player(spot, facing);
        }
        st.map = None;
        load_map(st, Some(module));
        fill_editor(&u, st);
        st.tab = 5;
        u.global::<App>().set_tab(5);
        fill_map(&u, st);
    });
    on!(ui, state, on_spawn_here, |u, st| {
        if needs_game(&u, st) {
            return;
        }
        let Some(MapPick::Floor(spot)) = st.map_pick.clone() else { return };
        // Facing the player, so the new arrival is seen at once.
        let player = st.save.as_ref().and_then(|s| s.player()).map(area::position).unwrap_or(spot);
        let facing = (player[1] - spot[1]).atan2(player[0] - spot[0]);
        st.spawn_at = Some((spot, facing));
        st.spawnable = spawnables(st);
        st.target = Target::Spawn;
        st.picker_query.clear();
        st.picker_tab = 0;
        let app = u.global::<App>();
        app.set_overlay_title(s("Spawn a Creature Here"));
        app.set_overlay_note(s(
            "A copy of someone in this area, or any creature the game has. A new one runs its own spawn script when the save loads, as if the area had just made it. A copy keeps its tag, so a copy of a story character can confuse that story's scripts; creatures made for another planet may lack their conversation here.",
        ));
        app.set_picker_tab(0);
        app.set_picker_tabs(model(SPAWN_TABS.iter().map(|t| s(t)).collect()));
        app.set_picker_placeholder(s("Type part of a name or tag: rakghoul, sith, dan13_ ..."));
        app.set_overlay(1);
        search_spawn(&u, st);
    });
    on!(ui, state, on_remove_pick, |u, st| {
        let Some(MapPick::Object(i)) = st.map_pick.clone() else { return };
        let Some(o) = st.map.as_ref().map(|a| a.objects[i].clone()) else { return };
        if o.list != area::CREATURES {
            return;
        }
        if let Some(save) = st.save.as_mut() {
            save.edit_area(&format!("creature{}", o.index), |git| area::remove_creature(git, o.index));
        }
        st.map = None;
        load_map(st, None);
        st.map_pick = None;
        refresh_after_edit(&u, st);
        let app = u.global::<App>();
        app.set_pick_title(s(format!("{} removed", o.name)));
        app.set_pick_note(s("Save Changes to keep it out of the save; Revert brings it back."));
    });
    on!(ui, state, on_toggle_skip, |u, st, key: SharedString| {
        let k = key.to_string();
        if !st.skip_open.remove(&k) {
            st.skip_open.insert(k);
        }
        fill_skip(&u, st);
    });
    on!(ui, state, on_timeline_ready, |u, st| {
        fill_skip(&u, st);
    });
    // A free planet moved one place earlier (-1) or later (1) on the galaxy map's route: a done
    // one among the done, any other among the rest. Nothing in the save changes: the
    // order is what Skip Ahead and Go Back then follow.
    on!(ui, state, on_route_move, |u, st, index: i32, dir: i32| {
        let route = story_route(st);
        let all = kse_core::questdb::milestones_ref(st.game);
        let Some(g) = route.groups.iter().position(|g| g.first() == Some(&(index.max(0) as usize))) else { return };
        let to = g as i32 + dir.signum();
        if to < 0 || to as usize >= route.groups.len() || (route.kinds[to as usize] == 0) != (route.kinds[g] == 0) {
            return;
        }
        let mut names: Vec<String> = route.groups.iter().map(|g| all[g[0]].0.to_string()).collect();
        names.swap(g, to as usize);
        st.route_plan = names;
        fill_skip(&u, st);
    });
    on!(ui, state, on_route_reset, |u, st| {
        st.route_plan.clear();
        fill_skip(&u, st);
    });
    on!(ui, state, on_open_chapter, |u, st, i: i32| {
        u.global::<App>().set_back_alignment(50);
        clear_story_preview(&u, st);
        st.timeline_pick = Some(i);
        st.pop_pick = None;
        fill_skip(&u, st);
        u.global::<App>().set_overlay(12);
    });
    // A row picked in the chapter's pop-up: what skipping or going back there does, with its
    // choices, shown in the same pop-up (the confirmation is no longer a second one).
    on!(ui, state, on_pick_row, |u, st, r: i32| {
        if needs_game(&u, st) { return; }
        let app = u.global::<App>();
        let Some(row) = app.get_skip_rows().row_data(r.max(0) as usize) else { return };
        if row.state == 3 || !(row.depth == 2 || row.depth == 5 || row.depth == 6) {
            return;
        }
        st.pop_pick = Some((row.depth, row.index, row.tag.to_string(), row.stage));
        let detail = if row.depth == 2 {
            st.journal().iter().find(|q| q.tag.eq_ignore_ascii_case(row.tag.as_str())).map(|q| q.name.clone()).unwrap_or_default()
        } else { row.title.to_string() };
        app.set_pop_detail(s(detail));
        let stage = if row.depth == 2 { row.stage } else { 0 };
        if row.state == 0 {
            ask_back(&u, st, row.index, &row.tag, stage);
        } else {
            st.skip_stage = (row.depth == 2).then(|| (row.tag.to_string(), row.stage as i64));
            ask_skip(&u, st, row.index);
        }
        app.set_pop_action(if row.state == 0 { 2 } else { 1 });
        app.set_pop_picked(r);
        app.set_overlay(12);
    });
    on!(ui, state, on_refresh_story_preview, |u, st| {
        let app = u.global::<App>();
        request_story_preview(&u, st, (app.get_overlay() == 12 && app.get_pop_action() == 2) || app.get_overlay() == 11);
    });
    on!(ui, state, on_story_preview_finished, |u, st| {
        finish_story_preview(&u, st);
    });
    // A save clicked on the timeline: first a warning that says which save opens and that this
    // one closes, with Cancel (2026-11-06, the maintainer). Open in it opens the save.
    on!(ui, state, on_open_timeline_save, |u, st, i: i32| {
        if unsaved(&u, st) {
            return;
        }
        let Some((_, sm)) = (i >= 0).then(|| st.saves.get(i as usize)).flatten() else { return };
        let other = st.timeline_saves.as_ref().and_then(|t| t.iter().find(|o| o.index == i as usize).cloned());
        let here = st.save.as_ref().and_then(|s| save::summary(&s.folder)).map(|x| x.name).unwrap_or_default();
        let mut body = String::new();
        if let Some(o) = &other {
            body.push_str(&format!("{}\n{}\n{} · {}\n\n", o.level, o.story, o.area, played(o.seconds)));
        } else {
            body.push_str(&format!("{} · {}\n\n", sm.area, played(sm.seconds)));
        }
        body.push_str(&format!(
            "{} closes and this save opens in its place. Nothing is written to either save.",
            if here.is_empty() { "The save open now".to_string() } else { format!("\u{201c}{}\u{201d}", here) }
        ));
        st.open_ask = Some(i as usize);
        let app = u.global::<App>();
        app.set_message_title(s(format!("Open \u{201c}{}\u{201d}?", sm.name)));
        app.set_message_body(s(body));
        app.set_overlay(15);
    });
    on!(ui, state, on_confirm_open_save, |u, st| {
        let Some(i) = st.open_ask.take() else { return };
        u.global::<App>().set_overlay(0);
        // the list can have changed under the warning (Refresh): the index must still be there
        if unsaved(&u, st) || i >= st.saves.len() {
            return;
        }
        st.selected = i;
        open_selected(&u, st);
    });
    on!(ui, state, on_do_back, |u, st| {
        if needs_game(&u, st) { return; }
        let Some(prepared) = take_story_preview(&u, st, true) else { return };
        // the story's XP is the skip's or the return's now: stage clicks start counting again
        st.quest_xp.clear();
        let party = st.save.as_ref().map(|s| s.party_members());
        st.save = Some(prepared.save);
        if st.save.as_ref().map(|s| s.party_members()) != party {
            update_party_portraits(st);
        }
        st.back_to = None;
        u.global::<App>().set_overlay(0);
        forget_timeline_saves(st);
        // the party can have changed, so everything is read again, not only the tab on screen
        st.map = None;
        st.map_pick = None;
        let keep = (st.tab, st.story, st.who);
        fill_editor(&u, st);
        (st.tab, st.story, st.who) = keep;
        refresh_after_edit(&u, st);
        story_problems(&u, &prepared.notes);
    });
    on!(ui, state, on_do_skip, |u, st| {
        if needs_game(&u, st) { return; }
        let Some(prepared) = take_story_preview(&u, st, false) else { return };
        // the story's XP is the skip's or the return's now: stage clicks start counting again
        st.quest_xp.clear();
        let party = st.save.as_ref().map(|s| s.party_members());
        st.save = Some(prepared.save);
        if st.save.as_ref().map(|s| s.party_members()) != party {
            update_party_portraits(st);
        }
        st.skip_stage = None;
        u.global::<App>().set_overlay(0);
        forget_timeline_saves(st);
        // the party can have changed, so everything is read again, not only the tab on screen
        st.map = None;
        st.map_pick = None;
        let keep = (st.tab, st.story, st.who);
        fill_editor(&u, st);
        (st.tab, st.story, st.who) = keep;
        refresh_after_edit(&u, st);
        story_problems(&u, &prepared.notes);
    });
    on!(ui, state, on_travel, |u, st, i: i32| {
        if needs_game(&u, st) {
            return;
        }
        let Some((tag, _)) = st.travel_ways.get(i.max(0) as usize).cloned() else { return };
        let Some(area) = st.map.as_ref() else { return };
        let module = area.module.clone();
        let Some(save) = st.save.as_mut() else { return };
        match save.queue_transition(&module, &tag, &[]) {
            Ok(()) => {
                refresh_after_edit(&u, st);
            }
            Err(e) => message(&u, "The trip could not be set up", &e),
        }
    });
    on!(ui, state, on_unlock_pick, |u, st| {
        let Some(MapPick::Object(i)) = st.map_pick.clone() else { return };
        let Some(o) = st.map.as_ref().map(|a| a.objects[i].clone()) else { return };
        let module = st.map.as_ref().map(|a| a.module.clone()).unwrap_or_default();
        if let Some(save) = st.save.as_mut() {
            save.edit_module_git(&module, &format!("unlock{}{}", o.list, o.index), |git| {
                if let Some(x) = git.list_mut(o.list).and_then(|l| l.get_mut(o.index)) {
                    x.set_int("Locked", 0, Value::Byte(0));
                    x.set_int("KeyRequired", 0, Value::Byte(0));
                }
            });
        }
        st.map = None;
        load_map(st, None);
        st.map_pick = Some(MapPick::Object(i));
        refresh_after_edit(&u, st);
    });
    on!(ui, state, on_follow_exit, |u, st| {
        let Some(MapPick::Object(i)) = st.map_pick.clone() else { return };
        let Some(m) = st.map.as_ref().map(|a| a.objects[i].leads_to.clone()) else { return };
        load_map(st, Some(m));
        fill_map(&u, st);
    });
    on!(ui, state, on_open_all_doors, |u, st| {
        // The area on the map, which need not be the one the save is in.
        let module = st.map.as_ref().map(|a| a.module.clone());
        if let Some(save) = st.save.as_mut() {
            let module = module.unwrap_or_else(|| save.module_name().to_string());
            let opened = save.edit_module_git(&module, "alldoors", |git| {
                if let Some(l) = git.list_mut(area::DOORS) {
                    for d in l.iter_mut() {
                        d.set_int("Locked", 0, Value::Byte(0));
                        d.set_int("KeyRequired", 0, Value::Byte(0));
                        d.set_int("OpenState", 1, Value::Byte(0));
                    }
                }
            });
            if !opened {
                message(&u, "No doors were opened", "This save has not been to that area, so the game builds it fresh when the player arrives.");
            }
        }
        st.map = None;
        refresh_after_edit(&u, st);
    });

    // ---- saving ----
    on!(ui, state, on_save_changes, |u, st| {
        let Some(game) = st.save.as_ref().map(|s| s.game) else { return };
        if st.read_only {
            let why = st.save.as_ref().and_then(|s| read_only_reason(&s.folder)).unwrap_or("It is read only.");
            message(&u, "This save cannot be changed", why);
            return;
        }
        if game_running(game) {
            message(
                &u,
                &format!("{} is running", game.title()),
                "Close the game first. A save changed while the game runs can be overwritten by it, or loaded half old and half new.",
            );
            return;
        }
        let backups = backups_of(st);
        let result = st.save.as_mut().unwrap().write(&backups);
        match result {
            Ok(backup) => {
                let folder = st.save.as_ref().unwrap().folder.clone();
                st.original = Save::open(&folder).ok();
                st.quest_xp.clear();
                if let Ok(fresh) = Save::open(&folder) {
                    st.save = Some(fresh);
                }
                st.shots.lock().unwrap().remove(&folder);
                st.pending_spawns.clear();
                st.status = (format!("Saved! Backup of the unedited save is in {}", backup.display()), false);
            }
            Err(e) if e.starts_with(save::HALF_WRITTEN) => st.status = (e, true),
            Err(e) => st.status = (format!("Nothing was changed: {}", e), true),
        }
        fill_editor(&u, st);
    });
    // Save as a New Save: the name asked in a pop-up (overlay 14), then the save written into
    // the next free slot beside it, the original left as it was (2026-10-24, the maintainer).
    on!(ui, state, on_ask_save_as, |u, st| {
        let name = st.save.as_ref().map(|s| save::summary(&s.folder).map(|x| x.name).unwrap_or_default()).unwrap_or_default();
        let app = u.global::<App>();
        app.set_save_as_name(s(if name.trim().is_empty() { "Edited save".to_string() } else { format!("{} (edited)", name.trim()) }));
        app.set_save_as_copy(false);
        app.set_overlay(14);
    });
    // Copy: the name asked (filled with "<the save's name> - Copy"), then the save as it is on
    // disk copied into the next free slot under it, the changes not in it; the save open stays
    // open, changes and all (2026-10-24, the maintainer).
    on!(ui, state, on_copy_save, |u, st| {
        let Some(folder) = copy_source(&u, st).map(|(_, f)| f) else { return };
        let name = save::summary(&folder).map(|x| x.name).unwrap_or_default();
        let app = u.global::<App>();
        app.set_save_as_name(s(if name.trim().is_empty() { "Copy".to_string() } else { format!("{} - Copy", name.trim()) }));
        app.set_save_as_copy(true);
        app.set_overlay(14);
    });
    on!(ui, state, on_do_copy_save, |u, st, name: SharedString| {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        u.global::<App>().set_overlay(0);
        let Some((game, folder)) = copy_source(&u, st) else { return };
        // A copy changes savenfo.res, which the signature in the .xbx files covers.
        if is_xbox_save(&folder) {
            message(&u, "Nothing was copied", "It is an original Xbox save; the Xbox would call a copy with a new name damaged.");
            return;
        }
        if game_running(game) {
            message(&u, &format!("{} is running", game.title()), "Close the game first: it can overwrite a save folder it did not expect.");
            return;
        }
        let Some(target) = save::next_save_folder(&folder) else {
            message(&u, "Nothing was copied", "No free save slot was found beside this save.");
            return;
        };
        // from the saves page: the copy is picked in the list
        if u.global::<App>().get_page() == 0 {
            match Save::copy_save(&folder, &target, &name) {
                Ok(()) => {
                    add_beside(st, &folder, &target);
                    fill_saves(&u, st);
                    pick_save(&u, st, &target);
                }
                Err(e) => message(&u, "Nothing was copied", &e),
            }
            return;
        }
        match Save::copy_save(&folder, &target, &name) {
            Ok(()) => {
                add_beside(st, &folder, &target);
                let keep = (st.tab, st.story, st.who);
                fill_saves(&u, st);
                if let Some(i) = st.saves.iter().position(|(f, _)| f.path == folder) {
                    st.selected = i;
                }
                (st.tab, st.story, st.who) = keep;
                forget_timeline_saves(st);
                st.status = (format!("Copied as \"{}\" ({}); the save open here is unchanged", name, target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), false);
            }
            Err(e) => st.status = (format!("Nothing was copied: {}", e), true),
        }
        u.global::<App>().set_page(1);
        fill_editor(&u, st);
    });
    // The saves page's pen: the selected save's name on the game's Load screen changed, the
    // save backed up first like any other write (2026-10-24, the maintainer).
    on!(ui, state, on_rename_save, |u, st, name: SharedString| {
        let name = name.trim().to_string();
        let Some(folder) = st.saves.get(st.selected).map(|(f, _)| f.path.clone()) else { return };
        let now = save::summary(&folder).map(|x| x.name).unwrap_or_default();
        if name.is_empty() || name == now.trim() {
            return;
        }
        if let Some(why) = read_only_reason(&folder) {
            message(&u, "This save cannot be changed", why);
            return;
        }
        if game_running(st.game) {
            message(&u, &format!("{} is running", st.game.title()), "Close the game first: it can overwrite a save it did not expect.");
            return;
        }
        // The saves page has no save open, so the folder decides, as it does for Save Changes.
        let root = st.installs.iter().find(|i| i.game == st.game).map(|i| i.path.clone());
        let backups = save::backup_root(&folder, root.as_deref());
        let result = Save::open(&folder).and_then(|mut s| {
            s.rename(&name);
            s.write(&backups).map(|_| ())
        });
        match result {
            Ok(()) => {
                fill_saves(&u, st);
                pick_save(&u, st, &folder);
            }
            Err(e) => message(&u, "The save was not renamed", &e),
        }
    });
    on!(ui, state, on_do_save_as, |u, st, name: SharedString| {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let Some(game) = st.save.as_ref().map(|s| s.game) else { return };
        u.global::<App>().set_overlay(0);
        if st.read_only {
            let why = st.save.as_ref().and_then(|s| read_only_reason(&s.folder)).unwrap_or("It is read only.");
            message(&u, "This save cannot be changed", why);
            return;
        }
        if game_running(game) {
            message(&u, &format!("{} is running", game.title()), "Close the game first: it can overwrite a save folder it did not expect.");
            return;
        }
        let Some(target) = st.save.as_ref().and_then(|s| save::next_save_folder(&s.folder)) else {
            st.status = ("Nothing was changed: no free save slot was found beside this save".to_string(), true);
            fill_bar(&u, st);
            return;
        };
        let source = st.save.as_ref().unwrap().folder.clone();
        let result = st.save.as_mut().unwrap().write_as(&target, &name);
        match result {
            Ok(()) => {
                st.original = Save::open(&target).ok();
                st.quest_xp.clear();
                if let Ok(fresh) = Save::open(&target) {
                    st.save = Some(fresh);
                }
                st.pending_spawns.clear();
                add_beside(st, &source, &target);
                // the saves list gains the new one, which is the one open now
                let keep = (st.tab, st.story, st.who);
                fill_saves(&u, st);
                if let Some(i) = st.saves.iter().position(|(f, _)| f.path == target) {
                    st.selected = i;
                }
                (st.tab, st.story, st.who) = keep;
                forget_timeline_saves(st);
                st.status = (format!("Saved as a new save, \"{}\" ({}); the original is unchanged", name, target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), false);
            }
            Err(e) => st.status = (format!("Nothing was changed: {}", e), true),
        }
        u.global::<App>().set_page(1);
        fill_editor(&u, st);
    });
    on!(ui, state, on_revert, |u, st| {
        if let Some(folder) = st.save.as_ref().map(|s| s.folder.clone()) {
            st.save = Save::open(&folder).ok();
            st.original = Save::open(&folder).ok();
            st.quest_xp.clear();
            st.status = (String::new(), false);
            st.map = None;
            st.map_pick = None;
            forget_pending_spawns(st);
            fill_editor(&u, st);
        }
    });
    on!(ui, state, on_open_backups, |_u, st| {
        let dir = backups_of(st);
        let _ = std::fs::create_dir_all(&dir);
        open_path(&dir);
    });

    // ---- pop-ups ----
    on!(ui, state, on_close_overlay, |u, st| {
        clear_story_preview(&u, st);
        st.target = Target::None;
        u.global::<App>().set_overlay(0);
    });
    on!(ui, state, on_select_picker_tab, |u, st, i: i32| {
        st.picker_tab = i.max(0) as usize;
        u.global::<App>().set_picker_tab(i);
        if st.target == Target::Spawn {
            search_spawn(&u, st);
        } else {
            search_items(&u, st);
        }
    });
    on!(ui, state, on_search_picker, |u, st, q: SharedString| {
        st.picker_query = q.to_string();
        match st.target {
            Target::Inventory | Target::Container => search_items(&u, st),
            Target::Spawn => search_spawn(&u, st),
            _ => search_list(&u, st),
        }
    });
    on!(ui, state, on_pick, |u, st, id: i32| {
        match st.target {
            Target::Spawn => {
                let Some(sp) = st.spawnable.get(id.max(0) as usize).cloned() else { return };
                let Some((spot, facing)) = st.spawn_at else { return };
                let Some(mut c) = spawn_creature_of(st, &sp) else {
                    message(&u, "That creature could not be read", "Its template is missing or damaged.");
                    return;
                };
                area::place_creature(&mut c, spot, facing);
                let tag = c.string("Tag").unwrap_or_default();
                let game = st.game;
                if let Some(save) = st.save.as_mut() {
                    let id = save.spawn_creature(c);
                    record_spawned(game, save.module_name(), id, &tag);
                    st.pending_spawns.push(spawned_key(game, save.module_name(), id, &tag));
                }
                st.target = Target::None;
                u.global::<App>().set_overlay(0);
                st.map = None;
                load_map(st, None);
                st.map_pick = None;
                fill_editor(&u, st);
                st.tab = 5;
                u.global::<App>().set_tab(5);
                fill_map(&u, st);
                let app = u.global::<App>();
                app.set_pick_title(s(format!("{} spawned", sp.name)));
                app.set_pick_note(s(
                    "It stands on the map now. Save Changes to keep it; Story, Creatures lists it and can remove it again.",
                ));
            }
            Target::Class => {
                let Some(cat) = st.catalog() else { return };
                let Some(class) = cat.class(id as i64).cloned() else { return };
                let level = st.save.as_ref().and_then(|s| s.creature(st.who())).map(creature::level).unwrap_or(1);
                let need = cat.xp_for(level + 1);
                u.global::<App>().set_overlay(0);
                st.target = Target::None;
                edit_who(&u, st, "class", |c| {
                    creature::add_class(c, &class, need);
                });
                fill_editor(&u, st);
            }
            Target::Inventory => {
                let like = st.save.as_ref().and_then(|s| s.inventory().first().cloned());
                if let Some(item) = new_item(st, id.max(0) as usize, like.as_ref()) {
                    if let Some(save) = st.save.as_mut() {
                        save.edit_inventory(&format!("add{}", id), |items| items.push(item));
                    }
                    u.global::<App>().set_overlay_note(s("Added. Add more, or close."));
                    refresh_after_edit(&u, st);
                } else {
                    u.global::<App>().set_overlay_note(s("That item could not be read from the game's files, so nothing was added."));
                }
            }
            Target::Container => {
                let Some((list, index)) = st.selected_box else { return };
                let like = st
                    .save
                    .as_ref()
                    .and_then(|s| s.area_git())
                    .and_then(|g| area::container(&g.root, list, index))
                    .and_then(|c| c.list("ItemList"))
                    .and_then(|l| l.first().cloned());
                let Some(mut item) = new_item(st, id.max(0) as usize, like.as_ref()) else {
                    u.global::<App>().set_overlay_note(s("That item could not be read from the game's files, so nothing was added."));
                    return;
                };
                if let Some(save) = st.save.as_mut() {
                    // From the module's counter, as for every other new object.
                    let next = save.new_object_id();
                    save.edit_area(&format!("box{}{}", list, index), |git| {
                        if let Some(c) = area::container_mut(git, list, index) {
                            if c.list("ItemList").is_none() {
                                c.set("ItemList", Value::List(Vec::new()));
                            }
                            let items = c.list_mut("ItemList").unwrap();
                            if list == area::STORES {
                                item.id = items.len() as u32;
                                if like.as_ref().map_or(false, |l| l.get("Infinite").is_some()) && item.get("Infinite").is_none() {
                                    item.fields.insert(0, ("Infinite".into(), Value::Byte(0)));
                                }
                                item.fields.retain(|(l, _)| l != "ObjectId");
                            } else {
                                item.set("ObjectId", Value::Dword(next));
                            }
                            items.push(item);
                        }
                    });
                }
                u.global::<App>().set_overlay_note(s("Added. Add more, or close."));
                refresh_after_edit(&u, st);
            }
            Target::Appearance | Target::Soundset | Target::Gender => {
                let target = st.target;
                u.global::<App>().set_overlay(0);
                st.target = Target::None;
                edit_who(&u, st, "look", move |c| match target {
                    Target::Appearance => creature::set_appearance(c, id as i64),
                    Target::Soundset => creature::set_soundset(c, id as i64),
                    _ => creature::set_gender(c, id as i64),
                });
                fill_character(&u, st);
            }
            Target::ChangeClass(index) => {
                let Some(cat) = st.catalog() else { return };
                let Some((old_id, _)) = st.save.as_ref().and_then(|s| s.creature(st.who())).and_then(|c| creature::classes(c).get(index).copied()) else { return };
                let (Some(old), Some(new)) = (cat.class(old_id).cloned(), cat.class(id as i64).cloned()) else { return };
                st.target = Target::None;
                edit_who(&u, st, &format!("class{}", index), move |c| creature::change_class(c, index, &old, &new));
                fill_editor(&u, st);
                u.global::<App>().set_overlay(6);
            }
            _ => {}
        }
    });
    on!(ui, state, on_pick_save, |u, st, i: i32| {
        let Some(folder) = st.other_saves.get(i.max(0) as usize).cloned() else { return };
        let target = st.target;
        u.global::<App>().set_overlay(0);
        st.target = Target::None;
        match target {
            Target::Compare => match Save::open(&folder.path) {
                Ok(other) => {
                    let list = other.globals.as_ref().map(|g| globals::list(&g.root)).unwrap_or_default();
                    let name = save::summary(&folder.path).map(|x| x.name).unwrap_or_default();
                    st.compare = Some((name, list));
                    st.global_filter.1 = true;
                    fill_globals(&u, st);
                }
                Err(e) => message(&u, "That save could not be opened", &e),
            },
            Target::CopyInventory => match Save::open(&folder.path) {
                Ok(other) => {
                    let result = st.save.as_mut().map(|s| s.copy_inventory(&other));
                    if let Some(Err(e)) = result {
                        message(&u, "The inventory was not copied", &e);
                    }
                    refresh_after_edit(&u, st);
                }
                Err(e) => message(&u, "That save could not be opened", &e),
            },
            Target::ImportNpc(n) => match Save::open(&folder.path) {
                Ok(other) => {
                    let result = st.save.as_mut().map(|s| s.import_npc(&other, n));
                    if let Some(Err(e)) = result {
                        message(&u, "They could not be brought over", &e);
                    }
                    if st.save.as_ref().map_or(false, |s| s.in_party(n)) {
                        update_party_portraits(st);
                    }
                    let tab = st.tab;
                    fill_editor(&u, st);
                    st.tab = tab;
                }
                Err(e) => message(&u, "That save could not be opened", &e),
            },
            _ => {}
        }
    });

    let _ = with(&ui, &state, |_, _| {});
    // Closing with changes not saved asks once; closing again with the very same changes
    // leaves. Until 2026-11-06 it compared only their number, so after a warning, then Save or
    // Revert, the same number of other changes closed without asking.
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let warned = std::cell::RefCell::new(None::<std::collections::BTreeSet<String>>);
        ui.window().on_close_requested(move || {
            let Some(u) = weak.upgrade() else { return slint::CloseRequestResponse::HideWindow };
            // busy (a handler still running): stay open rather than drop changes unasked
            let Ok(st) = state.try_borrow() else { return slint::CloseRequestResponse::KeepWindowShown };
            let changes = st.save.as_ref().map(|s| s.changes.clone()).unwrap_or_default();
            if changes.is_empty() || warned.borrow().as_ref() == Some(&changes) {
                return slint::CloseRequestResponse::HideWindow;
            }
            *warned.borrow_mut() = Some(changes);
            message(&u, "There are changes not saved", "Save them or press Revert first. Close the window again to leave without saving them.");
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
    ui.run().expect("the window closed with an error");
    let mut st = state.borrow_mut();
    // spawns left unsaved at closing are in no save: their spawned.txt lines go
    forget_pending_spawns(&mut st);
    st.settings.light = !ui.global::<Theme>().get_dark();
    st.settings.store();
}
