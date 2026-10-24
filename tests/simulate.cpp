#include "Arduino.h"
#include "EEPROM.h"
#include <cassert>
#include <iostream>
FakeSerial Serial;
FakeEEPROM EEPROM;
#include "../firmware/NanoOXO/NanoOXO.ino"

static void classicAndBossTest() {
  startStage(1);
  assert(hp == 1 && stage == 1);
  cells[0] = 1; cells[1] = 1;
  playHuman(2);
  assert(phase == 'W' && unlocked == 2);
  assert(EEPROM.read(1) == 2);
  startStage(2);
  assert(hp == 2);
  cells[0] = 1; cells[1] = 1;
  playHuman(2);
  assert(phase == 'P' && roundNum == 2 && hp == 1);
  // The bot should NOT get a free move on a reset board.
  for (uint8_t i=0; i<9; ++i) assert(cells[i] == 0);
  cells[0] = 1; cells[1] = 1;
  playHuman(2);
  assert(phase == 'W' && hp == 0 && unlocked == 3);
}
static void ultimateMappingTest() {
  startStage(3);
  assert(boardOf(0) == 0 && placeOf(0) == 0);
  assert(boardOf(4) == 1 && placeOf(4) == 1);
  assert(boardOf(40) == 4 && placeOf(40) == 4);
  assert(boardOf(80) == 8 && placeOf(80) == 8);
  doMove(40,1);
  assert(forced == 4);
  assert(!legal(0));
  assert(legal(30));
  // If the targeted mini-board is closed, any open board is legal.
  resetBoard();
  mini[4] = 1;
  doMove(10, 1); // top-left mini-board, center square -> mini 4
  assert(forced == 9);
  assert(legal(0));
  assert(!legal(40));
}
static void ultimateWinTest() {
  startStage(3);
  mini[0] = 1; mini[1] = 1;
  // Claim the top-right mini-board with X on its top row.
  cells[6] = 1; cells[7] = 1;
  doMove(8, 1);
  assert(mini[2] == 1);
  assert(phase == 'W' && unlocked == 4);
}
static void drawClosedTest() {
  startStage(3);
  for (int i=0; i<9; ++i) mini[i] = 3;
  mini[0] = 0;
  uint8_t draw[9] = {1,2,1,1,2,2,2,1,1};
  for (int k=0;k<9;++k) {
    uint8_t r = k/3, c = k%3;
    cells[r*9+c] = draw[k];
  }
  assert(!lineWinnerInMini(0));
  assert(miniFull(0));
  mini[0] = 3;
  assert(allMiniClosed() && macroWinner() == 0);
}
static void randomizedFullGames() {
  // Exercise the actual AI and legal-move validation in the firmware.
  for (int st = 1; st <= 5; ++st) {
    for (int game = 0; game < 25; ++game) {
      startStage(st);
      for (int turn = 0; turn < 260 && phase == 'P'; ++turn) {
        uint8_t moves[81], count = 0;
        uint8_t max = ultimate() ? 81 : 9;
        for (uint8_t i = 0; i < max; ++i) if (legal(i)) moves[count++] = i;
        assert(count > 0);
        uint8_t pick = moves[std::rand() % count];
        playHuman(pick);
        if (ultimate() && phase == 'P') {
          for (uint8_t b = 0; b < 9; ++b) {
            if (mini[b] == 1 || mini[b] == 2) assert(lineWinnerInMini(b) == mini[b]);
            if (mini[b] == 3) assert(miniFull(b));
          }
          assert(forced <= 9);
          if (forced != 9) assert(mini[forced] == 0);
        }
      }
      // If the human keeps winning boss rounds, the encounter still eventually ends.
      assert(phase != 'P');
    }
  }
  std::cout << "PASS: 125 randomized full games with the real firmware AI\n";
}

int main() {
  setup();
  assert(unlocked == 1);
  classicAndBossTest();
  ultimateMappingTest();
  ultimateWinTest();
  drawClosedTest();
  startStage(5);
  assert(hp == 3 && stage == 5 && ultimate());
  randomizedFullGames();
  std::cout << "PASS: 3x3, boss HP/progression, 81-cell mapping, forced board, closed board, macro win, draw, final stage\n";
}
