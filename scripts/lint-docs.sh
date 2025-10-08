#!/bin/bash
set -e

echo "🔍 Linting documentation..."

# Check if markdownlint is installed
if ! command -v markdownlint &> /dev/null; then
    echo "Installing markdownlint-cli..."
    npm install -g markdownlint-cli
fi

# Check if mermaid CLI is installed
if ! command -v mmdc &> /dev/null; then
    echo "Installing @mermaid-js/mermaid-cli..."
    npm install -g @mermaid-js/mermaid-cli
fi

# Lint markdown files
echo "📝 Linting Markdown syntax..."
markdownlint docs/**/*.md

# Extract and validate Mermaid diagrams
echo "📊 Validating Mermaid diagrams..."
find docs -name "*.md" -exec grep -l "\`\`\`mermaid" {} \; | while read -r file; do
    echo "Checking Mermaid diagrams in: $file"
    # This is a basic validation - you could extract diagrams to separate files for more precise checking
    if grep -A 50 "\`\`\`mermaid" "$file" | grep -B 50 "\`\`\`" | head -n -1 | tail -n +2 > /tmp/mermaid_temp.mmd; then
        mmdc -i /tmp/mermaid_temp.mmd -o /dev/null --parseOnly || echo "⚠️  Mermaid syntax error in $file"
    fi
done

echo "✅ Documentation linting complete!"