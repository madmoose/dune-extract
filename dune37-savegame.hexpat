#pragma author BigsFr
#pragma description Dune SAV files (need to be in uncompressed form)
#pragma endian little

#include <std/mem.pat>
#include <std/io.pat>
#include <std/sys.pat>
#include <std/string.pat>
#include <hex/provider.pat>

u8 version = 0;


bitfield MapAreaFlags {
    flag: 2;
};

bitfield CharacterStatus {
    todo: 8;
};

bitfield NpcStatus {
    unk4: 1;
    unk3: 1;
    back_from_desert_alone: 1;
    unk2: 1;
    unk1: 1;
    met: 1;
    following: 1;
    follow_me_disabled: 1;
};

bitfield Characters {
    
    Leto: 1;    // second byte lsb
    Jessica: 1;
    Thufir: 1;
    Duncan: 1;
    Gurney: 1;
    Stilgar: 1;
    Kynes: 1;
    Chani: 1;   // second byte msb
    
    Hara: 1;     // first byte lsb
    Baron: 1;
    Feyd: 1;
    Emperor: 1;
    Soldier: 1;
    Smuggler: 1;
    Fremen1: 1;
    Fremen2: 1;  // first byte msb
};

struct Npc {
    u8 x_or_room;
    u8 X_or_floorplan;
    u8 Y;
    u8 y;
    u16 dialog_ptr;
    u16 unk1;
    u16 date_Npc_started_following;
    u16 date_Npc_stopped_following;
    u16 unk2;
    u8 Npc_sprite_id;
    NpcStatus status;
};

struct Smuggler {
    u8 unk[17];
};

bitfield TroopEquipment {
    unused: 1;
    bulb: 1;
    atomic: 1;
    modules: 1;
    guns: 1;
    knives: 1;
    orni: 1;
    harvester: 1;
};

bitfield PlaceStatus {
    hidden: 1;
    prospected: 1;
    has_wind_trap: 1;
    visited: 1;
    infiltrated: 1;
    battle_won: 1;
    fremen_found: 1;
    under_attack: 1;
    has_vegetation: 1;
};
struct Place {
    u8 firstName;
    u8 lastName;
    u8 desert;
    u8 mapx;
    u8 mapy;
    u8 mapu;
    u8 anotherx;
    u8 anothery;
    u8 apparence;
    u8 troopid;
    PlaceStatus status;
    u8 discoverable_at_phase;
    u8 unk1;
    u8 unk2;
    u8 unk3;
    u8 unk4;
    u8 spicefieldid;
    u8 unk5;
    u8 spicedensity;
    u8 unk6;
    u8 nbrHarvesters;
    u8 nbrOrnis;
    u8 nbrKnifes;
    u8 nbrLasguns;
    u8 nbrWeirding_modules;
    u8 nbrAtoms;
    u8 nbrBulbs;
    u8 water;
};



struct Troop {
    u8 troop_id;
    u8 next_troop_id_in_same_place;
    u8 position_around_sietch_on_map;
    u8 status_or_and_job;
    u8 unk1;
    u8 unk2;
    u8 pos_x;
    u8 pos_X;
    u8 pos_Y;
    u8 pos_y;
    u16 start_activity_time;
    u8 spice_per_hour_if_miner;
    u8 unk3;
    u16 spice_mined_if_miner;
    u8 unk4;
    u8 unk5;
    u8 dissatisfaction;
    u8 speech_to_do_and_informations_to_display;
    u8 elapsed_days_since_something;
    
    u8 motivation; 
    
    //skills
    // 00-0F on trial
    // 10-1F novice
    // 20-2F average
    // 30-3F efficient
    // 40-4F skilled
    // 50-5F expert
    u8 spice_skill;
    u8 army_skill;
    u8 eco_skill;
    
    TroopEquipment equipment;
    u8 population;
};

// 256 bytes of game state
// condit.hsq variables are refering to this.
struct GameState  
{
                                               
