The Omikuji configuration file is defined as follows:

```yaml
networks:
  - name: ethereum
    rpc_url: https://eth.llamarpc.com
  - name: base
    rpc_url: https://base.llamarpc.com

datafeeds:
  - name: eth_usd
    networks: ethereum
    check_frequency: 60
    contract_address: 0x123
    contract_type: fluxmon
    read_contract_config: true
    minimum_update_frequency: 3600
    deviation_threshold_pct: 0.5
    feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
    feed_json_path: RAW.ETH.USD.PRICE
    feed_json_path_timestamp: RAW.ETH.USD.LASTUPDATE
  - name: eth_usd
    networks: base
    check_frequency: 60
    contract_address: 0x123
    contract_type: fluxmon
    read_contract_config: false
    decimals: 8
    min_value: 0
    max_value: 1000000
    minimum_update_frequency: 3600
    deviation_threshold_pct: 0.5
    feed_url: https://min-api.cryptocompare.com/data/pricemultifull?fsyms=ETH&tsyms=USD
    feed_json_path: RAW.ETH.USD.PRICE
    feed_json_path_timestamp: RAW.ETH.USD.LASTUPDATE

```

When Omikuji starts, it will check the configuration file and load the datafeeds. For each datafeed, it will check the networks and load the contract addresses. It will then check the check_frequency and minimum_update_frequency and start a timer to update the datafeed. If the datafeed is not updated within the minimum_update_frequency, it will log a warning. If the datafeed is updated within the minimum_update_frequency, it will check the deviation_threshold_pct and update the contract if the deviation is above the threshold. If the datafeed is updated within the minimum_update_frequency and the deviation is below the threshold, it will log a warning.

Omikuji will read the configuration at startup using the -c flag. If the -c flag is not specified, it will use the default configuration file located at ~/.omikuji/config.yaml.

Omikuji will also have a web interface that allows users to view the datafeeds and their status. The web interface will be available at http://localhost:8080. 

When implementing the configuration file reader we should use a validation library, e.g. the equivalent of Pydantic but for Rust.
