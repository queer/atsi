#[macro_use]
extern crate log;

mod engine;
mod util;

use std::collections::HashMap;

use crate::util::SyncResult;

use clap::{Arg, Command};
use tokio::time::Instant;

#[tokio::main]
async fn main() -> SyncResult<()> {
    let start = Instant::now();
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();
    let engine = engine::Engine::new(start);
    engine.init().await?;

    let matches = Command::new("@")
        .subcommand(
            Command::new("run")
                .visible_alias("r")
                .arg(
                    Arg::new("command")
                        .takes_value(true)
                        .help("The command to run inside of the container. Defaults to `sh`.")
                        .default_value("sh")
                        .long_help("The command to run inside of the container. This command will be pid 1\
                            inside the container, and will have a bare-minimum environment set up.\n\
                            \n\
                            The default value for the command is `sh`, to just always get a shell.\n\
                            \n\
                            Examples:\n\
                            - Get a shell: `@ run`\n\
                            - Install Python 3: `@ run -P python3`")
                )
                // .arg(Arg::new("detach").short('d').required(false))
                .arg(
                    Arg::new("immutable")
                        .short('i')
                        .long("immutable")
                        .required(false)
                        .takes_value(false)
                        .help("Makes the container's rootfs immutable (read-only).")
                        ,
                )
                .arg(
                    Arg::new("package")
                        .short('P')
                        .multiple_occurrences(true)
                        .takes_value(true)
                        .help("Specify a package to install. Can be specified multiple times.")
                        ,
                )
                .arg(
                    Arg::new("port")
                        .short('p')
                        .multiple_occurrences(true)
                        .takes_value(true)
                        .help("Expose a port to the host. Format is outer:inner, ex. `8080:8081`.")
                        ,
                )
                .arg(
                    Arg::new("rw")
                        .long("rw")
                        .multiple_occurrences(true)
                        .takes_value(true)
                        .help("Mount a file/directory read-write. Format is source:target, ex. `/home/me/file:/file`.")
                        ,
                )
                .arg(
                    Arg::new("ro")
                        .long("ro")
                        .multiple_occurrences(true)
                        .takes_value(true)
                        .help("Mount a file/directory read-only. Format is source:target, ex. `/home/me/file:/file`.")
                        ,
                )
                .arg(
                    Arg::new("alpine")
                        .long("alpine")
                        .takes_value(true)
                        .default_value(engine::alpine::VERSION)
                        .help(format!("The version of Alpine Linux to use. Default is {}", engine::alpine::VERSION).as_str())
                )
                .arg(
                    Arg::new("env")
                        .long("env")
                        .short('e')
                        .takes_value(true)
                        .help("Set an environment variable. Format is `VARIABLE=value`.")
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .takes_value(true)
                        .default_value(&haikunator::Haikunator::default().haikunate())
                        .help("The name of the container. A random name will be generated if none is provided.")
                )
                ,
        )
        .subcommand(Command::new("ps").arg(Arg::new("json").long("json").help("Output as JSON")))
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
                    (slice[0].clone(), slice[1].clone())
                })
                .collect()
            });
            let ro_mounts: Vec<(String, String)> = matches.values_of("ro").map_or(vec![], |v| {
                v.map(|p| {
                    let slice: Vec<String> = p.split(':').map(|s| s.to_string()).collect();
                    (slice[0].clone(), slice[1].clone())
                })
                .collect()
            });
            let alpine_version = matches
                .value_of("alpine")
                .unwrap_or(engine::alpine::VERSION);
            let env_vars: HashMap<String, String> =
                matches.values_of("env").map_or(HashMap::new(), |v| {
                    v.map(|e| {
                        if let Some((key, value)) = e.split_once('=') {
                            (key.to_string(), value.to_string())
                        } else {
                            error!("Invalid environment variable: {}", e);
                            panic!();
                        }
                    })
                    .collect()
                });
            let name = matches.value_of("name").unwrap();

            if engine.container_exists(name) {
                error!("@ container already exists: {}", name);
                return Ok(());
            }

            engine::slirp::download_slirp4netns().await?;
            debug!(
                "cached slirp4netns at: {}",
                engine::slirp::bin_path().display()
            );
            engine::alpine::download_rootfs(alpine_version).await?;
            debug!(
                "cached requested alpine rootfs at: {}",
                engine::alpine::rootfs_path(alpine_version).display()
            );

            engine
                .run(engine::RunOpts {
                    command: command.to_string(),
                    name: name.to_string(),
                    packages,
                    detach,
                    ports,
                    immutable,
                    rw_mounts,
                    ro_mounts,
                    alpine_version: alpine_version.to_string(),
                    env_vars,
                })
                .await?;
        }
        Some("ps") => {
            let matches = matches.subcommand_matches("ps").unwrap();
            let json = matches.is_present("json");

            engine::Engine::new(start).ps(json).await?;
        }
        _ => {}
    }

    Ok(())
}
