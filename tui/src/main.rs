//! NANO // OXO: REALMS — Rust TUI viewport.
//! All game rules and AI execute on the Arduino; this app sends keystrokes
//! and renders state frames received through the serial port.
use std::{
    env,
    error::Error,
    io::{self, Read, Write},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

const C_RESET: &str = "\x1b[0m";
const C_CYAN: &str = "\x1b[1;96m";
const C_PINK: &str = "\x1b[1;91m";
const C_GOLD: &str = "\x1b[1;93m";
const C_GREEN: &str = "\x1b[1;92m";
const C_DIM: &str = "\x1b[2;37m";
const C_BLUE: &str = "\x1b[1;94m";

#[derive(Clone)]
struct GameState {
    level: u8,
    phase: char,
    forced: u8,
    hp: u8,
    max_hp: u8,
    unlocked: u8,
    round: u8,
    board: [u8; 81],
    mini: [u8; 9],
}

impl GameState {
    fn parse(line: &str) -> Option<Self> {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() != 10 || p[0] != "S" { return None; }
        let board = p[8].as_bytes();
        let mini = p[9].as_bytes();
        if board.len() != 81 || mini.len() != 9 ||
           !board.iter().all(|c| matches!(*c, b'.' | b'X' | b'O')) ||
           !mini.iter().all(|c| matches!(*c, b'.' | b'X' | b'O' | b'#')) {
            return None;
        }
        let level = p[1].parse::<u8>().ok()?;
        let phase = p[2].chars().next()?;
        let forced = p[3].parse::<u8>().ok()?;
        let hp = p[4].parse::<u8>().ok()?;
        let max_hp = p[5].parse::<u8>().ok()?;
        let unlocked = p[6].parse::<u8>().ok()?;
        let round = p[7].parse::<u8>().ok()?;
        if !(1..=5).contains(&level) || !matches!(phase, 'P'|'W'|'L'|'D') ||
            forced > 9 || max_hp > 3 || hp > max_hp || !(1..=5).contains(&unlocked) {
            return None;
        }
        let mut b = [b'.'; 81];
        let mut m = [b'.'; 9];
        b.copy_from_slice(board);
        m.copy_from_slice(mini);
        Some(Self { level, phase, forced, hp, max_hp, unlocked, round, board: b, mini: m })
    }
    fn ultimate(&self) -> bool { self.level >= 3 }
    fn width(&self) -> usize { if self.ultimate() { 9 } else { 3 } }
    fn legal(&self, idx: usize) -> bool {
        if self.phase != 'P' { return false; }
        if !self.ultimate() { return idx < 9 && self.board[idx] == b'.'; }
        if idx >= 81 || self.board[idx] != b'.' { return false; }
        let row = idx / 9;
        let col = idx % 9;
        let sub = (row / 3) * 3 + (col / 3);
        self.mini[sub] == b'.' && (self.forced == 9 || self.forced as usize == sub)
    }
    fn suggested_cursor(&self, old: usize) -> usize {
        if self.legal(old) { return old; }
        let n = self.width() * self.width();
        (0..n).find(|&i| self.legal(i)).unwrap_or(0)
    }
}

fn stage_name(lvl: u8) -> &'static str {
    match lvl {
        1 => "01 // FIRST CONTACT",
        2 => "02 // THE GATEKEEPER",
        3 => "03 // NINE REALMS",
        4 => "04 // THE ARCHON",
        _ => "05 // FINAL KERNEL",
    }
}
fn opponent_name(lvl: u8) -> &'static str {
    match lvl {
        1 => "Training Drone",
        2 => "GATEKEEPER [BOSS]",
        3 => "Realm Wanderer",
        4 => "ARCHON [BOSS]",
        _ => "KERNEL [FINAL BOSS]",
    }
}
fn colour(c: u8) -> &'static str {
    match c { b'X' => C_CYAN, b'O' => C_PINK, b'#' => C_GOLD, _ => C_DIM }
}
fn colored_glyph(c: u8, selected: bool, available: bool) -> String {
    let glyph = if c == b'.' { '·' } else { c as char };
    if selected {
        format!("\x1b[7;1m {} {C_RESET}", glyph)
    } else if available && c == b'.' {
        format!("{C_GREEN} {} {C_RESET}", glyph)
    } else {
        format!("{} {} {C_RESET}", colour(c), glyph)
    }
}

