use clap::Parser;
use console::{Key, Term};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

use xmrs::prelude::*;
use xmrs::xm::xmmodule::XmModule;

use xmrsplayer::prelude::*;

const SAMPLE_RATE: u32 = 44100;

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
    #[arg(short = 'a', long, default_value = "0.25")]
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
                    cpal_play(
                        module.clone(),
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

fn cpal_play(
    module: Arc<Module>,
    amplification: f32,
    position: usize,
    loops: u8,
    debug: bool,
) -> Arc<Mutex<XmrsPlayer>> {

    // try to detect FT2 to play historical bugs
    let is_ft2 = module.comment == "FastTracker v2.00 (1.04)";

    let player = Arc::new(Mutex::new(XmrsPlayer::new(
        module.clone(),
        SAMPLE_RATE as f32,
        is_ft2,
    )));

    {
        let mut player_lock = player.lock().unwrap();
        player_lock.amplification = amplification;
        if debug {
            println!("Debug on");
        }
        player_lock.debug(debug);
        player_lock.set_max_loop_count(loops);
        player_lock.goto(position, 0);
    }

    start_audio_player(player.clone()).expect("failed to start player");

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
                    return player;
                }
                _ => {
                    println!("no way");
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        let ti = player.lock().unwrap().get_current_table_index();
        let p = player.lock().unwrap().get_current_pattern();
        println!("current table index:{:02x}, current pattern:{:02x}", ti, p);
    }
}

fn start_audio_player(player: Arc<Mutex<XmrsPlayer>>) -> Result<(), cpal::StreamError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = device
        .default_output_config()
        .expect("failed to get default output config");
    let sample_rate = config.sample_rate();

    println!("cpal sample rate: {:?}", sample_rate);

    std::thread::spawn(move || {
        let stream = device
            .build_output_stream(
                &config.config(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut player_lock = player.lock().unwrap();
                    for sample in data.iter_mut() {
                        *sample = player_lock.next().unwrap_or(0.0);
                    }
                },
                |_: cpal::StreamError| {},
                None,
            )
            .expect("failed to build output stream");

        stream.play().expect("failed to play stream");
        std::thread::sleep(std::time::Duration::from_secs_f32(60.0));
    });

    Ok(())
}
