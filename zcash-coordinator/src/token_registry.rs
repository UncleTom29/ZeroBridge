// zcash-coordinator/src/token_registry.rs
//! Token registry for cross-chain token mappings

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Token registry managing canonical token identifiers
pub struct TokenRegistry {
    mappings: HashMap<CanonicalTokenId, TokenMappings>,
    reverse_lookup: HashMap<(u64, String), CanonicalTokenId>,
}

/// Canonical token identifier (chain-agnostic)
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanonicalTokenId(pub String);

/// Token mappings across chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMappings {
    pub canonical_id: CanonicalTokenId,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub representations: Vec<ChainToken>,
}

/// Token representation on a specific chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainToken {
    pub chain_id: u64,
    pub chain_name: String,
    pub address: String,
    pub decimals: u8,
    pub native: bool,
    pub wrapped_version: Option<String>,
}

/// Token registry configuration file format
#[derive(Debug, Deserialize)]
struct TokenConfig {
    tokens: Vec<TokenDefinition>,
}

#[derive(Debug, Deserialize)]
struct TokenDefinition {
    symbol: String,
    name: String,
    decimals: u8,
    representations: Vec<TokenRepresentation>,
}

#[derive(Debug, Deserialize)]
struct TokenRepresentation {
    chain_id: u64,
    chain_name: String,
    address: String,
    #[serde(default)]
    decimals: Option<u8>,
    #[serde(default)]
    native: bool,
    #[serde(default)]
    wrapped_version: Option<String>,
}

impl TokenRegistry {
    /// Load token registry from configuration file
    pub async fn load(path: &str) -> Result<Self> {
        info!("Loading token registry from: {}", path);
        
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read token registry file")?;
        
        let config: TokenConfig = toml::from_str(&content)
            .context("Failed to parse token registry")?;
        
        let mut mappings = HashMap::new();
        let mut reverse_lookup = HashMap::new();
        
        for token_def in config.tokens {
            let canonical_id = Self::compute_canonical_id(&token_def.symbol);
            
            let mut representations = Vec::new();
            
            for repr in token_def.representations {
                let chain_token = ChainToken {
                    chain_id: repr.chain_id,
                    chain_name: repr.chain_name.clone(),
                    address: repr.address.clone(),
                    decimals: repr.decimals.unwrap_or(token_def.decimals),
                    native: repr.native,
                    wrapped_version: repr.wrapped_version,
                };
                
                // Add to reverse lookup
                reverse_lookup.insert(
                    (repr.chain_id, repr.address.to_lowercase()),
                    canonical_id.clone(),
                );
                
                representations.push(chain_token);
            }
            
            let token_mappings = TokenMappings {
                canonical_id: canonical_id.clone(),
                symbol: token_def.symbol,
                name: token_def.name,
                decimals: token_def.decimals,
                representations,
            };
            
            mappings.insert(canonical_id, token_mappings);
        }
        
        info!("Loaded {} tokens with {} representations", 
            mappings.len(),
            reverse_lookup.len()
        );
        
        Ok(Self {
            mappings,
            reverse_lookup,
        })
    }
    
    /// Get token for a specific chain
    pub fn get_token_for_chain(
        &self,
        chain_id: u64,
        token_address: &str,
    ) -> Result<ChainToken> {
        let canonical_id = self
            .reverse_lookup
            .get(&(chain_id, token_address.to_lowercase()))
            .context("Token not found in registry")?;
        
        let mappings = self
            .mappings
            .get(canonical_id)
            .context("Token mappings not found")?;
        
        mappings
            .representations
            .iter()
            .find(|t| t.chain_id == chain_id)
            .cloned()
            .context("Token not available on specified chain")
    }
    
    /// Get token by canonical ID for a specific chain
    pub fn get_token_by_id(
        &self,
        canonical_id: &CanonicalTokenId,
        chain_id: u64,
    ) -> Result<ChainToken> {
        let mappings = self
            .mappings
            .get(canonical_id)
            .context("Token not found")?;
        
        mappings
            .representations
            .iter()
            .find(|t| t.chain_id == chain_id)
            .cloned()
            .context("Token not available on specified chain")
    }
    
    /// Get canonical ID from chain-specific address
    pub fn get_canonical_id(
        &self,
        chain_id: u64,
        token_address: &str,
    ) -> Option<&CanonicalTokenId> {
        self.reverse_lookup
            .get(&(chain_id, token_address.to_lowercase()))
    }
    
    /// Get all representations for a token
    pub fn get_all_representations(
        &self,
        canonical_id: &CanonicalTokenId,
    ) -> Option<&TokenMappings> {
        self.mappings.get(canonical_id)
    }
    
    /// Check if token is supported on a chain
    pub fn is_supported(
        &self,
        chain_id: u64,
        token_address: &str,
    ) -> bool {
        self.reverse_lookup
            .contains_key(&(chain_id, token_address.to_lowercase()))
    }
    
    /// Get number of tokens
    pub fn token_count(&self) -> usize {
        self.mappings.len()
    }
    
    /// Get all supported chains for a token
    pub fn get_supported_chains(
        &self,
        canonical_id: &CanonicalTokenId,
    ) -> Vec<u64> {
        self.mappings
            .get(canonical_id)
            .map(|m| m.representations.iter().map(|r| r.chain_id).collect())
            .unwrap_or_default()
    }
    
    /// Compute canonical token ID from symbol
    fn compute_canonical_id(symbol: &str) -> CanonicalTokenId {
        use blake2::{Blake2b512, Digest};
        
        let mut hasher = Blake2b512::new();
        hasher.update(symbol.to_uppercase().as_bytes());
        let result = hasher.finalize();
        
        CanonicalTokenId(hex::encode(&result[..16]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_registry_loading() {
        // Create test config
        let config = r#"
[[tokens]]
symbol = "ETH"
name = "Ethereum"
decimals = 18

[[tokens.representations]]
chain_id = 1
chain_name = "Ethereum"
address = "0x0000000000000000000000000000000000000000"
native = true

[[tokens.representations]]
chain_id = 8453
chain_name = "Base"
address = "0x0000000000000000000000000000000000000000"
native = true

[[tokens]]
symbol = "USDC"
name = "USD Coin"
decimals = 6

[[tokens.representations]]
chain_id = 1
chain_name = "Ethereum"
address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"

[[tokens.representations]]
chain_id = 8453
chain_name = "Base"
address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
"#;
        
        // Write to temp file
        let temp_path = "/tmp/test_tokens.toml";
        tokio::fs::write(temp_path, config).await.unwrap();
        
        // Load registry
        let registry = TokenRegistry::load(temp_path).await.unwrap();
        
        assert_eq!(registry.token_count(), 2);
        
        // Test ETH lookup
        let eth_on_ethereum = registry
            .get_token_for_chain(1, "0x0000000000000000000000000000000000000000")
            .unwrap();
        assert!(eth_on_ethereum.native);
        
        // Test USDC lookup
        let usdc_on_base = registry
            .get_token_for_chain(8453, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
            .unwrap();
        assert_eq!(usdc_on_base.decimals, 6);
        
        // Clean up
        tokio::fs::remove_file(temp_path).await.ok();
    }
}