    u16 x00_rnd;        
    u16 x02_ingame_time;      
    u8 x04_paul_pos_x_or_room;                      
    u8 x05_paul_pos_X_or_floorplan;               
    u8 x06_paul_pos_Y;                            
    u8 x07_paul_pos_y;                             
    u8 x08_place_or_FF_if_in_desert;
    u8 x09_unk [[color("FF0000")]];
    u8 x0A_something_about_telepathie;
    u8 x0B_paul_room;                              
    u8 x0C_last_seen_room; 
    u8 x0D_previous_room;                          
    Characters x0E_met;   
    
                                                        
    Characters x10_following;                     
    Characters x12_inRoom;                    
    u8 x14_Npc_we_speak_to; 
    u8 x15_unk [[color("FF0000")]];                       
    u8 x16_maybe_ingame_date_of_last_speak;           
    u16 x17_or_maybe_ingame_date_of_last_speak;     
    CharacterStatus x19_statusOfSpeakingCharactern; 
    u8 x1A_not_first_phrase_of_pnf;    
    u8 x1B_unk [[color("FF0000")]];
    u8 x1C_speak_command_counter [[color("FF0000")]];
    u8 x1D_unk [[color("FF0000")]];
    u8 x1E_unk [[color("FF0000")]];
    u8 x1F_unk [[color("FF0000")]];
    
    
    u16 x20_amount_we_need_to_pay_to_smuggler;
    u8 x22_unk [[color("FF0000")]];
    u8 x23_unk [[color("FF0000")]];
    u8 x24_unk [[color("FF0000")]];                        
    u8 x25_nbr_sietch_visited;                      // 25
    u8 x26_is_FF_if_visiting_here_for_first_time;
    u8 x27_nbr_sietch_we_know_position;
    u8 x28_nbr_troops_hired;
    u8 x29_charism;
    u8 x29_game_phase;
    u8 x2B_unk;
    u16 x2C_unk;
    u8 x2E_maybe_troop_we_interact_with;
    u8 x2F_unk;
    
    
    u8 x30_unk;
    u8 x31_unk;
    u16 x32_unk;
    u16 x34_unk;
    u8 x36_unk;
    u8 x37_unk;
    u8 x38_to_x3F_unk[8] [[color("FF0000")]];
    
    u8 x40_unk;
    u8 x41_unk;
    u16 x42_unk;
    u16 x44_unk;
    u16 x46_unk;
    
    u8 x48_to_x4B_unk[4] [[color("FF0000")]];
    
    u8 x4C_unk;
    u8 x4D_unk_something_about_place;
    u16 x4E_unk;
    
    u8 x50_unk;
    u8 x51_unk;
    u8 x52_unk;
    u8 x53_unk_bitfield;
    u8 x54_unk;
    u8 x55_unk;
    u8 x56_unk;
    u8 x57_unk;
    u8 x58_unk;
    u8 x59_unk;
    u8 x5A_unk;
    u8 x5B_unk;
    
    u16 x5C_unk_bitfield;
    u16 x5E_unk_bitfield;
    
    u8 x60_unk;
    u8 x61_unk;
    u8 x62_to_x65_unk[4] [[color("FF0000")]];
    u8 x66_unk;
    u8 x67_to_x6F_unk[9] [[color("FF0000")]];
    
    u8 x70_to_x7F_unk[16] [[color("FF0000")]];
    
    u8 x80_to_x84_unk[5] [[color("FF0000")]];
    u8 x85_unk;
    u8 x86_to_x8F_unk[10] [[color("FF0000")]];
    
    u8 x90_unk;
    u8 x91_unk;
    u16 x92_percent_atreid;
    u16 x94_percent_harko;
    u16 x96_spice_prod_atreid;
    u16 x98_spice_prod_harko;
    u8 x9A_somthing_population_atreid;
    u8 x9B_somthing_population_atreid2;
    u8 x9C_somthing_population_harko;
    u8 x9D_somthing_population_harko2;
    u8 x9E_unk_bitfield;
    u8 x9F_unk;
    
    u16 xA0_spice;
    u16 xA2_unk;
    u16 xA4_unk;
    u8 xA6_to_xAF_unk[10] [[color("FF0000")]];
    
    u16 xB0_produced_spice_increased_by;
    u16 xB2_produced_spice_decreased_by;
    u8 xB4_to_xB5_unk[2] [[color("FF0000")]];
    u16 xB6_unk;
    u16 xB8_unk;
    u16 xBA_unk;
    u16 xBC_spice_asked_to_send;
    u8 xBE_unk;
    u8 xBF_unk_bitfield;
     
    
    u16 xC0_unk;
    u8 xC2_status_final_attack;
    u8 xC3_unk;
    u8 xC4_unk;
    u8 xC5_unk;
    u8 xC6_to_xC8_unk[3] [[color("FF0000")]];
    u8 xC9_something_about_new_telecom_message;
    u8 xCA_to_xCE_unk[5] [[color("FF0000")]];
    u8 xCF_days_left_before_sending_spice;
    
    
    
