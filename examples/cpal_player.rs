use clap::Parser;
use console::{Key, Term};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

use xmrs::amiga::amiga_module::AmigaModule;
use xmrs::prelude::*;
use xmrs::s3m::s3m_module::S3mModule;
use xmrs::xm::xmmodule::XmModule;

use xmrsplayer::prelude::*;

const SAMPLE_RATE: u32 = 44100;

#[derive(Parser)]
struct Cli {
    /// Choose XM or XmRs File
    #[arg(short = 'f', long, required = true, value_name = "filename")]
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
            println!("opening {}", filename);
            let contents = std::fs::read(filename.trim())?;
            match filename.split('.').last() {
                Some(extension) if extension == "xm" || extension == "XM" => {
                    match XmModule::load(&contents) {
                        Ok(xm) => {
                            drop(contents); // cleanup memory
                            let module = xm.to_module();
                            drop(xm);
                            println!("Playing {} !", module.name);

                            let module = Box::new(module);
                            let module_ref: &'static Module = Box::leak(module);
                            cpal_play(
                                module_ref,
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
                Some(extension) if extension == "mod" || extension == "MOD" => {
                    match AmigaModule::load(&contents) {
                        Ok(amiga) => {
                            drop(contents); // cleanup memory
                            let module = amiga.to_module();
                            drop(amiga);
                            println!("Playing {} !", module.name);
                            let module = Box::new(module);
                            let module_ref: &'static Module = Box::leak(module);
                            cpal_play(
                                module_ref,
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
                Some(extension) if extension == "s3m" || extension == "S3M" => {
                    match S3mModule::load(&contents) {
                        Ok(s3m) => {
                            drop(contents); // cleanup memory
                            let module = s3m.to_module();
                            drop(s3m);
                            println!("Playing {} !", module.name);
                            let module = Box::new(module);
                            let module_ref: &'static Module = Box::leak(module);
                            cpal_play(
                                module_ref,
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

fn cpal_play(module: &'static Module, amplification: f32, position: usize, loops: u8, debug: bool, ch: u8, speed: u16, historical: bool) {
    // try to detect FT2 to play historical bugs
    let is_ft2 = historical
        || module.comment == "FastTracker v2.00 (1.02)"
        || module.comment == "FastTracker v2.00 (1.03)"
        || module.comment == "FastTracker v2.00 (1.04)";

    let player = Arc::new(Mutex::new(XmrsPlayer::new(
        module,
        SAMPLE_RATE as f32,
        is_ft2,
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

    start_audio_player(Arc::clone(&player)).expect("failed to start player");

    let stdout = Term::stdout();
    println!(
        "Enter key for info, Space for pause, left or right arrow to move, escape key to exit..."
    );
    let mut playing = true;
    loop {
        if let Ok(character) = stdout.read_key() {
            match character {
                Key::Enter => {
                    let ti = player.lock().unwrap().get_current_table_index();
                    let p = player.lock().unwrap().get_current_pattern();
                    println!("current table index:{:02x}, current pattern:{:02x}", ti, p);
                }
                Key::Escape => {
                    println!("Have a nice day!");
                    return;
                }
                Key::ArrowLeft => {
                    let i = player.lock().unwrap().get_current_table_index();
                    if i != 0 {
                        player.lock().unwrap().goto(i - 1, 0, 0);
                    }
                }
                Key::ArrowRight => {
                    let len = module.pattern_order.len();
                    let i = player.lock().unwrap().get_current_table_index();
                    if i + 1 < len {
                        player.lock().unwrap().goto(i + 1, 0, 0);
                    }
                }
                Key::Char(' ') => {
                    if playing {
                        println!("Pause, press space to continue");
                        player.lock().unwrap().pause(true);
                        playing = false;
                        {
                            let player_lock = player.lock().unwrap();
                            let ti = player_lock.get_current_table_index();
                            let p = player_lock.get_current_pattern();
                            let row = player_lock.get_current_row();
                            println!("Pattern [{:02X}]={:02X}, Row {:02X}", ti, p, row);
                        }
                    } else {
                        println!("Playing");
                        player.lock().unwrap().pause(false);
                        playing = true;
                    }
                }
                _ => {}
            }
        }
    }
}

fn start_audio_player(player: Arc<Mutex<XmrsPlayer<'static>>>) -> Result<(), cpal::StreamError> {
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
        let player = Arc::clone(&player);
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
