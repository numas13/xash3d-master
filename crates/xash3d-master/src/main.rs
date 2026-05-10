#![deny(unsafe_code)]

#[macro_use]
extern crate log;

mod cli;
mod config;
mod hash_map;
mod logger;
mod master_server;
mod periodic;
mod stats;
mod str_arr;
mod time;

use std::process;

use async_signal::{Signal, Signals};
use smol::{channel, stream::StreamExt};

use crate::{
    cli::Cli,
    config::Config,
    logger::Logger,
    master_server::{Error, Master},
    stats::Stats,
    str_arr::StrArr,
};

fn load_config(cli: &Cli, logger: &Logger) -> Result<Config, config::Error> {
    let mut cfg = match cli.config_path {
        Some(ref p) => config::load(p.as_ref())?,
        None => Config::default(),
    };

    if let Some(level) = cli.log_level {
        cfg.log.level = level;
    }
    if let Some(ip) = cli.listen_ip {
        cfg.master.server.ip = ip;
    }
    if let Some(port) = cli.listen_port {
        cfg.master.server.port = port;
    }
    if let Some(format) = &cli.stats_format {
        cfg.stat.format = format.clone();
    }
    if let Some(interval) = cli.stats_interval {
        cfg.stat.interval = interval;
    }

    logger.update_config(&cfg.log);

    Ok(cfg)
}

async fn run() -> Result<(), Error> {
    let cli = cli::parse().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    let logger = logger::init();

    let cfg = load_config(&cli, logger).unwrap_or_else(|e| {
        match cli.config_path.as_deref() {
            Some(p) => eprintln!("Failed to load config \"{p}\": {e}"),
            None => eprintln!("{e}"),
        }
        process::exit(1);
    });

    let mut master = Master::new(cfg).await?;

    let mut signals = Signals::new([Signal::Int, Signal::Usr1])?;

    loop {
        let (tx, rx) = channel::bounded(1);
        let task = smol::spawn(async {
            master.run(Some(rx)).await?;
            Ok::<_, Error>(master)
        });

        let mut exit = false;
        while let Some(signal) = signals.next().await {
            match signal? {
                Signal::Int => {
                    exit = true;
                    break;
                }
                Signal::Usr1 => break,
                _ => {}
            }
        }

        tx.send(()).await.ok();
        master = task.await?;

        if exit {
            break;
        }

        if let Some(config_path) = cli.config_path.as_deref() {
            info!("Reloading config from {}", config_path);
            match load_config(&cli, logger) {
                Ok(cfg) => {
                    if let Err(e) = master.update_config(cfg).await {
                        error!("{}", e);
                    }
                }
                Err(e) => error!("failed to load config: {}", e),
            }
        } else {
            warn!("Use --config option to specify the path to a configuration file");
        }
    }

    info!("Server stopped");
    Ok(())
}

fn main() {
    smol::block_on(async {
        if let Err(e) = run().await {
            error!("{}", e);
            process::exit(1);
        }
    });
}
