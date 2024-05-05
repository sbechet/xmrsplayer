use clap::Parser;
use console::{Key, Term};
use rodio::Sink;
use std::sync::{Arc, Mutex};

use xmrs::amiga::amiga_module::AmigaModule;
use xmrs::prelude::*;
use xmrs::xm::xmmodule::XmModule;

mod bufferedsource;
use bufferedsource::BufferedSource;
use xmrsplayer::prelude::*;

const SAMPLE_RATE: u32 = 48000;

#[derive(Parser)]
struct Cli {
    /// Choose XM or XmRs File
    #[arg(
        short = 'f',
        long,
        default_value = "coretex_-_home.xm", // https://modarchive.org/index.php?request=view_by_moduleid&query=159594
        value_name = "filename"
    )]
    filename: Option<String>,

    /// Choose amplification
    #[arg(short = 'a', long, default_value = "0.5")]
    amplification: f32,

    /// Play only a specific channel (from 1 to n, 0 for all)
    #[arg(short = 'c', long, default_value = "0")]
    ch: u8,

    /// Turn debugging information on
    #[arg(short = 'd', long, default_value = "false")]
    debug: bool,

    /// How many loop (default: infinity)
    #[arg(short = 'l', long, default_value = "0")]
    loops: u8,

    /// Force historical fT2 replay (default: autodetect)
    #[arg(short = 't', long, default_value = "false")]
    historical: bool,

    /// Start at a specific pattern order table position
    #[arg(short = 'p', long, default_value = "0")]
    position: usize,

    /// Force speed
    #[arg(short = 's', long, default_value = "0")]
    speed: u16,
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();

    match cli.filename {
        Some(filename) => {
            // Term::stdout().clear_screen().unwrap();
            println!("--===~ XmRs Player Example ~===--");
            println!("(c) 2023-2024 Sébastien Béchet\n");
            println!("Because demo scene can't die :)\n");
            // let path = std::env::current_dir()?;
            // println!("The current directory is {}", path.display());
            println!("opening {}", filename);
            let contents = std::fs::read(filename.trim())?;
            match filename.split('.').last() {
                Some(extension) if extension == "xm" || extension == "XM" => {
                    match XmModule::load(&contents) {
                        Ok(xm) => {
                            drop(contents); // cleanup memory
                            let module = Arc::new(xm.to_module());
                            drop(xm);
                            println!("Playing {} !", module.name);
                            rodio_play(
                                module,
                                cli.amplification,
                                cli.position,
                                cli.loops,
                                cli.debug,
                                cli.ch,
                                cli.speed,
                                cli.historical,
                            );
                        }
                        Err(e) => {
                            println!("{:?}", e);
                        }
                    }
                }
                Some(extension) if extension == "mod" || extension == "MOD" => {
                    match AmigaModule::load(&contents) {
                        Ok(amiga) => {
                            drop(contents); // cleanup memory
                            let module = Arc::new(amiga.to_module());
                            drop(amiga);
                            println!("Playing {} !", module.name);
                            rodio_play(
                                module,
                                cli.amplification,
                                cli.position,
                                cli.loops,
                                cli.debug,
                                cli.ch,
                                cli.speed,
                                false,
                            );
                        }
                        Err(e) => {
                            println!("{:?}", e);
                        }
                    }
                }
                Some(_) | None => {
                    println!("File unknown?");
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn rodio_play(
    module: Arc<Module>,
    amplification: f32,
    position: usize,
    loops: u8,
    debug: bool,
    ch: u8,
    speed: u16,
    historical: bool,
) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink: Sink = rodio::Sink::try_new(&stream_handle).unwrap();

    // try to detect FT2 to play historical bugs
    let is_ft2 = historical || 
                        module.comment == "FastTracker v2.00 (1.02)" ||
                        module.comment == "FastTracker v2.00 (1.03)" ||
                        module.comment == "FastTracker v2.00 (1.04)";

    let player = Arc::new(Mutex::new(XmrsPlayer::new(
        Arc::clone(&module),
        SAMPLE_RATE as f32,
        is_ft2
    )));
    {
        let mut player_lock = player.lock().unwrap();
        player_lock.amplification = amplification;
        if debug {
            println!("Debug on");
            if is_ft2 {
                println!("FT2 Historical XM detected.")
            }
        }
        player_lock.debug(debug);
        if ch != 0 {
            player_lock.mute_all(true);
            player_lock.set_mute_channel((ch - 1).into(), false);
        }
        player_lock.set_max_loop_count(loops);
        player_lock.goto(position, 0, speed);
    }

    let player_clone = Arc::clone(&player);
    let source = BufferedSource::new(player, SAMPLE_RATE);
    sink.append(source);
    // sink.append(player.buffered());
    sink.play();

    let stdout = Term::stdout();
    println!(
        "Enter key for info, Space for pause, left or right arrow to move, escape key to exit..."
    );
    let mut playing = true;
    loop {
        if let Ok(character) = stdout.read_key() {
            match character {
                Key::Enter => {
                    let ti = player_clone.lock().unwrap().get_current_table_index();
                    let p = player_clone.lock().unwrap().get_current_pattern();
                    println!("current table index:{:02x}, current pattern:{:02x}", ti, p);
                }
                Key::Escape => {
                    println!("Have a nice day!");
                    sink.stop();
                    return;
                }
                Key::ArrowLeft => {
                    let mut player = player_clone.lock().unwrap();
                    let i = player.get_current_table_index();
                    if i != 0 {
                        player.goto(i - 1, 0, 0);
                    }
                }
                Key::ArrowRight => {
                    let mut player = player_clone.lock().unwrap();
                    let len = module.pattern_order.len();
                    let i = player.get_current_table_index();
                    if i + 1 < len {
                        player.goto(i + 1, 0, 0);
                    }
                }
                Key::Char(' ') => {
                    if playing {
                        println!("Pause, press space to continue");
                        sink.pause();
                        playing = false;
                        {
                            let player = player_clone.lock().unwrap();
                            let ti = player.get_current_table_index();
                            let p = player.get_current_pattern();
                            let row = player.get_current_row();
                            println!("Pattern [{:02X}]={:02X}, Row {:02X}", ti, p, row);
                        }
                    } else {
                        println!("Playing");
                        sink.play();
                        playing = true;
                    }
                }
                _ => {}
            }
        }
    }
}
