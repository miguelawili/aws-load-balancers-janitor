use crate::cloudwatch::get_metric_stats;
use crate::models::{LoadBalancerState, RunOption};
use crate::utils;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatch::{
    model::{Dimension, Metric},
    Client as CloudWatchClient,
};
use aws_sdk_elasticloadbalancingv2::model::LoadBalancer as LoadBalancerV2;
use aws_sdk_elasticloadbalancingv2::output::DeleteLoadBalancerOutput as DeleteOutput;
use aws_sdk_elasticloadbalancingv2::Client as ELBv2Client;
use aws_sdk_iam::Credentials;
use aws_sdk_sts::types::DateTime as StsDateTime;
use aws_sdk_sts::Client as StsClient;
use aws_types::region::Region;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct ElbV2Data {
    pub arn: String,
    pub state: LoadBalancerState,
    pub region: Region,
    pub vpc_id: String,
}

impl fmt::Debug for ElbV2Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElbV2Data")
            .field("name", &self.arn)
            .field("state", &self.state)
            .field("region", &self.region)
            .field("vpc_id", &self.vpc_id)
            .finish()
    }
}

impl ElbV2Data {
    pub fn new(arn: &str, state: LoadBalancerState, region: Region, vpc_id: String) -> Self {
        ElbV2Data {
            arn: arn.to_string(),
            state,
            region,
            vpc_id,
        }
    }
}

pub async fn process_account(
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
    let mut inactive_elbv2_data: Vec<ElbV2Data> = vec![];

    for region in regions {
        let elbv2_task = tokio::spawn(process_region(
            region,
            credentials.clone(),
            days,
            vpc_ids.clone(),
        ));
        tasks.push(elbv2_task);
    }

    for task in tasks {
        let elbv2 = task.await.unwrap();

        let mut elbv2 = elbv2
            .into_iter()
            .filter(|elbv2| elbv2.state == LoadBalancerState::Inactive)
            .collect::<Vec<ElbV2Data>>();

        inactive_elbv2_data.append(&mut elbv2);
    }

    match run_option {
        RunOption::List => {
            println!("=======================");
            println!("arn,state,region,vpc_id");
            println!("-----------------------");
            for elbv2_data in &inactive_elbv2_data {
                println!(
                    "{},{},{},{}",
                    elbv2_data.arn, elbv2_data.state, elbv2_data.region, elbv2_data.vpc_id
                );
            }
        }
        RunOption::Delete => {
            let mut tasks = Vec::new();

            let elbv2_task = tokio::spawn(process_elbv2(inactive_elbv2_data));

            tasks.push(elbv2_task);

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
) -> Vec<ElbV2Data> {
    let config = aws_config::from_env()
        .credentials_provider(credentials)
        .region(region)
        .load()
        .await;

    let elbv2_client = ELBv2Client::new(&config);
    let cw_client = CloudWatchClient::new(&config);

    let elbv2_lbs = get_elbv2_load_balancers(&elbv2_client).await;
    let elbv2_data: Arc<Mutex<Vec<ElbV2Data>>> = Arc::new(Mutex::new(vec![]));
    let sem = Arc::new(Semaphore::new(5));

    let mut tasks = Vec::new();

    for lb in elbv2_lbs {
        let client = elbv2_client.clone();
        let cw_client = cw_client.clone();
        let sem = Arc::clone(&sem);

        let arn = lb.load_balancer_arn().unwrap().to_string();
        let vpc_id = lb.vpc_id().unwrap().to_string();
        let vpc_ids = vpc_ids.clone();
        let region_string = utils::extract_region_from_elbv2_arn(&arn).unwrap();
        let region = Region::new(region_string);
        let elbv2_data = Arc::clone(&elbv2_data);

        let task = async move {
            println!("Processing ELBv2: {}", arn);
            let _perm = sem.acquire_owned().await;
            let state = get_elbv2_lb_state(arn.to_string(), &client, &cw_client, days).await;
            if let Some(state) = state {
                let mut elbv2_data = elbv2_data.lock().unwrap();
                if vpc_ids.len() > 0 && vpc_ids.contains_key(vpc_id.as_str()) {
                    elbv2_data.push(ElbV2Data::new(arn.as_str(), state, region, vpc_id));
                } else {
                    elbv2_data.push(ElbV2Data::new(arn.as_str(), state, region, vpc_id));
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

    let elbv2_data = elbv2_data.lock().unwrap();

    elbv2_data.to_vec()
}

pub async fn process_elbv2(elbv2s: Vec<ElbV2Data>) -> Vec<DeleteOutput> {
    let deletion_results: Arc<Mutex<Vec<DeleteOutput>>> = Arc::new(Mutex::new(vec![]));
    let mut tasks = Vec::new();

    for elbv2 in elbv2s {
        let region = elbv2.region;
        let arn = elbv2.arn;

        let config = aws_config::from_env().region(region).load().await;
        let client = ELBv2Client::new(&config);

        let deletion_results = Arc::clone(&deletion_results);

        let task = async move {
            println!("Processing ELBv2 deletion: {}", arn);
            let res = delete_elbv2(&arn, &client).await;
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
    let lb_namespace = utils::extract_namespace_from_lb_type(&arn).unwrap();
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
            .namespace(&lb_namespace)
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

async fn delete_elbv2(arn: &str, client: &ELBv2Client) -> DeleteOutput {
    let out = client
        .delete_load_balancer()
        .load_balancer_arn(arn)
        .send()
        .await
        .unwrap();
    println!("Deleted ELBv2 Load Balancer: {:?}", arn);
    out
}
