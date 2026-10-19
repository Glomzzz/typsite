use std::fs;
use std::num::NonZeroUsize;

use crate::compile::clean::clean_roots;
use crate::compile::compiler::Compiler;
use crate::compile::options::CompileOptions;
use crate::compile::pdf::{PdfCompiler, PdfOptions};
use crate::config::highlight::CodeHightlightConfig;
use crate::resource::default::copy_default_typsite;
use anyhow::{Context, Result, ensure};
use clap::Parser;
use std::path::{Path, PathBuf};

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
            Command::Pdf(cmd) => Self::execute_pdf(cmd).await,
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
        let cache_path = PathBuf::from(cmd.cache);
        let config_path = PathBuf::from(cmd.config);
        let input_path = PathBuf::from(cmd.input);
        let output_path = PathBuf::from(cmd.output);
        let packages_path = (!cmd.packages.is_empty())
            .then(|| PathBuf::from(cmd.packages))
            .filter(|path| path.is_dir());
        let typst = cmd.typst;

        println!(
            "  - Packages: {}",
            packages_path
                .as_ref()
                .map(|it| format!("included + {it:?}"))
                .unwrap_or("included".to_string())
        );
        println!("  - Cache dir: {cache_path:?}");
        println!("  - Config dir: {config_path:?}");
        println!("  - Input dir: {input_path:?}");
        println!("  - Output dir: {output_path:?}");
        if typst != "typst" {
            println!("  - Typst executable: {typst}")
        }

        let config = CompileOptions {
            watch: cmd.port != 0,
            short_slug: !cmd.no_short_slug,
            pretty_url: !cmd.no_pretty_url,
        };
        let mut compiler = Compiler::new(
            config,
            cache_path,
            config_path,
            input_path,
            output_path,
            packages_path,
            typst,
        )?;
        if let Some(jobs) = cmd.jobs {
            compiler = compiler.with_jobs(jobs);
        }
        Ok(compiler)
    }

    fn execute_clean(clean_cmd: CleanCmd) -> Result<()> {
        println!("Start cleaning...");
        clean_roots(
            Path::new(&clean_cmd.cache),
            Path::new(&clean_cmd.output),
            &[Path::new(&clean_cmd.input), Path::new(&clean_cmd.config)],
            clean_cmd.force,
        )?;
        println!("Cleaning done.");
        Ok(())
    }

    async fn execute_compile(compile_cmd: CompileCmd) -> Result<()> {
        let host = compile_cmd.host.clone();
        let port = compile_cmd.port;
        let compiler = Self::build_compiler(compile_cmd)?;
        match port {
            0 => {
                println!("Start compiling...");
                let report = tokio::task::spawn_blocking(move || compiler.compile())
                    .await
                    .context("Build task panicked")??;
                for diagnostic in &report.diagnostics {
                    eprintln!(
                        "{:?} {:?}: {}",
                        diagnostic.stage, diagnostic.path, diagnostic.message
                    );
                }
                if report.is_success() {
                    println!(
                        "Compiling done: {} exported, {} rendered, {} written, {} removed.",
                        report.exported.len(),
                        report.rendered,
                        report.written.len(),
                        report.removed.len()
                    );
                }
                ensure!(report.is_success(), "Compilation failed");
            }
            _ => {
                println!("Start watching...");
                compiler.watch(host, port).await?;
            }
        }
        Ok(())
    }

    async fn execute_pdf(cmd: PdfCmd) -> Result<()> {
        let compiler = PdfCompiler::new(
            cmd.input,
            cmd.config,
            cmd.cache,
            cmd.output.clone(),
            cmd.packages,
            PdfOptions {
                entry: cmd.entry,
                appendices: cmd.appendix,
                template: cmd.template,
                title: cmd.title,
                pretty_url: !cmd.no_pretty_url,
            },
            cmd.typst,
        )?;
        let compiler = if let Some(jobs) = cmd.jobs {
            compiler.with_jobs(jobs)
        } else {
            compiler
        };
        let report = tokio::task::spawn_blocking(move || compiler.compile())
            .await
            .context("PDF build task panicked")??;
        println!(
            "PDF {}: {} ({}).",
            if report.written {
                "written"
            } else {
                "unchanged"
            },
            cmd.output.display(),
            if report.compiled {
                "compiled"
            } else {
                "cached"
            }
        );
        Ok(())
    }

    fn execute_syntect(syntect_cmd: SyntectCmd) -> Result<()> {
        let config_path = Path::new(&syntect_cmd.config);
        let config = CodeHightlightConfig::load(config_path)?;
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

    /// Compile an entry article and its embeds into one native PDF book.
    Pdf(PdfCmd),

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

    /// Packages dir, will be installed to @local and will be synced to @local in watch mode, skip if empty or not found
    #[arg(short, long, default_value_t = format!(""), visible_alias = "p")]
    packages: String,

    /// Maximum concurrent article exports (default: available CPU/Rayon budget).
    #[arg(short = 'j', long)]
    jobs: Option<NonZeroUsize>,

    /// Typst executable path
    #[arg(short, long, default_value_t = format!("typst"), visible_alias = "t")]
    typst: String,

    // Pretty URL, remove the .html suffix from the URL, for example, /about.html -> /about
    #[arg(long, default_value_t = false)]
    no_pretty_url: bool,

    // Short slug, hide parent slug in the displayed slug, for example, /tutorials/install -> /install
    #[arg(long, default_value_t = false)]
    no_short_slug: bool,
}
#[derive(clap::Args)]
struct PdfCmd {
    /// Typst source root.
    #[arg(short, long, default_value = "./root")]
    input: PathBuf,
    #[arg(long, default_value = "./.typsite", alias = "cfg")]
    config: PathBuf,
    #[arg(long, default_value = "./.cache")]
    cache: PathBuf,
    /// One PDF file, separate from the HTML output set.
    #[arg(short, long, default_value = "./book/site.pdf")]
    output: PathBuf,
    /// Entry source path, relative to --input.
    #[arg(long, default_value = "index.typ")]
    entry: String,
    /// Additional appendix root; repeat to preserve a chosen order.
    #[arg(long)]
    appendix: Vec<String>,
    /// Module inside --input exporting book-style(body).
    #[arg(long)]
    template: Option<String>,
    /// PDF document title (defaults to site.title).
    #[arg(long)]
    title: Option<String>,
    #[arg(short, long)]
    packages: Option<PathBuf>,
    #[arg(short = 'j', long)]
    jobs: Option<NonZeroUsize>,
    #[arg(short, long, default_value = "typst")]
    typst: String,
    /// Keep .html on website links to articles outside the book.
    #[arg(long)]
    no_pretty_url: bool,
}

#[derive(clap::Args)]
struct SyntectCmd {
    /// Project config path
    #[arg(long, default_value_t = format!("./.typsite"), alias = "cfg")]
    config: String,
}

#[derive(clap::Args)]
pub struct CleanCmd {
    /// Protected Typst input directory (never cleaned).
    #[arg(long, default_value = "./root")]
    input: String,

    /// Protected configuration directory (never cleaned).
    #[arg(long, default_value = "./.typsite", alias = "cfg")]
    config: String,

    /// Allow cleaning unmarked legacy cache/output directories after checking paths.
    #[arg(long)]
    force: bool,

    /// Output dir.
    #[arg(short, long, default_value_t = format!("./publish"))]
    output: String,

    /// Cache dir, where the raw typst_html_export will be stored.
    #[arg(short, long, default_value_t = format!("./.cache"))]
    cache: String,
}
