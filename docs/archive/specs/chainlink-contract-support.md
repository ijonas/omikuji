# Chainlink Flux Monitor Support

Omikuji will support Chainlink Flux Monitor contracts as datafeeds. This will allow users to use Omikuji as a datafeed for their Chainlink Flux Monitor contracts.

Omikuji will call the FluxMonitor interface of the Chainlink Flux Monitor contract to update the datafeed. To update a feed, the FluxMonitor interface provides the `submit` function, which takes a round ID and a submission value as parameters. The submission value will be the latest answer of the FluxAggregator contract. The latest round ID of the FluxAggregator contract can be retrieved using the `latestRound` function.    