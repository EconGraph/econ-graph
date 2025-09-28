# EconGraph MCP

Model Context Protocol server implementation for the EconGraph system, providing AI model integration for economic data access. This crate enables AI assistants to interact with economic data through a standardized protocol, bridging the gap between AI models and economic data systems.

## Features

- **MCP Server**: Full Model Context Protocol server implementation
- **AI Integration**: Seamless integration with AI models and assistants
- **Economic Data Access**: Secure access to economic data through AI interfaces
- **Protocol Compliance**: Full compliance with MCP specification
- **Authentication**: Secure authentication for AI model access
- **Data Filtering**: Intelligent data filtering and access control

## Testing

The crate includes comprehensive tests to ensure MCP protocol compliance, AI integration, and data access functionality work correctly.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual MCP components in isolation
- **Examples**: Protocol message handling, data access logic, authentication, filtering
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test MCP server with real AI model interactions
- **Examples**: End-to-end AI model communication, data access workflows, protocol compliance
- **Benefits**: Catch integration issues, test real-world scenarios, validate AI interactions

#### **Protocol Tests**
- **Purpose**: Validate MCP protocol compliance and message handling
- **Examples**: Protocol message parsing, response generation, error handling, compliance validation
- **Benefits**: Ensure protocol compliance, validate message handling, test error scenarios

#### **AI Integration Tests**
- **Purpose**: Test AI model integration and data access functionality
- **Examples**: AI model communication, data filtering, access control, authentication
- **Benefits**: Ensure AI integration works, validate data access, test security measures

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test mcp_server
cargo test protocol
cargo test integration

# Run with AI model testing
AI_MODEL_TESTING=true cargo test
```

### Test Infrastructure

- **AI Model Mocking**: Controlled testing of AI model interactions
- **Protocol Testing**: Comprehensive MCP protocol compliance validation
- **Data Access Testing**: Secure data access and filtering validation
- **Authentication Testing**: AI model authentication and authorization testing

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
