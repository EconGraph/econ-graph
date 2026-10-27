# BLS API Experimental Findings - September 13, 2025

This report records the September 2025 experiment with generated BLS series IDs. Its code excerpts and approximate results describe that experiment, not the current crawler. Current discovery reads curated IDs from `bls_series.csv` and metadata from stored BLS survey series-file rows; it does not validate generated IDs through the data API. See the [current BLS source guide](../data-sources/bls.md#what-we-fetch-today).

The [historical BLS module](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs) contains the generator inputs and validation flow discussed below. Excerpts here are abbreviated; this source revision establishes the code, not the requests or aggregate results reported by the experiment.

## Experimental scope

The experiment attempted survey metadata and series-list URLs, reported 404s for those paths, and tried generating candidate IDs for validation through the time-series data endpoint. The attempted singular `/survey/` URLs differ from the plural `/surveys/` signature in BLS's documentation. The reported failures therefore do not establish that documented survey metadata was unavailable or that a complete series-list API had failed.

## Attempted and documented endpoints

### Paths reported to return 404

#### 1. Single Survey Metadata Endpoint

- **Attempted URL**: `GET https://api.bls.gov/publicAPI/v2/survey/{survey_abbreviation}`
- **Documented alternative**: [Single Survey metadata](https://www.bls.gov/developers/api_signature_v2.htm) uses `GET https://api.bls.gov/publicAPI/v2/surveys/{survey_abbreviation}` (plural).
- **Expected by the experiment**: Detailed metadata about a specific survey
- **Reported result**: HTTP 404 Not Found
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L313)

```rust
async fn fetch_bls_single_survey(
    client: &Client,
    survey_abbreviation: &str,
) -> AppResult<BlsSurveyMetadata> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/survey/{}",
        survey_abbreviation
    );
    // The experiment reported 404 responses for this attempted path.
}
```

#### 2. Survey Series Discovery Endpoint

- **Attempted URL**: `GET https://api.bls.gov/publicAPI/v2/survey/{survey_abbreviation}/series`
- **Documentation distinction**: [All Surveys](https://www.bls.gov/developers/api_signature_v2.htm) lists surveys, not all series within a survey; it does not document this attempted series-list path.
- **Expected by the experiment**: Complete list of series for a given survey
- **Reported result**: HTTP 404 Not Found
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L274)

```rust
async fn fetch_bls_survey_series(
    client: &Client,
    survey_abbreviation: &str,
) -> AppResult<Vec<BlsSeriesMetadata>> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/survey/{}/series",
        survey_abbreviation
    );
    // The experiment reported 404 responses for this attempted path.
}
```

### Later check of the URL distinction

During the October 2026 documentation review, public keyless GETs for `LA` returned HTTP 404 for `/publicAPI/v2/survey/LA` and `/publicAPI/v2/survey/LA/series`. The documented plural path `/publicAPI/v2/surveys/LA` returned HTTP 200 with `REQUEST_SUCCEEDED` and Local Area Unemployment Statistics metadata. These observations concern those three requests; they do not establish endpoint availability in September 2025 or behavior for every survey.

### Endpoints reported to work in the experiment

#### 1. All Surveys Endpoint

- **URL**: `GET https://api.bls.gov/publicAPI/v2/surveys`
- **Documentation**: [All Surveys](https://www.bls.gov/developers/api_signature_v2.htm)
- **Reported result**: Successful responses
- **Response**: List of all available surveys with abbreviations and names
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L245)

```rust
async fn fetch_bls_surveys(client: &Client) -> AppResult<Vec<BlsSurvey>> {
    let url = "https://api.bls.gov/publicAPI/v2/surveys";
    let response = client.get(url).send().await?;
    // Remaining response parsing omitted.
}
```

**Sample response as presented in the original report** (survey labels retained):

```json
{
  "status": "REQUEST_SUCCEEDED",
  "responseTime": 0,
  "message": [],
  "Results": {
    "survey": [
      {
        "survey_abbreviation": "LA",
        "survey_name": "Labor Force Statistics"
      },
      {
        "survey_abbreviation": "CE",
        "survey_name": "Employment Situation"
      }
    ]
  }
}
```

#### 2. Series Data Retrieval Endpoint

- **URL**: `GET https://api.bls.gov/publicAPI/v2/timeseries/data/{series_id}`
- **Documentation**: [Series Data](https://www.bls.gov/developers/api_signature_v2.htm)
- **Reported result**: Successful responses
- **Response**: Time series data for a specific series ID
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L524)

```rust
async fn test_bls_series_id(
    client: &Client,
    series_id: &str,
) -> AppResult<Option<BlsSeriesMetadata>> {
    let url = format!(
        "https://api.bls.gov/publicAPI/v2/timeseries/data/{}",
        series_id
    );
    // Remaining validation and metadata extraction omitted.
}
```

## Historical candidate generation

After the reported failures, the experiment generated candidate series IDs and checked them through the data endpoint. The historical implementation combined survey-specific patterns, generic survey-prefix patterns and a fallback ID list, then sorted and deduplicated the candidates. This tested individual candidates; it could not establish a complete survey inventory.

### Trial inputs and BLS series ID layouts

The following inputs preserve the experiment's generator assumptions. BLS's [Series ID Formats](https://www.bls.gov/help/hlpforma.htm) distinguish those trial inputs from valid field layouts; a plausible layout alone does not establish that an ID exists.

#### Local Area Unemployment Statistics (LA survey)

- **Historical trial formula**: `LAUCN{state_fips}{area_fips}{measure_code}{seasonal_adjustment}`
- **ID retained from the report**: `LAUCN040010000000005` (employment measure `05`)

BLS specifies `LA` + one seasonal character + a 15-character area + a two-character measure. The trial formula already fixes `U` after `LA` and appends another seasonal character; its padded measure inputs below are not BLS's two-character measure field. The measure codes are `03` unemployment rate, `04` unemployment, `05` employment and `06` labor force, also recorded in the [current dataset definitions](../../backend/crates/econ-graph-crawler/data/datasets/bls.toml). The original report incorrectly labeled the example as a statewide Arizona unemployment rate, and the historical array's comments misidentified the measures. The array values below are retained as trial inputs, not corrected generator code.

**Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L422)

```rust
"LA" => {
    // Labor Force Statistics - generate common patterns
    let states = [
        "00", "01", "02", "04", "05", "06", "08", "09", "10", "11", "12", "13", "15",
        "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28",
        "29", "30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "40", "41",
        "42", "44", "45", "46", "47", "48", "49", "50", "51", "53", "54", "55", "56",
    ];
    let areas = ["000000", "000001", "000002", "000003", "000004", "000005"];
    let measures = ["000003", "000004", "000005"]; // Historical padded trial inputs
    let seasonal = ["U", "S"]; // Unadjusted, Seasonally Adjusted
}
```

#### National employment, hours and earnings (CE survey)

- **Historical trial formula**: `CES{supersector}{industry}{data_type}`
- **Example ID**: `CES0000000001` (All Employees, Total Nonfarm)

BLS specifies `CE` + one seasonal character + an eight-character industry + a two-character data type. The historical generator concatenated `CES`, one eight-character entry from `supersectors`, and one four-character `data_types` value. Those four-character values do not match the example ID's two-character suffix. They remain below as historical trial inputs, rather than a validated format.

**Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L446)

