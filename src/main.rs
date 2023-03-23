// mod cloudwatch;
// mod elb;
// mod elbv2;
mod models;
mod utils;

use crate::models::AppConfig;

#[tokio::main]
async fn main() {
    let conf = AppConfig::new("./config/maya.toml");
    dbg!("conf {}", conf);
}
