# EconGraph MCP Server

## Overview

The EconGraph MCP (Model Context Protocol) Server provides AI models with direct access to economic data search, retrieval, and visualization capabilities. This server implements the MCP protocol to enable AI assistants to interact with economic data through a standardized interface.

## Features

### 🔍 Data Search
- **Tool**: `search_economic_series`
- **Purpose**: Find economic data series by name, description, or keywords
- **Input**: Search query and optional limit
- **Output**: List of matching economic series with metadata

### 📊 Data Retrieval
- **Tool**: `get_series_data`
- **Purpose**: Retrieve time series data points for specific economic series
- **Input**: Series ID, optional date range, and limit
- **Output**: Time series data points with dates and values

### 📋 Metadata Access
- **Tool**: `get_series_metadata`
- **Purpose**: Get detailed information about economic series
- **Input**: Series ID
- **Output**: Complete series metadata including source, frequency, units, etc.

### 📈 Data Visualization
- **Tool**: `create_data_visualization`
- **Purpose**: Create charts and graphs for economic data
- **Input**: Series IDs, chart type, title, and optional date range
- **Output**: Visualization data structure (ready for chart generation)

### 📚 Resource Access
- **Resources**: Data sources catalog and series catalog
- **Purpose**: Browse available data sources and economic series
- **Access**: Through MCP resource endpoints

## API Endpoints

### MCP Server Endpoint
- **URL**: `http://localhost:9876/mcp`
- **Method**: POST
- **Protocol**: JSON-RPC 2.0
- **Content-Type**: application/json

### Supported Methods

#### 1. List Available Tools
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "search_economic_series",
        "description": "Search for economic data series by name, description, or keywords",
        "input_schema": {
          "type": "object",
          "properties": {
            "query": {
              "type": "string",
              "description": "Search query for economic series"
            },
            "limit": {
              "type": "integer",
              "description": "Maximum number of results to return",
              "default": 10
            }
          },
          "required": ["query"]
        }
      }
      // ... other tools
    ]
  }
}
```

#### 2. Call a Tool
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "search_economic_series",
    "arguments": {
      "query": "GDP",
      "limit": 5
    }
  }
}
```

#### 3. List Available Resources
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resources/list"
}
```

#### 4. Read a Resource
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "resources/read",
  "params": {
    "uri": "econ-graph://data-sources"
  }
}
```

## Tool Details

### search_economic_series
Searches for economic data series using the GraphQL search API.

**Parameters:**
- `query` (string, required): Search query
- `limit` (integer, optional): Maximum results (default: 10)

**Example:**
```json
{
  "name": "search_economic_series",
  "arguments": {
    "query": "unemployment rate",
    "limit": 10
  }
}
```

### get_series_data
Retrieves time series data points for a specific economic series.

**Parameters:**
- `series_id` (string, required): UUID of the economic series
- `start_date` (string, optional): Start date in YYYY-MM-DD format
- `end_date` (string, optional): End date in YYYY-MM-DD format
- `limit` (integer, optional): Maximum data points (default: 100)

**Example:**
```json
{
  "name": "get_series_data",
  "arguments": {
    "series_id": "123e4567-e89b-12d3-a456-426614174000",
    "start_date": "2020-01-01",
    "end_date": "2023-12-31",
    "limit": 50
  }
}
```

### get_series_metadata
Gets detailed metadata about an economic series.

**Parameters:**
- `series_id` (string, required): UUID of the economic series

**Example:**
```json
{
  "name": "get_series_metadata",
  "arguments": {
    "series_id": "123e4567-e89b-12d3-a456-426614174000"
  }
}
```

### create_data_visualization
Creates visualization data for economic series.

**Parameters:**
- `series_ids` (array, required): Array of series UUIDs
- `chart_type` (string, optional): "line", "bar", or "scatter" (default: "line")
- `title` (string, optional): Chart title
- `start_date` (string, optional): Start date in YYYY-MM-DD format
- `end_date` (string, optional): End date in YYYY-MM-DD format

**Example:**
```json
{
  "name": "create_data_visualization",
  "arguments": {
    "series_ids": ["123e4567-e89b-12d3-a456-426614174000", "987fcdeb-51a2-43d1-b789-123456789abc"],
    "chart_type": "line",
    "title": "GDP vs Unemployment Rate",
    "start_date": "2020-01-01",
    "end_date": "2023-12-31"
  }
}
```

## Resource Details

### econ-graph://data-sources
Provides information about available economic data sources (FRED, BLS, etc.).

### econ-graph://series-catalog
Provides a catalog of all available economic series with basic metadata.

## Integration with AI Clients

### Claude Desktop
Add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "econ-graph": {
      "command": "curl",
      "args": [
        "-X", "POST",
        "http://localhost:9876/mcp",
        "-H", "Content-Type: application/json",
        "-d", "@-"
      ]
    }
  }
}
```

### Custom Integration
The MCP server uses standard JSON-RPC 2.0 protocol, making it compatible with any MCP client implementation.

## Error Handling

The server returns standard JSON-RPC error responses:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Internal error: Database connection failed"
  }
}
```

Common error codes:
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

## Development

### Running the Server
```bash
cd backend
cargo run
```

The MCP server will be available at `http://localhost:9876/mcp`.

### Testing
```bash
# Test the MCP server functionality
cargo run --bin mcp_test

# Test with curl
curl -X POST http://localhost:9876/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'
```

### Architecture

The MCP server is built using:
- **Rust** with `rust-mcp-sdk` for MCP protocol implementation
- **Warp** for HTTP server
- **Async-GraphQL** for data access
- **Diesel** for database operations
- **PostgreSQL** for data storage

## Use Cases

### Economic Research
AI assistants can help researchers find relevant economic data, retrieve time series, and create visualizations for analysis.

### Financial Analysis
Analysts can use AI to quickly access economic indicators, compare different metrics, and generate charts for reports.

### Educational Applications
Students and educators can explore economic data through natural language queries, making economic concepts more accessible.

### Business Intelligence
Companies can integrate economic data into their AI-powered analytics workflows for better decision-making.

## Future Enhancements

- **Real-time Data**: Support for streaming economic data updates
- **Advanced Visualizations**: More chart types and interactive features
- **Data Transformations**: Built-in support for growth rates, differences, and other calculations
- **Caching**: Improved performance through intelligent caching
- **Authentication**: Secure access control for sensitive data
- **Rate Limiting**: Protection against abuse and overuse

## Contributing

The MCP server is part of the EconGraph project. Contributions are welcome! Please see the main project README for contribution guidelines.

## License

This project is licensed under the same terms as the main EconGraph project.

