use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::str::FromStr;

#[derive(Clone, PartialEq)]
pub enum RunOption {
    List,
    Delete,
    Unknown,
}

impl Serialize for RunOption {
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.serialize_str(match *self {
            RunOption::List => "list",
            RunOption::Delete => "delete",
            _ => "unknown",
        })
    }
}

impl<'de> Deserialize<'de> for RunOption {
    fn deserialize<T>(deserializer: T) -> Result<Self, T::Error>
    where
        T: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "list" => RunOption::List,
            "delete" => RunOption::Delete,
            _ => RunOption::Unknown,
        })
    }
}

impl fmt::Debug for RunOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RunOption::List => write!(f, "List"),
            RunOption::Delete => write!(f, "Delete"),
            _ => write!(f, "Unknown"),
        }
    }
}

impl fmt::Display for RunOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RunOption::List => write!(f, "List"),
            RunOption::Delete => write!(f, "Delete"),
            _ => write!(f, "Unknown"),
        }
    }
}

impl FromStr for RunOption {
    type Err = ();

    fn from_str(input: &str) -> Result<RunOption, Self::Err> {
        match input.to_lowercase().as_str() {
            "list" => Ok(RunOption::List),
            "delete" => Ok(RunOption::Delete),
            _ => Err(()),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum ListFormat {
    Tabled,
    Csv,
}

impl fmt::Debug for ListFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ListFormat::Tabled => write!(f, "Tabled"),
            ListFormat::Csv => write!(f, "Csv"),
        }
    }
}

impl fmt::Display for ListFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ListFormat::Tabled => write!(f, "Tabled"),
            ListFormat::Csv => write!(f, "Csv"),
        }
    }
}

impl FromStr for ListFormat {
    type Err = ();

    fn from_str(input: &str) -> Result<ListFormat, Self::Err> {
        match input.to_lowercase().as_str() {
            "tabled" => Ok(ListFormat::Tabled),
            "csv" => Ok(ListFormat::Csv),
            _ => Err(()),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum LoadBalancerState {
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

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub run_option: RunOption,
    pub days: i32,
    pub aws: AwsConfig,
}

impl AppConfig {
    pub fn new(filepath: &str) -> Self {
        let conf = fs::read_to_string(filepath);
        match conf {
            Ok(conf) => match toml::from_str(&conf) {
                Ok(conf) => conf,
                Err(e) => panic!("Error parsing config as toml! {}", e),
            },
            Err(e) => {
                panic!("Error reading config file! {}", e);
            }
        }
    }
}

impl fmt::Display for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("name", &self.name)
            .field("run_option", &self.run_option)
            .field("days", &self.days)
            .field("aws", &self.aws)
            .finish()
    }
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("name", &self.name)
            .field("run_option", &self.run_option)
            .field("days", &self.days)
            .field("aws", &self.aws)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
pub struct AwsConfig {
    pub accounts: Vec<AwsAccount>,
}

impl fmt::Display for AwsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsConfig")
            .field("accounts", &self.accounts)
            .finish()
    }
}

impl fmt::Debug for AwsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsConfig")
            .field("accounts", &self.accounts)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
pub struct AwsAccount {
    pub iam_role: String,
    pub regions: Vec<String>,
    pub vpc_ids: Vec<String>,
}

impl fmt::Debug for AwsAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsAccount")
            .field("iam_role", &self.iam_role)
            .field("regions", &self.regions)
            .field("vpc_ids", &self.vpc_ids)
            .finish()
    }
}

impl fmt::Display for AwsAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsAccount")
            .field("iam_role", &self.iam_role)
            .field("regions", &self.regions)
            .field("vpc_ids", &self.vpc_ids)
            .finish()
    }
}
