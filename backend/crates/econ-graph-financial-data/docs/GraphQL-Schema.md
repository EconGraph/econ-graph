# GraphQL Schema Documentation

## Schema Definition

```graphql
# Economic time series data point
type DataPoint {
  id: ID!
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}

# Economic time series metadata
type EconomicSeries {
  id: ID!
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  startDate: Date
  endDate: Date
  isActive: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}

# Input for creating data points
input CreateDataPointInput {
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean!
}

# Input for creating economic series
input CreateEconomicSeriesInput {
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  startDate: Date
  endDate: Date
  isActive: Boolean!
}

# Root query type
type Query {
  # Get a specific economic series by ID
  series(id: ID!): EconomicSeries
  
  # Get data points for a series within a date range
  dataPoints(
    seriesId: ID!
    startDate: Date
    endDate: Date
  ): [DataPoint!]!
  
  # List all available economic series
  listSeries: [EconomicSeries!]!
  
  # Health check endpoint
  health: String!
}

# Root mutation type
type Mutation {
  # Create a new economic series
  createSeries(input: CreateEconomicSeriesInput!): EconomicSeries!
  
  # Create multiple data points for a series
  createDataPoints(inputs: [CreateDataPointInput!]!): [DataPoint!]!
}

# Custom scalar types
scalar Date
scalar DateTime
scalar Decimal
```

## Type Definitions

### DataPoint

Represents a single observation in an economic time series.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `ID!` | Unique identifier for the data point |
| `seriesId` | `ID!` | ID of the parent economic series |
| `date` | `Date!` | Observation date (YYYY-MM-DD format) |
| `value` | `Decimal` | Data value (nullable for missing observations) |
| `revisionDate` | `Date!` | Date when this value was last revised |
| `isOriginalRelease` | `Boolean!` | Whether this is the original release value |
| `createdAt` | `DateTime!` | Creation timestamp (ISO 8601) |
| `updatedAt` | `DateTime!` | Last update timestamp (ISO 8601) |

### EconomicSeries

Represents metadata for an economic time series.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `ID!` | Unique identifier for the series |
| `sourceId` | `ID!` | ID of the data source system |
| `externalId` | `String!` | External system identifier |
| `title` | `String!` | Human-readable series title |
| `description` | `String` | Detailed series description |
| `units` | `String` | Measurement units (e.g., "Billions of USD") |
| `frequency` | `String!` | Data frequency (DAILY, WEEKLY, MONTHLY, QUARTERLY, ANNUALLY) |
| `seasonalAdjustment` | `String` | Seasonal adjustment method (e.g., "SAAR", "NSA") |
| `startDate` | `Date` | Series start date |
| `endDate` | `Date` | Series end date |
| `isActive` | `Boolean!` | Whether the series is currently active |
| `createdAt` | `DateTime!` | Creation timestamp |
| `updatedAt` | `DateTime!` | Last update timestamp |

## Input Types

### CreateDataPointInput

Input for creating new data points.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `seriesId` | `ID!` | Yes | ID of the parent series |
| `date` | `Date!` | Yes | Observation date |
| `value` | `Decimal` | No | Data value (null for missing data) |
| `revisionDate` | `Date!` | Yes | Revision date |
| `isOriginalRelease` | `Boolean!` | Yes | Original release flag |

### CreateEconomicSeriesInput

