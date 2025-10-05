# rwe-assistant

**Research only – this project does not provide medical advice.** rwe-assistant ingests FDA FAERS case safety reports and PubMed literature, normalizes drug and adverse event terminology, extracts weakly Supervised drug–event relations from abstracts, computes disproportionality and trend-based safety signals, and serves ranked hypotheses over a local API plus static UI. Everything runs offline on a laptop with deterministic seeds for reproducibility.

## Install
- `rustup default stable`
- `cargo build`
- On first NLP run, `rust-bert` downloads CPU-friendly models to `~/.cache`. You can swap to ONNX by enabling the `onx` feature.
- Optional summaries: install [llama.cpp](https://github.com/ggerganov/llama.cpp) compatible GGUF (e.g., Llama 3.2 3B Instruct) and enable the `summaries` feature.

## Quick Start

### 1. Download FAERS Data
Download a quarterly FAERS archive from the [FDA website](https://fis.fda.gov/extensions/FPD-QDE-FAERS/FPD-QDE-FAERS.html):
```bash
# Example: Download 2025Q2
curl -o faers_ascii_2025q2.zip https://download-001.fda.gov/faers/FAERS_ASCII_2025Q2.zip

# Move to correct location (capital letters in filename required)
mkdir -p data/raw/faers
mv faers_ascii_2025q2.zip data/raw/faers/FAERS_ASCII_2025Q2.zip
```

### 2. Run the Pipeline
```bash
# Extract and filter FAERS data
cargo run -- fetch --quarters 2025Q2

# Normalize drug and event terms (builds 2x2 contingency tables)
# Note: This step can take 15-20 minutes for large quarters
cargo run -- normalize

# Compute statistical signals (ROR, Bayesian shrinkage, trend analysis)
cargo run -- signal

# Rank signals with literature support and generate output CSV
cargo run -- rank

# Start the web server
cargo run -- serve --port 8080
```

### 3. Explore Results
Open `http://localhost:8080` in your browser. The UI shows:
- **ROR** (Reporting Odds Ratio): How much more likely an event occurs with this drug vs. others
- **95% CI**: Confidence interval bounds (lower >1 indicates statistical signal)
- **Literature**: Number of PubMed citations supporting the drug-event association
- **Trend z**: Temporal trend strength (requires multiple quarters)
- **Score**: Combined ranking metric (higher = more significant signal)

### Advanced: Multi-Quarter Analysis with Literature
```bash
cp .env.example .env
# Edit .env and add your email for PubMed API

cargo run -- fetch --quarters 2024Q1,2024Q2,2024Q3,2024Q4
cargo run -- normalize
cargo run -- extract --mode weakly_supervised  # Extract drug-event relations from PubMed
cargo run -- embed  # Cluster similar adverse events
cargo run -- signal
cargo run -- rank
cargo run -- serve --port 8080
```

## Data Dictionary
- `data/clean/drugs.parquet`: canonical drug ids and names.
- `data/clean/events.parquet`: canonical adverse event ids and representative term.
- `data/clean/faers_norm.parquet`: 2x2 contingency table columns (`drug_id, event_id, year_quarter, a, b, c, d`).
- `data/clean/relations.parquet`: literature-derived relation confidences per sentence.
- `data/clean/event_clusters.parquet`: embedding-based clusters with representative term.
- `outputs/signals.csv`: scored signal hypotheses ready for review.

## Make Targets
```
make fetch
make normalize
make extract
make embed
make signal
make rank
make serve
make test
```

## Known Limits
- MedDRA is licensed; we rely on open proxies like RxNorm and SIDER.
- Weak supervision for relation extraction is heuristic and favors precision.
- Signals are hypothesis generating only and must be validated by specialists.