fn draw_classic(s: &GameState, cursor: usize) -> Vec<String> {
    let mut out = vec![format!("{C_BLUE}╔═══════════╦═══════════╦═══════════╗{C_RESET}")];
    for row in 0..3 {
        let mut line = format!("{C_BLUE}║{C_RESET}");
        for col in 0..3 {
            let idx = row * 3 + col;
            let glyph = colored_glyph(s.board[idx], idx == cursor, s.legal(idx));
            line.push_str(&format!("    {}    {C_BLUE}║{C_RESET}", glyph));
        }
        out.push(line);
        if row != 2 { out.push(format!("{C_BLUE}╠═══════════╬═══════════╬═══════════╣{C_RESET}")); }
    }
    out.push(format!("{C_BLUE}╚═══════════╩═══════════╩═══════════╝{C_RESET}"));
    out
}

fn draw_ultimate(s: &GameState, cursor: usize) -> Vec<String> {
    let mut out = vec![format!("{C_BLUE}╔═══════════╦═══════════╦═══════════╗{C_RESET}")];
    for row in 0..9 {
        let mut line = format!("{C_BLUE}║{C_RESET}");
        for subcol in 0..3 {
            line.push(' ');
            for local in 0..3 {
                let col = subcol * 3 + local;
                let idx = row * 9 + col;
                line.push_str(&colored_glyph(s.board[idx], idx == cursor, s.legal(idx)));
            }
            line.push_str(&format!(" {C_BLUE}║{C_RESET}"));
        }
        out.push(line);
        if row == 2 || row == 5 {
            out.push(format!("{C_BLUE}╠═══════════╬═══════════╬═══════════╣{C_RESET}"));
        }
    }
    out.push(format!("{C_BLUE}╚═══════════╩═══════════╩═══════════╝{C_RESET}"));
    out
}

fn hud(s: &GameState, cursor: usize, message: &str, compact: bool) -> Vec<String> {
    if compact {
        let mut v = vec![
            format!("{C_GOLD}╭─ CONTROL DECK ────────────────╮{C_RESET}"),
            format!("  {C_CYAN}X{C_RESET} YOU  vs  {C_PINK}O{C_RESET} {}", opponent_name(s.level)),
        ];
        if s.max_hp > 1 {
            v.push(format!("  Shields: {} / {}   Round: {}", s.hp, s.max_hp, s.round));
        }
        if s.ultimate() {
            v.push(format!("  Target: {}", if s.forced == 9 { "ANY".to_owned() } else { format!("MINI {}", s.forced+1) }));
            v.push("  Captured: (1-9 = open)".into());
            for r in 0..3 {
                let mut line = String::from("    ");
                for c in 0..3 {
                    let value = s.mini[r*3+c];
                    let glyph = if value == b'.' { (b'1' + (r*3+c) as u8) as char } else { value as char };
                    line.push_str(&format!("{} {glyph} {C_RESET}", colour(value)));
                }
                v.push(line);
            }
        }
        let label = match s.phase { 'P' => "YOUR TURN", 'W' => "STAGE CLEARED! [N]", 'L' => "DEFEAT [R]", _ => "DRAW [R]" };
        v.push(format!("  {C_GREEN}{label}{C_RESET}"));
        v.push(format!("  Level {}/5, unlocked 1..{}", s.level, s.unlocked));
        v.push(format!("{C_GOLD}╰────────────────────────────────╯{C_RESET}"));
        if !message.is_empty() {
            v.push(format!("{C_GOLD}EVENT:{C_RESET}"));
            for chunk in message.as_bytes().chunks(32) {
                v.push(format!("  {}", String::from_utf8_lossy(chunk)));
            }
        }
        return v;
    }
    let mut lines = vec![
        format!("{C_GOLD}╭─ MISSION STATUS ──────────────╮{C_RESET}"),
        format!("  YOU  {C_CYAN}X{C_RESET}  vs  {C_PINK}O{C_RESET}"),
        format!("  {}", opponent_name(s.level)),
        "".to_owned(),
    ];
    if s.max_hp > 1 {
        let hearts = format!("{}{}", "♥ ".repeat(s.hp as usize), "♡ ".repeat((s.max_hp-s.hp) as usize));
        lines.push(format!("  BOSS SHIELDS {C_PINK}{hearts}{C_RESET}"));
        lines.push(format!("  Encounter round: {}", s.round));
    } else {
        lines.push("  ENCOUNTER: single round".to_owned());
    }
    lines.push("".to_owned());
    lines.push(format!("{C_GOLD}  FIELD PROTOCOL{C_RESET}"));
    if s.ultimate() {
        let target = if s.forced == 9 { "ANY OPEN BOARD".to_string() }
            else { format!("MINI {}", s.forced + 1) };
        lines.push(format!("  Target: {C_GREEN}{target}{C_RESET}"));
        lines.push(format!("  Cursor: row {} col {}", cursor/9 + 1, cursor%9 + 1));
        lines.push("".to_owned());
        lines.push("  Captured mini-boards:".to_owned());
        for r in 0..3 {
            let mut txt = String::from("     ");
            for c in 0..3 {
                let glyph = s.mini[r*3+c];
                let glyph = if glyph == b'.' { (b'1' + (r*3+c) as u8) as char } else { glyph as char };
                txt.push_str(&format!("{} {} {C_RESET}", colour(s.mini[r*3+c]), glyph));
            }
            lines.push(txt);
        }
    } else {
        lines.push("  CLASSIC 3x3".to_owned());
        lines.push(format!("  Selected: square {}", cursor+1));
    }
    lines.push("".to_owned());
    let status = match s.phase {
        'P' => format!("{C_GREEN}YOUR TURN // ENGAGE{C_RESET}"),
        'W' => format!("{C_GREEN}STAGE CLEARED!{C_RESET}"),
        'L' => format!("{C_PINK}DEFEAT // RETRY{C_RESET}"),
        _ => format!("{C_GOLD}DRAW // RETRY{C_RESET}"),
    };
    lines.push(format!("  {status}"));
    lines.push("".to_owned());
    lines.push(format!("{C_GOLD}  UNLOCKED REALMS{C_RESET}"));
    for lvl in 1..=5u8 {
        let mark = if lvl == s.level { "▶" } else if lvl <= s.unlocked { "◆" } else { "×" };
        let text = match lvl { 1=>"TRAINING",2=>"GATEKEEPER",3=>"ULTIMATE",4=>"ARCHON",_=>"KERNEL" };
        let clr = if lvl == s.level { C_GREEN } else if lvl <= s.unlocked { C_CYAN } else { C_DIM };
        lines.push(format!("  {clr}{mark} F{lvl}: {text}{C_RESET}"));
    }
    lines.push(format!("{C_GOLD}╰───────────────────────────────╯{C_RESET}"));
    if !message.is_empty() {
        lines.push("".to_owned());
        lines.push(format!("{C_GOLD}EVENT:{C_RESET}"));
        // Keep lines short for a narrow console.
        for chunk in message.as_bytes().chunks(32) {
            lines.push(format!("  {}", String::from_utf8_lossy(chunk)));
        }
    }
    lines
}

