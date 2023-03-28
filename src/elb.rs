use crate::cloudwatch::get_metric_stats;
use crate::models::{LoadBalancerState, RunOption};
use crate::utils;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatch::{
    model::{Dimension, Metric},
    Client as CloudWatchClient,
};
use aws_sdk_elasticloadbalancing::model::LoadBalancerDescription as LoadBalancer;
use aws_sdk_elasticloadbalancing::output::DeleteLoadBalancerOutput as DeleteOutput;
use aws_sdk_elasticloadbalancing::Client as ELBClient;
use aws_sdk_iam::Credentials;
use aws_sdk_sts::types::DateTime as StsDateTime;
use aws_sdk_sts::Client as StsClient;
use aws_types::region::Region;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct ElbData {
    pub name: String,
    pub state: LoadBalancerState,
    pub region: Region,
    pub vpc_id: String,
}

impl fmt::Debug for ElbData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElbData")
            .field("name", &self.name)
            .field("state", &self.state)
            .field("region", &self.region)
            .field("vpc_id", &self.vpc_id)
            .finish()
    }
}

impl ElbData {
    pub fn new(name: &str, state: LoadBalancerState, region: Region, vpc_id: String) -> Self {
        ElbData {
            name: name.to_string(),
            state,
            region,
            vpc_id,
        }
    }

    pub fn to_string(&mut self) -> String {
        format!(
            "{},{},{},{}",
            self.name, self.state, self.region, self.vpc_id
        )
    }
}

pub async fn process_account(
    account_id: &str,
    run_option: RunOption,
    days: i64,
    iam_role: &str,
    vpc_ids: Vec<String>,
    regions: Vec<String>,
) -> () {
    let regions = utils::parse_regions_arg(&regions);
    let vpc_ids = utils::parse_vpc_ids_arg(&vpc_ids);

    let region_provider = RegionProviderChain::default_provider().or_else("ap-southeast-1");

    let config = aws_config::from_env().region(region_provider).load().await;
    let sts_client: StsClient = StsClient::new(&config);

    let assumed_role = sts_client
        .assume_role()
        .role_arn(iam_role)
        .role_session_name(&format!("lb_janitor_assumerole_session"))
        .send()
        .await;

    let assumed_role = assumed_role.unwrap();
    let credentials = assumed_role.credentials().unwrap();
    let access_key_id = credentials.access_key_id().unwrap();
    let secret_access_key = credentials.secret_access_key().unwrap();
    let session_token = credentials.session_token().unwrap();
    let expiry: StsDateTime = *credentials.expiration().unwrap();
    let expiry: SystemTime = SystemTime::try_from(expiry).unwrap();

    let credentials = Credentials::new(
        access_key_id,
        secret_access_key,
        Some(session_token.to_string()),
        Some(expiry),
        "AWS",
    );

    let mut tasks = Vec::new();
    let mut inactive_elb_data: Vec<ElbData> = vec![];

    for region in regions {
        let elb_task = tokio::spawn(process_region(
            region,
            credentials.clone(),
            days,
            vpc_ids.clone(),
        ));
        tasks.push(elb_task);
    }

    for task in tasks {
        let elb = task.await.unwrap();

        let mut elb = elb
            .into_iter()
            .filter(|elb| elb.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbData>>();

        inactive_elb_data.append(&mut elb);
    }

    match run_option {
        RunOption::List => {
            let mut to_write: Vec<String> = vec![];
            to_write.push("name,state,region,vpc_id".to_string());
            for elb_data in inactive_elb_data.iter_mut() {
                let line = elb_data.to_string();
                to_write.push(line);
            }

            let file_name = format!("outputs/{}_inactive_elbs.csv", &account_id);
            if let Err(e) = utils::write_csv(file_name.as_str(), to_write) {
                println!("Error writing to csv file! {}", e);
            }
        }
        RunOption::Delete => {
            let mut tasks = Vec::new();

            let elb_task = tokio::spawn(process_elb(inactive_elb_data));

            tasks.push(elb_task);

            futures::future::join_all(tasks).await;
        }
        RunOption::Unknown => {
            panic!("Run option invalid!");
        }
    }
}

pub async fn process_region(
    region: Region,
    credentials: Credentials,
    days: i64,
    vpc_ids: HashMap<String, bool>,
) -> Vec<ElbData> {
    let config = aws_config::from_env()
        .credentials_provider(credentials)
        .region(region)
        .load()
        .await;

    let elb_client = ELBClient::new(&config);
    let cw_client = CloudWatchClient::new(&config);

    let elb_lbs = get_elb_load_balancers(&elb_client).await;
    let elb_data: Arc<Mutex<Vec<ElbData>>> = Arc::new(Mutex::new(vec![]));
    let sem = Arc::new(Semaphore::new(10));

    let mut tasks = Vec::new();

    for lb in elb_lbs {
        let cw_client = cw_client.clone();
        let lb_name = lb.load_balancer_name().unwrap().to_string();
        let vpc_ids = vpc_ids.clone();
        let vpc_id = lb.vpc_id().unwrap().to_string();
        let dns_name = lb.dns_name().unwrap().to_string();

        let region_string = utils::extract_region_from_elb_dns(&dns_name).unwrap();
        let region = Region::new(region_string);
        let elb_data = Arc::clone(&elb_data);
        let sem = Arc::clone(&sem);

        let task = async move {
            let _perm = sem.acquire_owned().await;
            println!("Processing ELB: {}", lb_name);
            let state = get_elb_lb_state(lb_name.to_string(), &cw_client, days).await;
            if let Some(state) = state {
                let mut elb_data = elb_data.lock().unwrap();
                if vpc_ids.len() > 0 && vpc_ids.contains_key(vpc_id.as_str()) {
                    elb_data.push(ElbData::new(lb_name.as_str(), state, region, vpc_id));
                } else {
                    elb_data.push(ElbData::new(lb_name.as_str(), state, region, vpc_id));
                }
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

pub async fn process_elb(elbs: Vec<ElbData>) -> Vec<DeleteOutput> {
    let deletion_results: Arc<Mutex<Vec<DeleteOutput>>> = Arc::new(Mutex::new(vec![]));
    let mut tasks = Vec::new();

    for elb in elbs {
        let region = elb.region;
        let name = elb.name;

        let config = aws_config::from_env().region(region).load().await;
        let client = ELBClient::new(&config);

        let deletion_results = Arc::clone(&deletion_results);

        let task = async move {
            println!("Processing ELB deletion: {}", name);
            let res = delete_elb(&name, &client).await;
            let mut deletion_results = deletion_results.lock().unwrap();
            deletion_results.push(res);
        };

        tasks.push(task);
    }

    let mut futures = Vec::new();
    for task in tasks {
        futures.push(task);
    }
    futures::future::join_all(futures).await;

    let deletion_results = deletion_results.lock().unwrap();
    deletion_results.to_vec()
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

async fn delete_elb(name: &str, client: &ELBClient) -> DeleteOutput {
    let out = client
        .delete_load_balancer()
        .load_balancer_name(name)
        .send()
        .await
        .unwrap();
    println!("Deleted Classic Load Balancer: {:?}", name);
    out
}
