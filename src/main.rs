use aws_sdk_cloudwatch::types::DateTime as CloudwatchDateTime;
use aws_sdk_cloudwatch::{
    model::{Dimension, Metric, MetricDataQuery, MetricDataResult, MetricStat},
    Client as CloudWatchClient,
};
use aws_sdk_elasticloadbalancing::model::LoadBalancerDescription as LoadBalancer;
use aws_sdk_elasticloadbalancing::Client as ELBClient;
use aws_sdk_elasticloadbalancingv2::model::LoadBalancer as LoadBalancerV2;
use aws_sdk_elasticloadbalancingv2::Client as ELBv2Client;
use aws_smithy_types_convert::date_time::DateTimeExt;
use aws_types::region::Region;
use chrono::{Duration, Utc};
use futures::future::Either;
use std::fmt;
use std::sync::{Arc, Mutex};
use tabled::{Table, Tabled};

#[derive(Clone, PartialEq)]
enum LoadBalancerState {
    Active,
    Inactive,
}

impl fmt::Debug for LoadBalancerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadBalancerState::Active => write!(f, "Active"),
            LoadBalancerState::Inactive => write!(f, "Inactive"),
        }
    }
}

impl fmt::Display for LoadBalancerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadBalancerState::Active => write!(f, "Active"),
            LoadBalancerState::Inactive => write!(f, "Inactive"),
        }
    }
}

