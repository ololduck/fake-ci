use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::{App, Arg, SubCommand};
use log::{debug, info};
use pretty_env_logger::try_init;

use fakeci::conf::FakeCIBinaryConfig;
use fakeci::launch;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    let _ = try_init();
    let matches = App::new("fake-ci")
        .version(VERSION)
        .author("Paul O.")
        .about("A CI system written in rust")
        .arg(Arg::with_name("config").short("c").long("config").value_name("FILE").help("Sets a config file").takes_value(true).default_value("fake-ci.toml"))
        .subcommand(SubCommand::with_name("watch").about("Runs FakeCI in pulling mode; it will watch predefined repositories and attempt to pull them"))
        .get_matches();
    let mut config = read_fakeci_config_file(matches.value_of("config").unwrap())?;
    debug!("config: {:#?}", config);
    if let Some(_matches) = matches.subcommand_matches("watch") {
        debug!("found subcommand watch");
        let _ = watch(&mut config);
    }
    Ok(())
}

fn watch(config: &mut FakeCIBinaryConfig) -> Result<()> {
    debug!("watch() called with config {:#?}", config);
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    let wait_period = Duration::from_secs(config.watch_interval as u64);
    let _ = config
        .repositories
        .iter_mut()
        .map(|r| {
            debug!("updating repo {}", r.name);
            r.init();
        })
        .collect::<Vec<_>>();
    while !term.load(Ordering::Relaxed) {
        for repo in config.repositories.iter_mut() {
            debug!("Checking repo {}", repo.name);
            // fetch and see if there's changes, and on which branches
            let changes = repo.update_branches()?;
            // if there's changes, execute the CI
            if changes.is_empty() {
                continue;
            }
            for branch in changes
                .keys()
                .filter(|k| repo.br_regexps.iter().any(|r| r.matches(k)))
            {
                info!("Detected change in {}#{}!", repo.name, branch);
                launch(&repo.uri, &branch)?;
            }
        }
        thread::sleep(wait_period);
    }
    Ok(())
}

fn read_fakeci_config_file(config_file: &str) -> Result<FakeCIBinaryConfig> {
    let mut s = String::new();
    let mut f = File::open(config_file)?;
    f.read_to_string(&mut s)?;
    Ok(toml::from_str(&s)?)
}