#!/bin/bash
set -e

# CI/CD Workflow Diagram Generator
# Generates Mermaid diagrams that look like GitHub Actions workflow visualizations

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="$(dirname "$SCRIPT_DIR")/docs/diagrams"

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "🎨 Generating CI/CD Workflow Diagrams..."

# Generate the current unified workflow diagram
cat > "$OUTPUT_DIR/current-workflow.mmd" << 'EOF'
graph TD
    A[Code Push/PR] --> B[Checkout Code]
    B --> C[Setup Docker Buildx]
    C --> D[Build Backend Image]
    C --> E[Build Frontend Image]
    C --> F[Build Test Runner Image]

    D --> G[Push to Registry]
    E --> G
    F --> G

    G --> H[Pull Images]
    H --> I[Run Core Tests]
    H --> J[Run Analysis Tests]
    H --> K[Run Debug Tests]
    H --> L[Run Comprehensive Tests]
    H --> M[Run Mobile Core Tests]
    H --> N[Run Mobile Analysis Tests]
    H --> O[Run Mobile Comprehensive Tests]

    I --> P[Upload Results]
    J --> P
    K --> P
    L --> P
    M --> P
    N --> P
    O --> P

    P --> Q[Deploy/Notify]

    style A fill:#e1f5fe
    style G fill:#f3e5f5
    style P fill:#e8f5e8
    style Q fill:#fff3e0
EOF

# Generate the separated build/run workflow diagram
cat > "$OUTPUT_DIR/separated-workflow.mmd" << 'EOF'
graph TD
    subgraph "Build Phase"
        A[Code Push/PR] --> B[Checkout Code]
        B --> C[Setup Docker Buildx]
        C --> D[Build Backend Image]
        C --> E[Build Frontend Image]
        C --> F[Build Test Runner Image]
        D --> G[Save Images as Artifacts]
        E --> G
        F --> G
    end

    subgraph "Test Phase"
        G --> H[Download Images]
        H --> I[Load Images]
        I --> J[Run Core Tests]
        I --> K[Run Analysis Tests]
        I --> L[Run Debug Tests]
        I --> M[Run Comprehensive Tests]
        I --> N[Run Mobile Core Tests]
        I --> O[Run Mobile Analysis Tests]
        I --> P[Run Mobile Comprehensive Tests]

        J --> Q[Upload Test Results]
        K --> Q
        L --> Q
        M --> Q
        N --> Q
        O --> Q
        P --> Q
    end

    Q --> R[Deploy/Notify]

    style A fill:#e1f5fe
    style G fill:#f3e5f5
    style Q fill:#e8f5e8
    style R fill:#fff3e0
EOF

# Generate the optimized workflow with caching
cat > "$OUTPUT_DIR/optimized-workflow.mmd" << 'EOF'
graph TD
    subgraph "Build Phase (with Caching)"
        A[Code Push/PR] --> B[Checkout Code]
        B --> C[Setup Docker Buildx]
        C --> D[Login to Registry]
        D --> E[Build & Push Backend]
        D --> F[Build & Push Frontend]
        D --> G[Build & Push Test Runner]

        E --> H[Cache Layers]
        F --> H
        G --> H
    end

    subgraph "Test Phase (Pull & Run)"
        H --> I[Pull Images from Registry]
        I --> J[Tag for Local Use]
        J --> K[Run Core Tests]
        J --> L[Run Analysis Tests]
        J --> M[Run Debug Tests]
        J --> N[Run Comprehensive Tests]
        J --> O[Run Mobile Core Tests]
        J --> P[Run Mobile Analysis Tests]
        J --> Q[Run Mobile Comprehensive Tests]

        K --> R[Upload Test Results]
        L --> R
        M --> R
        N --> R
        O --> R
        P --> R
        Q --> R
    end

    R --> S[Deploy/Notify]

    style A fill:#e1f5fe
    style H fill:#f3e5f5
    style R fill:#e8f5e8
    style S fill:#fff3e0
EOF

