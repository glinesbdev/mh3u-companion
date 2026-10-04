//! Quests, read from the game's `.quest` files (`quest/us/q_NNNNN.quest`).
//!
//! A file is `QTDS`, a version (5), the quest's texts in five languages, and a binary part. A text is five strings (English, French,
//! German, Italian, Spanish), each a u32 length and the bytes. In order: the title, the quest id (u16), the main goal, the star
//! rank and one more byte, a block of five empty strings, the time limit in minutes (u16), the failure condition, six bytes, the
//! client and the description.
//!
//! The binary part holds, at offsets from its start: the kind of quest (byte 2: 1 slay, 2 deliver, 4 capture, 5 hunt), up to five
//! 11-byte records of large monsters from byte 10 (the first byte of each is the monster's id in the name table, 0 for none), and
//! from byte 71 the rewards as 4-byte entries `[item u16, quantity, chance]`. The entries alternate between two reward boxes: the
//! even ones are the main box (monster parts), the odd ones the second box (supplies); an entry with chance 0 is given every time,
//! and each box's chances add up to 100. Found by reading files and checking them against the quests played; the meaning of the
//! rest of the binary part (the stage, the fees, the small monsters) is not known.

use anyhow::{Result, bail, ensure};

/// The kind of quest, from the binary part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Slay,
    Deliver,
    Capture,
    Hunt,
    /// Anything else (a quest with a special goal, like surviving until time runs out), with the byte as found.
    Other(u8),
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Slay => "Slay",
            Kind::Deliver => "Deliver",
            Kind::Capture => "Capture",
            Kind::Hunt => "Hunt",
            Kind::Other(_) => "Other",
        }
    }
}

/// Where a quest is taken: the village (Moga Village, quest ids below 10000) or the guild hall (ids 10000 to 19999). Worked out from the
/// ids and checked against the guild card counts of a hunter whose finished quests were all known: 8 village quests had ids below 10000
/// and 3 hall quests had ids from 11105 to 11112. Other ids (arena and event quests, 20000 and up) are `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Village,
    Hall,
    Other,
}

impl Place {
    pub fn of_quest(id: u16) -> Place {
        match id {
            0..10_000 => Place::Village,
            10_000..20_000 => Place::Hall,
            _ => Place::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Place::Village => "Village",
            Place::Hall => "Hall",
            Place::Other => "Event",
        }
    }
}

/// The name of a map by the number in the quest file, for the numbers a quest board reading has named. A hunter read these maps off
/// three quests: Bug Hunt (0x24, Deserted Island), The Fisherman's Tale (0x17, Flooded Forest) and Rathian's Wrath (0x16, Sandy Plains).
/// The other numbers (0x0a is the most common) are maps too, but which is not known.
pub fn stage_name(stage: u8) -> Option<&'static str> {
    match stage {
        0x24 => Some("Deserted Island"),
        0x17 => Some("Flooded Forest"),
        0x16 => Some("Sandy Plains"),
        _ => None,
    }
}

/// One thing a quest can give: `percent` is the chance among its box; 0 means every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reward {
    pub item: u16,
    pub quantity: u8,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quest {
    pub id: u16,
    /// The quest's name, e.g. `Bear Trap`.
    pub title: String,
    /// What to do, e.g. `Capture an Arzuros`.
    pub goal: String,
    pub client: String,
    pub description: String,
    pub stars: u8,
    pub minutes: u16,
    pub kind: Kind,
    pub place: Place,
    /// What the quest board shows: the zenny reward and the fee to take the quest (read from the end of the file and checked on three
    /// quests: Bug Hunt 600 and 100, The Fisherman's Tale 4000 and 400, Rathian's Wrath 9200 and 920; the fee is a tenth of the reward in
    /// hall quests).
    pub reward: u32,
    pub fee: u32,
    /// The hunter rank points it gives (the save's total at 0x5a46 grew by this for 4 of the 6 quests a hunter finished and by less for
    /// the other 2).
    pub rank_points: u32,
    /// The map, as a number from the second byte of the binary part. One map has several numbers (its day and night versions, say), and
    /// only three are named: see [`stage_name`].
    pub stage: u8,
    /// Large monsters (ids in the monster name table), in the order the file lists them.
    pub monsters: Vec<u16>,
    /// The two reward boxes: the main one and the second.
    pub rewards: [Vec<Reward>; 2],
}

