#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    typsite::cli::cli().await
}
