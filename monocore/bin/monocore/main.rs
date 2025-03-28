#[path = "mod.rs"]
mod internal;

use clap::{CommandFactory, Parser};
use internal::handlers;
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand, ServerSubcommand},
    management::{image, orchestra, server},
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const START_SCRIPT: &str = "start";
const SHELL_SCRIPT: &str = "shell";

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = MonocoreArgs::parse();
    match args.subcommand {
        Some(MonocoreSubcommand::Init {
            path,
            path_with_flag,
        }) => {
            handlers::init_subcommand(path, path_with_flag).await?;
        }
        Some(MonocoreSubcommand::Add {
            sandbox,
            build,
            group,
            names,
            image,
            ram,
            cpus,
            volumes,
            ports,
            envs,
            env_file,
            depends_on,
            workdir,
            shell,
            scripts,
            imports,
            exports,
            reach,
            path,
            config,
        }) => {
            handlers::add_subcommand(
                sandbox, build, group, names, image, ram, cpus, volumes, ports, envs, env_file,
                depends_on, workdir, shell, scripts, imports, exports, reach, path, config,
            )
            .await?;
        }
        Some(MonocoreSubcommand::Remove {
            sandbox,
            build,
            group,
            names,
            path,
            config,
        }) => {
            handlers::remove_subcommand(sandbox, build, group, names, path, config).await?;
        }
        Some(MonocoreSubcommand::List {
            sandbox,
            build,
            group,
            path,
            config,
        }) => {
            handlers::list_subcommand(sandbox, build, group, path, config).await?;
        }
        Some(MonocoreSubcommand::Pull {
            image,
            image_group,
            name,
            layer_path,
        }) => {
            image::pull(name, image, image_group, layer_path).await?;
        }
        Some(MonocoreSubcommand::Run {
            sandbox,
            build,
            name,
            args,
            path,
            config,
            detach,
            exec,
        }) => {
            handlers::run_subcommand(sandbox, build, name, args, path, config, detach, exec)
                .await?;
        }
        Some(MonocoreSubcommand::Start {
            sandbox,
            build,
            name,
            args,
            path,
            config,
            detach,
        }) => {
            handlers::script_run_subcommand(
                sandbox,
                build,
                name,
                START_SCRIPT.to_string(),
                args,
                path,
                config,
                detach,
                None,
            )
            .await?;
        }
        Some(MonocoreSubcommand::Shell {
            sandbox,
            build,
            name,
            args,
            path,
            config,
            detach,
        }) => {
            handlers::script_run_subcommand(
                sandbox,
                build,
                name,
                SHELL_SCRIPT.to_string(),
                args,
                path,
                config,
                detach,
                None,
            )
            .await?;
        }
        Some(MonocoreSubcommand::Tmp {
            image: _image,
            name,
            cpus,
            ram,
            volumes,
            ports,
            envs,
            workdir,
            exec,
            args,
        }) => {
            handlers::tmp_subcommand(name, cpus, ram, volumes, ports, envs, workdir, exec, args)
                .await?;
        }
        Some(MonocoreSubcommand::Apply { path, config }) => {
            orchestra::apply(path.as_deref(), config.as_deref()).await?;
        }
        Some(MonocoreSubcommand::Up {
            sandbox,
            build,
            group,
            names,
            path,
            config,
        }) => {
            handlers::up_subcommand(sandbox, build, group, names, path, config).await?;
        }
        Some(MonocoreSubcommand::Down {
            sandbox,
            build,
            group,
            names,
            path,
            config,
        }) => {
            handlers::down_subcommand(sandbox, build, group, names, path, config).await?;
        }
        Some(MonocoreSubcommand::Log {
            sandbox,
            build,
            group,
            name,
            path,
            config,
            follow,
            tail,
        }) => {
            handlers::log_subcommand(sandbox, build, group, name, path, config, follow, tail)
                .await?;
        }
        Some(MonocoreSubcommand::Server { subcommand }) => match subcommand {
            ServerSubcommand::Start {
                port,
                path,
                disable_default,
            } => {
                server::start(port, path, disable_default, true).await?;
            }
            ServerSubcommand::Stop => {
                server::stop().await?;
            }
        },
        Some(_) => (), // TODO: implement other subcommands
        None => {
            MonocoreArgs::command().print_help()?;
        }
    }

    Ok(())
}
