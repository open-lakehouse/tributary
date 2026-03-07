use clap::{Args, Parser, Subcommand};

use crate::{init::InitArgs, run::RunArgs};

mod app;
mod error;
mod init;
mod run;
mod server;
mod telemetry;
mod tui;

#[derive(clap::Parser)]
#[command(name = "caspers-universe", version, about = "Running Caspers Universe", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Args)]
struct GlobalOpts {
    /// Server URL
    #[clap(
        long,
        global = true,
        env = "UC_SERVER_URL",
        default_value = "http://localhost:8080"
    )]
    server: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a simulation
    Run(RunArgs),
    /// Initialize a simulation setup
    Init(InitArgs),
    /// Run the servers
    Server(ServerArgs),
}

#[derive(Debug, Args)]
struct ServerArgs {
    /// Server URL
    #[clap(
        long,
        global = true,
        env = "CASPERS_SERVER_URL",
        default_value = "0.0.0.0:8000"
    )]
    server: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let tui_mode = matches!(cli.command, Commands::Run(_));

    telemetry::init_tracer_provider();
    let _guard = telemetry::init_tracing_subscriber(tui_mode);

    match cli.command {
        Commands::Run(args) => run::handle(args).await?,
        Commands::Init(args) => init::handle(args).await?,
        Commands::Server(args) => server::handle(args).await?,
    }

    Ok(())
}