Input for creating new economic series.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sourceId` | `ID!` | Yes | Source system ID |
| `externalId` | `String!` | Yes | External system identifier |
| `title` | `String!` | Yes | Series title |
| `description` | `String` | No | Series description |
| `units` | `String` | No | Measurement units |
| `frequency` | `String!` | Yes | Data frequency |
| `seasonalAdjustment` | `String` | No | Seasonal adjustment method |
| `startDate` | `Date` | No | Series start date |
| `endDate` | `Date` | No | Series end date |
| `isActive` | `Boolean!` | Yes | Active status |

## Query Operations

### series(id: ID!): EconomicSeries

Retrieve a specific economic series by its ID.

**Arguments:**
- `id`: The unique identifier of the series

**Returns:**
- `EconomicSeries` object if found, `null` otherwise

**Example:**
```graphql
query GetSeries {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    frequency
    isActive
    startDate
    endDate
  }
}
```

### dataPoints(seriesId: ID!, startDate: Date, endDate: Date): [DataPoint!]!

Retrieve data points for a series within an optional date range.

**Arguments:**
- `seriesId`: ID of the series to query
- `startDate`: Optional start date (inclusive)
- `endDate`: Optional end date (inclusive)

**Returns:**
- Array of `DataPoint` objects

**Example:**
```graphql
query GetDataPoints {
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

### listSeries: [EconomicSeries!]!

List all available economic series.

**Returns:**
- Array of all `EconomicSeries` objects

**Example:**
```graphql
query ListAllSeries {
  listSeries {
    id
    title
    frequency
    isActive
  }
}
```

### health: String!

Check the health status of the service.

**Returns:**
- Health status string

**Example:**
```graphql
query HealthCheck {
  health
}
```

## Mutation Operations

### createSeries(input: CreateEconomicSeriesInput!): EconomicSeries!

Create a new economic series.

**Arguments:**
- `input`: Series creation data

**Returns:**
- Created `EconomicSeries` object

**Example:**
```graphql
mutation CreateSeries {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_US"
    title: "US Gross Domestic Product"
    description: "Quarterly GDP data for the United States"
    units: "Billions of USD"
    frequency: "QUARTERLY"
    seasonalAdjustment: "SAAR"
    startDate: "1947-01-01"
    endDate: "2023-12-31"
    isActive: true
  }) {
    id
    title
    frequency
    isActive
  }
}
```

### createDataPoints(inputs: [CreateDataPointInput!]!): [DataPoint!]!

Create multiple data points for a series.

**Arguments:**
- `inputs`: Array of data point creation data

**Returns:**
- Array of created `DataPoint` objects

**Example:**
```graphql
mutation CreateDataPoints {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000.5"
      revisionDate: "2023-01-15"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-04-01"
      value: "25100.2"
      revisionDate: "2023-04-15"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
    isOriginalRelease
  }
}
```

## Scalar Types

### Date

Represents a date in YYYY-MM-DD format.

**Format:** `YYYY-MM-DD`
**Example:** `"2023-12-25"`

### DateTime

Represents a timestamp in ISO 8601 format.

**Format:** `YYYY-MM-DDTHH:mm:ss.sssZ`
**Example:** `"2023-12-25T10:30:00.000Z"`

### Decimal

Represents high-precision decimal numbers for financial data.

**Format:** String representation of decimal number
**Example:** `"25000.50"`

**Features:**
- High-precision arithmetic
- No floating-point rounding errors
- Suitable for financial calculations

## Error Handling

### GraphQL Errors

All operations can return structured errors:

```json
{
  "errors": [
    {
      "message": "Series not found",
      "locations": [{"line": 2, "column": 3}],
      "path": ["series"],
      "extensions": {
        "code": "NOT_FOUND",
        "resource": "EconomicSeries",
        "id": "123e4567-e89b-12d3-a456-426614174000"
      }
    }
  ]
}
```

### Common Error Codes

| Code | Description |
|------|-------------|
| `VALIDATION_ERROR` | Input validation failed |
| `NOT_FOUND` | Resource not found |
| `STORAGE_ERROR` | Storage operation failed |
| `INTERNAL_ERROR` | Internal server error |

## Performance Considerations

### Query Optimization

- **Date Range Queries**: Use `startDate` and `endDate` parameters to limit data transfer
- **Batch Operations**: Use `createDataPoints` for bulk data insertion
- **Field Selection**: Only request needed fields to minimize response size

### Best Practices

1. **Use Date Ranges**: Always specify date ranges for data point queries
2. **Batch Mutations**: Group related data point creations
3. **Field Selection**: Request only necessary fields
4. **Error Handling**: Implement proper error handling for all operations

## Examples

### Complete Workflow

```graphql
# 1. Create a series
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_US"
    title: "US Gross Domestic Product"
    frequency: "QUARTERLY"
    isActive: true
  }) {
    id
    title
  }
}

# 2. Add data points
mutation {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000.5"
      revisionDate: "2023-01-15"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}

# 3. Query the data
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    title
    frequency
  }
  
  dataPoints(
    seriesId: "123e4567-e89b-12d3-a456-426614174000"
    startDate: "2023-01-01"
    endDate: "2023-12-31"
  ) {
    date
    value
  }
}
```

### Bulk Data Import

```graphql
mutation BulkImport {
  createDataPoints(inputs: [
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-01-01"
      value: "25000.5"
      revisionDate: "2023-01-15"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-04-01"
      value: "25100.2"
      revisionDate: "2023-04-15"
      isOriginalRelease: true
    },
    {
      seriesId: "123e4567-e89b-12d3-a456-426614174000"
      date: "2023-07-01"
      value: "25200.8"
      revisionDate: "2023-07-15"
      isOriginalRelease: true
    }
  ]) {
    id
    date
    value
  }
}
```

### Health Monitoring

```graphql
query ServiceHealth {
  health
}
```

---

*This schema documentation is automatically generated from the GraphQL schema definition.*
