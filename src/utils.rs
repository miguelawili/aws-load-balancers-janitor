use crate::models::RunOption;
use aws_types::region::Region;
use std::collections::HashMap;
use std::str::FromStr;

pub fn parse_regions_arg(regions: &str) -> Vec<Region> {
    let regions = regions.split(",");
    let mut regions_obj: Vec<Region> = Vec::new();

    for region in regions {
        regions_obj.push(Region::new(region.to_string()));
    }

    regions_obj
}

pub fn parse_vpc_ids_arg(vpc_ids: &str) -> HashMap<String, bool> {
    let vpc_ids = vpc_ids.split(",");
    let mut vpc_ids_map: HashMap<String, bool> = HashMap::new();

    for vpc_id in vpc_ids {
        vpc_ids_map.insert(vpc_id.to_string(), true);
    }

    vpc_ids_map
}

pub fn parse_run_option_arg(run_option: &str) -> RunOption {
    match RunOption::from_str(run_option) {
        Ok(val) => val,
        Err(_) => panic!("Shouldn't happen!"),
    }
}

pub fn extract_id_from_lb_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 6 {
        let sub_parts: Vec<&str> = parts[5].split('/').skip(1).collect();
        Some(sub_parts.join("/"))
    } else {
        None
    }
}

pub fn extract_id_from_tg_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 6 {
        let sub_parts: Vec<&str> = parts[5].split('/').collect();
        Some(sub_parts.join("/"))
    } else {
        None
    }
}

pub fn extract_region_from_elbv2_arn(arn: &str) -> Option<String> {
    let parts: Vec<&str> = arn.split(':').collect();
    if parts.len() >= 4 {
        Some(parts[3].to_string())
    } else {
        None
    }
}

pub fn extract_region_from_elb_dns(dns_name: &str) -> Option<String> {
    let parts: Vec<&str> = dns_name.split('.').collect();
    if parts.len() > 2 {
        Some(parts[1].to_string())
    } else {
        None
    }
}
