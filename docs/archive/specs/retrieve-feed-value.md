Retrieving the feed value

For each of the datafeeds listed in the config.yaml file Omikuji should retrieve the latest HTTP response the data feed's feed_url. Omikuji should send the 'Accept: application/json' HTTP header along with the request.

Upon receiving the response, we should check the HTTP status code and anythingthat isn't a 200, should be logged as an error.

An 200 status code will then cause the system to parse the JSON response. Datafeed JSON structure is out of our control so we need to be flexible and can only mandate that the response is valid JSON.
Once we have parsed the JSON we are going to extract the feed value using the feed_json_path as defined for the datafeed in the config file.

The feed_json_path should be split by dot-notation, such that RAW.ETH.USD.PRICE produces a list of path components [RAW, ETH, USD, PRICE].

We will then walk down the JSON body using the path components in order as they're defined in the list. Such that the example path components above applied to the example JSON below 

{
  "RAW": {
    "ETH": {
      "USD": {
        "PRICE": 2045.34,
        "LASTUPDATE": 1748068861
      }
    }
  }
}

will produce a feed value of 2045.34. Optionally if feed_json_path_timestamp is defined in the datafeed config, then it should using the same JSON path mechanism to extract a UNIX timestamp value from the JSON data. 
If the feed_json_path_timestamp is not defined in the datafeed config, then the system should generate a current UNIX timestamp value.

It should log feed value and timestamp to the console.

Feed value should be assumed to be a float. The last updated timestamp should be an integer. Any data conversion errors should be logged to the console.

This process should repeat itself every interval in seconds, as defined in the datafeed's check_frequency parameter.

Multiple feeds should run in multiple threads, so that one feed's update process does not delay another feed's update process.
