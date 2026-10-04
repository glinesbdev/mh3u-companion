# File formats

Everything here was worked out from the US Wii U game and checked against in-game values supplied by the project owner. All
numbers are big-endian. "Confirmed" means verified against in-game screens or save diffs; "inferred" means it fits the data but was
not directly verified.

## Save file (`user1`, 35,364 bytes, unencrypted)

| Offset | Size | Field | Status |
|--------|------|-------|--------|
| 0x2b | up to 21 | hunter name (UTF-8, NUL-terminated) | confirmed |
| 0x49 | 3 | zenny | confirmed |
| 0xc0 | u16 | worn weapon: slot of the equipment box; 0xffff = none | confirmed (two saves: slot 11 = the equipped Bow; slot 0 = the Iron Sword+ in another) |
| 0xc2..0xcc | 5 x u16 | worn armor: slots (body, arms, waist, legs, head) of the equipment box; 0xffff = none | body/arms/waist/legs confirmed by save diffs; head inferred (the one pointer that stayed when the others were cleared) |
| 0xd0 | 24 x 4 | item pouch: (u16 item id, u16 count) | confirmed (24 slots inferred) |
| 0x1b0 | 1000 x 4 | item box, same layout | confirmed (1000 slots inferred) |
| 0x1150 | 1000 x 16 | equipment box | partly decoded |

Equipment record: byte 0 `kind`, byte 1 (for a talisman: its **gem slots**, 0 to 3; for other pieces an upgrade state, not decoded), bytes
2-3 piece id (u16), bytes 4-15 talisman skills / decorations. A talisman (kind 6) holds `(skill id, points)` byte pairs from byte 4: the
first pair (a Pawn Talisman, id 1, bytes `06 00 00 01 25 0a`: Auto-Guard +10 confirmed in game) and a second pair at bytes 6-7 (written with
the `talisman` debug command and read back by the game as two skills: `06 03 00 01 25 0a 63 05` was Auto-Guard +10, Psychic +5 and 3 slots;
byte 1 was written as 3 with `equip 0 @1 3` and the game gave the talisman 3 slots). Decorations socketed in a talisman are u16s from byte 8, one per slot (`00 91 00 15` = slots 1 and 2; slot 3 at byte 12 by the same pattern, unseen):
a 1-based number into the decoration table at data 0x7fe (12-byte records: penalty skill, penalty points, slots needed, 0, item id u16, u32 price, skill, points; 202 of them,
some DUMMY); 0 is an empty socket. Pairs beyond the second and what the id decides are
untested. The worn talisman is a u16 slot number at 0xcc (after the five armor pointers; 0xffff = none), and the 16 bytes at 0xb0 are a copy of
its record. Kinds: 1 body, 2 arms, 3 waist, 4 legs, 5 head, 6 talisman, 7 great sword, 8 sword & shield, 9 hammer,
10 lance, 11 heavy bowgun, 13 light bowgun, 14 long sword, 15 switch axe, 16 gunlance, 17 bow, 18 dual blades, 19 hunting horn.

The equipment box lists everything you own, including worn gear; worn gear is just a pointer into it. Upgrading a weapon replaces its parent in the same box slot, so the parent is gone afterwards.

## Archives (`.arc`)

