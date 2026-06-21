use rand::Rng;
use serialport::{ClearBuffer, SerialPort, SerialPortInfo, SerialPortType};
use std::env;
use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

const N_SPECIES: usize = 10;
const UNKNOWN_JAR_INDEX: usize = N_SPECIES;
const DEFAULT_BAUD_RATE: u32 = 115_200;
const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 30.0;
const DEFAULT_DELAY_MIN_SECS: u64 = 5;
const DEFAULT_DELAY_MAX_SECS: u64 = 10;

const HEADER_CMD: u8 = 0xAA;
const HEADER_STATUS: u8 = 0x55;
const PACKET_SIZE: usize = 8;

type AppResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Command {
    MoveTo = 0x01,
    Home = 0x02,
    ServoOn = 0x05,
    GetStatus = 0x08,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Command::MoveTo => "MOVE_TO",
            Command::Home => "HOME",
            Command::ServoOn => "SERVO_ON",
            Command::GetStatus => "GET_STATUS",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Idle,
    Moving,
    InPosition,
    Alarm,
    NotReady,
    Unknown(u8),
}

impl Status {
    fn from_u8(value: u8) -> Self {
        match value {
            0x00 => Status::Idle,
            0x01 => Status::Moving,
            0x02 => Status::InPosition,
            0x03 => Status::Alarm,
            0x04 => Status::NotReady,
            other => Status::Unknown(other),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Idle => f.write_str("IDLE"),
            Status::Moving => f.write_str("MOVING"),
            Status::InPosition => f.write_str("IN_POSITION"),
            Status::Alarm => f.write_str("ALARM"),
            Status::NotReady => f.write_str("NOT_READY"),
            Status::Unknown(value) => write!(f, "UNKNOWN({value:#04X})"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct StatusPacket {
    status: Status,
    jar_index: u8,
    alarm_code: u8,
    seq: u8,
}

impl StatusPacket {
    fn parse(bytes: [u8; PACKET_SIZE]) -> AppResult<Self> {
        if bytes[0] != HEADER_STATUS {
            return Err(format!(
                "invalid response header: expected 0x55, got {:#04X}",
                bytes[0]
            )
            .into());
        }

        let expected_checksum = checksum(&bytes[..PACKET_SIZE - 1]);
        if bytes[PACKET_SIZE - 1] != expected_checksum {
            return Err(format!(
                "invalid checksum: expected {expected_checksum:#04X}, got {:#04X}",
                bytes[PACKET_SIZE - 1]
            )
            .into());
        }

        Ok(Self {
            status: Status::from_u8(bytes[1]),
            jar_index: bytes[2],
            alarm_code: bytes[3],
            seq: bytes[6],
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RGB({}, {}, {})", self.red, self.green, self.blue)
    }
}

#[derive(Clone, Copy, Debug)]
enum MosquitoLedColor {
    SoftRed,  // Aedes aegypti: red warning color.
    Orange,   // Aedes albopictus: orange.
    Yellow,   // Anopheles gambiae: yellow.
    Green,    // Culex pipiens: green.
    Mint,     // Culex quinquefasciatus: mint green.
    SkyBlue,  // Anopheles stephensi: light blue.
    DeepBlue, // Aedes polynesiensis: deep blue.
    Purple,   // Mansonia uniformis: purple.
    Magenta,  // Toxorhynchites sp.: magenta.
    Rose,     // Armigeres subalbatus: rose pink.
}

impl MosquitoLedColor {
    fn rgb(self) -> Rgb {
        match self {
            MosquitoLedColor::SoftRed => Rgb {
                red: 255,
                green: 50,
                blue: 50,
            },
            MosquitoLedColor::Orange => Rgb {
                red: 255,
                green: 100,
                blue: 0,
            },
            MosquitoLedColor::Yellow => Rgb {
                red: 255,
                green: 255,
                blue: 0,
            },
            MosquitoLedColor::Green => Rgb {
                red: 0,
                green: 255,
                blue: 0,
            },
            MosquitoLedColor::Mint => Rgb {
                red: 0,
                green: 255,
                blue: 128,
            },
            MosquitoLedColor::SkyBlue => Rgb {
                red: 0,
                green: 128,
                blue: 255,
            },
            MosquitoLedColor::DeepBlue => Rgb {
                red: 0,
                green: 0,
                blue: 255,
            },
            MosquitoLedColor::Purple => Rgb {
                red: 128,
                green: 0,
                blue: 255,
            },
            MosquitoLedColor::Magenta => Rgb {
                red: 255,
                green: 0,
                blue: 255,
            },
            MosquitoLedColor::Rose => Rgb {
                red: 255,
                green: 0,
                blue: 128,
            },
        }
    }
}

impl fmt::Display for MosquitoLedColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            MosquitoLedColor::SoftRed => "soft red",
            MosquitoLedColor::Orange => "orange",
            MosquitoLedColor::Yellow => "yellow",
            MosquitoLedColor::Green => "green",
            MosquitoLedColor::Mint => "mint",
            MosquitoLedColor::SkyBlue => "sky blue",
            MosquitoLedColor::DeepBlue => "deep blue",
            MosquitoLedColor::Purple => "purple",
            MosquitoLedColor::Magenta => "magenta",
            MosquitoLedColor::Rose => "rose",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum MosquitoSpecies {
    AedesAegypti = 0,
    AedesAlbopictus = 1,
    AnophelesGambiae = 2,
    CulexPipiens = 3,
    CulexQuinquefasciatus = 4,
    AnophelesStephensi = 5,
    AedesPolynesiensis = 6,
    MansoniaUniformis = 7,
    Toxorhynchites = 8,
    ArmigeresSubalbatus = 9,
}

impl MosquitoSpecies {
    const ALL: [Self; N_SPECIES] = [
        Self::AedesAegypti,
        Self::AedesAlbopictus,
        Self::AnophelesGambiae,
        Self::CulexPipiens,
        Self::CulexQuinquefasciatus,
        Self::AnophelesStephensi,
        Self::AedesPolynesiensis,
        Self::MansoniaUniformis,
        Self::Toxorhynchites,
        Self::ArmigeresSubalbatus,
    ];

    fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    fn jar_index(self) -> usize {
        self as usize
    }

    fn label(self) -> &'static str {
        match self {
            Self::AedesAegypti => "Muoi van (Aedes aegypti)",
            Self::AedesAlbopictus => "Muoi ho chau A (Aedes albopictus)",
            Self::AnophelesGambiae => "Muoi Anopheles gambiae",
            Self::CulexPipiens => "Muoi nha (Culex pipiens)",
            Self::CulexQuinquefasciatus => "Muoi Culex quinquefasciatus",
            Self::AnophelesStephensi => "Muoi Anopheles stephensi",
            Self::AedesPolynesiensis => "Muoi Polynesia (Aedes polynesiensis)",
            Self::MansoniaUniformis => "Muoi Mansonia uniformis",
            Self::Toxorhynchites => "Muoi khong lo (Toxorhynchites sp.)",
            Self::ArmigeresSubalbatus => "Muoi Armigeres subalbatus",
        }
    }

    fn led_color(self) -> MosquitoLedColor {
        match self {
            Self::AedesAegypti => MosquitoLedColor::SoftRed,
            Self::AedesAlbopictus => MosquitoLedColor::Orange,
            Self::AnophelesGambiae => MosquitoLedColor::Yellow,
            Self::CulexPipiens => MosquitoLedColor::Green,
            Self::CulexQuinquefasciatus => MosquitoLedColor::Mint,
            Self::AnophelesStephensi => MosquitoLedColor::SkyBlue,
            Self::AedesPolynesiensis => MosquitoLedColor::DeepBlue,
            Self::MansoniaUniformis => MosquitoLedColor::Purple,
            Self::Toxorhynchites => MosquitoLedColor::Magenta,
            Self::ArmigeresSubalbatus => MosquitoLedColor::Rose,
        }
    }
}

#[derive(Debug)]
struct AiDetection {
    species: MosquitoSpecies,
    confidence: f64,
}

#[derive(Debug)]
struct AppConfig {
    port_name: Option<String>,
    baud_rate: u32,
    confidence_threshold: f64,
    frame_limit: Option<u64>,
    delay_min_secs: u64,
    delay_max_secs: u64,
    dry_run: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port_name: None,
            baud_rate: DEFAULT_BAUD_RATE,
            confidence_threshold: DEFAULT_CONFIDENCE_THRESHOLD,
            frame_limit: None,
            delay_min_secs: DEFAULT_DELAY_MIN_SECS,
            delay_max_secs: DEFAULT_DELAY_MAX_SECS,
            dry_run: false,
        }
    }
}

struct Esp32SerialController {
    port: Option<Box<dyn SerialPort>>,
    seq: u8,
}

impl Esp32SerialController {
    fn connect(config: &AppConfig) -> AppResult<Self> {
        if config.dry_run {
            println!("  [SERIAL] Dry-run mode: khong mo cong COM.");
            return Ok(Self { port: None, seq: 0 });
        }

        let port_name = match &config.port_name {
            Some(name) => name.clone(),
            None => auto_detect_esp32_port()?,
        };

        println!(
            "  [SERIAL] Opening {} @ {} baud...",
            port_name, config.baud_rate
        );

        let port = serialport::new(&port_name, config.baud_rate)
            .timeout(Duration::from_secs(3))
            .open()?;

        thread::sleep(Duration::from_secs(2));
        let _ = port.clear(ClearBuffer::All);

        println!("  [SERIAL] Connected to ESP32 on {port_name}.");
        Ok(Self {
            port: Some(port),
            seq: 0,
        })
    }

    fn send_command(&mut self, command: Command, param1: u8) -> AppResult<StatusPacket> {
        self.seq = self.seq.wrapping_add(1);
        let packet = build_command_packet(command, param1, self.seq);

        println!(
            "  [TX] {:<10} p1={:<3} seq={:<3} raw=[{}]",
            command,
            param1,
            self.seq,
            format_bytes(&packet)
        );

        let Some(port) = self.port.as_mut() else {
            let dry_response = StatusPacket {
                status: Status::InPosition,
                jar_index: param1,
                alarm_code: 0,
                seq: self.seq,
            };
            println!(
                "  [RX] dry-run status={} jar={} alarm={} seq={}",
                dry_response.status,
                dry_response.jar_index,
                dry_response.alarm_code,
                dry_response.seq
            );
            return Ok(dry_response);
        };

        port.write_all(&packet)?;
        port.flush()?;

        let mut response = [0u8; PACKET_SIZE];
        port.read_exact(&mut response)?;
        let parsed = StatusPacket::parse(response)?;

        println!(
            "  [RX] status={} jar={} alarm={} seq={} raw=[{}]",
            parsed.status,
            parsed.jar_index,
            parsed.alarm_code,
            parsed.seq,
            format_bytes(&response)
        );

        Ok(parsed)
    }
}

fn fake_ai_detect() -> AiDetection {
    let mut rng = rand::thread_rng();
    let species_index = rng.gen_range(0..N_SPECIES);
    let species = MosquitoSpecies::from_index(species_index)
        .expect("species index must be generated inside MOSQUITO_DB range");
    let confidence = rng.gen_range(1.0..=100.0);

    AiDetection {
        species,
        confidence,
    }
}

fn classify_target(detection: &AiDetection, threshold: f64) -> usize {
    if detection.confidence < threshold {
        UNKNOWN_JAR_INDEX
    } else {
        detection.species.jar_index()
    }
}

fn send_led_color_for_species(
    controller: &mut Esp32SerialController,
    species: MosquitoSpecies,
) -> AppResult<()> {
    let jar_index = species.jar_index();
    let led_color = species.led_color();

    println!(
        "  [LED] {} -> {} ({})",
        species.label(),
        led_color,
        led_color.rgb()
    );

    let response = controller.send_command(Command::MoveTo, jar_index as u8)?;
    if response.status != Status::InPosition {
        println!(
            "  [WARN] ESP32 did not confirm IN_POSITION; current status={}",
            response.status
        );
    }

    Ok(())
}

fn build_command_packet(command: Command, param1: u8, seq: u8) -> [u8; PACKET_SIZE] {
    let mut packet = [
        HEADER_CMD,
        command as u8,
        param1,
        0x00,
        0x00,
        0x00,
        seq,
        0x00,
    ];
    packet[PACKET_SIZE - 1] = checksum(&packet[..PACKET_SIZE - 1]);
    packet
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |acc, byte| acc ^ byte)
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn auto_detect_esp32_port() -> AppResult<String> {
    let ports = serialport::available_ports()?;
    if ports.is_empty() {
        return Err("khong tim thay cong COM nao; hay cam ESP32 va thu --port COMx".into());
    }

    println!("  [SERIAL] Available ports:");
    for port in &ports {
        println!("    - {}", describe_port(port));
    }

    if let Some(port) = ports.iter().find(|port| looks_like_esp32(port)) {
        println!(
            "  [SERIAL] Auto-detected ESP32 candidate: {}",
            port.port_name
        );
        return Ok(port.port_name.clone());
    }

    if ports.len() == 1 {
        println!(
            "  [SERIAL] Only one port found; using {}.",
            ports[0].port_name
        );
        return Ok(ports[0].port_name.clone());
    }

    Err("khong auto-detect duoc ESP32; hay chay voi --port COMx".into())
}

fn looks_like_esp32(port: &SerialPortInfo) -> bool {
    let SerialPortType::UsbPort(usb) = &port.port_type else {
        return false;
    };

    let text = format!(
        "{} {} {}",
        usb.manufacturer.as_deref().unwrap_or_default(),
        usb.product.as_deref().unwrap_or_default(),
        usb.serial_number.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();

    text.contains("ch340")
        || text.contains("ch910")
        || text.contains("cp210")
        || text.contains("silicon labs")
        || text.contains("wch")
        || text.contains("usb serial")
        || text.contains("esp32")
}

fn describe_port(port: &SerialPortInfo) -> String {
    match &port.port_type {
        SerialPortType::UsbPort(usb) => format!(
            "{} | USB vid={:04X} pid={:04X} manufacturer={} product={}",
            port.port_name,
            usb.vid,
            usb.pid,
            usb.manufacturer.as_deref().unwrap_or("unknown"),
            usb.product.as_deref().unwrap_or("unknown")
        ),
        other => format!("{} | {:?}", port.port_name, other),
    }
}

fn print_banner(config: &AppConfig) {
    println!("============================================================");
    println!("  MOSQUITO SORTING DEMO - Rust desktop controller");
    println!("  Windows Rust -> USB Serial -> ESP32-S3 RGB LED GPIO45");
    println!("============================================================");
    println!(
        "  threshold={:.1}% baud={} dry_run={} frames={}",
        config.confidence_threshold,
        config.baud_rate,
        config.dry_run,
        config
            .frame_limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "continuous".to_string())
    );
    println!("============================================================");
}

fn print_color_mapping() {
    println!("\n[COLOR MAP]");
    for species in MosquitoSpecies::ALL {
        let color = species.led_color();
        println!(
            "  jar {:>2}: {:<45} -> {:<9} {}",
            species.jar_index(),
            species.label(),
            color,
            color.rgb()
        );
    }
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run -- --port COM5");
    println!("  cargo run -- --port COM5 --frames 20 --threshold 30");
    println!("  cargo run -- --dry-run --frames 5 --delay-min 0 --delay-max 0");
    println!();
    println!("Options:");
    println!("  --port COMx        Serial port cua ESP32. Neu bo qua se auto-detect.");
    println!("  --baud N           Baud rate, default 115200.");
    println!("  --threshold PCT    Nguong confidence phan loai, default 30.0.");
    println!("  --frames N         So frame demo. Neu bo qua se chay lien tuc.");
    println!("  --delay-min N      Delay toi thieu giua 2 frame, default 5 seconds.");
    println!("  --delay-max N      Delay toi da giua 2 frame, default 10 seconds.");
    println!("  --dry-run          Khong mo COM, chi in packet gui di.");
    println!("  -h, --help         Hien thi huong dan.");
}

fn parse_args() -> AppResult<AppConfig> {
    let mut config = AppConfig::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--port" => config.port_name = Some(next_arg(&mut args, "--port")?),
            "--baud" => config.baud_rate = parse_arg(&next_arg(&mut args, "--baud")?, "--baud")?,
            "--threshold" => {
                config.confidence_threshold =
                    parse_arg(&next_arg(&mut args, "--threshold")?, "--threshold")?
            }
            "--frames" => {
                config.frame_limit = Some(parse_arg(&next_arg(&mut args, "--frames")?, "--frames")?)
            }
            "--delay-min" => {
                config.delay_min_secs =
                    parse_arg(&next_arg(&mut args, "--delay-min")?, "--delay-min")?
            }
            "--delay-max" => {
                config.delay_max_secs =
                    parse_arg(&next_arg(&mut args, "--delay-max")?, "--delay-max")?
            }
            "--dry-run" => config.dry_run = true,
            _ if arg.starts_with("--port=") => {
                config.port_name = Some(arg.trim_start_matches("--port=").to_string());
            }
            _ if arg.starts_with("--baud=") => {
                config.baud_rate = parse_arg(arg.trim_start_matches("--baud="), "--baud")?;
            }
            _ if arg.starts_with("--threshold=") => {
                config.confidence_threshold =
                    parse_arg(arg.trim_start_matches("--threshold="), "--threshold")?;
            }
            _ if arg.starts_with("--frames=") => {
                config.frame_limit =
                    Some(parse_arg(arg.trim_start_matches("--frames="), "--frames")?);
            }
            _ if arg.starts_with("--delay-min=") => {
                config.delay_min_secs =
                    parse_arg(arg.trim_start_matches("--delay-min="), "--delay-min")?;
            }
            _ if arg.starts_with("--delay-max=") => {
                config.delay_max_secs =
                    parse_arg(arg.trim_start_matches("--delay-max="), "--delay-max")?;
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    if !(0.0..=100.0).contains(&config.confidence_threshold) {
        return Err("--threshold phai nam trong khoang 0..=100".into());
    }

    if config.delay_min_secs > config.delay_max_secs {
        return Err("--delay-min khong duoc lon hon --delay-max".into());
    }

    Ok(config)
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}").into())
}

fn parse_arg<T>(value: &str, flag: &str) -> AppResult<T>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    value
        .parse::<T>()
        .map_err(|error| format!("invalid value for {flag}: {error}").into())
}

fn delay_before_next_frame(config: &AppConfig) {
    if config.delay_max_secs == 0 {
        return;
    }

    let delay_secs = if config.delay_min_secs == config.delay_max_secs {
        config.delay_min_secs
    } else {
        rand::thread_rng().gen_range(config.delay_min_secs..=config.delay_max_secs)
    };

    println!("  [WAIT] Dang cho muoi tiep theo ({}s)...", delay_secs);
    thread::sleep(Duration::from_secs(delay_secs));
}

fn run() -> AppResult<()> {
    let config = parse_args()?;
    print_banner(&config);
    print_color_mapping();

    println!("\n[INIT]");
    let mut controller = Esp32SerialController::connect(&config)?;
    controller.send_command(Command::ServoOn, 0)?;
    controller.send_command(Command::Home, 0)?;

    let mut jar_counters = vec![0u32; N_SPECIES + 1];
    let mut total_count = 0u32;
    let mut frame = 0u64;

    println!("\n[RUN] Bat dau demo. Nhan Ctrl+C de dung neu chay continuous.");

    loop {
        if config
            .frame_limit
            .is_some_and(|frame_limit| frame >= frame_limit)
        {
            break;
        }

        frame += 1;
        println!("\n--- Frame #{frame} ---");

        let detection = fake_ai_detect();
        let target_jar = classify_target(&detection, config.confidence_threshold);

        if target_jar == UNKNOWN_JAR_INDEX {
            jar_counters[UNKNOWN_JAR_INDEX] += 1;
            total_count += 1;
            println!(
                "  [AI] Confidence thap: {} ({:.1}%) -> UNKNOWN, khong doi mau LED.",
                detection.species.label(),
                detection.confidence
            );
        } else {
            jar_counters[target_jar] += 1;
            total_count += 1;
            println!(
                "  [AI] Phat hien: {} | confidence={:.1}% | jar={}",
                detection.species.label(),
                detection.confidence,
                target_jar
            );
            send_led_color_for_species(&mut controller, detection.species)?;
        }

        print_counters(&jar_counters, total_count);
        delay_before_next_frame(&config);
    }

    println!("\n[SHUTDOWN]");
    controller.send_command(Command::GetStatus, 0)?;
    println!("  Demo finished.");
    Ok(())
}

fn print_counters(jar_counters: &[u32], total_count: u32) {
    print!("  [STATS] ");
    for (index, count) in jar_counters.iter().enumerate() {
        if index == UNKNOWN_JAR_INDEX {
            print!("UNKNOWN:{count}");
        } else {
            print!("{index}:{count} | ");
        }
    }
    println!(" | total={total_count}");
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error}");
        eprintln!("Run with --help for usage.");
        std::process::exit(1);
    }
}
