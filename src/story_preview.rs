//! Prepare a story change on an independent in-memory save. The preview and Apply use
//! the same result; neither writes files. Save Changes still goes through Save::write.

use crate::{
    creature,
    formats::gff::{Gff, Struct, Value},
    gamedata::GameData,
    questdb,
    save::{Save, Who},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    Skip {
        chapters: Vec<serde_json::Value>,
        stage: Option<(String, i64)>,
        class: Option<usize>,
        dark: bool,
        prestige: Option<i64>,
        train: bool,
    },
    Back {
        kept: Vec<serde_json::Value>,
        undo: Vec<serde_json::Value>,
        stage: Option<(String, i64)>,
        alignment: i64,
        /// The chapters of `undo` begun and never finished, by their planets.
        unfinished: Vec<String>,
    },
}

pub struct Prepared {
    pub save: Save,
    pub notes: Vec<String>,
}

impl Plan {
    pub fn prepare(&self, before: &Save, data: &GameData) -> Result<Prepared, String> {
        let mut save = before.clone();
        let notes = match self {
            Self::Skip {
                chapters,
                stage: Some((tag, stage)),
                class,
                dark,
                prestige,
                train,
            } => questdb::skip_to_stage(
                &mut save, data, chapters, tag, *stage, *class, *dark, *prestige, *train,
            )?,
            Self::Skip {
                chapters,
                stage: None,
                class,
                dark,
                prestige,
                train,
            } => questdb::skip_through_with(
                &mut save, data, chapters, *class, *dark, *prestige, *train,
            )?,
            Self::Back {
                kept,
                undo,
                stage,
                alignment,
                unfinished,
            } => questdb::go_back_part_played(
                &mut save,
                data,
                kept,
                undo,
                false,
                stage.clone(),
                *alignment,
                unfinished,
            )?,
        };
        Ok(Prepared { save, notes })
    }
}

#[derive(Clone, Debug)]
pub struct ItemChange {
    pub item: Struct,
    pub difference: i64,
}

/// Resolve a companion from the save being previewed, rather than a former occupant
/// of that NPC slot in the open save (for example Trask before Bastila is recruited).
pub fn companion_name(save: &Save, data: &GameData, index: usize, fallback: &str) -> String {
    if let Some(c) = save.creature(Who::Npc(index)) {
        let name = creature::name(c);
        if !name.trim().is_empty() {
            return name;
        }
        if let Some(Value::LocString(name)) = c.get("FirstName") {
            let name = data.loc(name);
            if !name.trim().is_empty() {
                return name;
            }
        }
    }
    if fallback.is_empty() {
        format!("Member {}", index + 1)
    } else {
        fallback.to_string()
    }
}

// Ignore inventory placement and runtime identity, but keep charges, upgrades and
// every other item property. Two differently upgraded items must not cancel out.
fn item_key(item: &Struct) -> Vec<u8> {
    let mut item = item.clone();
    item.id = 0;
    item.fields.retain(|(name, _)| {
        !matches!(
            name.as_str(),
            "ObjectId" | "StackSize" | "Repos_PosX" | "Repos_PosY" | "EquipSlot"
        )
    });
    fn sort_fields(item: &mut Struct) {
        item.fields.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, value) in &mut item.fields {
            match value {
                Value::Struct(s) => sort_fields(s),
                Value::List(list) => list.iter_mut().for_each(sort_fields),
                _ => {}
            }
        }
    }
    sort_fields(&mut item);
    Gff {
        file_type: *b"UTI ",
        root: item,
    }
    .write()
}

/// Sum stacks rather than compare row indexes. Reordering or splitting one stack is
/// not an inventory gain or loss. The representative keeps the actual icon and name.
pub fn inventory_changes(before: &[Struct], after: &[Struct]) -> Vec<ItemChange> {
    let mut counts: BTreeMap<Vec<u8>, ItemChange> = BTreeMap::new();
    for (items, sign) in [(before, -1), (after, 1)] {
        for item in items {
            let count = item.int("StackSize").unwrap_or(1).max(0);
            let row = counts.entry(item_key(item)).or_insert_with(|| ItemChange {
                item: item.clone(),
                difference: 0,
            });
            row.difference += sign * count;
        }
    }
    counts.into_values().filter(|r| r.difference != 0).collect()
}

/// The party inventory plus gear held by the player and recruited companions. Moving
/// equipment into inventory must not look like a new item. A departing companion's
/// gear leaves the accessible inventory with them.
pub fn accessible_items(save: &Save) -> Vec<Struct> {
    let mut items = save.inventory();
    let whos = std::iter::once(Who::Player).chain(
        save.npcs()
            .into_iter()
            .filter(|i| save.recruited(*i))
            .map(Who::Npc),
    );
    for who in whos {
        if let Some(c) = save.creature(who) {
            items.extend(creature::equipped(c).into_iter().cloned());
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(count: i64, object: i64, upgrade: i64) -> Struct {
        let mut s = Struct::new(0);
        s.set("TemplateResRef", Value::ResRef(b"test_item".to_vec()));
        s.set_int("StackSize", count, Value::Word(1));
        s.set_int("ObjectId", object, Value::Dword(0));
        s.set_int("UpgradeLevel", upgrade, Value::Byte(0));
        s
    }
    #[test]
    fn stack_splits_and_runtime_ids_do_not_create_changes() {
        assert!(inventory_changes(&[item(5, 1, 0)], &[item(2, 2, 0), item(3, 3, 0)]).is_empty());
    }
    #[test]
    fn placement_and_field_order_do_not_create_an_item() {
        let before = item(1, 1, 0);
        let mut moved = item(1, 2, 0);
        moved.id = 16;
        moved.set_int("Repos_PosX", 8, Value::Word(0));
        moved.set_int("Repos_PosY", 4, Value::Word(0));
        moved.fields.reverse();
        assert!(inventory_changes(&[before], &[moved]).is_empty());
    }
    #[test]
    fn grants_losses_and_upgrades_are_counted_separately() {
        let rows = inventory_changes(
            &[item(5, 1, 0), item(1, 2, 1)],
            &[item(3, 3, 0), item(2, 4, 2)],
        );
        let changes: BTreeMap<_, _> = rows
            .iter()
            .map(|r| (r.item.int("UpgradeLevel").unwrap(), r.difference))
            .collect();
        assert_eq!(changes, BTreeMap::from([(0, -2), (1, -1), (2, 2)]));
    }
}