#[tokio::main]
async fn main() {
    let days: i64 = 45;
    let regions = vec![Region::new("ap-southeast-1"), Region::new("us-west-2")];
    let delete_inactive = false;

    let mut tasks = Vec::new();

    for region in regions {
        let task = tokio::spawn(process_region(region, days, delete_inactive));
        tasks.push(task);
    }

    let mut inactive_elbv2_data: Vec<ElbV2Data> = vec![];
    let mut inactive_elb_data: Vec<ElbData> = vec![];

    for task in tasks {
        let (elbv2, elb) = task.await.unwrap();

        let mut elbv2 = elbv2
            .into_iter()
            .filter(|elbv2| elbv2.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbV2Data>>();
        let mut elb = elb
            .into_iter()
            .filter(|elb| elb.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbData>>();

        inactive_elbv2_data.append(&mut elbv2);
        inactive_elb_data.append(&mut elb);
    }

    let elbv2_table = Table::new(&inactive_elbv2_data).to_string();
    let elb_table = Table::new(&inactive_elb_data).to_string();

    println!("{}", elbv2_table);
    println!("{}", elb_table);

    // println!("======== INACTIVE LIST ========");
    // println!("---------- ALB / NLB -----------");
    // for lb in inactive_elbv2_data {
    //     println!("{:?}", lb);
    // }
    // println!("--------- Classic ELB ---------");
    // for lb in inactive_elb_data {
    //     println!("{:?}", lb);
    // }
}

#[derive(Clone, Tabled)]
struct ElbV2Data {
    arn: String,
    state: LoadBalancerState,
    region: Region,
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
    fn new(arn: &str, state: LoadBalancerState, region: Region) -> Self {
        ElbV2Data {
            arn: arn.to_string(),
            state,
            region,
        }
    }
}

#[derive(Clone, Tabled)]
struct ElbData {
    name: String,
    state: LoadBalancerState,
    region: Region,
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
    fn new(name: &str, state: LoadBalancerState, region: Region) -> Self {
        ElbData {
            name: name.to_string(),
            state,
            region,
        }
    }
}

async fn process_region(
    region: Region,
    days: i64,
    _delete_inactive: bool,
) -> (Vec<ElbV2Data>, Vec<ElbData>) {
    let config = aws_config::from_env().region(region).load().await;
    let elbv2_client = ELBv2Client::new(&config);
    let elb_client = ELBClient::new(&config);
    let cw_client = CloudWatchClient::new(&config);

    let elbv2_lbs = get_elbv2_load_balancers(&elbv2_client).await;
    let elb_lbs = get_elb_load_balancers(&elb_client).await;
    let elbv2_data: Arc<Mutex<Vec<ElbV2Data>>> = Arc::new(Mutex::new(vec![]));
    let elb_data: Arc<Mutex<Vec<ElbData>>> = Arc::new(Mutex::new(vec![]));

    let mut tasks = Vec::new();

    for lb in elbv2_lbs {
        let client = elbv2_client.clone();
        let cw_client = cw_client.clone();

        let arn = lb.load_balancer_arn().unwrap().to_string();
        let region_string = extract_region_from_elbv2_arn(&arn).unwrap();
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
        tasks.push(Either::Left(task));
    }

    for lb in elb_lbs {
        let cw_client = cw_client.clone();

        let lb_name = lb.load_balancer_name().unwrap().to_string();
        let dns_name = lb.dns_name().unwrap().to_string();
        let region_string = extract_region_from_elb_dns(&dns_name).unwrap();
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
        tasks.push(Either::Right(task));
    }

    let mut left_futures = Vec::new();
    let mut right_futures = Vec::new();
    for task in tasks {
        match task {
            Either::Left(task) => {
                left_futures.push(task);
            }
            Either::Right(task) => {
                right_futures.push(task);
            }
        }
    }
    futures::future::join_all(left_futures).await;
    futures::future::join_all(right_futures).await;

    let elbv2_data = elbv2_data.lock().unwrap();
    let elb_data = elb_data.lock().unwrap();

    (elbv2_data.to_vec(), elb_data.to_vec())
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

    let lb_value = extract_id_from_lb_arn(&arn).unwrap();
    let mut active = false;

    for tg in target_groups {
        let tg_arn = tg.target_group_arn().unwrap();
        let tg_value = extract_id_from_tg_arn(&tg_arn).unwrap();

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

async fn get_metric_stats(
    cw_client: &CloudWatchClient,
    metric: Metric,
    days: i64,
) -> Option<MetricDataResult> {
    let start_time = Utc::now() - Duration::days(days);
    let end_time = Utc::now();
    let start_time: CloudwatchDateTime = CloudwatchDateTime::from_chrono_utc(start_time);
    let end_time: CloudwatchDateTime = CloudwatchDateTime::from_chrono_utc(end_time);

    let metric_data_query = MetricDataQuery::builder()
        .id("m1")
        .metric_stat(
            MetricStat::builder()
                .metric(metric.clone())
                .period(60)
                .stat("Minimum")
                .build(),
        )
        .build();

    let response = cw_client
        .get_metric_data()
        .metric_data_queries(metric_data_query)
        .start_time(start_time)
        .end_time(end_time)
        .send()
        .await;

    match response {
        Ok(output) => {
            if let Some(metric_data_results) = output.metric_data_results() {
                if let Some(metric_data_result) = metric_data_results.first() {
                    return Some(metric_data_result.clone());
                }
            }
            None
        }
        Err(e) => {
            eprintln!("Error getting metric stats: {}", e);
            None
        }
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

async fn _delete_elb_lb(name: &str, client: &ELBClient) {
    client
        .delete_load_balancer()
        .load_balancer_name(name)
        .send()
        .await
        .unwrap();
    println!("Deleted Classic Load Balancer: {:?}", name);
}

fn extract_id_from_lb_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 6 {
        let sub_parts: Vec<&str> = parts[5].split('/').skip(1).collect();
        Some(sub_parts.join("/"))
    } else {
        None
    }
}

fn extract_id_from_tg_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 6 {
        let sub_parts: Vec<&str> = parts[5].split('/').collect();
        Some(sub_parts.join("/"))
    } else {
        None
    }
}

fn extract_region_from_elbv2_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 4 {
        Some(parts[3].to_string())
    } else {
        None
    }
}

fn extract_region_from_elb_dns(dns_name: &str) -> Option<String> {
    let parts: Vec<&str> = dns_name.split('.').collect();
    if parts.len() > 2 {
        Some(parts[1].to_string())
    } else {
        None
    }
}
