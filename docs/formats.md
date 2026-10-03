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
decorations (not decoded). Kinds: 1 body, 2 arms, 3 waist, 4 legs, 5 head, 6 talisman, 7 great sword, 8 sword & shield, 9 hammer,
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

## Not found / not decoded

- **Armor max defense.** The 32-byte price rows (`docs/prices.md`) hold six growth bytes that determine it, but no formula is known.
- Weapon sharpness and element; talisman data.
- Which pieces the blacksmith has unlocked.
