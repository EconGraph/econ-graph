# Private Chart API for MCP Server Integration

## Overview

The Private Chart API provides secure chart generation capabilities for the MCP (Model Context Protocol) server. This API is **only accessible from the backend/MCP server within the private network** and uses the existing frontend charting components to generate professional-quality chart configurations.

## Architecture

```
┌─────────────────┐    Private Network    ┌──────────────────┐
│   MCP Server    │ ────────────────────► │  Private Chart   │
│   (Backend)     │                       │      API         │
│                 │                       │  (Frontend)      │
└─────────────────┘                       └──────────────────┘
         │                                         │
         │                                         │
         ▼                                         ▼
┌─────────────────┐                       ┌──────────────────┐
│   GraphQL API   │                       │   Chart.js       │
│   (Data Source) │                       │   Components     │
└─────────────────┘                       └──────────────────┘
```

## Security Features

### Network Access Control
- **Internal Network Only**: Only accessible from `127.0.0.1` and private IP ranges
- **Header Validation**: Requires `X-MCP-Server-Request: true` header
- **CORS Disabled**: No cross-origin requests allowed
- **IP Whitelist**: Validates against internal network ranges

### Request Validation
- **Method Restrictions**: Only POST requests for chart generation
- **Content Validation**: Validates chart request structure
- **Size Limits**: 10MB request body limit

## API Endpoints

### 1. Chart Generation
**POST** `/api/private/chart/generate`

Generates a complete Chart.js configuration for economic data visualization.

#### Request Body
```json
{
  "seriesData": [
    {
      "id": "series-1",
      "name": "GDP",
      "dataPoints": [
        {
          "date": "2020-01-01",
          "value": 100.0
        },
        {
          "date": "2020-02-01", 
          "value": 101.5
        }
      ],
      "color": "#1976d2",
      "type": "line"
    }
  ],
  "chartType": "line",
  "title": "Economic Data Visualization",
  "startDate": "2020-01-01",
  "endDate": "2024-01-01",
  "showLegend": true,
  "showGrid": true,
  "yAxisLabel": "Value",
  "xAxisLabel": "Date"
}
```

#### Response
```json
{
  "success": true,
  "chartConfig": {
    "type": "line",
    "data": {
      "datasets": [
        {
          "label": "GDP",
          "data": [
            {"x": "2020-01-01", "y": 100.0},
            {"x": "2020-02-01", "y": 101.5}
          ],
          "borderColor": "#1976d2",
          "backgroundColor": "#1976d220",
          "fill": false,
          "tension": 0.1
        }
      ]
    },
    "options": {
      "responsive": true,
      "plugins": {
        "title": {
          "display": true,
          "text": "Economic Data Visualization"
        },
        "legend": {
          "display": true,
          "position": "top"
        }
      },
      "scales": {
        "x": {
          "type": "time",
          "title": {
            "display": true,
            "text": "Date"
          }
        },
        "y": {
          "title": {
            "display": true,
            "text": "Value"
          }
        }
      }
    }
  },
  "chartData": { /* Chart.js data object */ },
  "metadata": {
    "seriesCount": 1,
    "dataPointCount": 2,
    "dateRange": {
      "start": "2020-01-01",
      "end": "2020-02-01"
    }
  },
  "message": "Chart configuration generated successfully"
}
```

### 2. Health Check
**GET** `/api/private/chart/health`

Returns the health status of the private chart API.

#### Response
```json
{
  "success": true,
  "message": "Private chart API is healthy",
  "timestamp": "2024-01-15T10:30:00.000Z",
  "version": "1.0.0"
}
```

## MCP Server Integration

### Enhanced Visualization Tool

The MCP server's `create_data_visualization` tool now:

1. **Collects Data**: Retrieves economic series data via GraphQL
2. **Calls Private API**: Sends chart request to private frontend API
3. **Returns Chart Config**: Provides complete Chart.js configuration
4. **Fallback Support**: Falls back to data structure if API unavailable

### Example MCP Tool Usage

```json
{
  "tool": "create_data_visualization",
  "arguments": {
    "series_ids": ["uuid-1", "uuid-2"],
    "chart_type": "line",
    "title": "GDP vs Unemployment Rate",
    "start_date": "2020-01-01",
    "end_date": "2024-01-01"
  }
}
```

### Response Format

```json
{
  "content": [{
    "type": "text",
    "text": "✅ Chart visualization generated successfully using private frontend API!\n\n📊 Chart Configuration:\n{...}\n\n📈 Metadata:\n{...}"
  }],
  "is_error": false
}
```

