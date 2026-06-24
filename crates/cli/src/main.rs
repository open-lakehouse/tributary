use crate::run::RunArgs;

mod app;
mod error;
mod run;
mod telemetry;
mod time_parse;
mod tui;

#[derive(clap::Parser)]
#[command(name = "caspers", version, about = "Run Chef Casper's ghost kitchen simulations", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Run a simulation
    Run(RunArgs),
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = <Cli as clap::Parser>::parse();

    let tui_mode = matches!(cli.command, Commands::Run(_));

    telemetry::init_tracer_provider();
    let _guard = telemetry::init_tracing_subscriber(tui_mode);

    match cli.command {
        Commands::Run(args) => run::handle(args).await?,
    }

    Ok(())
}
