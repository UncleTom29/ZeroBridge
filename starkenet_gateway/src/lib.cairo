pub mod contract {
    pub mod zero_bridge_gateway;
}
pub mod interfaces {
    pub mod izero_bridge_gateway;
    pub mod ierc20;
}
pub mod types;
pub mod events;

pub use contract::zero_bridge_gateway::ZeroBridgeGateway;
pub use interfaces::izero_bridge_gateway::IZeroBridgeGateway;