## Setup and Configuration

### 1. Install Dependencies

```bash
cd frontend
npm install express@^4.18.2 cors@^2.8.5
```

### 2. Start Private Chart API Server

```bash
# Development
npm run start-private-chart-api

# Or directly
node src/server/privateChartServer.js
```

### 3. Configure MCP Server

Set environment variable for chart API URL:

```bash
export FRONTEND_CHART_API_URL="http://127.0.0.1:3001/api/private/chart"
```

### 4. Start Backend with MCP Server

```bash
cd backend
cargo run
```

## Chart Types Supported

### Line Charts
- **Use Case**: Time series data, trends over time
- **Features**: Smooth curves, multiple series overlay
- **Example**: GDP growth, unemployment rates

### Bar Charts  
- **Use Case**: Categorical comparisons, discrete data
- **Features**: Grouped bars, stacked bars
- **Example**: Quarterly GDP, sector contributions

### Scatter Plots
- **Use Case**: Correlation analysis, relationship visualization
- **Features**: Point markers, trend lines
- **Example**: GDP vs inflation, employment vs wages

## Advanced Features

### Time Series Support
- **Automatic Date Parsing**: Handles various date formats
- **Time Scale**: Uses Chart.js time scale for proper date handling
- **Date Formatting**: Configurable display formats

### Professional Styling
- **Color Schemes**: Consistent color palettes
- **Typography**: Professional fonts and sizing
- **Grid Lines**: Configurable grid display
- **Legends**: Positioned legends with proper styling

### Data Validation
- **Series Validation**: Ensures valid data structure
- **Date Validation**: Validates date formats and ranges
- **Value Validation**: Handles numeric data properly

## Error Handling

### API Errors
- **400 Bad Request**: Invalid chart request format
- **403 Forbidden**: Access denied (not from MCP server)
- **500 Internal Error**: Server-side processing error

### Fallback Behavior
- **Network Issues**: Falls back to data structure format
- **API Unavailable**: Returns structured data for manual visualization
- **Invalid Data**: Provides error messages with guidance

## Monitoring and Logging

### Request Logging
- **Access Logs**: All requests logged with IP addresses
- **Error Logs**: Detailed error information for debugging
- **Performance Metrics**: Response times and success rates

### Health Monitoring
- **Health Endpoint**: Regular health checks
- **Status Reporting**: API availability status
- **Version Information**: API version tracking

## Security Considerations

### Network Security
- **Private Network Only**: No external access
- **IP Validation**: Strict IP range checking
- **Header Validation**: Required security headers

### Data Security
- **No Data Persistence**: Charts generated on-demand
- **Memory Cleanup**: Automatic cleanup of temporary data
- **Request Size Limits**: Prevents resource exhaustion

### Access Control
- **MCP Server Only**: Restricted to backend requests
- **Header Authentication**: Custom headers for validation
- **Rate Limiting**: Prevents abuse (future enhancement)

## Future Enhancements

### Chart Export
- **Image Generation**: PNG/SVG chart export
- **PDF Reports**: Multi-chart PDF generation
- **Interactive Charts**: Enhanced interactivity

### Advanced Visualizations
- **Candlestick Charts**: Financial data visualization
- **Heat Maps**: Correlation matrices
- **3D Charts**: Three-dimensional data visualization

### Performance Optimization
- **Caching**: Chart configuration caching
- **Compression**: Response compression
- **CDN Integration**: Chart asset delivery

## Troubleshooting

### Common Issues

1. **Connection Refused**
   - Ensure private chart API server is running
   - Check port 3001 is available
   - Verify firewall settings

2. **Access Denied**
   - Confirm request is from MCP server
   - Check security headers are present
   - Verify IP address is in allowed range

3. **Invalid Chart Request**
   - Validate series data structure
   - Check required fields are present
   - Ensure data types are correct

### Debug Mode

Enable debug logging:

```bash
DEBUG=private-chart-api node src/server/privateChartServer.js
```

### Health Check

Test API availability:

```bash
curl -H "X-MCP-Server-Request: true" http://127.0.0.1:3001/api/private/chart/health
```

## Conclusion

The Private Chart API provides a secure, efficient way for the MCP server to generate professional-quality chart configurations using the existing frontend charting infrastructure. With proper security controls and fallback mechanisms, it ensures reliable chart generation while maintaining the privacy and security of the internal network.