# Generate the test group architecture diagram
cat > "$OUTPUT_DIR/test-architecture.mmd" << 'EOF'
graph TD
    subgraph "Test Groups"
        A[Core Tests<br/>Authentication<br/>Series Explorer<br/>Visual Checks]
        B[Analysis Tests<br/>Professional Analysis<br/>Global Analysis]
        C[Debug Tests<br/>Visual Checks<br/>Console Logging]
        D[Comprehensive Tests<br/>Complete Workflow<br/>End-to-End Journey]
        E[Mobile Core Tests<br/>Mobile Authentication<br/>Mobile Navigation]
        F[Mobile Analysis Tests<br/>Mobile Charts<br/>Mobile Interactions]
        G[Mobile Comprehensive Tests<br/>Mobile Workflows]
    end

    subgraph "Infrastructure"
        H[PostgreSQL Database]
        I[Backend Service]
        J[Frontend Service]
        K[Test Runner Container]
    end

    H --> I
    I --> J
    J --> K

    K --> A
    K --> B
    K --> C
    K --> D
    K --> E
    K --> F
    K --> G

    style A fill:#e3f2fd
    style B fill:#e8f5e8
    style C fill:#fff3e0
    style D fill:#fce4ec
    style E fill:#f3e5f5
    style F fill:#e0f2f1
    style G fill:#fff8e1
    style H fill:#ffebee
    style I fill:#e8eaf6
    style J fill:#e0f7fa
    style K fill:#f1f8e9
EOF

# Generate the Docker architecture diagram
cat > "$OUTPUT_DIR/docker-architecture.mmd" << 'EOF'
graph TD
    subgraph "Docker Services"
        A[PostgreSQL<br/>Database]
        B[Backend<br/>Rust Service]
        C[Frontend<br/>React App]
        D[Test Runner<br/>Playwright]
    end

    subgraph "Docker Images"
        E[postgres:17]
        F[econ-graph-backend<br/>Built from Cargo.toml]
        G[econ-graph-frontend<br/>Built from package.json]
        H[econ-graph-test-runner<br/>Unified with args]
    end

    subgraph "Test Execution"
        I[Core Tests]
        J[Analysis Tests]
        K[Debug Tests]
        L[Comprehensive Tests]
        M[Mobile Core Tests]
        N[Mobile Analysis Tests]
        O[Mobile Comprehensive Tests]
    end

    E --> A
    F --> B
    G --> C
    H --> D

    A --> B
    B --> C
    C --> D

    D --> I
    D --> J
    D --> K
    D --> L
    D --> M
    D --> N
    D --> O

    style A fill:#ffebee
    style B fill:#e8eaf6
    style C fill:#e0f7fa
    style D fill:#f1f8e9
    style E fill:#fce4ec
    style F fill:#e3f2fd
    style G fill:#e8f5e8
    style H fill:#fff3e0
EOF

# Generate a performance comparison diagram
cat > "$OUTPUT_DIR/performance-comparison.mmd" << 'EOF'
graph LR
    subgraph "Before: Sequential"
        A1[Build Backend] --> A2[Build Frontend]
        A2 --> A3[Build Test Runner]
        A3 --> A4[Run Core Tests]
        A4 --> A5[Run Analysis Tests]
        A5 --> A6[Run Debug Tests]
        A6 --> A7[Run Comprehensive Tests]
        A7 --> A8[Run Mobile Tests]
        A8 --> A9[Total: ~45 minutes]
    end

    subgraph "After: Parallel"
        B1[Build All Images<br/>in Parallel]
        B2[Run All Test Groups<br/>in Parallel]
        B1 --> B2
        B2 --> B3[Total: ~15 minutes]
    end

    style A9 fill:#ffcdd2
    style B3 fill:#c8e6c9
EOF

echo "✅ Generated workflow diagrams:"
echo "  📊 $OUTPUT_DIR/current-workflow.mmd"
echo "  📊 $OUTPUT_DIR/separated-workflow.mmd"
echo "  📊 $OUTPUT_DIR/optimized-workflow.mmd"
echo "  📊 $OUTPUT_DIR/test-architecture.mmd"
echo "  📊 $OUTPUT_DIR/docker-architecture.mmd"
echo "  📊 $OUTPUT_DIR/performance-comparison.mmd"
echo ""
echo "🎨 To view these diagrams:"
echo "  1. Copy the .mmd content to https://mermaid.live/"
echo "  2. Or use VS Code with Mermaid extension"
echo "  3. Or use GitHub (renders Mermaid automatically in .md files)"
echo ""
echo "📝 To include in documentation, add to .md files:"
echo "  \`\`\`mermaid"
echo "  [paste .mmd content here]"
echo "  \`\`\`"
