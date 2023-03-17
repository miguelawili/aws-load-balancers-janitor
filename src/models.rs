use std::fmt;

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
