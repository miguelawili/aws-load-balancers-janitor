mod cloudwatch;
mod elb;
mod elbv2;
mod models;
mod utils;

use crate::elb::{process_elb as delete_elb, process_region as process_elb, ElbData};
use crate::elbv2::{process_elbv2 as delete_elbv2, process_region as process_elbv2, ElbV2Data};
use crate::models::{ListFormat, LoadBalancerState, RunOption};
use clap::Parser;
use tabled::Table;

#[derive(Parser, Debug)]
#[command(author = "Miguel Awili", version = "1.0.0", about, long_about = None)]
struct Args {
    /// AWS Regions to check
    #[arg(short = 'r', long = "regions")]
    regions: String,

    /// VPC IDs to list/delete
    #[arg(short = 'v', long = "vpc_ids")]
    vpc_ids: String,

    /// Days of metric to check
    #[arg(short = 'd', long = "days")]
    days: i64,

    /// Run option to use;
    /// Currently supports "list" and "delete"
    #[arg(short = 'o', long = "option")]
    run_option: String,

    /// List format
    /// Currently supports "tabled" and "csv"
    #[arg(short = 'f', long = "format")]
    format: String,
}

#[tokio::main]
async fn main() {
    let cli_args = Args::parse();

    let regions = utils::parse_regions_arg(&cli_args.regions);
    let run_option = utils::parse_run_option_arg(&cli_args.run_option);
    let vpc_ids = utils::parse_vpc_ids_arg(&cli_args.vpc_ids);
    let list_format = utils::parse_list_format_arg(&cli_args.format);
    let days = cli_args.days;

    let mut elbv2_tasks = Vec::new();
    let mut elb_tasks = Vec::new();

    for region in regions {
        let elbv2_task = tokio::spawn(process_elbv2(region.clone(), days, vpc_ids.clone()));
        let elb_task = tokio::spawn(process_elb(region.clone(), days, vpc_ids.clone()));
        elbv2_tasks.push(elbv2_task);
        elb_tasks.push(elb_task);
    }

    let mut inactive_elbv2_data: Vec<ElbV2Data> = vec![];
    let mut inactive_elb_data: Vec<ElbData> = vec![];

    for task in elbv2_tasks {
        let elbv2 = task.await.unwrap();

        let mut elbv2 = elbv2
            .into_iter()
            .filter(|elbv2| elbv2.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbV2Data>>();

        inactive_elbv2_data.append(&mut elbv2);
    }

    for task in elb_tasks {
        let elb = task.await.unwrap();

        let mut elb = elb
            .into_iter()
            .filter(|elb| elb.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbData>>();

        inactive_elb_data.append(&mut elb);
    }

    match run_option {
        RunOption::List => match list_format {
            ListFormat::Tabled => {
                let elbv2_table = Table::new(inactive_elbv2_data).to_string();
                let elb_table = Table::new(inactive_elb_data).to_string();

                println!("{}", elbv2_table);
                println!("{}", elb_table);
            }
            ListFormat::Csv => {
                println!("arn,state,region,vpc_id");
                for elbv2_data in inactive_elbv2_data {
                    println!(
                        "{},{},{},{}",
                        elbv2_data.arn, elbv2_data.state, elbv2_data.region, elbv2_data.vpc_id
                    );
                }

                println!("name,state,region,vpc_id");
                for elb_data in inactive_elb_data {
                    println!(
                        "{},{},{},{}",
                        elb_data.name, elb_data.state, elb_data.region, elb_data.vpc_id
                    );
                }
            }
        },
        RunOption::Delete => {
            let mut elbv2_tasks = Vec::new();
            let mut elb_tasks = Vec::new();

            let elbv2_task = tokio::spawn(delete_elbv2(inactive_elbv2_data));
            let elb_task = tokio::spawn(delete_elb(inactive_elb_data));

            elbv2_tasks.push(elbv2_task);
            elb_tasks.push(elb_task);

            futures::future::join_all(elbv2_tasks).await;
            futures::future::join_all(elb_tasks).await;
        }
    }
}
