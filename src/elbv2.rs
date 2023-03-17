use crate::cloudwatch::get_metric_stats;
use crate::models::LoadBalancerState;
use crate::utils;

use aws_sdk_cloudwatch::{
    model::{Dimension, Metric},
    Client as CloudWatchClient,
};
use aws_sdk_elasticloadbalancingv2::model::LoadBalancer as LoadBalancerV2;
use aws_sdk_elasticloadbalancingv2::Client as ELBv2Client;
use aws_types::region::Region;
use std::fmt;
use std::sync::{Arc, Mutex};
use tabled::Tabled;

#[derive(Clone, Tabled)]
pub struct ElbV2Data {
    pub arn: String,
    pub state: LoadBalancerState,
    pub region: Region,
}

impl fmt::Debug for ElbV2Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElbV2Data")
            .field("name", &self.arn)
            .field("state", &self.state)
            .field("region", &self.region)
            .finish()
    }
}

impl ElbV2Data {
    pub fn new(arn: &str, state: LoadBalancerState, region: Region) -> Self {
        ElbV2Data {
            arn: arn.to_string(),
            state,
            region,
        }
    }
}

pub async fn process_region(region: Region, days: i64, _delete_inactive: bool) -> Vec<ElbV2Data> {
    let config = aws_config::from_env().region(region).load().await;
    let elbv2_client = ELBv2Client::new(&config);
    let cw_client = CloudWatchClient::new(&config);

    let elbv2_lbs = get_elbv2_load_balancers(&elbv2_client).await;
    let elbv2_data: Arc<Mutex<Vec<ElbV2Data>>> = Arc::new(Mutex::new(vec![]));

    let mut tasks = Vec::new();

    for lb in elbv2_lbs {
        let client = elbv2_client.clone();
        let cw_client = cw_client.clone();

        let arn = lb.load_balancer_arn().unwrap().to_string();
        let region_string = utils::extract_region_from_elbv2_arn(&arn).unwrap();
        let region = Region::new(region_string);
        let elbv2_data = Arc::clone(&elbv2_data);

        let task = async move {
            println!("Processing ELBv2: {}", arn);
            let state = get_elbv2_lb_state(arn.to_string(), &client, &cw_client, days).await;
            if let Some(state) = state {
                let mut elbv2_data = elbv2_data.lock().unwrap();
                elbv2_data.push(ElbV2Data::new(arn.as_str(), state, region));
            }
        };
        tasks.push(task);
    }

    let mut futures = Vec::new();
    for task in tasks {
        futures.push(task);
    }
    futures::future::join_all(futures).await;

    let elbv2_data = elbv2_data.lock().unwrap();

    elbv2_data.to_vec()
}

async fn get_elbv2_load_balancers(client: &ELBv2Client) -> Vec<LoadBalancerV2> {
    let mut lbs = Vec::new();
    let mut next_marker = None;

    loop {
        let resp = client
            .describe_load_balancers()
            .set_marker(next_marker)
            .send()
            .await
            .unwrap();

        lbs.extend(resp.load_balancers.unwrap_or_default());
        next_marker = resp.next_marker;
        if next_marker.is_none() {
            break;
        }
    }
    lbs
}

async fn get_elbv2_lb_state(
    arn: String,
    elbv2_client: &ELBv2Client,
    cw_client: &CloudWatchClient,
    days: i64,
) -> Option<LoadBalancerState> {
    let target_groups = elbv2_client
        .describe_target_groups()
        .load_balancer_arn(arn.clone())
        .send()
        .await
        .unwrap()
        .target_groups
        .unwrap_or_default();

    let lb_value = utils::extract_id_from_lb_arn(&arn).unwrap();
    let mut active = false;

    for tg in target_groups {
        let tg_arn = tg.target_group_arn().unwrap();
        let tg_value = utils::extract_id_from_tg_arn(&tg_arn).unwrap();

        let dimensions = vec![
            Dimension::builder()
                .name("LoadBalancer")
                .value(lb_value.clone())
                .build(),
            Dimension::builder()
                .name("TargetGroup")
                .value(tg_value.clone())
                .build(),
        ];

        let metric = Metric::builder()
            .namespace("AWS/ApplicationELB")
            .metric_name("HealthyHostCount")
            .set_dimensions(Some(dimensions))
            .build();

        let stats = get_metric_stats(&cw_client, metric, days).await;

        match stats {
            Some(stats) => {
                let values = stats.values().unwrap_or(&[]);
                let sum: f64 = values.iter().sum();
                if sum > 0.0 {
                    active = true;
                    break;
                }
            }
            None => (),
        }
    }

    if active {
        Some(LoadBalancerState::Active)
    } else {
        Some(LoadBalancerState::Inactive)
    }
}

async fn _delete_elbv2_lb(arn: &str, client: &ELBv2Client) {
    client
        .delete_load_balancer()
        .load_balancer_arn(arn)
        .send()
        .await
        .unwrap();
    println!("Deleted ELBv2 Load Balancer: {:?}", arn);
}
