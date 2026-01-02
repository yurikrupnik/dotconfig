// Simple Flagsmith Feature Flags Example - Go
//
// Install: go get github.com/Flagsmith/flagsmith-go-client/v3
// Run: FLAGSMITH_ENV_KEY=your-key go run go_example.go
package main

import (
	"fmt"
	"log"
	"os"

	flagsmith "github.com/Flagsmith/flagsmith-go-client/v3"
)

func main() {
	envKey := os.Getenv("FLAGSMITH_ENV_KEY")
	if envKey == "" {
		log.Fatal("FLAGSMITH_ENV_KEY environment variable required")
	}

	// Initialize Flagsmith client
	client := flagsmith.NewClient(envKey,
		flagsmith.WithBaseURL(getEnvOrDefault("FLAGSMITH_API_URL", "https://edge.api.flagsmith.com/api/v1/")),
	)

	// --- Example 1: Environment-level flags ---
	fmt.Println("=== Environment Flags ===")
	envFlags, err := client.GetEnvironmentFlags()
	if err != nil {
		log.Fatalf("Failed to get environment flags: %v", err)
	}

	// Check if a feature is enabled
	isNewCheckout, err := envFlags.IsFeatureEnabled("new-checkout")
	if err != nil {
		fmt.Printf("new-checkout: error - %v\n", err)
	} else {
		fmt.Printf("new-checkout enabled: %v\n", isNewCheckout)
	}

	// Get a remote config value
	apiVersion, err := envFlags.GetFeatureValue("api-version")
	if err != nil {
		fmt.Printf("api-version: error - %v\n", err)
	} else {
		fmt.Printf("api-version value: %v\n", apiVersion)
	}

	// --- Example 2: User-specific flags (Identity) ---
	fmt.Println("\n=== User-Specific Flags ===")

	// Get flags for a specific user with traits
	traits := []*flagsmith.Trait{
		{TraitKey: "plan", TraitValue: "premium"},
		{TraitKey: "country", TraitValue: "US"},
		{TraitKey: "signup_date", TraitValue: "2024-01-15"},
	}

	userFlags, err := client.GetIdentityFlags("user-123", traits)
	if err != nil {
		log.Fatalf("Failed to get identity flags: %v", err)
	}

	hasBetaAccess, _ := userFlags.IsFeatureEnabled("beta-features")
	fmt.Printf("user-123 beta-features: %v\n", hasBetaAccess)

	rateLimit, _ := userFlags.GetFeatureValue("rate-limit")
	fmt.Printf("user-123 rate-limit: %v\n", rateLimit)

	// --- Example 3: Feature flag in application logic ---
	fmt.Println("\n=== Application Logic Example ===")

	darkMode, _ := envFlags.IsFeatureEnabled("dark-mode")
	if darkMode {
		fmt.Println("Dark mode is available - showing toggle in UI")
	} else {
		fmt.Println("Dark mode disabled - hiding toggle")
	}

	// A/B testing with remote config
	checkoutVariant, _ := envFlags.GetFeatureValue("checkout-variant")
	switch checkoutVariant {
	case "A":
		fmt.Println("Showing checkout variant A (single page)")
	case "B":
		fmt.Println("Showing checkout variant B (multi-step)")
	default:
		fmt.Println("Showing default checkout")
	}

	// --- Example 4: Gradual rollout check ---
	fmt.Println("\n=== Gradual Rollout ===")
	users := []string{"alice", "bob", "charlie", "diana"}

	for _, userID := range users {
		flags, err := client.GetIdentityFlags(userID, nil)
		if err != nil {
			fmt.Printf("%s: error - %v\n", userID, err)
			continue
		}
		hasNewFeature, _ := flags.IsFeatureEnabled("new-dashboard")
		fmt.Printf("%s: new-dashboard = %v\n", userID, hasNewFeature)
	}

	// --- Example 5: Using in HTTP handler ---
	fmt.Println("\n=== HTTP Handler Pattern ===")
	fmt.Println(`
func featureFlagMiddleware(client *flagsmith.Client) func(http.Handler) http.Handler {
    return func(next http.Handler) http.Handler {
        return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
            userID := r.Header.Get("X-User-ID")
            if userID != "" {
                flags, _ := client.GetIdentityFlags(userID, nil)
                ctx := context.WithValue(r.Context(), "flags", flags)
                r = r.WithContext(ctx)
            }
            next.ServeHTTP(w, r)
        })
    }
}`)
}

func getEnvOrDefault(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}
