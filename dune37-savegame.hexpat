#pragma endian little

#include <std/mem.pat>

struct Header {
    u16 flags;
    u16 size;
};


bitfield MapAreaFlags {
    flag: 2;
};

struct UISimpleSprite {
    u16 index;
    s16 y;
    s16 x;
};

struct UISimpleSpriteList {
    UISimpleSprite sprites[while(std::mem::read_unsigned($, 2) != 0xFFFF)];
    u16 eom;
};

struct DataSegment {
    u16 rand_bits;
    u16 ds_0002;
    u16 ds_0004_current_location_and_room;
    u8 ds_0006;
    u8 ds_0007;

    padding[0x441d + 0x00e8 - $];
    u8 ui_head_index;

    padding[0x441d + 0x11dd - $];
    UISimpleSpriteList intro_28_night_attack_ui_list;

    padding[0x441d + 0x120b - $];
    UISimpleSpriteList palace_plan;

    padding[0x567e - $];
};

struct Save {
    Header header;
    MapAreaFlags map_flags[50684];
    u8 unknown[162];
    u8 dialogue[4600];
    DataSegment data_segment;
};


Save save @ 0x00;
