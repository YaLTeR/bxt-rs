use bitflags::bitflags;

bitflags! {
    pub struct Buttons: u16 {
        const IN_ATTACK = 1;
        const IN_JUMP = 1 << 1;
        const IN_DUCK = 1 << 2;
        const IN_FORWARD = 1 << 3;
        const IN_BACK = 1 << 4;
        const IN_USE = 1 << 5;
        const IN_CANCEL = 1 << 6;
        const IN_LEFT = 1 << 7;
        const IN_RIGHT = 1 << 8;
        const IN_MOVELEFT = 1 << 9;
        const IN_MOVERIGHT = 1 << 10;
        const IN_ATTACK2 = 1 << 11;
        const IN_RUN = 1 << 12;
        const IN_RELOAD = 1 << 13;
        const IN_ALT1 = 1 << 14;
        const IN_SCORE = 1 << 15;
    }
}
