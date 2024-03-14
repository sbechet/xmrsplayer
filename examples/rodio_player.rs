use clap::Parser;
use console::{Key, Term};
use rodio::Sink;
use std::sync::{Arc, Mutex};

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

    /// Start at a specific pattern order table position
    #[arg(short = 'p', long, default_value = "0")]
    position: usize,

    /// How many loop (default: infinity)
    #[arg(short = 'l', long, default_value = "0")]
    loops: u8,

    /// Turn debugging information on
    #[arg(short = 'd', long, default_value = "false")]
    debug: bool,
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();

    match cli.filename {
        Some(filename) => {
            Term::stdout().clear_screen().unwrap();
            println!("--===~ XmRs Player Example ~===--");
            println!("(c) 2023-2024 Sébastien Béchet\n");
            println!("Because demo scene can't die :)\n");
            // let path = std::env::current_dir()?;
            // println!("The current directory is {}", path.display());
            println!("opening {}", filename);
            let contents = std::fs::read(filename.trim())?;
            match XmModule::load(&contents) {
                Ok(xm) => {
                    drop(contents); // cleanup memory
                    print!("XM '{}' loaded...", xm.header.name);
                    let module = Arc::new(xm.to_module());
                    drop(xm);
                    println!("Playing {} !", module.name);
                    rodio_play(
                        module,
                        cli.amplification,
                        cli.position,
                        cli.loops,
                        cli.debug,
                    );
                }
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn rodio_play(module: Arc<Module>, amplification: f32, position: usize, loops: u8, debug: bool) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink: Sink = rodio::Sink::try_new(&stream_handle).unwrap();

    let player = Arc::new(Mutex::new(XmrsPlayer::new(module, SAMPLE_RATE as f32)));
    {
        let mut player_lock = player.lock().unwrap();
        player_lock.amplification = amplification;
        if debug {
            println!("Debug on");
        }
        player_lock.debug(debug);
        // player_lock.set_mute_channel(0, true);
        // player_lock.set_mute_channel(1, true);
        // player_lock.set_mute_channel(2, true);
        // player_lock.set_mute_channel(3, true);
        player_lock.set_max_loop_count(loops);
        player_lock.goto(position, 0);
    }

    let player_clone = Arc::clone(&player);
    let source = BufferedSource::new(player, SAMPLE_RATE);
    sink.append(source);
    // sink.append(player.buffered());
    sink.play();

    let stdout = Term::stdout();
    println!("Enter key for info, escape key to exit...");
    loop {
        if let Ok(character) = stdout.read_key() {
            match character {
                Key::Enter => {
                    println!("Example");
                }
                Key::Escape => {
                    println!("Have a nice day!");
                    sink.stop();
                    return;
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        let ti = player_clone.lock().unwrap().get_current_table_index();
        let p = player_clone.lock().unwrap().get_current_pattern();
        println!("current table index:{:02x}, current pattern:{:02x}", ti, p);
    }
}
