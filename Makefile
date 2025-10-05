CARGO ?= cargo
BIN := $(CARGO) run --

fetch:
	$(BIN) fetch --drugs imatinib,pembrolizumab --quarters 2024Q1,2024Q2

normalize:
	$(BIN) normalize

extract:
	$(BIN) extract --mode weakly_supervised

embed:
	$(BIN) embed

signal:
	$(BIN) signal

rank:
	$(BIN) rank

serve:
	$(BIN) serve --port 8080

summarize:
	$(BIN) summarize --drug imatinib --event hepatotoxicity --topk 5

test:
	$(CARGO) test

.PHONY: fetch normalize extract embed signal rank serve summarize test
