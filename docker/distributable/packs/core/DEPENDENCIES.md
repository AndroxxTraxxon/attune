# Core Pack Dependencies

**Philosophy:** The core pack has **zero runtime dependencies** beyond standard system utilities.

## Why Zero Dependencies?

1. **Portability:** Works in any environment with standard Unix utilities
2. **Reliability:** No version conflicts, no package installation failures
3. **Security:** Minimal attack surface, no third-party library vulnerabilities
4. **Performance:** Fast startup, no runtime initialization overhead
5. **Simplicity:** Easy to audit, test, and maintain

## Required System Utilities

All core pack actions rely only on utilities available in standard Linux/Unix environments:

| Utility | Purpose | Used By |
|---------|---------|---------|
| `bash` | Shell scripting | All shell actions |
| `jq` | JSON parsing/generation | All actions (parameter handling) |
| `curl` | HTTP client | `http_request.sh` |
| Standard Unix tools | Text processing, file operations | Various actions |

These utilities are:
- ✅ Pre-installed in all Attune worker containers
- ✅ Standard across Linux distributions
- ✅ Stable, well-tested, and widely used
- ✅ Available via package managers if needed

## No Runtime Dependencies

The core pack **does not require:**
- ❌ Python interpreter or packages
- ❌ Node.js runtime or npm modules
- ❌ Ruby, Perl, or other scripting languages
- ❌ Third-party libraries or frameworks
- ❌ Package installations at runtime

## Action Implementation Guidelines

### ✅ Preferred Approaches

**Use bash + standard utilities:**
```bash
#!/bin/bash
# Read params with jq
INPUT=$(cat)
PARAM=$(echo "$INPUT" | jq -r '.param // "default"')

# Process with standard tools
RESULT=$(echo "$PARAM" | tr '[:lower:]' '[:upper:]')

# Output with jq
jq -n --arg result "$RESULT" '{result: $result}'
```

**Use curl for HTTP:**
```bash
# Make HTTP requests with curl
curl -s -X POST "$URL" \
    -H "Content-Type: application/json" \
    -d '{"key": "value"}'
```

**Use jq for JSON processing:**
```bash
# Parse JSON responses
echo "$RESPONSE" | jq '.data.items[] | .name'

# Generate JSON output
jq -n \
    --arg status "success" \
    --argjson count 42 \
    '{status: $status, count: $count}'
```

### ❌ Avoid

**Don't add runtime dependencies:**
```bash
# ❌ DON'T DO THIS
pip install requests
python3 script.py

# ❌ DON'T DO THIS
npm install axios
node script.js

# ❌ DON'T DO THIS
gem install httparty
ruby script.rb
```

**Don't use language-specific features:**
```python
# ❌ DON'T DO THIS in core pack
#!/usr/bin/env python3
import requests  # External dependency!
response = requests.get(url)
```

Instead, use bash + curl:
```bash
# ✅ DO THIS in core pack
#!/bin/bash
response=$(curl -s "$url")
```

## When Runtime Dependencies Are Acceptable

For **custom packs** (not core pack), runtime dependencies are fine:
- ✅ Pack-specific Python libraries (installed in pack virtualenv)
- ✅ Pack-specific npm modules (installed in pack node_modules)
- ✅ Language runtimes (Python, Node.js) for complex logic
- ✅ Specialized tools for specific integrations

The core pack serves as a foundation with zero dependencies. Custom packs can have dependencies managed via:
- `requirements.txt` for Python packages
- `package.json` for Node.js modules
- Pack runtime environments (isolated per pack)

## Migration from Runtime Dependencies

If an action currently uses a runtime dependency, consider:

1. **Can it be done with bash + standard utilities?**
   - Yes → Rewrite in bash
   - No → Consider if it belongs in core pack

2. **Is the functionality complex?**
   - Simple HTTP/JSON → Use curl + jq
   - Complex API client → Move to custom pack

3. **Is it a specialized integration?**
   - Yes → Move to integration-specific pack
   - No → Keep in core pack with bash implementation

### Example: http_request Migration

**Before (Python with dependency):**
```python
#!/usr/bin/env python3
import requests  # ❌ External dependency

response = requests.get(url, headers=headers)
print(response.json())
```

**After (Bash with standard utilities):**
```bash
#!/bin/bash
# ✅ No dependencies beyond curl + jq

response=$(curl -s -H "Authorization: Bearer $TOKEN" "$URL")
echo "$response" | jq '.'
```

## Testing Without Dependencies

Core pack actions can be tested anywhere with standard utilities:

```bash
# Local testing (no installation needed)
echo '{"param": "value"}' | ./action.sh

# Docker testing (minimal base image)
docker run --rm -i alpine:latest sh -c '
    apk add --no-cache bash jq curl &&
    /bin/bash < action.sh
'

# CI/CD testing (standard tools available)
./action.sh < test-params.json
```

## Benefits Realized

### For Developers
- No dependency management overhead
- Immediate action execution (no runtime setup)
- Easy to test locally
- Simple to audit and debug

### For Operators
- No version conflicts between packs
- No package installation failures
- Faster container startup
- Smaller container images

### For Security
- Minimal attack surface
- No third-party library vulnerabilities
- Easier to audit (standard tools only)
- No supply chain risks

### For Performance
- Fast action startup (no runtime initialization)
- Low memory footprint
- No package loading overhead
- Efficient resource usage

## Standard Utility Reference

### jq (JSON Processing)
```bash
# Parse input
VALUE=$(echo "$JSON" | jq -r '.key')

# Generate output
jq -n --arg val "$VALUE" '{result: $val}'

# Transform data
echo "$JSON" | jq '.items[] | select(.active)'
```

### curl (HTTP Client)
```bash
# GET request
curl -s "$URL"

# POST with JSON
curl -s -X POST "$URL" \
    -H "Content-Type: application/json" \
    -d '{"key": "value"}'

# With authentication
curl -s -H "Authorization: Bearer $TOKEN" "$URL"
```

### Standard Text Tools
```bash
# grep - Pattern matching
echo "$TEXT" | grep "pattern"

# sed - Text transformation
echo "$TEXT" | sed 's/old/new/g'

# awk - Text processing
echo "$TEXT" | awk '{print $1}'

# tr - Character translation
echo "$TEXT" | tr '[:lower:]' '[:upper:]'
```

## Future Considerations

The core pack will:
- ✅ Continue to have zero runtime dependencies
- ✅ Use only standard Unix utilities
- ✅ Serve as a reference implementation
- ✅ Provide foundational actions for workflows

Custom packs may:
- ✅ Have runtime dependencies (Python, Node.js, etc.)
- ✅ Use specialized libraries for integrations
- ✅ Require specific tools or SDKs
- ✅ Manage dependencies via pack environments

## Summary

**Core Pack = Zero Dependencies + Standard Utilities**

This philosophy ensures the core pack is:
- Portable across all environments
- Reliable without version conflicts
- Secure with minimal attack surface
- Performant with fast startup
- Simple to test and maintain

For actions requiring runtime dependencies, create custom packs with proper dependency management via `requirements.txt`, `package.json`, or similar mechanisms.