    u8 xD0_to_xD4_unk[5] [[color("FF0000")]];
    u8 xD5_unk;
    u16 xD6_unk;
    u8 xD8_to_xDA_unk[3] [[color("FF0000")]];
    u8 xDB_unk;
    u8 xDC_to_xDF_unk[4] [[color("FF0000")]];
    
    
    
    u8 xE0_to_xE1_unk[2] [[color("FF0000")]];
    u16 xE2_unk;
    u8 xE4_to_xE6_unk[3] [[color("FF0000")]];
    u8 xE7_died_alone_in_desert;
    u8 xE8_unk;
    u8 xE9_unk;
    u8 xEA_unk;
    u8 xEB_unk;
    u8 xEC_unk;
    u8 xED_unk;    
    u8 xEE_to_xEF_unk[2] [[color("FF0000")]];
    
    u8 xF0_to_xF1_unk[2] [[color("FF0000")]];
    u16 xF2_unk;
    u8 xF4_desert_walk_counter;
    u8 xF5_unk;
    u8 xF6_unk;
    u8 xF7_unk;
    u8 xF8_unk;
    u8 xF9_unk;    
    u8 xFA_unk;
    u8 xFB_unk;
    u8 xFC_unk;     
    u8 xFD_unk;
    u8 xFE_unk; 
    u8 xFF_maybe_nbr_days;   

};


struct Dialog_data {
    u32 dialog [while(std::mem::read_unsigned($, 2) != 0xFFFF)] [[color("007777")]];
    u16 end [[color("880000")]];
};

struct Save {
    str filename = hex::prv::get_information("file_path","");
    std::print("analysing {}", filename);
    
    
    
    if (std::string::contains(filename, "DUNE21")) {version = 21;}
    else if (std::string::contains(filename, "DUNE23")) {version = 23;}
    else if (std::string::contains(filename, "DUNE24")) {version = 24;}
    else if (std::string::contains(filename, "DUNE37")) {version = 37;}
    else if (std::string::contains(filename, "DUNE38")) {version = 38;}
    
    std::assert(version != 0, "unsupported version");
    std::print("version is {}", version);
    
    u16 ingame_time_displayed_for_save  [[color("00FF00")]];
    u16 unk00;
    u16 unk01 [[color("FF0000")]];
    MapAreaFlags map_flags[50680] [[color("FF00FF")]];
    u8 unk02[163] [[color("FF0000")]];
    
    u16 dialogue_offsets[while(std::mem::read_unsigned($, 2) != 0xFFFF)] [[color("3333FF")]];
   
    u16 nbr_offsets = sizeof (dialogue_offsets)/2;
    std::print("nbr offsets = {}", nbr_offsets);        
    Dialog_data dialogue_data[nbr_offsets]; 
    
    if (version ==21)
    u8 unk03[36] [[color("000000")]];
    if (version ==23)
    u8 unk03[48] [[color("000000")]];
    if (version ==24)
    u8 unk03[50] [[color("000000")]];
    if (version >=37)
    u8 unk03[104] [[color("000000")]];
    
    
    GameState gamestate; //256 bytes
    
    //if (version > 24)
    //u8 unk0C[28] [[color("FF0000")]];
    
    
    Place places[while(std::mem::read_unsigned($, 2) != 0xFFFF)] [[color("33FF33")]]; 
    u16 end_of_places [[color("880000")]];
    
    
    Troop troops[67] [[color("6666FF")]];
    
    u8 unk0D[29] [[color("000000")]];
    
    Npc npcs[16] [[color("6666BB")]];
    
    Smuggler smugglers[6] [[color("66FF66")]];
    
    
    
    if (version ==21)
    u8 unk0E[50] [[color("000000")]];
    else
    u8 unk0E[52] [[color("000000")]];

    
    u16 unk_again  [[color("000000")]];
    u16 ingame_time_again [[color("00FF00")]];
    u8 contact_distance;
    
    
    

    if (version <=24)
    u8 unk0F[249] [[color("000000")]];
    else
    u8 unk0F[234] [[color("000000")]];
   
};


Save save @ 0x00;