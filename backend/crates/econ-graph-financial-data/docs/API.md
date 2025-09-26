# Financial Data Service API Documentation

## Overview

The Financial Data Service provides a GraphQL API for managing economic time series data. It's designed as a standalone service that can be integrated with the main EconGraph backend through Apollo Federation.

## GraphQL Schema

### Queries

#### `health: String!`
Returns the health status of the service.

**Example:**
```graphql
query {
  health
}
```

**Response:**
```json
{
  "data": {
    "health": "Financial Data Service is healthy"
  }
}
```

#### `series(id: UUID!): EconomicSeries`
Retrieves a specific economic series by ID.

**Arguments:**
- `id`: UUID of the series to retrieve

**Example:**
```graphql
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    externalId
    frequency
    units
    isActive
  }
}
```

#### `dataPoints(seriesId: UUID!, startDate: Date, endDate: Date): [DataPoint!]!`
Retrieves data points for a series within an optional date range.

**Arguments:**
- `seriesId`: UUID of the series
- `startDate`: Optional start date (ISO format: YYYY-MM-DD)
- `endDate`: Optional end date (ISO format: YYYY-MM-DD)

**Example:**
```graphql
query {
  dataPoints(
    seriesId: "123e4567-e89b-12d3-a456-426614174000"
    startDate: "2023-01-01"
    endDate: "2023-12-31"
  ) {
    id
    date
    value
    isOriginalRelease
  }
}
```

### Mutations

#### `createSeries(input: CreateEconomicSeriesInput!): EconomicSeries!`
Creates a new economic series.

**Arguments:**
- `input`: CreateEconomicSeriesInput object

**Example:**
```graphql
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_USA"
    title: "US Gross Domestic Product"
    description: "Quarterly GDP data for the United States"
    units: "USD"
    frequency: "quarterly"
    seasonalAdjustment: "seasonally_adjusted"
    startDate: "2020-01-01"
    endDate: "2023-12-31"
    isActive: true
  }) {
    id
    title
    externalId
  }
}
```

#### `createDataPoints(inputs: [CreateDataPointInput!]!): [DataPoint!]!`
Creates multiple data points for a series.

**Arguments:**
- `inputs`: Array of CreateDataPointInput objects

**Example:**
```graphql
mutation {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000000000000"
      revisionDate: "2023-01-01"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-04-01"
      value: "25200000000000"
      revisionDate: "2023-04-01"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}
```

## Data Types

### EconomicSeries
Represents a financial time series with metadata.

**Fields:**
- `id: UUID!` - Unique identifier
- `sourceId: UUID!` - ID of the data source
- `externalId: String!` - External identifier from the source
- `title: String!` - Human-readable title
- `description: String` - Optional description
- `units: String` - Optional units of measurement
- `frequency: String!` - Data frequency (daily, weekly, monthly, quarterly, yearly)
- `seasonalAdjustment: String` - Optional seasonal adjustment method
- `startDate: Date` - Optional start date of the series
- `endDate: Date` - Optional end date of the series
- `isActive: Boolean!` - Whether the series is currently active
- `createdAt: DateTime!` - Creation timestamp
- `updatedAt: DateTime!` - Last update timestamp

### DataPoint
Represents a single data point in a time series.

**Fields:**
- `id: UUID!` - Unique identifier
- `seriesId: UUID!` - ID of the parent series
- `date: Date!` - Date of the data point
- `value: Decimal` - Optional numeric value (null for missing data)
- `revisionDate: Date!` - Date when this value was last revised
- `isOriginalRelease: Boolean!` - Whether this is the original release value
- `createdAt: DateTime!` - Creation timestamp
- `updatedAt: DateTime!` - Last update timestamp

### CreateEconomicSeriesInput
Input type for creating a new economic series.

**Fields:**
- `sourceId: UUID!` - ID of the data source
- `externalId: String!` - External identifier from the source
- `title: String!` - Human-readable title
- `description: String` - Optional description
- `units: String` - Optional units of measurement
- `frequency: String!` - Data frequency
- `seasonalAdjustment: String` - Optional seasonal adjustment method
- `startDate: Date` - Optional start date of the series
- `endDate: Date` - Optional end date of the series
- `isActive: Boolean!` - Whether the series is currently active

### CreateDataPointInput
Input type for creating a new data point.

**Fields:**
- `seriesId: UUID!` - ID of the parent series
- `date: Date!` - Date of the data point
- `value: Decimal` - Optional numeric value
- `revisionDate: Date!` - Date when this value was last revised
- `isOriginalRelease: Boolean!` - Whether this is the original release value

## Error Handling

The service returns GraphQL errors for various conditions:

- **Validation Errors**: Invalid input data
- **Not Found**: Requested resource doesn't exist
- **Internal Errors**: Server-side processing errors

**Example Error Response:**
```json
{
  "data": null,
  "errors": [
    {
      "message": "Series not found",
      "locations": [{"line": 2, "column": 3}],
      "path": ["series"]
    }
  ]
}
```

## Usage Examples

### Complete Workflow

1. **Create a Series:**
```graphql
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "UNEMPLOYMENT_RATE"
    title: "Unemployment Rate"
    description: "Monthly unemployment rate"
    units: "percent"
    frequency: "monthly"
    isActive: true
  }) {
    id
    title
  }
}
```

2. **Add Data Points:**
```graphql
mutation {
  createDataPoints(inputs: [
    {
      seriesId: "generated-series-id"
      date: "2023-01-01"
      value: "3.5"
      revisionDate: "2023-01-01"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}
```

3. **Query the Data:**
```graphql
query {
  series(id: "generated-series-id") {
    title
    frequency
    units
  }
  dataPoints(seriesId: "generated-series-id") {
    date
    value
  }
}
```

## Service Configuration

The service runs on port 3001 by default and provides:

- **GraphQL Endpoint**: `http://localhost:3001/graphql`
- **GraphQL Playground**: `http://localhost:3001/`

## Development

### Running the Service
```bash
cargo run
```

### Running Tests
```bash
cargo test
```

### Running Integration Tests
```bash
cargo test --test integration
```

## Future Enhancements

- **Apollo Federation**: Integration with the main backend
- **Storage Tiering**: MinIO-based storage with automatic tiering
- **Iceberg Integration**: Apache Iceberg for large-scale data management
- **Monitoring**: Prometheus metrics and Grafana dashboards
- **Authentication**: JWT-based authentication for secure access
