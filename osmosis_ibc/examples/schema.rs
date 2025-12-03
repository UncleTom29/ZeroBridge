// ============================================
// contracts/osmosis/examples/schema.rs
// Schema generation

use cosmwasm_schema::write_api;
use zerobridge_osmosis_gateway::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}