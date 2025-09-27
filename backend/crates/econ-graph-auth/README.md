# EconGraph Auth

The authentication and authorization layer for the EconGraph system, providing secure user management, OAuth integration, and enterprise-grade security features. This crate serves as the security foundation that enables secure access control across all EconGraph components with comprehensive user authentication and authorization capabilities.

## Features

- **OAuth Integration**: Support for Google, GitHub, and other OAuth providers
- **JWT Authentication**: Secure token-based authentication with configurable expiration
- **Role-Based Access Control**: Granular permissions and role management
- **User Management**: Complete user lifecycle management and profile handling
- **Security Middleware**: Request authentication and authorization middleware
- **Session Management**: Secure session handling and token refresh

## Testing

The crate includes comprehensive tests to ensure authentication security, OAuth integration, and authorization functionality work correctly.

### Test Types

#### **Unit Tests**
- **Purpose**: Test individual authentication components in isolation
- **Examples**: JWT token generation/validation, OAuth flow handling, role checking, middleware logic
- **Benefits**: Fast execution, no external dependencies, catch logic errors early

#### **Integration Tests**
- **Purpose**: Test authentication flows with real OAuth providers and database interactions
- **Examples**: OAuth callback handling, user registration, session management, role assignment
- **Benefits**: Catch integration issues, test real-world scenarios, validate OAuth flows

#### **Security Tests**
- **Purpose**: Validate authentication security measures and attack prevention
- **Examples**: Token validation, session hijacking prevention, CSRF protection, rate limiting
- **Benefits**: Ensure security measures work, prevent vulnerabilities, validate attack resistance

#### **Middleware Tests**
- **Purpose**: Test authentication and authorization middleware functionality
- **Examples**: Request authentication, permission checking, role validation, error handling
- **Benefits**: Ensure middleware works correctly, validate authorization logic, test error scenarios

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test auth::handlers
cargo test auth::middleware
cargo test integration

# Run with OAuth provider testing
OAUTH_TESTING=true cargo test
```

### Test Infrastructure

- **OAuth Mocking**: Controlled testing of OAuth provider integrations
- **Database Integration**: Real database operations with test user setup and cleanup
- **Security Testing**: Automated security measure validation and attack simulation
- **Middleware Testing**: Request/response cycle testing with authentication middleware

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