```rust
"CE" => {
    // Employment Situation - generate common patterns
    let supersectors = [
        "00000000", "05000000", "10000000", "20000000", "30000000", "40000000",
        "50000000", "55000000", "60000000", "70000000", "80000000", "90000000",
    ];
    let data_types = ["0001", "0003", "0007"]; // Historical four-character trial inputs
}
```

#### Consumer Price Index (CU Survey)

- **Pattern**: `CUSR0000{series_code}`
- **Example**: `CUSR0000SA0` (CPI for All Urban Consumers: All Items)
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L461)

```rust
"CU" => {
    // Consumer Price Index - generate common patterns
    let series_codes = [
        "SA0", "SA0L1E", "SA0L2", "SA0L5", "SETB01", "SETB02", "SETB03", "SETB04",
        "SETB05",
    ];
}
```

#### Producer Price Index (WP Survey)

- **Pattern**: `WPU{commodity_code}`
- **Example**: `WPU00000000` (Producer Price Index by Commodity: All Commodities)
- **Historical source**: [related implementation](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L473)

```rust
"WP" => {
    // Producer Price Index - generate common patterns
    let commodities = [
        "00000000", "FD49507", "FD49508", "FD49509", "FD49510", "FD49511", "FD49512",
    ];
}
```

### Validation strategy

The [historical loop](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L196) checked each candidate through the data endpoint. A nonempty data response produced metadata to store; `Ok(None)` skipped an invalid or empty result, while `Err(e)` logged an API error and continued:

