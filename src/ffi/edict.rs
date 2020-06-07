use bitflags::bitflags;

bitflags! {
    pub struct Flags: i32 {
        const FL_FLY = 1;
        const FL_SWIM = 1 << 1;
        const FL_CONVEYOR = 1 << 2;
        const FL_CLIENT = 1 << 3;
        const FL_INWATER = 1 << 4;
        const FL_MONSTER = 1 << 5;
        const FL_GODMODE = 1 << 6;
        const FL_NOTARGET = 1 << 7;
        const FL_SKIPLOCALHOST = 1 << 8;
        const FL_ONGROUND = 1 << 9;
        const FL_PARTIALGROUND = 1 << 10;
        const FL_WATERJUMP = 1 << 11;
        const FL_FROZEN = 1 << 12;
        const FL_FAKECLIENT = 1 << 13;
        const FL_DUCKING = 1 << 14;
        const FL_FLOAT = 1 << 15;
        const FL_GRAPHED = 1 << 16;
        const FL_IMMUNE_WATER = 1 << 17;
        const FL_IMMUNE_SLIME = 1 << 18;
        const FL_IMMUNE_LAVA = 1 << 19;
        const FL_PROXY = 1 << 20;
        const FL_ALWAYSTHINK = 1 << 21;
        const FL_BASEVELOCITY = 1 << 22;
        const FL_MONSTERCLIP = 1 << 23;
        const FL_ONTRAIN = 1 << 24;
        const FL_WORLDBRUSH = 1 << 25;
        const FL_SPECTATOR = 1 << 26;
        const FL_CUSTOMENTITY = 1 << 29;
        const FL_KILLME = 1 << 30;
        const FL_DORMANT = 1 << 31;
    }
}
