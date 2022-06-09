#[macro_use]
extern crate log;

mod engine;
mod util;

use crate::util::SyncResult;

use clap::{Arg, Command};
use tokio::time::Instant;

#[tokio::main]
async fn main() -> SyncResult<()> {
    let start = Instant::now();
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();

    engine::alpine::download_rootfs(engine::alpine::VERSION).await?;
    debug!(
        "cached alpine rootfs at: {}",
        engine::alpine::rootfs_path(engine::alpine::VERSION).display()
    );
    engine::slirp::download_slirp4netns().await?;
    debug!(
        "cached slirp4netns at: {}",
        engine::slirp::bin_path().display()
    );

    let matches = Command::new("@")
        .subcommand(
            Command::new("run")
                .visible_alias("r")
                .arg(Arg::new("command").takes_value(true).required(true))
                // .arg(Arg::new("detach").short('d').required(false))
                .arg(
                    Arg::new("immutable")
                        .short('i')
                        .long("immutable")
                        .required(false)
                        .takes_value(false),
                )
                .arg(
                    Arg::new("package")
                        .short('P')
                        .multiple_occurrences(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::new("port")
                        .short('p')
                        .multiple_occurrences(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::new("rw")
                        .long("rw")
                        .multiple_occurrences(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::new("ro")
                        .long("ro")
                        .multiple_occurrences(true)
                        .takes_value(true),
                ),
        )
        .subcommand(Command::new("ps"))
        .get_matches();

    match matches.subcommand_name() {
        Some("run") => {
            let matches = matches.subcommand_matches("run").unwrap();
            let command = matches.value_of("command").unwrap();
            let detach: bool = false; // matches.is_present("detach");
            let packages: Vec<String> = matches
                .values_of("package")
                .map_or(vec![], |v| v.map(|f| f.to_string()).collect());
            let ports: Vec<(u16, u16)> = matches.values_of("port").map_or(vec![], |v| {
                v.map(|p| {
                    let slice: Vec<&str> = p.split(':').collect();
                    (
                        slice[0].parse().expect("outer port must be valid u16"),
                        slice[1].parse().expect("inner port must be valid u16"),
                    )
                })
                .collect()
            });
            let immutable: bool = matches.is_present("immutable");
            let rw_mounts: Vec<(String, String)> = matches.values_of("rw").map_or(vec![], |v| {
                v.map(|p| {
                    let slice: Vec<String> = p.split(':').map(|s| s.to_string()).collect();
                    (
                        slice[0].clone(),
                        slice[1].clone(),
                    )
                })
                .collect()
            });
            let ro_mounts: Vec<(String, String)> = matches.values_of("ro").map_or(vec![], |v| {
                v.map(|p| {
                    let slice: Vec<String> = p.split(':').map(|s| s.to_string()).collect();
                    (
                        slice[0].clone(),
                        slice[1].clone(),
                    )
                })
                .collect()
            });

            engine::Engine::new(start)
                .run(engine::RunOpts {
                    command: command.to_string(),
                    packages,
                    detach,
                    ports,
                    immutable,
                    rw_mounts,
                    ro_mounts,
                })
                .await?;
        }
        Some("ps") => {
            engine::Engine::new(start).ps().await?;
        }
        _ => {}
    }

    Ok(())
}
