# NANO // OXO: REALMS

A microcontroller-first tic-tac-toe RPG in your terminal. **The entire game, bot, rules, level progression, and boss HP execute on the Arduino Nano** (ATmega328P). A small **Rust TUI** only draws the console viewport and sends input via USB Serial. No display, buttons, or other electronic components needed.

Inspired by [oxo-w3r.vercel.app](https://oxo-w3r.vercel.app/). Ultimate 9-board rules are based on the common variant; this is an independent recreation, not a port of the website's unpublished source.

## Campaign

| Level | Encounter | Rules | Victory condition |
|---|---|---|---|
| 1 | FIRST CONTACT | Classic 3×3, random bot | Win 1 round |
| 2 | THE GATEKEEPER | Classic 3×3, tactical boss | Win 2 rounds (2 shields) |
| 3 | NINE REALMS | Ultimate 9×9, beginner bot | Control 3 mini-boards in a row |
| 4 | THE ARCHON | Ultimate 9×9, harder boss | Win 2 rounds (2 shields) |
| 5 | FINAL KERNEL | Ultimate 9×9, harder final boss | Win 3 rounds (3 shields) |

Levels 2, 4, and 5 have *real multi-round HP*. If you win a round, the boss loses one shield and a fresh board starts. If you lose or draw, restart the encounter with **R**. Unlocked levels persist in **Arduino EEPROM**, including when USB power is disconnected. Active board positions do **not** persist across power cycles.

The bot is lightweight heuristic AI, *not* a deep-search or unbeatable opponent. This is intentional: Nano has 2 KB RAM and the campaign must be possible to beat.

## Hardware

- Arduino Nano compatible with classic **ATmega328P (5 V)**, plus a **data-capable USB cable**.
- Computer running Windows, Linux, or macOS, with [Arduino IDE](https://www.arduino.cc/en/software/) and the [Rust toolchain](https://rustup.rs/).
- USB-C Nano clones generally work as long as they have the ATmega328P and a working serial port/driver.
- No breadboard, resistor, display, or external wiring is needed.

Other Arduino Nano variants (Nano Every, ESP32-based Nano, etc.) may require changes; this project specifically targets the classic AVR Nano.

## Step 1 — Flash the Arduino

1. Open `firmware/NanoOXO/NanoOXO.ino` in Arduino IDE.
2. Select **Tools → Board → Arduino AVR Boards → Arduino Nano**.
3. Select **Tools → Processor → ATmega328P**. If flashing fails on an older clone, retry **ATmega328P (Old Bootloader)**.
4. Select the Arduino's **Tools → Port** and click **Upload (→)**.
5. **Close the Arduino IDE Serial Monitor / Plotter** before starting the Rust TUI, or the COM/tty port will be busy.

The Serial speed is **115200** baud; the terminal frontend sets this automatically. If you manually open Serial Monitor, set 115200 to see `HELLO OXO1` and game state messages (ASCII protocol).

## Step 2 — Install and run the Rust viewport

From the project folder:

```bash
cd tui
cargo run --release -- --list
```

This lists serial ports. Choose the port corresponding to your Nano. On Windows, for example:

```powershell
cargo run --release -- --port COM5
```

Linux:

```bash
cargo run --release -- --port /dev/ttyUSB0
```

macOS:

```bash
cargo run --release -- --port /dev/cu.usbserial-XXXX
```

If **only one** serial port is available, `cargo run --release` selects it automatically. If more than one is found, supply `--port` explicitly.

Recommended terminal: Windows Terminal, modern PowerShell, iTerm2 or a modern Linux terminal, ~**85 columns × 25 rows** or larger, with UTF-8 support. A wide/tall terminal shows the full campaign HUD. Rust downloads its two dependencies from crates.io at first build.

On Linux you may need permission to use `/dev/ttyUSB0` or `/dev/ttyACM0` (often via membership in the `dialout` group) and to log out/in after adding your user to that group.

Note: opening a classic Nano serial port commonly resets the board due to DTR. The frontend retries synchronization while the bootloader starts, so it is normal for the first screen to display "Waiting for Arduino" briefly.

## Controls

| Key | Action |
|---|---|
| Arrow keys | Move the selected square |
| Enter / Space | Place X in the selected legal square |
| `1`…`9` | Place X immediately in that square (classic 3×3 only) |
| `R` | Restart current level/encounter (reset boss shields) |
| `N` | Advance to next level after winning |
| `F1`…`F5` | Jump to any previously unlocked level |
| `S` | Resend handshake and synchronize state |
| `Q` / Escape / Ctrl+C | Quit the TUI |

**Colors:** cyan = human X, red = enemy O, green dot = allowed empty cell, `#` = drawn mini-board. In Ultimate mode, the right panel indicates which mini-board is active; selecting a cell in another mini-board cannot make a move until the rule allows it.

## Ultimate Tic-Tac-Toe rules (level 3+)

- The big grid has 9 little 3×3 boards, total **81 squares**.
- Human X opens anywhere.
- The *local position* you play in determines the *little board* the opponent must use. For example, the **center** square of any little board sends your opponent to the **center** little board.
- When the targeted mini-board is already won or tied, the next player can play in **any unfinished mini-board**.
- Three X or O in a row captures a mini-board; closed boards accept no more moves.
- Three captured mini-boards in a row wins the round. A full mini-board with no winner becomes `#` and belongs to neither player.
- When all nine mini-boards are closed and the big board has no 3-in-a-row, the round is a draw.

## Architecture

```text
               USB Serial @ 115200
  ┌────────────────────┐  MOVE 40 / RETRY / ...  ┌────────────────────┐
  │ Rust TUI           │ ──────────────────────→ │ Arduino Nano       │
  │ terminal viewport  │ ←────────────────────── │ AVR ATmega328P     │
  │ arrows + ANSI/UTF8 │  S ... 81 cells + ...   │ rules, AI, EEPROM  │
  └────────────────────┘                         └────────────────────┘
```

The firmware sends a compact line protocol:

```text
HELLO OXO1
S 3 P 9 1 1 3 1 ................................................................................. .........
```

`S <level> <phase> <forced> <hp> <maxHp> <unlocked> <round> <81 glyphs> <9 mini-board glyphs>`.

- `phase`: P=playing, W=won, L=lost, D=draw
- `forced`: 0–8 (required mini-board), 9=any open mini-board
- `glyphs`: `.`=empty, X, O, and `#` for a tied mini-board
- PC commands: `SYNC`, `MOVE <index>`, `RETRY`, `NEXT`, `LEVEL <1..5>`, `PING`

The Rust frontend **does not implement game rules or AI**. It only does a client-side legality check for keyboard feedback; the firmware is authoritative.

## Troubleshooting

- **No port:** ensure it's a USB *data cable*; for CH340 clones install the correct CH340 driver if needed.
- **Upload fails:** try Nano **Old Bootloader** in Arduino IDE.
- **"Port busy" / access denied:** close Serial Monitor/Plotter and any other serial terminal. On Linux, check device permissions.
- **Blank waiting screen:** confirm flashed firmware, correct port, board/processor selection, and 115200 baud; press **S** to resync.
- **Garbled characters:** use a UTF-8 terminal and a mono-spaced font; USB serial settings are 115200.
- **No sound:** there is no audio feature; this game is intentionally a serial/TUI project requiring no external components.

## Developer tests

A small simulated Arduino environment tests the firmware's actual C++ game functions on a computer (requires `g++`):

```bash
g++ -std=c++17 -Wall -Wextra -Werror -Itests tests/simulate.cpp -o /tmp/nano_oxo_sim
/tmp/nano_oxo_sim
```

Rust protocol tests, with Rust and the crates available:

```bash
cd tui
cargo test
```

**Testing status:** firmware's native C++ simulation tests pass (including 125 randomized full games). The AVR toolchain and Rust toolchain were unavailable in the artifact-creation environment, so the `.ino` has not been uploaded to physical hardware and the Rust executable was not compiled there. See above for the steps to build/flash on your computer.
