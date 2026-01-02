#!/usr/bin/env bash
# Simple Flagsmith Feature Flags Examples - curl/shell
#
# These examples show direct API usage without SDKs.
# Useful for debugging, scripting, or when no SDK is available.
#
# Usage: FLAGSMITH_ENV_KEY=your-key ./curl_examples.sh

set -euo pipefail

FLAGSMITH_ENV_KEY="${FLAGSMITH_ENV_KEY:-}"
FLAGSMITH_API_URL="${FLAGSMITH_API_URL:-https://edge.api.flagsmith.com/api/v1}"

if [[ -z "$FLAGSMITH_ENV_KEY" ]]; then
    echo "Error: FLAGSMITH_ENV_KEY environment variable required"
    exit 1
fi

echo "=== Example 1: Get All Environment Flags ==="
curl -s -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    "$FLAGSMITH_API_URL/flags/" | jq '.'

echo ""
echo "=== Example 2: Get Specific Flag ==="
# Get all flags and filter for a specific one
curl -s -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    "$FLAGSMITH_API_URL/flags/" | jq '.[] | select(.feature.name == "new-checkout")'

echo ""
echo "=== Example 3: Get Flags for User Identity ==="
curl -s -X POST \
    -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    -H "Content-Type: application/json" \
    -d '{
        "identifier": "user-123",
        "traits": [
            {"trait_key": "plan", "trait_value": "premium"},
            {"trait_key": "country", "trait_value": "US"}
        ]
    }' \
    "$FLAGSMITH_API_URL/identities/" | jq '.'

echo ""
echo "=== Example 4: Check if Feature is Enabled (scripting) ==="
is_enabled() {
    local feature_name="$1"
    local result
    result=$(curl -s -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
        "$FLAGSMITH_API_URL/flags/" | \
        jq -r --arg name "$feature_name" '.[] | select(.feature.name == $name) | .enabled')

    if [[ "$result" == "true" ]]; then
        return 0
    else
        return 1
    fi
}

if is_enabled "new-checkout"; then
    echo "new-checkout is ENABLED"
else
    echo "new-checkout is DISABLED"
fi

echo ""
echo "=== Example 5: Get Feature Value ==="
get_feature_value() {
    local feature_name="$1"
    curl -s -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
        "$FLAGSMITH_API_URL/flags/" | \
        jq -r --arg name "$feature_name" '.[] | select(.feature.name == $name) | .feature_state_value // "null"'
}

echo "api-version value: $(get_feature_value 'api-version')"
echo "rate-limit value: $(get_feature_value 'rate-limit')"

echo ""
echo "=== Example 6: List All Feature Names ==="
curl -s -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    "$FLAGSMITH_API_URL/flags/" | \
    jq -r '.[] | "\(.feature.name): enabled=\(.enabled), value=\(.feature_state_value // "none")"'

echo ""
echo "=== Example 7: User Identity with Traits (for targeting) ==="
# This creates/updates the identity with traits and returns their flags
curl -s -X POST \
    -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    -H "Content-Type: application/json" \
    -d '{
        "identifier": "premium-user@example.com",
        "traits": [
            {"trait_key": "subscription_tier", "trait_value": "enterprise"},
            {"trait_key": "company_size", "trait_value": 500},
            {"trait_key": "beta_tester", "trait_value": true}
        ]
    }' \
    "$FLAGSMITH_API_URL/identities/" | jq '.flags[] | {name: .feature.name, enabled: .enabled, value: .feature_state_value}'

echo ""
echo "=== Example 8: Health Check ==="
# Check if Flagsmith API is reachable
if curl -s -o /dev/null -w "%{http_code}" -H "X-Environment-Key: $FLAGSMITH_ENV_KEY" \
    "$FLAGSMITH_API_URL/flags/" | grep -q "200"; then
    echo "Flagsmith API is healthy"
else
    echo "Flagsmith API is not reachable"
fi
