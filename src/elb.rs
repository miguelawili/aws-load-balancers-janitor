use crate::cloudwatch::get_metric_stats;
use crate::models::LoadBalancerState;
use crate::utils;

use aws_sdk_cloudwatch::{
    model::{Dimension, Metric},
    Client as CloudWatchClient,
};
use aws_sdk_elasticloadbalancing::model::LoadBalancerDescription as LoadBalancer;
use aws_sdk_elasticloadbalancing::Client as ELBClient;
use aws_types::region::Region;
use std::fmt;
use std::sync::{Arc, Mutex};
use tabled::Tabled;

#[derive(Clone, Tabled)]
pub struct ElbData {
    pub name: String,
    pub state: LoadBalancerState,
    pub region: Region,
}

impl fmt::Debug for ElbData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElbData")
            .field("name", &self.name)
            .field("state", &self.state)
            .field("region", &self.region)
            .finish()
    }
}

impl ElbData {
    pub fn new(name: &str, state: LoadBalancerState, region: Region) -> Self {
        ElbData {
            name: name.to_string(),
            state,
            region,
        }
    }
}

pub async fn process_region(region: Region, days: i64, _delete_inactive: bool) -> Vec<ElbData> {
    let config = aws_config::from_env().region(region).load().await;
    let elb_client = ELBClient::new(&config);
    let cw_client = CloudWatchClient::new(&config);

    let elb_lbs = get_elb_load_balancers(&elb_client).await;
    let elb_data: Arc<Mutex<Vec<ElbData>>> = Arc::new(Mutex::new(vec![]));

    let mut tasks = Vec::new();

    for lb in elb_lbs {
        let cw_client = cw_client.clone();

        let lb_name = lb.load_balancer_name().unwrap().to_string();
        let dns_name = lb.dns_name().unwrap().to_string();
        let region_string = utils::extract_region_from_elb_dns(&dns_name).unwrap();
        let region = Region::new(region_string);
        let elb_data = Arc::clone(&elb_data);

        let task = async move {
            println!("Processing ELB: {}", lb_name);
            let state = get_elb_lb_state(lb_name.to_string(), &cw_client, days).await;
            if let Some(state) = state {
                let mut elb_data = elb_data.lock().unwrap();
                elb_data.push(ElbData::new(lb_name.as_str(), state, region));
            }
        };
        tasks.push(task);
    }

    let mut futures = Vec::new();
    for task in tasks {
        futures.push(task);
    }
    futures::future::join_all(futures).await;

    let elb_data = elb_data.lock().unwrap();

    elb_data.to_vec()
}

async fn get_elb_load_balancers(client: &ELBClient) -> Vec<LoadBalancer> {
    let mut lbs = Vec::new();
    let mut next_marker = None;

    loop {
        let resp = client
            .describe_load_balancers()
            .set_marker(next_marker)
            .send()
            .await
            .unwrap();

        lbs.extend(resp.load_balancer_descriptions.unwrap_or_default());
        next_marker = resp.next_marker;
        if next_marker.is_none() {
            break;
        }
    }
    lbs
}

async fn get_elb_lb_state(
    arn: String,
    cw_client: &CloudWatchClient,
    days: i64,
) -> Option<LoadBalancerState> {
    let lb_value = arn.split(':').last().unwrap();

    let dimensions = Dimension::builder()
        .name("LoadBalancerName")
        .value(lb_value.to_string())
        .build();

    let metric = Metric::builder()
        .namespace("AWS/ELB")
        .metric_name("HealthyHostCount")
        .set_dimensions(Some(vec![dimensions]))
        .build();

    let stats = get_metric_stats(&cw_client, metric, days).await;

    match stats {
        Some(stats) => {
            let values = stats.values().unwrap_or(&[]);
            let sum: f64 = values.iter().sum();
            if sum > 0.0 {
                Some(LoadBalancerState::Active)
            } else {
                Some(LoadBalancerState::Inactive)
            }
        }
        None => Some(LoadBalancerState::Inactive),
    }
}

async fn _delete_elb_lb(name: &str, client: &ELBClient) {
    client
        .delete_load_balancer()
        .load_balancer_name(name)
        .send()
        .await
        .unwrap();
    println!("Deleted Classic Load Balancer: {:?}", name);
}