```rust
// Historical validation loop; response handling abbreviated from the linked module.
for series_id in &candidate_series_ids {
    tested_count += 1;
    if tested_count % 100 == 0 {
        println!(
            "Tested {}/{} candidate series IDs",
            tested_count, total_candidates
        );
    }

    // Test if this series ID returns valid data
    match test_bls_series_id(client, &series_id).await {
        Ok(Some(series_metadata)) => {
            store_discovered_bls_series(pool, &bls_source.id, &series_metadata).await?;
            discovered_series.push(series_id.clone());
            valid_series.push(series_metadata);

            if valid_series.len() % 50 == 0 {
                println!("Found {} valid BLS series so far", valid_series.len());
            }
        }
        Ok(None) => {
            // Series ID doesn't exist or has no data
            continue;
        }
        Err(e) => {
            // API error - log and continue
            eprintln!("Error testing series {}: {}", series_id, e);
            continue;
        }
    }

    // Add a small delay to be polite to the API
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}
```

## Reported experimental results

These approximate figures are retained from the September 2025 report. They were not reconstructed during the documentation review and do not demonstrate that the LA or CE trial formulas above produced valid IDs.

### Discovery success rate

- **Total Candidate Series Generated**: ~1,200 patterns
- **Valid Series Found**: ~150-200 (varies by survey)
- **Success Rate**: ~12-17% of generated patterns yield valid series
- **API Response Time**: 100-500ms per validation request

### Historical fallback IDs

The report described the [historical fallback list](https://github.com/EconGraph/econ-graph/blob/8f7a19373f5c53f80bd2fd0f7eb99262235e4e58/backend/src/services/series_discovery/bls.rs#L601) as verified working IDs. The list is retained as historical input, not a current guarantee of availability or a catalog of verified titles. Comments below identify the three LA measure suffixes correctly; the other labels are the report's descriptions and were not independently validated in this review.

```rust
// Historical fallback list; LA measure comments corrected below.
fn get_known_bls_series_ids() -> Vec<String> {
    vec![
        // Labor Force Statistics (LA survey)
        "LAUCN040010000000005".to_string(), // Employment (measure 05)
        "LAUCN040010000000003".to_string(), // Unemployment rate (measure 03)
        "LAUCN040010000000004".to_string(), // Unemployment (measure 04)

        // Consumer Price Index (CU survey)
        "CUSR0000SA0".to_string(), // CPI for All Urban Consumers: All Items in U.S. City Average
        "CUSR0000SA0L1E".to_string(), // CPI for All Urban Consumers: All Items Less Food and Energy

        // Employment Situation (CE survey)
        "CES0000000001".to_string(), // All Employees, Total Nonfarm
        "CES0500000003".to_string(), // Average Hourly Earnings of All Employees, Total Private
        "CES0000000007".to_string(), // Average Weekly Hours of All Employees, Total Private

        // Producer Price Index (WP survey)
        "WPU00000000".to_string(), // Producer Price Index by Commodity: All Commodities
        "WPUFD49507".to_string(), // Producer Price Index by Commodity: Finished Goods

        // Import/Export Price Indexes (MX survey)
        "MXUS0000000000".to_string(), // Import Price Index: All Imports
        "MXUS0000000001".to_string(), // Export Price Index: All Exports

        // Employment Cost Index (CI survey)
        "CIU2010000000000A".to_string(), // Employment Cost Index: Wages and Salaries: Private Industry Workers
        "CIU2020000000000A".to_string(), // Employment Cost Index: Benefits: Private Industry Workers
    ]
}
```

## Historical request pacing and error handling

The loop shown above paused for 100 ms only after a successful validation and storage. Both `Ok(None)` and `Err(e)` executed `continue` before that pause. Even a pause after every request would permit up to 600 requests per minute before network time; this code did not enforce the original report's stated 100-per-minute target.

The historical validation helper returned `Ok(None)` for non-success HTTP responses, a BLS status other than `REQUEST_SUCCEEDED`, or missing series/data. Transport and JSON parsing errors propagated to the loop's logging branch. Neither that helper nor the shown loop demonstrates exponential backoff. The report also described a 30-second timeout and identifying User-Agent/contact headers, but the abbreviated excerpts do not establish those client settings.

For the current crawler's enforced limits, retries, timeout and User-Agent, see [Crawler Politeness](./CRAWLER_POLITENESS.md). Its BLS policy is 25 requests per minute, burst 1 and concurrency 1; API daily quotas are a separate constraint.

## Lessons and conclusion

The experiment illustrated how candidate validation can retain IDs with data while discarding empty responses and continuing after errors. It also exposed limits of that approach: incorrect field widths and measure labels can make candidates malformed or misinterpreted, successful candidates do not establish complete coverage, and a delay bypassed by failure paths does not bound the request rate.

The reported 404s concern the attempted singular URLs. They do not support the original conclusion that BLS's documented survey metadata API was broken. The experiment remains background for understanding the earlier discovery approach; current discovery obtains metadata for curated IDs from BLS reference files, as described in the [BLS source guide](../data-sources/bls.md).
