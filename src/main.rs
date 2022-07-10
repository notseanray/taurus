use taurus::run;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    run().await;
}
