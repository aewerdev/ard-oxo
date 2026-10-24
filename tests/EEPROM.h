#pragma once
#include <stdint.h>
struct FakeEEPROM {
    uint8_t mem[1024] = {};
    uint8_t read(int at) { return mem[at]; }
    void update(int at, uint8_t val) { mem[at] = val; }
};
extern FakeEEPROM EEPROM;
