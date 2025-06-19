use std::{env, fs, process::exit};

use crate::{compile::compiler::Compiler, util::path::verify_if_relative_path};
use crate::compile::options::CompileOptions;
use crate::config::highlight::CodeHightlightConfig;
use crate::config::resources::copy_default_typsite;
use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;

pub async fn cli() -> Result<()> {
    Executor::execute(Cli::parse().command).await
}

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

struct Executor;
impl Executor {
    async fn execute(command: Command) -> Result<()> {
        match command {
            Command::Init(init_cmd) => Self::execute_init(init_cmd),
            Command::Compile(compile_cmd) => Self::execute_compile(compile_cmd).await,
            Command::Clean(clean_cmd) => Self::execute_clean(clean_cmd),
            Command::Syntect(syntect_cmd) => Self::execute_syntect(syntect_cmd),
        }
    }

    fn execute_init(init_cmd: InitCmd) -> Result<()> {
        let root = Path::new(init_cmd.dir.as_str());
        let config = root.join(".typsite");
        if config.exists() && fs::read_dir(root)?.next().is_some() {
            println!("Project config directory {config:?} is not empty, cancel the init");
            return Ok(());
        }
        copy_default_typsite(root).context("Failed to initialize project")?;
        println!("Project initialized in {root:?}");
        Ok(())
    }

    fn build_compiler(cmd: CompileCmd) -> Result<Compiler> {
        println!("Preparing compiler...");
        let cwd = env::current_dir().context("Failed to get current work dir")?;
        let cache_path = Path::new(cmd.cache.as_str());
        let config_path = Path::new(cmd.config.as_str());
        let input_path = Path::new(cmd.input.as_str());
        let output_path = Path::new(cmd.output.as_str());

        let cache_path = verify_if_relative_path(&cwd, cache_path)?;
        let config_path = verify_if_relative_path(&cwd, config_path)?;
        let input_path = verify_if_relative_path(&cwd, input_path)?;
        let output_path = verify_if_relative_path(&cwd, output_path)?;

        println!("  - Cache dir: {cache_path:?}");
        println!("  - Config dir: {config_path:?}");
        println!("  - Input dir: {input_path:?}");
        println!("  - Output dir: {output_path:?}");


        let config = CompileOptions {
            watch: cmd.port != 0,
            short_slug: !cmd.no_short_slug,
            pretty_url: !cmd.no_pretty_url,
        };
        let compiler = Compiler::new(config, cache_path, config_path, input_path, output_path)?;
        Ok(compiler)
    }

    fn execute_clean(clean_cmd: CleanCmd) -> Result<()> {
        println!("Start cleaning...");
        let cache = Path::new(clean_cmd.cache.as_str());
        Self::clean(cache)?;
        let output = Path::new(clean_cmd.output.as_str());
        Self::clean(output)?;
        println!("Cleaning done.");
        Ok(())
    }

    fn clean(path: &Path) -> Result<()> {
        if path.exists() {
            println!("  - Cleaning dir: {path:?}");
            fs::remove_dir_all(path).context(format!("Failed to clean {path:?}"))?;
        }
        Ok(())
    }

    async fn execute_compile(compile_cmd: CompileCmd) -> Result<()> {
        let host = compile_cmd.host.clone();
        let port = compile_cmd.port;
        let compiler = Self::build_compiler(compile_cmd)?;
        match port {
            0 => {
                println!("Start compiling...");
                if let (_, true) = compiler.compile()? {
                    println!("Compiling done.");
                } else {
                    exit(1);
                }
            }
            _ => {
                println!("Start watching...");
                Self::clean(&compiler.cache_path)?;
                Self::clean(&compiler.output_path)?;
                compiler.watch(host, port).await?;
            }
        }
        Ok(())
    }

    fn execute_syntect(syntect_cmd: SyntectCmd) -> Result<()> {
        let config_path = Path::new(&syntect_cmd.config);
        let config = CodeHightlightConfig::load(config_path);
        println!("{config}");

        Ok(())
    }
}

#[derive(clap::Subcommand)]
enum Command {
    /// Initialize a new typsite in the specified directory.
    Init(InitCmd),

    /// Compile or watch the project with specified input and output directories.
    #[command(visible_alias = "c")]
    Compile(CompileCmd),

    /// Clean the cache & output directory.
    Clean(CleanCmd),

    /// Check syntect syntaxes & themes
    #[command(visible_alias = "s")]
    Syntect(SyntectCmd),
}

#[derive(clap::Args)]
struct InitCmd {
    /// Project root directory.
    #[arg(short, long, default_value_t = format!("./"))]
    dir: String,
}

#[derive(clap::Args)]
struct CompileCmd {
    /// Serve host
    #[arg(long, default_value_t = format!("localhost"), alias = "h")]
    host: String,
    /// Serve port, must be specified to watch mode
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Project config
    #[arg(long, default_value_t = format!("./.typsite"), alias = "cfg")]
    config: String,

    /// Cache dir
    #[arg(long, default_value_t = format!("./.cache"))]
    cache: String,

    /// Typst root dir, where your typst files are stored.
    #[arg(short, long, default_value_t = format!("./root"), visible_alias = "i")]
    input: String,

    /// Output dir.
    #[arg(short, long, default_value_t = format!("./publish"), visible_alias = "o")]
    output: String,

    // Pretty URL, remove the .html suffix from the URL, for example, /about.html -> /about
    #[arg(long, default_value_t = false)]
    no_pretty_url: bool,

    // Short slug, hide parent slug in the displayed slug, for example, /tutorials/install -> /install
    #[arg(long, default_value_t = false)]
    no_short_slug: bool,
}
#[derive(clap::Args)]
struct SyntectCmd {
    /// Project config path
    #[arg(long, default_value_t = format!("./.typsite"), alias = "cfg")]
    config: String,
}

#[derive(clap::Args)]
pub struct CleanCmd {
    /// Output dir.
    #[arg(short, long, default_value_t = format!("./publish"))]
    output: String,

    /// Cache dir, where the raw typst_html_export will be stored.
    #[arg(short, long, default_value_t = format!("./.cache"))]
    cache: String,
}
