#!/bin/bash
# Pack Testing Framework Demo
#
# This script demonstrates the pack testing capabilities in Attune

set -e

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Attune Pack Testing Framework Demo"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Navigate to project root
cd "$(dirname "$0")/../.."

# Build the CLI if needed
if [ ! -f "./target/debug/attune" ]; then
    echo "🔨 Building Attune CLI..."
    cargo build --package attune-cli
    echo ""
fi

ATTUNE_CLI="./target/debug/attune"

echo "📦 Testing Core Pack"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Basic test execution
echo "1️⃣  Basic Test Execution"
echo "   Command: attune pack test packs/core"
echo ""
$ATTUNE_CLI pack test packs/core
echo ""

# JSON output
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "2️⃣  JSON Output (for scripting)"
echo "   Command: attune pack test packs/core --output json"
echo ""
RUST_LOG=error $ATTUNE_CLI pack test packs/core --output json | jq '{
  pack: .packRef,
  version: .packVersion,
  totalTests: .totalTests,
  passed: .passed,
  failed: .failed,
  passRate: (.passRate * 100 | tostring + "%"),
  duration: (.durationMs / 1000 | tostring + "s"),
  suites: .testSuites | map({
    name: .name,
    type: .runnerType,
    passed: .passed,
    total: .total
  })
}'
echo ""

# Verbose output
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "3️⃣  Verbose Output (shows test cases)"
echo "   Command: attune pack test packs/core --verbose"
echo ""
$ATTUNE_CLI pack test packs/core --verbose
echo ""

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Demo Complete!"
echo ""
echo "Available Commands:"
echo "  attune pack test <pack>              # Test a pack"
echo "  attune pack test <pack> --verbose    # Show test case details"
echo "  attune pack test <pack> --detailed   # Show stdout/stderr"
echo "  attune pack test <pack> --output json  # JSON output"
echo "  attune pack test <pack> --output yaml  # YAML output"
echo ""
echo "Test Configuration:"
echo "  Pack tests are configured in pack.yaml under the 'testing' section"
echo "  See packs/core/pack.yaml for an example"
echo ""
echo "Documentation:"
echo "  docs/pack-testing-framework.md - Full design document"
echo "  packs/core/tests/README.md - Core pack test documentation"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