`\0CRA` magic, u16 version, u16 entry count, 4 padding bytes, then 80-byte entries: 64-byte name (no extension, `\` separators), u32
type hash, u32 compressed size, u32 size, u32 offset. Data is zlib-compressed. The top 3 bits of the size fields are masked off; their meaning is unknown.

## Text (`.gmd`)

`\0DMG` magic. u32 string count at 0x18, u32 string-section length at 0x20; the strings are the last section-length bytes of the file,
NUL-separated. Item and equipment ids index directly into these tables. English text for everything used here is in
`arc/ID/ID_arena_eng.arc`: `Item00_eng`, `Helm_eng`, `Body_eng`, `Arm_eng`, `Waist_eng`, `Leg_eng`, `Acce_eng`, the weapon files
(`Lsword` = Great Sword, `Lsword2` = Long Sword, `Sword` = Sword & Shield, `WSword` = Dual Blades, `Pipe` = Hunting Horn, ...), and
`Skill_Type_eng` (skill names, indexed by skill id).

## Executable (`.rpx`)

Big-endian ELF32. Sections flagged `0x08000000` are zlib-compressed with a u32 uncompressed size in front. The data section at
virtual address `0x1010a000` holds all the tables below (offsets are within that section, for the US v32 executable).

### Recipes (confirmed for the pieces checked, see below)

Create recipes: 24-byte records `[flag u8, 0, id u16] [item u16, count u16] x4 [tier u8, 0, 0, 0]`, one table per equipment kind
(`recipes.rs`). A zero record may pad a table. The `flag` and `tier` bytes are unexplained. Armor tables are indexed in
name-table order; weapon tables list only weapons that can be created from scratch.

Weapon upgrade recipes: 28-byte records indexed by weapon id: 4 x u16 child ids, upgrade materials (4 slots of item/count), 2 x u16
more child ids (`recipes.rs`). Parents are found by inverting the child lists. The resulting parent links agree with Kiranico for
1,379 of 1,413 weapons; the game table is trusted where they differ.

### Armor stats (`armor.rs`)

24-byte records, one table per slot, contiguous, indexed by piece id: byte 0 base defense, byte 6 flags (bits 0-1 gender: 1 male, 2 female, 3 both; bits 2-3 class: 1 blademaster, 2 gunner, 3 both), byte 7 rarity - 1, bytes 8-12 resistances
(fire, water, thunder, ice, dragon, signed), byte 13 gem slots, bytes 14-23 five (skill id, signed points) pairs. Matches 895 of 968
pieces on every field when compared with Kiranico, and 24 of 25 checks against in-game values (one skill value differs by 1).
Bytes 1-5 are model ids and are not decoded (maximum defense comes from the price rows, see below). The gender/class flags were checked against Kiranico on 964
pieces (no exceptions) and against five in-game values; all 1,605 non-dummy records in the five tables decode.

## Weapon stats

The weapon tables used for prices (`docs/prices.md`, `weapons.rs`) also hold, relative to the price field `p` (a big-endian u32):

| type | stride | rarity - 1 | attack (u16) | affinity (i8, %) | gem slots |
|---|---|---|---|---|---|
| melee (9 types) | 28 | `p-2` | `p+6` | `p+9` | `p+16` |
| heavy/light bowgun, bow | 100 | `p-10` | `p-8` | `p+6` | `p+5` |

The stored attack is a base number; the displayed attack is that times a per-type factor: great sword 4.8, sword & shield 1.4,
hammer 5.2, lance 2.3, long sword 3.3, switch axe 4.6, gunlance 2.3, dual blades 1.4, hunting horn 4.6, light bowgun 1.3,
bow 1.2, heavy bowgun 1.48 (rounded down). Compared with Kiranico's weapon list, 97 to 100% of weapons agree on attack (the rest
are ids past the tables' ends, which hold zeros, and a few disagreements in that list); slots and affinity agree on 97 to 99%.
Not yet checked against the game's own screens. Sharpness and element are not in these records and were not found in the
executable's data or rodata sections as plain bytes, 16-bit values, cumulative values or scaled values (a table in an archive
or in code is still possible).

## Monster drops (`drops.rs`)

Three pointer tables per rank (low, high, G) at `0x765e0`, `0x76dc8` and `0x775b0` (data section offsets, monster 1's entry): body
carves, tail carves and shiny drops, `0x194` bytes (101 entries) apart. Entry `n` is for the monster whose name is entry `n` of
`Monster_eng`; it points to a list of 4-byte records `[0, chance %, item id u16]` ended by `ff ff 00 00`. The first byte is always 0
(every carve gives one item). The chances in a real list add up to 100; monsters with nothing to carve point at 0xff filler, which
that check rejects. 81 body-carve, 35 tail-carve and 58 shiny-drop lists per rank are found. Checked against a published list for
Rathian and Rathalos: all 18 of their lists (3 ranks x 3 kinds) are identical, chances included, though the game orders them differently.

Capture rewards and part-break rewards are lists of `[item id u16, quantity, chance]` ended by a zero record. Their pointers sit in one
flat table per rank (203 entries, from `0x78fd8`, `0x79304`, `0x79630`) with 2 to 6 lists per monster and no table saying where one
monster's lists end. So they are assigned by content: each list goes to the monster whose carve items it mostly holds (the monster ids
are non-decreasing down the table, which is solved with a small dynamic program), and each run of lists given to one monster is its
group. A capturable monster's group is `[capture, break lists..., one more list]`, where the last list repeats the capture's items with
different chances and is not used; a monster that cannot be captured (Ceadeus) has only break lists. The break lists are in the game's
order but nothing says which body part each is, so the app calls them "Part break 1, 2, ..." unless the table below names them. Checked against published lists for 12
monsters: 106 of the 108 capture and break lists (three ranks) are exactly where the inference puts them, and the two others are
probably disagreements in that list. The grouping is inferred, not read from a table, so a monster with generic items only could be
misplaced. Quest rewards are in the `.quest` files (`QTDS` format: multilingual text followed by binary data), which are not decoded.

## Blacksmith unlock (worked out from play, 2026-10-03)

A piece is on offer once **a monster that drops the first material of its recipe has been hunted** (killed or captured) at least once.
The count is in the save (see "Monsters hunted" below); the monster comes from the drop tables (`drops.rs`). Holding the material
does not count: on a test hunter, Great Jaggi pieces stayed off the list while 94 Great Jaggi Hides sat in the box (put there with
the debug `give` command) and appeared when the first Great Jaggi quest was done. In play on that hunter: Jaggi, Ludroth and Bone
pieces came with their monsters (Jaggi, Ludroth, Kelbi), the Sponge Gear I bow with Ludroth, and all ten Arzuros pieces and the
Jawblade right after a captured Arzuros; Lagiacrus, Wroggi and Zinogre pieces were absent. On an early save of the main hunter the
rule predicts the two Jaggi legs.

Recipe `flag` is 1 only on starting gear (always on offer) and `tier` (tail byte 0) is 0 on special village and event pieces (Yukumo
armor, a few earrings), which the rule does not cover.

The higher-rank sets (S, X, ...) want first materials that drop only in high rank (6 stars and up in solo play) or G rank (opened by
the Hunter Rank 5 urgent quest "Throne of the Abyss", which brings sets like Jaggi X, Wroggi X, Volvidon X, Rathalos X and Brachydios X).
The count is per monster, not per rank, so the app can only say a hunt in that rank is needed. Whether the game needs the rank or
something else for those is not observed: no hunter in the saves has reached high rank. Pieces whose first material is not a monster
drop (ores, bugs, fish) are not covered either.

The game never removes a piece from the blacksmith's list, so the app remembers each piece it has seen on offer in `unlocked.tsv`
next to the price ledger. `blacksmith.rs` has the rule and tests on the saves before and after the Arzuros quest.

## The blacksmith's menu order

The menu lists the pieces on offer in the order of the executable's recipe tables (see `recipes.rs`), not by piece id. For the head
menu of one hunter it showed Leather Headgear (1), Piscine Mask (2), Chainmail Headgear (3), Hunter's Helm (4) and Cap (5), Alloy
Helm (8) and Cap (9), Bone Helm (6) and Cap (7), Jaggi Helm (10) and Cap (11), Arzuros Helm (56) and Cap (57). The table has rows
1, 66, 2, 3, 4, 5, 8, 9, 6, 7, 12-15, 10, 11, 56, 57..., so the menu is the table with the rows that are not on offer left out. Where the
game records which rows are on offer is not found: `:scan` (see `docs/live.md`) found no flag array or id list that follows the menu for two
hunters. The hunt rule below also fails on one case seen later: a hunter whose save counts Kelbi x6 and Altaroth x3 is not offered Bone
Helm or Cap, which the rule says it should be, while another with Kelbi x4, Altaroth x6, Ludroth x7, Epioth x4 and Bnahabra x1 is. His menu
was exactly the starting gear (the rows with flag 1).

## Guild card

Read by comparing the saves of two hunters with the numbers on their guild cards in the game. The **play time** is a u32 in seconds at
0x4c, straight after the zenny (checked on two hunters to the second: 6863 s was 1 h 54 min and 7820 s was 2 h 10 min). The two quest counts are bytes in the guild card's own record, which starts at 0x7a30 with the hunter's name: the village quests at 0x7a4d and the hall quests at 0x7a51 (6, 8, 8, 8 and 1, 1, 2, 3 over four snapshots; the card agreed each time; a hunter with none has zeros). The same numbers sit elsewhere
too, but not reliably: 0x7b61 follows the hall count, while 0x7568 (village) lags the card record by a quest in some snapshots and 0x792f (hall) stopped
at 2 when the card went to 3, so they are something else. A byte at 0x7578 counts hall quests since the first (0, 0, 1, 2) and may be the progress
towards the next rank. A byte at 0x7569 is 1 for the hunter with rank 1 and 0 for the one with rank 0 and did not change with hall quests, so it
may be the **hunter rank**; that waits for a rank-up to be sure. A 16-bit number at 0x5a46 is a total of the quest points: it grew with every
quest (170, 230, 275, 475, 595, 805, 1045 over seven snapshots), by the amount the quest file gives (see "Quests") for 4 of the 6 quests, village ones included, so it is not the hall's hunter rank points alone. The weapon usage list (great sword 11 to 14 over four saves) is not found: no
byte or 16-bit number goes up by one with each quest, so it is stored in some other form or not in this file.

## Which part a break list is

Not found in the game's data (see `docs/ideas.md` for what was searched). The lists of a monster are in an order of its own that does not
depend on the rank, and each is one of about 20 kinds of part, the same kinds for every monster. `mh3u-core/src/breakparts.rs` keeps a
hand-made table of the kinds for 43 monsters: Rathian is head, wing; Gigginox tail, head, stomach; Lagiacrus head, front leg, chest, back;
Agnaktor head, back leg, chest, fin. It was made by matching each list's items against a published monster database (the same set of
items, for every rank of a monster where the database has the lists) and reading off the database's part names, which the game's text
does not have ("Oral cavity", "Tail end"). A test checks that each monster in the table has as many break lists as parts in some rank, and
the app names a monster's lists only for a rank where the numbers agree (some high and G rank decodes have extra lists, which stay
numbered). A name can be off.

## Monster hit zones

Each monster's archive `arc/enemy/emNNN.arc` (NNN = the monster's id in the name table) holds `enemy\emNNN\em_status00`, a "SME" block.
After a header of about 0x100 bytes come tables of 10-byte rows, one per hit zone: eight percentages (cut, impact, shot, fire, water,
ice, thunder, dragon), a dizzy byte and a fixed `0x64`; unused rows are filled with `0x64`. The first table starts at 0xD0, 0x110 or
elsewhere, so `hitzones.rs` finds it by the row shape (starts non-zero, ends `0x64`, at most 180) and reads up to the filler. It is the
normal state; later tables (other states) are not read. Compared with Kiranico on 49 monsters, 319 of the 432 zones it lists are in the
first table. Nothing in the file names a zone, and nothing found says which zone each part-break list belongs to (the order of a monster's break lists is not the order of its zones: Gigginox breaks tail, head, stomach and its zones run head first).

## Hunter's Notes

The text archive's `GUI\font\HNote_eng` holds a description of every monster in its first 73 entries (0 to 72); the rest are help pages. The
order is the in-game journal's (small monsters from Aptonoth, then large ones from Great Jaggi, a species before its subspecies), not the
order of the monster name table, and nothing in the data links a note to a monster. `notes.rs` has the table (monster id to note
number), made by reading the notes: each one either describes the species ("Fire-breathing female wyverns..." is Rathian), says "a
subspecies of X", or names the monster. A test checks that each note contains a word of its monster's name where it names anything.
A few small monsters share a note (the two Slagtoth ids, Giggi and Giggi Sac).

## Items: sell price

The data section holds a run of 20-byte records indexed by item id (the name table's ids), from `0x1188` up to item 1532 (`items.rs`). A
big-endian u32 at the start of a record is the **sell price** in zenny; the 681 items a published list gives a price for all agree.
Byte 7 of a record looks like the carry limit (99 for most items, 10 or 1 for others) and agrees for 652 of those 681, so it is not
used. The buying price is not in this table: items in shops are priced elsewhere (a few shop prices are 2 to 10 times the sell price).
The other bytes are not decoded (byte 5 and 6 vary with the kind of item; the last four bytes hold a number that is not the price).

## Armor maximum defense

Read from the 32-byte price rows (`docs/prices.md`): byte 2 is 1 for the gunner version of a piece, and bytes 3 to 8 are six numbers g3 to
g8. The defense gained by fully upgrading is `6*g7 + 2*g8 - g3 - g4 - g5 - 3*g6 - 2` for a blademaster piece and `3*g7 + g8 - g4 - g6 - 2`
for a gunner piece; the maximum is the base defense plus that. Found by fitting the 969 head, body and arm pieces that a published list
gives a maximum for: the blademaster formula is exact on all 56 distinct rows of numbers, the gunner formula on 51 of 53, and 950 of the 969
pieces agree. The other 19 are the Rathian S, Nargacuga X, Brachydios X, Damascus X and Miralis sets (1 or 2 away) and the Rhenoplos set
(1 away), plus one earring: either those pieces carry one more rule or the list is wrong for them, which the game's screens would settle.
The waist and legs tables have the same layout and use the same formulas, but no list was available to check them against. The weights
are an empirical fit, not something read from the game's own text. `armor::max_defense`.

## Not found / not decoded

- Weapon sharpness and element; talismans with more than two skills, and what the talisman id decides; decorations in armor records; which body part each part-break list is, and the names of the hit zones.
- How the game stores which pieces the blacksmith has unlocked, and what the high-rank (S, X...) sets need. The app uses the hunt rule.

## Per-piece flag tables (partly understood)

Five tables of one byte per piece sit in the save at `0x61a0`, `0x6320`, `0x6480`, `0x6600` and `0x6770` (about `0x160-0x170` bytes
apart, so one per armor slot; which table is which slot is not settled). A new hunter has a single `01` in each (at offsets 5, 3, 14, 1
and 10). Finishing quests and receiving items sets runs of neighbouring entries to `06` (bits 1 and 2), and the first of a run to `07`;
after a visit to the blacksmith's screen the `06`/`07` became `02`/`03` (bit 2 cleared, as for a "new" marker). Seen in saves of two
hunters. They look like "this piece has turned up" flags, but they do **not** match what the blacksmith offers: on slot 2 before the
Great Jaggi quest, one table was already flagged while the Jaggi pieces were not offered, and Bone Helm was offered with its table
unflagged. Do not use them for the blacksmith until a controlled before/after test (one action between two saves) explains them.

### Monsters hunted (found 2026-10-03, matches the blacksmith)

A table of u16 counters starts at `0x57a0`; entry `n` is for the monster whose id in the game's name table is `n + 6` (Jaggi 11,
Jaggia 5, Great Jaggi 1, Ludroth 7, Kelbi 4, Arzuros 3 in the WornTester save). Capturing counts as well as killing. Of the pieces tested
in play, the blacksmith offers a piece once the monster its materials come from has a count above zero: Jaggi armor after Great Jaggi
was hunted (holding the materials from `give` did nothing), Ludroth and Bone pieces with their monsters, all ten Arzuros pieces
and the Jawblade right after a captured Arzuros. Higher-rank variants (S, X, ...) need more than a first hunt (presumably quest
rank) and are not explained. The five per-piece flag tables above did NOT change when the Arzuros pieces appeared, so they are not the
blacksmith's list.

The drop tables are not in the order of the name table: their rows are the name table's order with small creatures slipped in
between (and no row for the fish), so a row number is not a name id. `drops.rs` maps the rows by their contents (`ROW_RUNS`); rows that
belong to no named monster are left out. Earlier versions showed the later monsters' drops under the wrong names.

## Quests (`quest/us/q_NNNNN.quest`, found 2026-10-03)

`QTDS`, a u32 version (5), then the texts, then a binary part. A text is five strings (English, French, German, Italian, Spanish),
each a u32 length and the bytes (UTF-8, `\r\n` for line breaks). In order: the **title** (`Bear Trap`), the quest id (u16), the main
**goal** (`Capture an Arzuros`), the star rank (u8) and the **map** (u8, see below), five empty strings, the time limit in minutes (u16), the
failure condition, six bytes, the **client** and the **description**. All 413 files of the US dump read this way.

The binary part (442 bytes for most quests, longer for 59 with extra data) starts right after the description. Found so far:

| Offset | What | How sure |
|---|---|---|
| 2 | kind: 1 slay, 2 deliver, 4 capture, 5 hunt (others: special goals) | matches the goal text on every quest looked at |
| 10 + 11 k (k < 5) | large monster records: the first byte is the monster's id in the name table, 0 for none | matches the goal text (including two-monster hunts) |
| 71 + 4 k (k < 43) | rewards `[item u16, quantity, chance]`; even k is the main box (monster parts), odd k the second box (supplies); chance 0 = always | the boxes add up to 100 on 412 of 413 quests (quest 1707's main box adds up to 110); the Arzuros capture quest matches what a hunter received |
| 327 + 4 k (k < 4) | four u32: the **fee**, the zenny **reward**, a third of the reward, and the **points** (hunter rank points in the hall; the game gives none for village quests, though the number is there) | fee and reward match the quest board on 3 quests (Bug Hunt 100 and 600, The Fisherman's Tale 400 and 4000, Rathian's Wrath 920 and 9200); the points matched the rise in the save's total at 0x5a46 for 4 of 6 quests (hall and village), and the sum for two; the other 2 gained less |
| 0, 1 | the two **small monsters** of the quest (ids in the name table, 0 for none) | Bug Hunt has Altaroth and Bnahabra, Farm Aid Jaggia and Jaggi, No Love for Ludroth Ludroth and Fish; fits the goal text of the quests looked at |
| 344 on | spawn lists | not decoded |

**Where a quest is taken** follows from its id (also the file name `q_NNNNN`): below 10000 is a village quest, 10000 to 19999 a hall quest
(the hall's 1★ quests are 111xx, 6★ ones 116xx), 20000 and up are arena and event quests. Checked on a hunter whose guild card showed 8 village
and 3 hall quests: the 8 titles in its quest history were quests 1101, 1102, 1103, 1202, 1203, 1204, 1205 and 2201, and the 3 hall ones 11105,
11106 and 11112. The same title can exist in both places (Playing with Fire is quests 1302, 11105 and 11611). 
The save keeps a total of the points at 0x5a46 (see "Guild card"); it rose after village quests too, so it is not the hall's hunter rank points alone.

**The rank** (low, high, G) is not stored as such; it follows from the place and the stars, as a player described the game's tiers: village quests of
1 to 5 stars are low rank and 6 to 9 high rank; hall quests of 1 and 2 stars are low rank (hunter rank 1 to 2), 3 to 5 high rank (3 to 5) and 6 to
8 G rank (6 and up). G rank exists only in the hall. The files agree with this: the monster record's tenth byte, a difficulty number, is 0 or 1 up to
5 stars in the village and 10 to 14 from 6 stars, and in the hall it is 1 to 7 up to 5 stars and 18 to 26 from 6 stars. A published list of village
quests with stars differs by one for some quests (Guts: It's What's for Dinner is 1 star in the files, The Merchant's Mission 6), so the files'
stars are used.

**The map** is the byte after the star rank in the text part: 1 Deserted Island, 2 Sandy Plains, 3 Flooded Forest, 4 Tundra, 5 Volcano,
6 Great Desert, 7 Underwater Ruins, 8 Land Arena, 11 Sacred Land, 12 Water Arena (named from its quests, which are all about the sea),
13 Misty Peaks; 14 and 15 are Elder Dragon places not named. Found by looking up the quests of a published list of one quest per map and
checked by a quest board reading of three (Bug Hunt Deserted Island, The Fisherman's Tale Flooded Forest, Rathian's Wrath Sandy Plains).
Day and night quests of one map have the same number; no byte of the quest was found that tells them apart. (An earlier note here that
named the map from the first byte of the binary part was wrong: those bytes are the small monsters.)

The quest folders: `quest/us` holds the English quests; `quest/eu` has the same files (byte for byte, for the one compared; every quest holds all five languages); `quest/btl` and
`quest/support` hold other kinds of files (`.quest_btl`, `.supp`) that are not read. The `DLC/us` folder was empty in this dump.
