#!/usr/bin/env python3
"""
Simple Flagsmith Feature Flags Example - Python

Install: pip install flagsmith
Run: FLAGSMITH_ENV_KEY=your-key python python_example.py
"""

import os
from flagsmith import Flagsmith
from flagsmith.models import DefaultFlag

FLAGSMITH_ENV_KEY = os.environ.get("FLAGSMITH_ENV_KEY", "")
FLAGSMITH_API_URL = os.environ.get(
    "FLAGSMITH_API_URL", "https://edge.api.flagsmith.com/api/v1/"
)


def main():
    # Initialize Flagsmith client
    flagsmith = Flagsmith(
        environment_key=FLAGSMITH_ENV_KEY,
        api_url=FLAGSMITH_API_URL,
        # Optional: Set default flags for offline/error scenarios
        default_flag_handler=lambda name: DefaultFlag(
            enabled=False,
            value=None,
        ),
    )

    # --- Example 1: Environment-level flags ---
    print("=== Environment Flags ===")
    env_flags = flagsmith.get_environment_flags()

    # Check if a feature is enabled
    is_new_checkout = env_flags.is_feature_enabled("new-checkout")
    print(f"new-checkout enabled: {is_new_checkout}")

    # Get a remote config value
    api_version = env_flags.get_feature_value("api-version")
    print(f"api-version value: {api_version}")

    # --- Example 2: User-specific flags (Identity) ---
    print("\n=== User-Specific Flags ===")

    # Get flags for a specific user with traits
    user_flags = flagsmith.get_identity_flags(
        identifier="user-123",
        traits={
            "plan": "premium",
            "country": "US",
            "signup_date": "2024-01-15",
        },
    )

    has_beta_access = user_flags.is_feature_enabled("beta-features")
    print(f"user-123 beta-features: {has_beta_access}")

    rate_limit = user_flags.get_feature_value("rate-limit")
    print(f"user-123 rate-limit: {rate_limit}")

    # --- Example 3: Feature flag in application logic ---
    print("\n=== Application Logic Example ===")

    if env_flags.is_feature_enabled("dark-mode"):
        print("Dark mode is available - showing toggle in UI")
    else:
        print("Dark mode disabled - hiding toggle")

    # A/B testing with remote config
    checkout_variant = env_flags.get_feature_value("checkout-variant")
    if checkout_variant == "A":
        print("Showing checkout variant A (single page)")
    elif checkout_variant == "B":
        print("Showing checkout variant B (multi-step)")
    else:
        print("Showing default checkout")

    # --- Example 4: Gradual rollout check ---
    print("\n=== Gradual Rollout ===")
    users = ["alice", "bob", "charlie", "diana"]

    for user_id in users:
        flags = flagsmith.get_identity_flags(user_id)
        has_new_feature = flags.is_feature_enabled("new-dashboard")
        print(f"{user_id}: new-dashboard = {has_new_feature}")

    # --- Example 5: Using with context manager (async) ---
    print("\n=== Additional Patterns ===")

    # Get all flags as a dictionary
    all_flags = env_flags.all_flags()
    print(f"Total flags: {len(all_flags)}")
    for flag in all_flags:
        print(f"  - {flag.feature_name}: enabled={flag.enabled}, value={flag.value}")


if __name__ == "__main__":
    main()
