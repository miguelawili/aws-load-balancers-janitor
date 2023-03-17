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
