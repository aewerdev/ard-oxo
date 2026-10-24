/*
 * NANO // OXO : REALMS
 * Arduino Nano (ATmega328P, 16 MHz, 2 KB SRAM)
 * The game, enemy AI, progression and rules run on the Arduino.
 * Rust terminal app is only an I/O viewport over Serial at 115200 baud.
 * Protocol version: OXO1
 *
 * PC -> Nano: SYNC | MOVE <0..80> | RETRY | NEXT | LEVEL <1..5> | PING
 * Nano -> PC:
 *   HELLO OXO1
 *   S <level> <phase> <forced> <hp> <maxhp> <unlocked> <round> <81 cells> <9 mini-winners>
 *   M <text>
 *   E <text>
 *   PONG
 * Cells/winners: . = empty, X / O, # = drawn board.
 */

#include <Arduino.h>
#include <EEPROM.h>
#include <stdlib.h>
#include <string.h>

const uint8_t N_LEVELS = 5;
const uint8_t ANY_BOARD = 9;
const byte EEPROM_MAGIC = 0xA7;

uint8_t cells[81]; // 0 = blank, 1 = human X, 2 = boss O
uint8_t mini[9];   // 0 = open, 1 = X, 2 = O, 3 = drawn
uint8_t stage = 1;
uint8_t unlocked = 1;
uint8_t hp = 1, maxHp = 1, roundNum = 1;
uint8_t forced = ANY_BOARD;
char phase = 'P'; // P playing, W stage won, L lost, D draw
char input[24];
uint8_t inputLen = 0;

const uint8_t LINES[8][3] = {
  {0,1,2}, {3,4,5}, {6,7,8},
  {0,3,6}, {1,4,7}, {2,5,8},
  {0,4,8}, {2,4,6}
};

bool ultimate() { return stage >= 3; }

uint8_t readCell(uint8_t b, uint8_t k) {
  if (!ultimate()) return cells[k];
  uint8_t r = (b / 3) * 3 + k / 3;
  uint8_t c = (b % 3) * 3 + k % 3;
  return cells[r * 9 + c];
}

uint8_t boardOf(uint8_t index) {
  return (index / 9 / 3) * 3 + (index % 9) / 3;
}

uint8_t placeOf(uint8_t index) {
  return ((index / 9) % 3) * 3 + (index % 9) % 3;
}

uint8_t lineWinnerInMini(uint8_t b) {
  for (uint8_t n = 0; n < 8; ++n) {
    uint8_t a = readCell(b, LINES[n][0]);
    if (a && a == readCell(b, LINES[n][1]) && a == readCell(b, LINES[n][2])) return a;
  }
  return 0;
}

bool miniFull(uint8_t b) {
  for (uint8_t k = 0; k < 9; ++k) if (readCell(b, k) == 0) return false;
  return true;
}

uint8_t macroWinner() {
  for (uint8_t n = 0; n < 8; ++n) {
    uint8_t a = mini[LINES[n][0]];
    if (a != 0 && a != 3 && a == mini[LINES[n][1]] && a == mini[LINES[n][2]]) return a;
  }
  return 0;
}

bool allMiniClosed() {
  for (uint8_t i = 0; i < 9; ++i) if (mini[i] == 0) return false;
  return true;
}

bool legal(uint8_t index) {
  if (phase != 'P') return false;
  if (!ultimate()) return index < 9 && cells[index] == 0;
  if (index >= 81 || cells[index] != 0) return false;
  uint8_t b = boardOf(index);
  return mini[b] == 0 && (forced == ANY_BOARD || forced == b);
}

// Temporary simulation of a candidate move, for low-cost evaluation.
bool wouldWinMini(uint8_t index, uint8_t player) {
  uint8_t b = ultimate() ? boardOf(index) : 0;
  if (cells[index] != 0 || (ultimate() && mini[b] != 0)) return false;
  cells[index] = player;
  bool win = lineWinnerInMini(b) == player;
  cells[index] = 0;
  return win;
}

bool wouldWinMacro(uint8_t b, uint8_t player) {
  if (mini[b] != 0) return false;
  mini[b] = player;
  bool win = macroWinner() == player;
  mini[b] = 0;
  return win;
}

