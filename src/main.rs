use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::{App, Arg, SubCommand};
use log::{debug, info, trace, LevelFilter};

use fakeci::conf::FakeCIBinaryConfig;
use fakeci::launch;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(LevelFilter::Trace)
        .init();
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
    for r in config.repositories.iter_mut() {
        debug!("updating repo {}", r.name);
        r.init();
    }
    while !term.load(Ordering::Relaxed) {
        for repo in config.repositories.iter_mut() {
            debug!("Checking repo {}", repo.name);
            trace!("repo before update: {:#?}", repo);
            // fetch and see if there's changes, and on which branches
            let changes = repo.update_branches()?;
            trace!("repo after update: {:#?}", repo);
            info!("found changes: {:?}", changes);
            // if there's changes, execute the CI
            if changes.is_empty() {
                continue;
            }
            for branch in changes.keys().filter(|k| {
                repo.br_regexps.iter().any(|r| {
                    trace!("pattern: {}, k: {}", r, k);
                    r.matches(k)
                })
            }) {
                info!("Detected change in {}#{}!", repo.name, branch);
                let mut res = launch(&repo.uri, branch)?;
                res.context.repo_name = String::from(&repo.name);
                res.context.repo_url = String::from(&repo.uri);
                if let Some(notifiers) = &repo.notifiers {
                    for notifier in notifiers {
                        notifier.send(&res)?;
                    }
                }
            }
            trace!("finished execution, persisting branch values…");
            repo.persist()?;
        }
        trace!("Waiting {:?} seconds", wait_period);
        thread::sleep(wait_period);
    }
    info!("Exiting");
    Ok(())
}

fn read_fakeci_config_file(config_file: &str) -> Result<FakeCIBinaryConfig> {
    let mut s = String::new();
    let mut f = File::open(config_file)?;
    f.read_to_string(&mut s)?;
    Ok(serde_yaml::from_str(&s)?)
}
