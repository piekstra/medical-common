CARGO ?= cargo

.PHONY: verify fmt fmt-check lint test feature-check deny clean

# What CI runs. No target builds a binary: these are libraries.
verify: fmt-check lint test feature-check deny

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all --check

lint:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

test:
	$(CARGO) test --workspace --all-features

# Each crate builds and lints with and without each feature: the mock server
# never leaks into a consumer's normal build, and `rsa` stays optional.
feature-check:
	$(CARGO) clippy -p pk-med-fhir --lib -- -D warnings
	$(CARGO) clippy -p pk-med-records --lib -- -D warnings
	$(CARGO) clippy -p pk-med-fhir --lib --features test-util -- -D warnings
	$(CARGO) clippy -p pk-med-records --lib --features test-util -- -D warnings
	$(CARGO) clippy -p pk-med-smart --lib -- -D warnings
	$(CARGO) clippy -p pk-med-smart --lib --features rsa -- -D warnings

deny:
	$(CARGO) deny check

clean:
	$(CARGO) clean