uint8_t nextForced(uint8_t index) {
  if (!ultimate()) return ANY_BOARD;
  uint8_t target = placeOf(index);
  return mini[target] == 0 ? target : ANY_BOARD;
}

void resetBoard() {
  memset(cells, 0, sizeof(cells));
  memset(mini, 0, sizeof(mini));
  forced = ANY_BOARD;
  phase = 'P';
}

void startStage(uint8_t value) {
  stage = value;
  maxHp = (stage == 2 || stage == 4) ? 2 : ((stage == 5) ? 3 : 1);
  hp = maxHp;
  roundNum = 1;
  resetBoard();
}

char glyph(uint8_t value) {
  if (value == 1) return 'X';
  if (value == 2) return 'O';
  if (value == 3) return '#';
  return '.';
}

void sendState() {
  Serial.print(F("S "));
  Serial.print(stage); Serial.write(' ');
  Serial.write(phase); Serial.write(' ');
  Serial.print(forced); Serial.write(' ');
  Serial.print(hp); Serial.write(' ');
  Serial.print(maxHp); Serial.write(' ');
  Serial.print(unlocked); Serial.write(' ');
  Serial.print(roundNum); Serial.write(' ');
  for (uint8_t i = 0; i < 81; ++i) Serial.write(glyph(cells[i]));
  Serial.write(' ');
  for (uint8_t i = 0; i < 9; ++i) Serial.write(glyph(mini[i]));
  Serial.println();
}

void finishRound(uint8_t winner) {
  if (winner == 1) {
    if (hp > 1) {
      --hp;
      ++roundNum;
      resetBoard();
      Serial.println(F("M DIRECT HIT! Boss lost one shield. Next round!"));
    } else {
      hp = 0;
      phase = 'W';
      if (stage < N_LEVELS && unlocked < stage + 1) {
        unlocked = stage + 1;
        EEPROM.update(1, unlocked); // only written on new unlock
      }
      if (stage == 5) Serial.println(F("M FINAL CORE DESTROYED. Campaign complete!"));
      else Serial.println(F("M STAGE CLEARED! Press N to continue."));
    }
  } else if (winner == 2) {
    phase = 'L';
    Serial.println(F("M SYSTEM OVERRUN. Press R to retry."));
  } else {
    phase = 'D';
    Serial.println(F("M DEADLOCK. Press R to retry."));
  }
}

void doMove(uint8_t index, uint8_t player) {
  cells[index] = player;
  if (ultimate()) {
    uint8_t b = boardOf(index);
    uint8_t w = lineWinnerInMini(b);
    if (w) mini[b] = w;
    else if (miniFull(b)) mini[b] = 3;
    forced = nextForced(index);
    w = macroWinner();
    if (w) finishRound(w);
    else if (allMiniClosed()) finishRound(3);
  } else {
    uint8_t w = lineWinnerInMini(0);
    if (w) finishRound(w);
    else if (miniFull(0)) finishRound(3);
  }
}

// On classic 3x3, the bot isn't unbeatable: later levels are tougher,
// but deliberately leave openings so campaign progression is possible.
uint8_t pickClassic() {
  uint8_t available[9], n = 0;
  for (uint8_t i = 0; i < 9; ++i) if (cells[i] == 0) available[n++] = i;
  if (!n) return 255;
  if (stage == 1) return available[random(n)];
  for (uint8_t k = 0; k < n; ++k) if (wouldWinMini(available[k], 2)) return available[k];
  if (random(100) < 73) {
    for (uint8_t k = 0; k < n; ++k) if (wouldWinMini(available[k], 1)) return available[k];
  }
  if (cells[4] == 0 && random(100) < 60) return 4;
  return available[random(n)];
}

