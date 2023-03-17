use aws_sdk_cloudwatch::types::DateTime as CloudwatchDateTime;
use aws_sdk_cloudwatch::{
    model::{Metric, MetricDataQuery, MetricDataResult, MetricStat},
    Client as CloudWatchClient,
};
use aws_smithy_types_convert::date_time::DateTimeExt;
use chrono::{Duration, Utc};

pub async fn get_metric_stats(
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