fn visible_width(line: &str) -> usize {
    let mut count = 0;
    let mut escape = false;
    for ch in line.chars() {
        if ch == '\x1b' { escape = true; }
        else if escape { if ch == 'm' { escape = false; } }
        else { count += 1; }
    }
    count
}
fn render(s: Option<&GameState>, cursor: usize, message: &str, (width, height): (u16,u16)) -> String {
    let mut out = String::new();
    let compact = width < 90 || height < 34;
    if compact {
        if width < 70 {
            out.push_str(&format!("{C_CYAN}  NANO // OXO: REALMS{C_RESET}\r\n"));
        } else {
            out.push_str(&format!("{C_CYAN}  NANO // OXO: REALMS  {C_DIM}━━━━━━━━━━━━━━━━━  Arduino-powered TUI{C_RESET}\r\n"));
        }
    } else {
        out.push_str(&format!("{C_CYAN}  ███╗   ██╗ █████╗ ███╗   ██╗ ██████╗      ██████╗ ██╗  ██╗ ██████╗{C_RESET}\r\n"));
        out.push_str(&format!("{C_CYAN}  ████╗  ██║██╔══██╗████╗  ██║██╔═══██╗    ██╔═══██╗╚██╗██╔╝██╔═══██╗{C_RESET}\r\n"));
        out.push_str(&format!("{C_CYAN}  ██╔██╗ ██║███████║██╔██╗ ██║██║   ██║    ██║   ██║ ╚███╔╝ ██║   ██║{C_RESET}\r\n"));
        out.push_str(&format!("{C_CYAN}  ██║╚██╗██║██╔══██║██║╚██╗██║██║   ██║    ██║   ██║ ██╔██╗ ██║   ██║{C_RESET}\r\n"));
        out.push_str(&format!("{C_CYAN}  ██║ ╚████║██║  ██║██║ ╚████║╚██████╔╝    ╚██████╔╝██╔╝ ██╗╚██████╔╝{C_RESET}\r\n"));
        out.push_str(&format!("{C_DIM}  ───────────────────────────────  R E A L M S  ───────────────────────{C_RESET}\r\n\r\n"));
    }
    if width < 43 || height < 15 {
        out.push_str("  Please enlarge the terminal to at least 43x15.\r\n");
        return out;
    }
    match s {
        None => {
            out.push_str(&format!("  {C_GOLD}Waiting for Arduino Nano...{C_RESET}\r\n"));
            out.push_str("  Check USB cable and correct serial port.\r\n");
            out.push_str(&format!("  {C_DIM}Press Q to exit. Port speed: 115200 baud.{C_RESET}\r\n"));
        }
        Some(s) => {
            out.push_str(&format!("  {C_GOLD}{}{C_RESET}   {}\r\n\r\n", stage_name(s.level),
                if s.ultimate() { "[81 CELLS / ULTIMATE]" } else { "[9 CELLS / CLASSIC]" }));
            let board = if s.ultimate() { draw_ultimate(s, cursor) } else { draw_classic(s, cursor) };
            let info = hud(s, cursor, message, compact);
            if width >= 78 {
                let pad = board.iter().map(|line| visible_width(line)).max().unwrap_or(0);
                for i in 0..board.len().max(info.len()) {
                    let left = board.get(i).map(String::as_str).unwrap_or("");
                    let right = info.get(i).map(String::as_str).unwrap_or("");
                    let space = pad.saturating_sub(visible_width(left)) + 3;
                    out.push_str(&format!("  {left}{}{right}\r\n", " ".repeat(space)));
                }
            } else {
                for line in board { out.push_str(&format!("  {line}\r\n")); }
                for line in info { out.push_str(&format!("  {line}\r\n")); }
            }
            out.push_str("\r\n");
            out.push_str(&format!("  {C_GREEN}[ARROWS]{C_RESET} move  {C_GREEN}[ENTER/SPACE]{C_RESET} mark  {C_GREEN}[R]{C_RESET} retry  {C_GREEN}[N]{C_RESET} next\r\n"));
            out.push_str(&format!("  {C_DIM}[F1-F5] unlocked level  [Q/ESC] quit  [S] sync"));
            if !s.ultimate() { out.push_str("  [1-9] mark"); }
            out.push_str(&format!("{C_RESET}\r\n"));
        }
    }
    out
}

struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(e);
        }
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, Print(C_RESET));
        let _ = terminal::disable_raw_mode();
    }
}

fn port_help() {
    println!("NANO // OXO: REALMS | Rust serial terminal adapter\n\nUsage:\n  nano-oxo --list\n  nano-oxo --port COM5            (Windows)\n  nano-oxo --port /dev/ttyUSB0    (Linux)\n  nano-oxo --port /dev/cu.usbserial-XXXX (macOS)\n\nBaud: 115200. Flash firmware/NanoOXO/NanoOXO.ino first.");
}
fn list_ports() -> Result<Vec<String>, Box<dyn Error>> {
    let ports = serialport::available_ports()?;
    let mut names: Vec<String> = ports.into_iter().map(|p| p.port_name).collect();
    names.sort();
    Ok(names)
}
fn parse_args() -> Result<Option<String>, Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") { port_help(); return Ok(None); }
    if args.len() == 1 && args[0] == "--list" {
        println!("Serial ports:");
        for port in list_ports()? { println!("  {port}"); }
        return Ok(None);
    }
    if args.len() == 2 && args[0] == "--port" { return Ok(Some(args[1].clone())); }
    if args.is_empty() {
        let names = list_ports()?;
        if names.len() == 1 { return Ok(Some(names[0].clone())); }
        eprintln!("Found {} serial ports. Specify the Arduino's port with --port:", names.len());
        for name in names { eprintln!("  {name}"); }
        port_help();
        return Ok(None);
    }
    port_help();
    Ok(None)
}
fn send(port: &mut dyn serialport::SerialPort, cmd: &str) -> io::Result<()> {
    port.write_all(cmd.as_bytes())?;
    port.write_all(b"\n")?;
    Ok(())
}

