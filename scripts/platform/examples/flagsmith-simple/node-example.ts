/**
 * Simple Flagsmith Feature Flags Example - Node.js/TypeScript
 *
 * Install: npm install flagsmith-nodejs
 * Run: FLAGSMITH_ENV_KEY=your-key npx ts-node node-example.ts
 */

import Flagsmith from "flagsmith-nodejs";

const FLAGSMITH_ENV_KEY = process.env.FLAGSMITH_ENV_KEY || "";
const FLAGSMITH_API_URL =
  process.env.FLAGSMITH_API_URL || "https://edge.api.flagsmith.com/api/v1/";

async function main() {
  // Initialize Flagsmith client
  const flagsmith = new Flagsmith({
    environmentKey: FLAGSMITH_ENV_KEY,
    apiUrl: FLAGSMITH_API_URL,
  });

  // --- Example 1: Environment-level flags ---
  console.log("=== Environment Flags ===");
  const envFlags = await flagsmith.getEnvironmentFlags();

  // Check if a feature is enabled
  const isNewCheckoutEnabled = envFlags.isFeatureEnabled("new-checkout");
  console.log(`new-checkout enabled: ${isNewCheckoutEnabled}`);

  // Get a remote config value
  const apiVersion = envFlags.getFeatureValue("api-version");
  console.log(`api-version value: ${apiVersion}`);

  // --- Example 2: User-specific flags (Identity) ---
  console.log("\n=== User-Specific Flags ===");

  // Get flags for a specific user with traits
  const userFlags = await flagsmith.getIdentityFlags("user-123", {
    plan: "premium",
    country: "US",
    signupDate: "2024-01-15",
  });

  const hasBetaAccess = userFlags.isFeatureEnabled("beta-features");
  console.log(`user-123 beta-features: ${hasBetaAccess}`);

  const rateLimit = userFlags.getFeatureValue("rate-limit");
  console.log(`user-123 rate-limit: ${rateLimit}`);

  // --- Example 3: Feature flag in application logic ---
  console.log("\n=== Application Logic Example ===");

  if (envFlags.isFeatureEnabled("dark-mode")) {
    console.log("Dark mode is available - showing toggle in UI");
  } else {
    console.log("Dark mode disabled - hiding toggle");
  }

  // A/B testing with remote config
  const checkoutVariant = envFlags.getFeatureValue("checkout-variant");
  switch (checkoutVariant) {
    case "A":
      console.log("Showing checkout variant A (single page)");
      break;
    case "B":
      console.log("Showing checkout variant B (multi-step)");
      break;
    default:
      console.log("Showing default checkout");
  }

  // --- Example 4: Gradual rollout check ---
  console.log("\n=== Gradual Rollout ===");
  const users = ["alice", "bob", "charlie", "diana"];

  for (const userId of users) {
    const flags = await flagsmith.getIdentityFlags(userId);
    const hasNewFeature = flags.isFeatureEnabled("new-dashboard");
    console.log(`${userId}: new-dashboard = ${hasNewFeature}`);
  }
}

main().catch(console.error);
