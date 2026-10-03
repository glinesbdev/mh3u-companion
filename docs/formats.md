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

Equipment record: byte 0 `kind`, byte 1 upgrade/slot state (not decoded), bytes 2-3 piece id (u16), bytes 4-15 talisman skills /
decorations. A talisman (kind 6) holds `(skill id, points)` byte pairs from byte 4; only the first pair is verified (a Pawn Talisman, id 1, bytes `25 0a`: Auto-Guard +10 confirmed in game; no slots, rest zero). Kinds: 1 body, 2 arms, 3 waist, 4 legs, 5 head, 6 talisman, 7 great sword, 8 sword & shield, 9 hammer,
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
Bytes 1-5 (model ids, max defense data) are not decoded. The gender/class flags were checked against Kiranico on 964
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
order but nothing says which body part each is, so the app calls them "Part break 1, 2, ...". Checked against published lists for 12
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

## Not found / not decoded

- **Armor max defense.** The 32-byte price rows (`docs/prices.md`) hold six growth bytes that determine it, but no formula is known.
- Weapon sharpness and element; talisman points, second skill, slots and what the talisman id decides (one sample); which body part each part-break list is; quest rewards.
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
**goal** (`Capture an Arzuros`), the star rank (u8) and one more byte, five empty strings, the time limit in minutes (u16), the
failure condition, six bytes, the **client** and the **description**. All 413 files of the US dump read this way.

The binary part (442 bytes for most quests, longer for 59 with extra data) starts right after the description. Found so far:

| Offset | What | How sure |
|---|---|---|
| 2 | kind: 1 slay, 2 deliver, 4 capture, 5 hunt (others: special goals) | matches the goal text on every quest looked at |
| 10 + 11 k (k < 5) | large monster records: the first byte is the monster's id in the name table, 0 for none | matches the goal text (including two-monster hunts) |
| 71 + 4 k (k < 43) | rewards `[item u16, quantity, chance]`; even k is the main box (monster parts), odd k the second box (supplies); chance 0 = always | the boxes add up to 100 on 412 of 413 quests (quest 1707's main box adds up to 110); the Arzuros capture quest matches what a hunter received |
| 327 on | stage, fees, zenny, spawn lists | not decoded |

The quest folders: `quest/us` holds the English quests; `quest/eu` has the same files (byte for byte, for the one compared; every quest holds all five languages); `quest/btl` and
`quest/support` hold other kinds of files (`.quest_btl`, `.supp`) that are not read. The `DLC/us` folder was empty in this dump.