fn run(port_name: String) -> Result<(), Box<dyn Error>> {
    let mut port = serialport::new(port_name.as_str(), 115_200)
        .timeout(Duration::from_millis(12))
        .open()?;
    let _guard = TerminalGuard::enter()?;
    let mut state: Option<GameState> = None;
    let mut cursor = 0_usize;
    let mut message = String::from("Handshake in progress...");
    let mut rx: Vec<u8> = Vec::with_capacity(200);
    let mut buf = [0_u8; 256];
    let mut last_sync = Instant::now() - Duration::from_secs(4);
    let mut dirty = true;
    let mut pending_move = false;
    let mut last_render = Instant::now() - Duration::from_secs(4);
    loop {
        if state.is_none() && last_sync.elapsed() >= Duration::from_millis(800) {
            send(&mut *port, "SYNC")?;
            last_sync = Instant::now();
        }
        match port.read(&mut buf) {
            Ok(n) => {
                for &byte in &buf[..n] {
                    if byte == b'\n' {
                        let line = String::from_utf8_lossy(&rx).trim().to_owned();
                        rx.clear();
                        if let Some(parsed) = GameState::parse(&line) {
                            cursor = parsed.suggested_cursor(cursor);
                            state = Some(parsed);
                            pending_move = false;
                            dirty = true;
                        } else if let Some(msg) = line.strip_prefix("M ") {
                            message = msg.to_string(); dirty = true;
                        } else if let Some(err) = line.strip_prefix("E ") {
                            message = format!("ERROR: {err}"); pending_move = false; dirty = true;
                        } else if line == "HELLO OXO1" {
                            if state.is_none() { message = "Arduino online. Synchronizing...".into(); dirty = true; }
                        }
                    } else if byte != b'\r' && rx.len() < 256 {
                        rx.push(byte);
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => return Err(format!("Lost serial connection to {port_name}: {e}").into()),
        }
        if event::poll(Duration::from_millis(14))? {
            match event::read()? {
                Event::Key(k) if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    if matches!(k.code, KeyCode::Esc | KeyCode::Char('q')) ||
                        (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)) {
                        break;
                    }
                    if k.code == KeyCode::Char('s') {
                        send(&mut *port, "SYNC")?;
                    }
                    if let Some(ref s) = state {
                        let side = s.width();
                        let mut next = cursor;
                        match k.code {
                            KeyCode::Up => if cursor / side > 0 { next -= side; },
                            KeyCode::Down => if cursor / side < side-1 { next += side; },
                            KeyCode::Left => if cursor % side > 0 { next -= 1; },
                            KeyCode::Right => if cursor % side < side-1 { next += 1; },
                            KeyCode::Char('r') => { send(&mut *port, "RETRY")?; pending_move = false; },
                            KeyCode::Char('n') => { send(&mut *port, "NEXT")?; pending_move = false; },
                            KeyCode::F(i @ 1..=5) => {
                                send(&mut *port, &format!("LEVEL {i}"))?;
                                pending_move = false;
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if !pending_move && s.legal(cursor) {
                                    send(&mut *port, &format!("MOVE {cursor}"))?;
                                    pending_move = true;
                                } else if !pending_move { message = "Not a legal square. Choose a green dot.".into(); }
                            }
                            KeyCode::Char(n @ '1'..='9') if !s.ultimate() => {
                                let idx = n as usize - '1' as usize;
                                if !pending_move && s.legal(idx) {
                                    send(&mut *port, &format!("MOVE {idx}"))?;
                                    pending_move = true;
                                }
                            }
                            _ => (),
                        }
                        if next != cursor { cursor = next; }
                        dirty = true;
                    }
                }
                Event::Resize(_, _) => dirty = true,
                _ => (),
            }
        }
        if dirty || last_render.elapsed() > Duration::from_secs(3) {
            let output = render(state.as_ref(), cursor, &message, terminal::size()?);
            execute!(io::stdout(), MoveTo(0,0), Clear(ClearType::All), Print(output))?;
            dirty = false;
            last_render = Instant::now();
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    if let Some(port) = parse_args()? {
        if let Err(e) = run(port) { eprintln!("NANO OXO: {e}"); return Err(e); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_state_parses() {
        let msg = format!("S 3 P 9 1 1 3 1 {} {}", ".".repeat(81), ".".repeat(9));
        let state = GameState::parse(&msg).unwrap();
        assert!(state.ultimate());
        assert!(state.legal(80));
        assert!(!state.legal(81));
    }
    #[test]
    fn forced_board_is_enforced() {
        let msg = format!("S 3 P 4 1 1 3 1 {} {}", ".".repeat(81), ".".repeat(9));
        let state = GameState::parse(&msg).unwrap();
        assert!(!state.legal(0));
        assert!(state.legal(30)); // row 3, col 3 => center mini-board
    }
    #[test]
    fn selected_cursor_skips_occupied_cells() {
        let msg = format!("S 1 P 9 1 1 1 1 X{} {}", ".".repeat(80), ".".repeat(9));
        let state = GameState::parse(&msg).unwrap();
        assert_eq!(state.suggested_cursor(0),1);
    }
}
