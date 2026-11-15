#![forbid(unsafe_code)]

use clap::Parser;
use std::path::PathBuf;
use swarmforge::app::SwarmForgeApp;
use swarmforge::assembly::tick_factory;
use swarmforge::model::GameState;
use swarmforge::navigation::enforce_spacing_and_migrate;
use swarmforge::persistence::{export_json_file, import_json_file};

#[derive(Parser, Debug)]
#[command(
    name = "swarmforge",
    version = "1.3.0",
    about = "SwarmForge v1.3.0: 3D Isometric Automated Robotic Factory & Swarm Intelligence Game"
)]
struct Cli {
    #[arg(long, default_value = "swarmforge_saves.db")]
    db_path: PathBuf,

    #[arg(long)]
    import_json: Option<PathBuf>,

    #[arg(long)]
    headless_ticks: Option<u64>,

    #[arg(long)]
    export_json: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut state = if let Some(ref json_in) = cli.import_json {
        let mut loaded = import_json_file(json_in)?;
        enforce_spacing_and_migrate(&mut loaded);
        loaded
    } else {
        GameState::new_starter_factory()
    };

    if let Some(ticks) = cli.headless_ticks {
        for _ in 0..ticks {
            tick_factory(&mut state, 0.1);
        }
        println!(
            "SwarmForge v1.3.0 Headless Summary ({ticks} ticks / {:.1}s): Credits=₡{:.1} | RP={:.1} | Shipped={} | Pathing η={:.1}% | F1={}x{}",
            state.elapsed_secs,
            state.credits,
            state.research_points,
            state.total_products_shipped,
            state.layout_efficiency * 100.0,
            state.main_floor_half_extent * 2,
            state.main_floor_half_extent * 2,
        );
        if let Some(ref out) = cli.export_json {
            export_json_file(out, &state)?;
            println!("Exported state JSON to {}", out.display());
        }
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SwarmForge v1.3.0 — 3D Isometric Robotic Factory")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    let db_path = cli.db_path.clone();
    eframe::run_native(
        "SwarmForge v1.3.0",
        options,
        Box::new(move |_cc| Ok(Box::new(SwarmForgeApp::new(state, db_path)))),
    )?;
    Ok(())
}
