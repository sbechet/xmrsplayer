use clap::Parser;
use console::{Key, Term};
use rodio::Sink;
use std::sync::Arc;

use xmrs::prelude::*;
use xmrs::xm::xmmodule::XmModule;

use xmrsplayer::modulesource::ModuleSource;
use xmrsplayer::prelude::*;

const SAMPLE_RATE: u32 = 48000;

#[derive(Parser)]
struct Cli {
    /// Choose XM or XmRs File
    #[arg(
        short = 'f',
        long,
        default_value = "DEADLOCK.XM",
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
            println!("(c) 2023 Sébastien Béchet\n");
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

fn rodio_play(module: Arc<Module>, amplification: f32, position: usize, loops: u8, debug: bool) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink: Sink = rodio::Sink::try_new(&stream_handle).unwrap();

    let mut player = XmrsPlayer::new(module.clone(), SAMPLE_RATE as f32);
    player.amplification = amplification;
    if debug {
        println!("Debug on");
    }
    player.debug(debug);
    player.set_max_loop_count(loops);
    player.goto(position, 0);

    let source = ModuleSource::new(player, SAMPLE_RATE);
    sink.append(source);
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
    }
}
