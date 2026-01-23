# Test Makefile
# Shortcuts for running k6 and Kubetest tests

.PHONY: help install k6 kube infra clean

# Default target
help:
	@echo "Test Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make install       - Install k6 and chainsaw"
	@echo "  make infra-up      - Start test infrastructure"
	@echo "  make infra-down    - Stop test infrastructure"
	@echo ""
	@echo "k6 Load Tests:"
	@echo "  make k6            - Run all k6 smoke tests"
	@echo "  make k6-load       - Run k6 load tests"
	@echo "  make k6-stress     - Run k6 stress tests"
	@echo "  make k6-grafana    - Test Grafana only"
	@echo "  make k6-influxdb   - Test InfluxDB only"
	@echo "  make k6-api        - Test API only"
	@echo ""
	@echo "Kubernetes Tests:"
	@echo "  make kube          - Run all kube tests"
	@echo "  make kube-operator - Test operator"
	@echo "  make kube-deploy   - Test deployment"
	@echo "  make kube-dapr     - Test Dapr components"
	@echo "  make kube-scale    - Test scaling"
	@echo "  make kube-network  - Test network policies"
	@echo ""
	@echo "Quick:"
	@echo "  make smoke         - Quick smoke tests"
	@echo "  make full          - Full test suite"

# ============================================
# Setup
# ============================================

install:
	@echo "Installing test tools..."
	@command -v k6 >/dev/null 2>&1 || brew install k6
	@command -v chainsaw >/dev/null 2>&1 || brew install kyverno/tap/chainsaw
	@echo "Done!"

infra-up:
	@docker compose -f scripts/nu/local-dev/compose.yaml up -d
	@sleep 5
	@echo "Infrastructure ready"

infra-down:
	@docker compose -f scripts/nu/local-dev/compose.yaml down

infra-status:
	@docker compose -f scripts/nu/local-dev/compose.yaml ps

# ============================================
# k6 Tests
# ============================================

SCENARIO ?= smoke
K6_DIR = tests/k6

k6: k6-results-dir
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/grafana.test.js
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/influxdb.test.js
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/api.test.js

k6-load:
	$(MAKE) k6 SCENARIO=load

k6-stress:
	$(MAKE) k6 SCENARIO=stress

k6-spike:
	$(MAKE) k6 SCENARIO=spike

k6-grafana: k6-results-dir
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/grafana.test.js

k6-influxdb: k6-results-dir
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/influxdb.test.js

k6-api: k6-results-dir
	k6 run --env SCENARIO=$(SCENARIO) $(K6_DIR)/api.test.js

k6-full-stack: k6-results-dir
	k6 run $(K6_DIR)/full-stack.test.js

k6-results-dir:
	@mkdir -p $(K6_DIR)/results

# ============================================
# Kubernetes Tests (Chainsaw)
# ============================================

KUBETEST_DIR = tests/kubetest
CHAINSAW_OPTS ?=

kube:
	chainsaw test --config $(KUBETEST_DIR)/chainsaw-config.yaml --test-dir $(KUBETEST_DIR) $(CHAINSAW_OPTS)

kube-operator:
	chainsaw test --test-dir $(KUBETEST_DIR)/operator $(CHAINSAW_OPTS)

kube-deploy:
	chainsaw test --test-dir $(KUBETEST_DIR)/deployment $(CHAINSAW_OPTS)

kube-dapr:
	chainsaw test --test-dir $(KUBETEST_DIR)/dapr $(CHAINSAW_OPTS)

kube-scale:
	chainsaw test --test-dir $(KUBETEST_DIR)/scaling $(CHAINSAW_OPTS)

kube-network:
	chainsaw test --test-dir $(KUBETEST_DIR)/network $(CHAINSAW_OPTS)

# Keep resources after test
kube-debug:
	$(MAKE) kube CHAINSAW_OPTS="--skip-delete"

# ============================================
# Quick Commands
# ============================================

smoke: infra-up
	@sleep 5
	$(MAKE) k6 SCENARIO=smoke
	@if kubectl cluster-info >/dev/null 2>&1; then \
		$(MAKE) kube-deploy; \
	else \
		echo "Kubernetes not available, skipping kube tests"; \
	fi

full: infra-up
	@sleep 10
	$(MAKE) k6-load
	$(MAKE) k6-full-stack
	@if kubectl cluster-info >/dev/null 2>&1; then \
		$(MAKE) kube; \
	fi

# ============================================
# Cleanup
# ============================================

clean:
	rm -rf $(K6_DIR)/results
	rm -f chainsaw-report.json