const MAGIC: &[u8; 4] = b"QTDS";
const VERSION: u32 = 5;
/// Where the large-monster records start in the binary part, how long each is, and how many there can be.
const MONSTERS_AT: usize = 10;
const MONSTER_LEN: usize = 11;
const MONSTER_SLOTS: usize = 5;
const REWARDS_AT: usize = 71;
const REWARD_ENTRIES: usize = 43;
/// Where the four payment numbers start in the binary part: the fee, the reward, a third of the reward, the rank points.
const PAY_AT: usize = 327;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.pos + n <= self.data.len(), "quest file ends early");
        self.pos += n;
        Ok(&self.data[self.pos - n..self.pos])
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Five strings, one per language; the English one.
    fn texts(&mut self) -> Result<String> {
        let mut english = String::new();
        for language in 0..5 {
            let b = self.bytes(4)?;
            let len = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
            ensure!(len < 4000, "a quest text is {len} bytes long");
            let text = String::from_utf8_lossy(self.bytes(len)?).into_owned();
            if language == 0 {
                english = text;
            }
        }
        Ok(english)
    }
}

/// Read one quest file.
pub fn parse(data: &[u8]) -> Result<Quest> {
    ensure!(data.starts_with(MAGIC), "not a quest file");
    let mut r = Reader { data, pos: 4 };
    let version = u32::from_le_bytes(r.bytes(4)?.try_into()?);
    if version != VERSION {
        bail!("quest file version {version}, expected {VERSION}");
    }
    let title = r.texts()?;
    let id = r.u16()?;
    let goal = r.texts()?;
    let stars = r.u8()?;
    r.u8()?;
    r.texts()?; // five empty strings in every quest seen
    let minutes = r.u16()?;
    r.texts()?; // the failure condition
    r.bytes(6)?;
    let client = r.texts()?;
    let description = r.texts()?;
    let tail = &data[r.pos..];
    ensure!(tail.len() >= REWARDS_AT + 4 * REWARD_ENTRIES, "quest file has no reward table");

    let kind = match tail[2] {
        1 => Kind::Slay,
        2 => Kind::Deliver,
        4 => Kind::Capture,
        5 => Kind::Hunt,
        other => Kind::Other(other),
    };
    let monsters = (0..MONSTER_SLOTS)
        .map(|k| u16::from(tail[MONSTERS_AT + MONSTER_LEN * k]))
        .take_while(|&m| m != 0)
        .collect();
    ensure!(tail.len() >= PAY_AT + 16, "quest file ends before its payment numbers");
    let pay: [u32; 4] = std::array::from_fn(|k| {
        u32::from_le_bytes([
            tail[PAY_AT + 4 * k],
            tail[PAY_AT + 4 * k + 1],
            tail[PAY_AT + 4 * k + 2],
            tail[PAY_AT + 4 * k + 3],
        ])
    });
    let mut rewards: [Vec<Reward>; 2] = Default::default();
    for k in 0..REWARD_ENTRIES {
        let e = &tail[REWARDS_AT + 4 * k..REWARDS_AT + 4 * k + 4];
        let reward = Reward {
            item: u16::from_le_bytes([e[0], e[1]]),
            quantity: e[2],
            percent: e[3],
        };
        if reward.item != 0 {
            rewards[k % 2].push(reward);
        }
    }
    Ok(Quest {
        id,
        title,
        goal,
        client,
        description,
        stars,
        minutes,
        kind,
        place: Place::of_quest(id),
        fee: pay[0],
        reward: pay[1],
        rank_points: pay[3],
        stage: tail[1],
        monsters,
        rewards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(english: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for language in ["", "fr", "de", "it", "es"] {
            let t = format!("{english}{language}");
            out.extend((t.len() as u32).to_le_bytes());
            out.extend(t.as_bytes());
        }
        out
    }

    /// A quest file built by hand: Great Jaggi hunt, 2 stars, 50 minutes, a guaranteed hide and two random parts in the main box.
    fn sample() -> Vec<u8> {
        let mut f = b"QTDS".to_vec();
        f.extend(5u32.to_le_bytes());
        f.extend(texts("Pain in the Plains"));
        f.extend(1206u16.to_le_bytes());
        f.extend(texts("Hunt a Great Jaggi"));
        f.extend([2, 1]);
        f.extend(texts(""));
        f.extend(50u16.to_le_bytes());
        f.extend(texts("Reward hits 0"));
        f.extend([0; 6]);
        f.extend(texts("The Guild"));
        f.extend(texts("A big one."));
        let mut tail = vec![0u8; 442];
        tail[2] = 5;
        tail[10] = 12; // Great Jaggi
        tail[21] = 8; // and a Barroth
        let entries = [(447u16, 1u8, 0u8), (400, 1, 0), (448, 1, 60), (449, 2, 25), (450, 1, 40), (0, 0, 0)];
        for (k, (item, qty, pct)) in entries.into_iter().enumerate() {
            let at = REWARDS_AT + 4 * k;
            tail[at..at + 2].copy_from_slice(&item.to_le_bytes());
            tail[at + 2] = qty;
            tail[at + 3] = pct;
        }
        f.extend(tail);
        f
    }

    #[test]
    fn a_quest_file_reads_into_its_texts_monsters_and_two_reward_boxes() {
        let q = parse(&sample()).unwrap();
        assert_eq!(
            (q.id, q.title.as_str(), q.goal.as_str()),
            (1206, "Pain in the Plains", "Hunt a Great Jaggi")
        );
        assert_eq!((q.stars, q.minutes, q.kind), (2, 50, Kind::Hunt));
        assert_eq!((q.client.as_str(), q.description.as_str()), ("The Guild", "A big one."));
        assert_eq!(q.monsters, [12, 8]);
        let main: Vec<(u16, u8, u8)> = q.rewards[0].iter().map(|r| (r.item, r.quantity, r.percent)).collect();
        assert_eq!(main, [(447, 1, 0), (448, 1, 60), (450, 1, 40)], "even entries");
        let second: Vec<u16> = q.rewards[1].iter().map(|r| r.item).collect();
        assert_eq!(second, [400, 449], "odd entries; an empty entry is skipped");
    }

    #[test]
    fn a_quest_is_in_the_village_or_the_hall_by_its_id() {
        assert_eq!(Place::of_quest(1202), Place::Village);
        assert_eq!(Place::of_quest(9999), Place::Village);
        assert_eq!(Place::of_quest(11112), Place::Hall);
        assert_eq!(Place::of_quest(60001), Place::Other);
        assert_eq!(Place::Hall.label(), "Hall");
    }

    #[test]
    fn bad_files_are_refused_not_trusted() {
        assert!(parse(b"nope").is_err());
        assert!(parse(&sample()[..100]).is_err(), "cut short");
        let mut wrong_version = sample();
        wrong_version[4] = 9;
        assert!(parse(&wrong_version).is_err());
        let mut huge_text = sample();
        huge_text[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge_text).is_err());
    }

    /// Every quest in the player's dump reads, its box chances add up to 100, and the monsters are known ids. Skips without the dump.
    #[test]
    fn every_quest_in_the_dump_reads_and_its_rewards_add_up() {
        let Some(home) = std::env::var_os("HOME") else { return };
        let Ok(dumps) = std::fs::read_dir(std::path::Path::new(&home).join("games/wiiu")) else {
            return;
        };
        let Some(dir) = dumps
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.to_string_lossy().contains("[Game]"))
            .map(|p| p.join("content/nativeCafe/quest/us"))
        else {
            return;
        };
        let Ok(files) = std::fs::read_dir(&dir) else { return };
        let (mut read, mut off_by) = (0, 0);
        // the rank points the save gained after these quests (a hunter's rank points went up by exactly these amounts)
        let mut points = std::collections::HashMap::new();
        let mut quests = Vec::new();
        for f in files
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "quest"))
        {
            let q = parse(&std::fs::read(f.path()).unwrap()).unwrap();
            read += 1;
            points.insert(q.id, q.rank_points);
            quests.push(q.clone());
            assert!(q.monsters.iter().all(|&m| m < 100), "{}: monster {:?}", q.title, q.monsters);
            for (box_, rewards) in q.rewards.iter().enumerate() {
                let total: u32 = rewards.iter().map(|r| u32::from(r.percent)).sum();
                if total != 0 && total != 100 {
                    off_by += 1;
                    eprintln!("quest {} box {box_} adds up to {total}", q.id);
                }
            }
        }
        assert!(read > 300, "read {read} quests");
        for (id, gained) in [(1205, 60), (11105, 200), (1202, 50), (1203, 70), (11106, 210)] {
            assert_eq!(points[&id], gained, "quest {id}");
        }
        // what the quest board showed: reward, fee and map
        for (id, reward, fee, map) in [
            (1202, 600, 100, "Deserted Island"),
            (11106, 4000, 400, "Flooded Forest"),
            (11615, 9200, 920, "Sandy Plains"),
        ] {
            let q = quests.iter().find(|q| q.id == id).unwrap();
            assert_eq!((q.reward, q.fee, stage_name(q.stage)), (reward, fee, Some(map)), "quest {id}");
        }
        assert!(off_by <= 3, "{off_by} boxes do not add up to 100");
    }
}
