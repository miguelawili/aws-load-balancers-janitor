use std::fmt;
use std::str::FromStr;

#[derive(Clone, PartialEq)]
pub enum RunOption {
    List,
    Delete,
}

impl fmt::Debug for RunOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RunOption::List => write!(f, "List"),
            RunOption::Delete => write!(f, "Delete"),
        }
    }
}

impl fmt::Display for RunOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RunOption::List => write!(f, "List"),
            RunOption::Delete => write!(f, "Delete"),
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
