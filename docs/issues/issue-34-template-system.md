# Issue #34: CLI Template System

## Status
- **Status**: OPEN
- **Type**: Enhancement
- **Parent Issue**: #32
- **Labels**: enhancement, feature, cli

## Overview
Implement a template system to help users quickly set up common datafeed configurations without starting from scratch.

## Proposed Features

### 1. Built-in Templates
Provide pre-configured templates for common use cases:

```bash
# List available templates
omikuji template list

# Show template details
omikuji template show chainlink-price-feed

# Initialize config from template
omikuji init --template chainlink-price-feed
omikuji init --template multi-network-oracle
```

**Built-in Templates:**
- `chainlink-price-feed`: Standard Chainlink price feed setup
- `multi-network-oracle`: Same feed across multiple networks
- `high-frequency-feed`: Optimized for frequent updates
- `low-cost-feed`: Optimized for gas efficiency
- `dev-environment`: Local development setup

### 2. Template Customization
Interactive customization during initialization:

```bash
# Interactive template setup
omikuji init --template chainlink-price-feed --interactive

# Questions asked:
# - Network to deploy on? [ethereum/polygon/arbitrum]
# - Feed pair? [ETH/USD, BTC/USD, custom]
# - Update frequency? [60s]
# - Deviation threshold? [0.5%]
# - Contract address? [0x...]
```

### 3. Custom Template Creation
Create templates from existing configurations:

```bash
# Create template from current config
omikuji template create my-template

# Create template from specific config
omikuji template create my-template --from config.yaml

# Export template for sharing
omikuji template export my-template > my-template.yaml
```

### 4. Template Repository
Community template sharing:

```bash
# Import template from file
omikuji template import custom-template.yaml

# Import from URL
omikuji template import https://example.com/template.yaml

# List user templates
omikuji template list --user
```

## Implementation Details

### Template Structure
```yaml
# Template metadata
metadata:
  name: chainlink-price-feed
  description: Standard Chainlink price feed configuration
  version: 1.0.0
  author: omikuji-team

# Variable definitions
variables:
  - name: network
    description: Network to deploy on
    type: choice
    choices: [ethereum, polygon, arbitrum]
    default: ethereum
  
  - name: feed_pair
    description: Price feed pair
    type: string
    default: ETH/USD
  
  - name: contract_address
    description: FluxAggregator contract address
    type: address
    required: true

# Template content with variable substitution
template:
  networks:
    - name: "{{ network }}"
      rpc_url: "{{ network_rpc_url }}"
      transaction_type: eip1559
      
  datafeeds:
    - name: "{{ feed_pair | lower | replace('/', '_') }}"
      networks: "{{ network }}"
      contract_address: "{{ contract_address }}"
      contract_type: fluxmon
      check_frequency: 60
      minimum_update_frequency: 3600
      deviation_threshold_pct: 0.5
```

### Storage Location
```
~/.omikuji/
├── templates/
│   ├── built-in/
│   │   ├── chainlink-price-feed.yaml
│   │   └── ...
│   └── user/
│       ├── my-custom-template.yaml
│       └── ...
```

## Dependencies
- `handlebars` or `tera` for template rendering
- `serde_yaml` for template parsing

## Success Criteria
1. Users can quickly bootstrap configurations
2. Templates cover common use cases
3. Easy customization of template variables
4. Ability to share templates
5. Clear documentation for each template