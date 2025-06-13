//! Tasks related to configuring, building and testing mogwai.
//!
//! Run `cargo xtask help` for more info.
use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};

fn build_example(name: Option<String>) -> anyhow::Result<()> {
    let root = PathBuf::from(get_root_prefix()?);
    log::debug!("root: '{}'", root.display());
    anyhow::ensure!(root.is_dir(), "root is not a dir");
    anyhow::ensure!(root.exists(), "root dir does not exist");

    let book_examples_dir = PathBuf::from(&root).join("book_examples");
    if !book_examples_dir.exists() {
        std::fs::create_dir(&book_examples_dir).context("could not create book examples dir")?;
    } else {
        log::warn!("book_examples dir already exists, will overwrite");
    }

    let examples_dir = PathBuf::from(&root).join("examples");
    let examples = std::fs::read_dir(&examples_dir)?;
    for example in examples {
        let entry = example?;
        let example_path = entry.path();
        let example_name = example_path
            .file_name()
            .context("could not get example name")?
            .to_str()
            .context("could not make str")?;
        let excludes = [".DS_Store", "multipage", "sandbox", "focus-follower"];
        if excludes.contains(&example_name) {
            continue;
        }
        if let Some(name) = name.as_ref() {
            if name.as_str() != example_name {
                continue;
            }
        }

        log::info!(
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
            log::warn!(
                "destination {} already exists - removing it first",
                example_destination.display()
            );
            duct::cmd!("rm", "-rf", &example_destination)
                .run()
                .context("could not remove stale destination")?;
        }
        std::fs::create_dir_all(example_destination)
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

    log::info!("done building");

    Ok(())
}

fn build_cookbook(cookbook_root_path: Option<PathBuf>) -> anyhow::Result<()> {
    let root = PathBuf::from(get_root_prefix()?);
    std::env::set_current_dir(&root).context("could not cd to root")?;

    let build_cookbook_cmd = duct::cmd!("mdbook", "build", "cookbook");
    let build_cookbook_cmd = if let Some(path) = cookbook_root_path {
        log::info!("building cookbook with root path '{}'", path.display());
        build_cookbook_cmd.env(
            "MDBOOK_preprocessor__variables__variables__cookbookroot",
            format!("{}", path.display()),
        )
    } else {
        build_cookbook_cmd
    };
    build_cookbook_cmd
        .run()
        .context("could not build cookbook")?;

    duct::cmd!(
        "cp",
        "-R",
        &root.join("book_examples"),
        "cookbook/book/html/examples"
    )
    .run()
    .context("could not copy examples into book")?;

    Ok(())
}

#[derive(Parser)]
#[clap(author, version, about, subcommand_required = true)]
struct Cli {
    /// Skip installing dependencies
    #[clap(long)]
    skip_install_deps: bool,

    /// The task to run
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
struct Cookbook {
    /// Skip building the examples.
    #[clap(long)]
    skip_examples: bool,
    /// Set the root path of the cookbook, eg "/guides/mogwai-cookbook"
    #[clap(long)]
    root_path: Option<PathBuf>,
}

impl Cookbook {
    fn build(self) -> anyhow::Result<()> {
        let Cookbook {
            skip_examples,
            root_path,
        } = self;
        if !skip_examples {
            build_example(None)?;
        } else {
            log::info!("skipping building examples");
        }

        build_cookbook(root_path)
    }
}

#[derive(Parser)]
enum Artifact {
    /// One or all of the examples
    Example {
        /// The name of the example to comple. If ommitted, all examples will be
        /// compiled
        name: Option<String>,
    },

    /// The cookbook
    Cookbook(Cookbook),
}

impl Artifact {
    fn build(self) -> anyhow::Result<()> {
        match self {
            Artifact::Example { name } => build_example(name),
            Artifact::Cookbook(cookbook) => cookbook.build(),
        }
    }
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

fn have_program(bin: &str) -> bool {
    let have_it = duct::cmd!("hash", bin).run().is_ok();
    if have_it {
        log::debug!("have {}", bin);
    } else {
        log::error!("missing {}", bin);
    }
    have_it
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
    let cargo_deps = [
        "wasm-pack",
        "mdbook",
        "mdbook-linkcheck",
        "mdbook-variables",
        "cargo-generate",
        "trunk",
    ];
    for dep in cargo_deps.iter() {
        if !have_program(dep) {
            log::info!("installing {}", dep);
            duct::cmd!("cargo", "install", "--locked", dep)
                .run()
                .context(format!("could not install {}", dep))?;
        }
    }

    Ok(())
}

#[derive(Parser, Default)]
struct TestEverything {
    #[clap(long)]
    skip_cargo_test: bool,
    #[clap(long)]
    skip_cargo_doc: bool,
    #[clap(long)]
    skip_wasm_pack_test: bool,
    #[clap(long)]
    skip_mogwai_template: bool,
}

#[derive(Subcommand)]
enum Test {
    Everything(TestEverything),
    Cargo,
    CargoDoc,
    Wasm,
    Template,
}

impl Default for Test {
    fn default() -> Self {
        Self::Everything(Default::default())
    }
}

impl Test {
    fn test_cargo() -> anyhow::Result<()> {
        log::info!("running cargo tests");
        duct::cmd!("cargo", "test").run()?;
        Ok(())
    }

    fn test_cargo_doc() -> anyhow::Result<()> {
        log::info!("running cargo doc");
        duct::cmd!("cargo", "doc").run()?;
        Ok(())
    }

    fn test_wasm() -> anyhow::Result<()> {
        log::info!("testing mogwai in wasm");
        duct::cmd!(
            "wasm-pack",
            "test",
            "--firefox",
            "--headless",
            "crates/mogwai"
        )
        .run()?;
        Ok(())
    }

    fn test_template() -> anyhow::Result<()> {
        log::info!("testing mogwai-template");
        let dir = tempfile::tempdir().context("could not create temp dir for template test")?;

        duct::cmd!(
            "cargo",
            "generate",
            "--git",
            "https://github.com/schell/mogwai-template.git",
            "--name",
            "gentest",
            "-d",
            "authors=test",
            "--destination",
            dir.path(),
        )
        .run()?;
        anyhow::ensure!(Path::new("gentest").exists(), "gentest does not exist");

        log::info!("building gentest");
        duct::cmd!("wasm-pack", "build", "--target", "web")
            .dir(dir.path())
            .run()?;
        Ok(())
    }

    fn test_everything(
        TestEverything {
            skip_cargo_test,
            skip_cargo_doc,
            skip_wasm_pack_test,
            skip_mogwai_template,
        }: TestEverything,
    ) -> anyhow::Result<()> {
        if !skip_cargo_test {
            Self::test_cargo()?;
        }
        if !skip_cargo_doc {
            Self::test_cargo_doc()?;
        }
        if !skip_wasm_pack_test {
            Self::test_wasm()?;
        }

        if !skip_mogwai_template {
            Self::test_template()?;
        }

        Ok(())
    }

    fn run(self) -> anyhow::Result<()> {
        log::info!("testing mogwai");
        match self {
            Self::Everything(e) => Self::test_everything(e),
            Test::Cargo => Self::test_cargo(),
            Test::CargoDoc => Self::test_cargo_doc(),
            Test::Wasm => Self::test_wasm(),
            Test::Template => Self::test_template(),
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Build an artifact
    #[clap(subcommand)]
    Build(Artifact),
    /// Test everything
    #[clap(subcommand)]
    Test(Test),
    /// Push the cookbook to AWS
    PushCookbook {
        /// Path to the s3 bucket we are pushing the cookbook to
        #[clap(long, default_value = "s3://zyghost.com/guides/mogwai-cookbook")]
        s3_path: String,

        #[clap(flatten)]
        cookbook: Cookbook,
    },
    /// Copy a mogwai-js-framework-benchmark dist to another directory (the js-framework-benchmark repo)
    CopyJsFrameworkDist {
        /// Path to the folder to copy into
        #[clap(long)]
        copy_into: std::path::PathBuf,
    },
}

fn workspace_dir() -> std::path::PathBuf {
    std::env!("CARGO_WORKSPACE_DIR").into()
}

fn main() -> anyhow::Result<()> {
    env_logger::builder().init();

    let cli = Cli::parse();

    ensure_paths()?;

    if !cli.skip_install_deps {
        install_deps()?;
    }

    match cli.command {
        Command::Build(artifact) => artifact.build()?,
        Command::Test(test) => test.run()?,
        Command::PushCookbook {
            s3_path,
            mut cookbook,
        } => {
            if !have_program("aws") {
                anyhow::bail!("missing 'aws' - please install 'aws' cli tool");
            }

            if cookbook.root_path.is_none() {
                log::warn!("root-path is None - setting to '/guides/mogwai-cookbook'");
                cookbook.root_path = Some(PathBuf::from("/guides/mogwai-cookbook"));
            }
            cookbook.build()?;

            duct::cmd!(
                "aws",
                "s3",
                "sync",
                "cookbook/book/html",
                &s3_path,
                "--acl",
                "public-read"
            )
            .run()?;
        }
        Command::CopyJsFrameworkDist { copy_into } => {
            log::info!("building mogwai-js-framework-benchmark with trunk");
            duct::cmd!(
                "trunk",
                "build",
                "--config",
                "crates/mogwai-js-framework-benchmark/Trunk.toml",
                "--release",
            )
            .run()?;

            let source_dir = workspace_dir().join("crates/mogwai-js-framework-benchmark/dist");
            for entry in std::fs::read_dir(&source_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                log::info!("reading file: '{}'", path.display());
                let (bytes, is_index_html) = match path.extension().and_then(|ext| ext.to_str()) {
                    Some("html") => {
                        let contents = std::fs::read_to_string(&path).unwrap();
                        let contents = contents.replace("/mogwai-js", "./mogwai-js");
                        (contents.into_bytes(), true)
                    }
                    _ => (std::fs::read(&path).unwrap(), false),
                };
                let filename = path.file_name().unwrap().to_str().unwrap();
                let destination = copy_into.join("bundled-dist").join(filename);
                log::info!("copying '{filename}' into '{}'", destination.display());
                std::fs::write(&destination, &bytes).unwrap();
                if is_index_html {
                    let another_destination = copy_into.join(filename);
                    log::info!(
                        "also copying '{filename}' into '{}'",
                        another_destination.display()
                    );
                    std::fs::write(&another_destination, bytes).unwrap();
                }
            }
        }
    }

    Ok(())
}
