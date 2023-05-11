use clap::Parser;

use rodio::{buffer::SamplesBuffer, Sink};

use xmrs::prelude::*;
use xmrs::xm::xmmodule::XmModule;

use xmrsplayer::prelude::*;

const SAMPLE_RATE: u32 = 48000;
const BUFFER_LEN: usize = 48000;

#[derive(Parser)]
struct Cli {
    /// Choose XM or XmRs File
    #[arg(
        short = 'f',
        long,
        default_value = "default.xmrs",
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
            println!("--===~ XmRs Player Example ~===--");
            println!("(c) 2023 Sébastien Béchet\n");
            println!("Because demo scene can't die :)\n");
            let path = std::env::current_dir()?;
            println!("The current directory is {}", path.display());
            println!("opening {}", filename);
            let contents = std::fs::read(filename.trim())?;
            match XmModule::load(&contents) {
                Ok(xm) => {
                    println!("XM '{}' loaded...", xm.header.name);

                    let mut module: Module = xm.to_module();
                    println!("'{}' converted to module...", module.name);
                    rodio_play(&mut module, cli.amplification, cli.position, cli.loops, cli.debug);
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

fn rodio_play(module: &Module, amplification: f32, position: usize, loops: u8, debug: bool) {
    let mut player = XmrsPlayer::new(&module, SAMPLE_RATE as f32);
    player.amplification = amplification;
    player.debug(debug);
    player.set_max_loop_count(loops);
    player.goto(position, 0);

    let mut buffer: [f32; BUFFER_LEN] = [0.0; BUFFER_LEN];

    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink: Sink = rodio::Sink::try_new(&stream_handle).unwrap();

    while loops == 0 || player.get_loop_count() < loops {
        player.generate_samples(&mut buffer);
        // println!("{:02?}", &buffer);
        let source = SamplesBuffer::new(2, SAMPLE_RATE, buffer);
        sink.append(source);
        sink.play();
    }
    sink.stop();
}
