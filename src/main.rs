mod cloudwatch;
mod elb;
mod elbv2;
mod models;
mod utils;

use clap::Parser;
use elb::process_account as process_elbs;
use elbv2::process_account as process_elbv2s;
use models::AppConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(short = 'c', long = "--config-file")]
    config_file: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let conf = AppConfig::new(&args.config_file);

    // dbg!("conf {}", conf);
    let mut elbv2_tasks = Vec::new();
    let mut elb_tasks = Vec::new();

    for aws_account in conf.aws.accounts {
        let days = conf.days;
        let run_option = conf.run_option.clone();
        let iam_role = aws_account.iam_role.clone();
        let regions = aws_account.regions.clone();
        let vpc_ids = aws_account.vpc_ids.clone();

        let elbv2_task = tokio::spawn(async move {
            process_elbv2s(run_option, days, iam_role.as_str(), vpc_ids, regions).await;
        });

        let days = conf.days;
        let run_option = conf.run_option.clone();
        let iam_role = aws_account.iam_role.clone();
        let regions = aws_account.regions.clone();
        let vpc_ids = aws_account.vpc_ids.clone();

        let elb_task = tokio::spawn(async move {
            process_elbs(run_option, days, iam_role.as_str(), vpc_ids, regions).await;
        });

        elbv2_tasks.push(elbv2_task);
        elb_tasks.push(elb_task);
    }

    futures::future::join_all(elbv2_tasks).await;
    futures::future::join_all(elb_tasks).await;
}
