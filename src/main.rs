#[tokio::main]
async fn main() -> anyhow::Result<()> {
    typsite::cli::cli().await
}