int16_t evaluateUltimate(uint8_t index) {
  uint8_t b = boardOf(index);
  uint8_t p = placeOf(index);
  bool winO = wouldWinMini(index, 2);
  bool winX = wouldWinMini(index, 1);
  int16_t score = random(0, stage == 3 ? 90 : 22);
  if (winO) score += wouldWinMacro(b, 2) ? 1800 : 280;
  if (winX) score += wouldWinMacro(b, 1) ? 1100 : 180;
  if (b == 4) score += 24;
  if (b == 0 || b == 2 || b == 6 || b == 8) score += 8;
  if (p == 4) score += 18;
  if (p == 0 || p == 2 || p == 6 || p == 8) score += 7;

  if (stage >= 4) {
    // Avoid sending the human into a board where they can instantly win.
    // Count threats with a tiny 9-cell scan (no heap, no recursion).
    uint8_t target = p;
    if (target == b && winO) {
      // This move itself closes target, so human gets a free choice.
      score -= 55;
    } else if (mini[target] != 0) {
      score -= 55;
    } else {
      for (uint8_t k = 0; k < 9; ++k) {
        uint8_t r = (target / 3) * 3 + k / 3;
        uint8_t c = (target % 3) * 3 + k % 3;
        if (wouldWinMini(r * 9 + c, 1)) score -= 65;
      }
    }
  }
  return score;
}

uint8_t pickUltimate() {
  // Reservoir tie-breaking avoids a 81-byte array of candidate moves.
  int16_t bestScore = -32767;
  uint8_t best = 255;
  for (uint8_t i = 0; i < 81; ++i) {
    if (!legal(i)) continue;
    int16_t score = evaluateUltimate(i);
    if (score > bestScore) {
      best = i;
      bestScore = score;
    }
  }
  return best;
}

void playHuman(uint8_t index) {
  if (phase != 'P') {
    Serial.println(F("E Round is over. Press R or N."));
    return;
  }
  if (!legal(index)) {
    Serial.println(F("E Illegal move: occupied, closed or wrong mini-board."));
    return;
  }
  uint8_t previousRound = roundNum;
  doMove(index, 1);
  if (phase == 'P' && roundNum == previousRound) {
    uint8_t botIndex = ultimate() ? pickUltimate() : pickClassic();
    if (botIndex != 255) doMove(botIndex, 2);
  }
  sendState();
}

void command(char *cmd) {
  if (strcmp(cmd, "SYNC") == 0) {
    Serial.println(F("HELLO OXO1"));
    sendState();
  } else if (strcmp(cmd, "PING") == 0) {
    Serial.println(F("PONG"));
  } else if (strcmp(cmd, "RETRY") == 0) {
    startStage(stage);
    Serial.println(F("M Stage restarted."));
    sendState();
  } else if (strcmp(cmd, "NEXT") == 0) {
    if (phase == 'W' && stage < N_LEVELS && unlocked >= stage + 1) {
      startStage(stage + 1);
      Serial.println(F("M Entering the next realm."));
      sendState();
    } else Serial.println(F("E Win the current stage first."));
  } else if (strncmp(cmd, "LEVEL ", 6) == 0) {
    int n = atoi(cmd + 6);
    if (n >= 1 && n <= N_LEVELS && n <= unlocked) {
      startStage((uint8_t)n);
      Serial.println(F("M Realm selected."));
      sendState();
    } else Serial.println(F("E Level locked."));
  } else if (strncmp(cmd, "MOVE ", 5) == 0) {
    char *end;
    long n = strtol(cmd + 5, &end, 10);
    if (*end == '\0' && n >= 0 && n < 81) playHuman((uint8_t)n);
    else Serial.println(F("E Invalid cell index."));
  } else {
    Serial.println(F("E Unknown command."));
  }
}

void setup() {
  Serial.begin(115200);
  randomSeed((unsigned long)analogRead(A0) ^ micros());
  if (EEPROM.read(0) != EEPROM_MAGIC || EEPROM.read(1) < 1 || EEPROM.read(1) > N_LEVELS) {
    EEPROM.update(0, EEPROM_MAGIC);
    EEPROM.update(1, 1);
  }
  unlocked = EEPROM.read(1);
  startStage(unlocked);
  Serial.println(F("HELLO OXO1"));
  sendState();
}

void loop() {
  while (Serial.available() > 0) {
    char c = (char)Serial.read();
    if (c == '\r') continue;
    if (c == '\n') {
      input[inputLen] = '\0';
      if (inputLen) command(input);
      inputLen = 0;
    } else if (inputLen < sizeof(input) - 1) {
      input[inputLen++] = c;
    } else {
      inputLen = 0;
      Serial.println(F("E Command too long."));
    }
  }
}
