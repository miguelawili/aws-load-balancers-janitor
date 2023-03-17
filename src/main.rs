mod cloudwatch;
mod elb;
mod elbv2;
mod models;
mod utils;

use crate::elb::{process_region as process_elb, ElbData};
use crate::elbv2::{process_region as process_elbv2, ElbV2Data};
use crate::models::LoadBalancerState;
use aws_types::region::Region;
use tabled::Table;

#[tokio::main]
async fn main() {
    let days: i64 = 45;
    let regions = vec![Region::new("ap-southeast-1"), Region::new("us-west-2")];
    let delete_inactive = false;

    let mut elbv2_tasks = Vec::new();
    let mut elb_tasks = Vec::new();

    for region in regions {
        let elbv2_task = tokio::spawn(process_elbv2(region.clone(), days, delete_inactive));
        let elb_task = tokio::spawn(process_elb(region.clone(), days, delete_inactive));
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

    let elbv2_table = Table::new(inactive_elbv2_data).to_string();
    let elb_table = Table::new(inactive_elb_data).to_string();

    println!("{}", elbv2_table);
    println!("{}", elb_table);
}
