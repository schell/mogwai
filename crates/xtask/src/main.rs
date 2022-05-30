//! Tasks related to configuring, building and testing mogwai.
//!
//! Run `cargo xtask help` for more info.
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

const RUSTUP_TOOLCHAIN: &'static str = "nightly";

#[derive(Parser)]
#[clap(author, version, about, subcommand_required = true)]
struct Cli {
    /// Sets the verbosity level
    #[clap(short, parse(from_occurrences))]
    verbosity: usize,
    /// Skip installing dependencies
    #[clap(long)]
    skip_install_deps: bool,

    /// The task to run
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Artifact {
    /// One or all of the examples
    Example {
        /// The name of the example to comple. If ommitted, all examples will be compiled
        name: Option<String>,
    },

    /// The cookbook
    Cookbook,
}

impl Artifact {
    fn build_example(name: Option<String>) -> anyhow::Result<()> {
        let root = PathBuf::from(get_root_prefix()?);
        tracing::debug!("root: '{}'", root.display());
        anyhow::ensure!(root.is_dir(), "root is not a dir");
        anyhow::ensure!(root.exists(), "root dir does not exist");

        let book_examples_dir = PathBuf::from(&root).join("book_examples");
        if !book_examples_dir.exists() {
            std::fs::create_dir(&book_examples_dir)
                .context("could not create book examples dir")?;
        } else {
            tracing::warn!("book_examples dir already exists, will overwrite");
        }

        let examples_dir = PathBuf::from(&root).join("examples");
        let examples = std::fs::read_dir(&examples_dir)?;
        for example in examples {
            let entry = example?;
            let example_path = entry.path();
            let example_name = example_path
                .file_name()
                .context("could not get example name")?;
            if example_name == "multipage" {
                continue;
            }
            if let Some(name) = name.as_ref() {
                if name.as_str() != example_name {
                    continue;
                }
            }

            tracing::info!(
                "building example {:?} from {}",
                example_name,
                examples_dir.display()
            );
            duct::cmd!(
                "wasm-pack",
                "build",
                "--debug",
                "--target",
                "web",
                &example_path
            )
            .run()
            .context("could not build example")?;
            let example_destination = &book_examples_dir.join(example_name);
            if example_destination.exists() {
                tracing::warn!(
                    "destination {} already exists - removing it first",
                    example_destination.display()
                );
                duct::cmd!("rm", "-rf", &example_destination)
                    .run()
                    .context("could not remove stale destination")?;
            }
            std::fs::create_dir_all(&example_destination)
                .context("could not create example destination")?;
            std::env::set_current_dir(&example_path).context("could not cd")?;

            duct::cmd!("cp", "-R", "index.html", "pkg", &example_destination)
                .run()
                .context("could not copy files into place")?;
            let style = PathBuf::from("style.css");
            if style.exists() {
                duct::cmd!("cp", &style, &example_destination)
                    .run()
                    .context("could not copy style.css")?;
            }
        }

        tracing::info!("done building");

        Ok(())
    }

    fn build_cookbook() -> anyhow::Result<()> {
        let root = PathBuf::from(get_root_prefix()?);
        std::env::set_current_dir(&root).context("could not cd to root")?;

        duct::cmd!("mdbook", "build", "cookbook")
            .env(
                "MDBOOK_preprocessor__variables__variables__cookbookroot",
                "/guides/mogwai-cookbook",
            )
            .run()
            .context("could not build cookbook")?;

        duct::cmd!(
            "mv",
            &root.join("book_examples"),
            "cookbook/book/html/examples"
        )
        .run()
        .context("could not copy examples into book")?;

        Ok(())
    }

    fn build(self) -> anyhow::Result<()> {
        match self {
            Artifact::Example { name } => Self::build_example(name),
            Artifact::Cookbook => {
                Self::build_example(None)?;
                Self::build_cookbook()
            }
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Build an artifact
    #[clap(subcommand)]
    Build(Artifact),
    ///// Test everything
    //Test(Test),
}

fn get_root_prefix() -> anyhow::Result<String> {
    let output = duct::cmd!("git", "rev-parse", "--show-toplevel")
        .stdout_capture()
        .run()
        .context("could not get git branch name")?;
    Ok(String::from_utf8(output.stdout)
        .context("could not convert stdout to string")?
        .trim()
        .to_string())
}

fn have_program(program: &str) -> anyhow::Result<bool> {
    let output = duct::cmd!("hash", program, "2>/dev/null;")
        .run()
        .context(format!("could not determine if '{}' is available", program))?;
    Ok(output.status.success())
}

fn ensure_paths() -> anyhow::Result<()> {
    // make sure the cargo bin is on the PATH
    #[allow(deprecated)]
    let home = std::env::home_dir().context("no home dir")?;
    let path = std::env::var_os("PATH").context("no PATH var")?;
    let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
    paths.push(home.join(".cargo/bin"));
    let new_path = std::env::join_paths(paths)?;
    std::env::set_var("PATH", &new_path);

    Ok(())
}

fn install_deps() -> anyhow::Result<()> {
    anyhow::ensure!(have_program("rustup")?, "missing rustup");
    duct::cmd!("rustup", "toolchain", "install", RUSTUP_TOOLCHAIN)
        .run()
        .context("could not install 1.56")?;
    duct::cmd!("rustup", "default", RUSTUP_TOOLCHAIN)
        .run()
        .context("could not default to 1.56")?;

    let cargo_deps = vec![
        "wasm-pack",
        "mdbook",
        "mdbook-linkcheck",
        "mdbook-variables",
    ];
    for dep in cargo_deps.iter() {
        if !have_program(dep)? {
            tracing::info!("installing {}", dep);
            duct::cmd!("cargo", "install", dep)
                .run()
                .context(format!("could not install {}", dep))?;
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbosity {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };
    // use the verbosity level later when we build TVM
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ensure_paths()?;

    if !cli.skip_install_deps {
        install_deps()?;
    }

    match cli.command {
        Command::Build(artifact) => artifact.build()?,
    }

    Ok(())
}
