# LinkedIn Post: Signal-Scope

🔬 **Open-Source Drug Safety Signal Detection: From 21M FDA Reports to Actionable Insights**

I'm excited to share a new pharmacovigilance tool that processes FDA's FAERS (Adverse Event Reporting System) data entirely offline on your laptop.

**The Challenge:**
Drug safety teams wade through millions of adverse event reports to identify potential safety signals. Traditional approaches require expensive commercial tools and cloud infrastructure.

**The Solution: signal-scope**
A Rust-based pipeline that:

✅ **Ingests raw FAERS data** (21M+ reports per quarter)
✅ **Normalizes drug/event terminology** using RxNorm and MedDRA proxies
✅ **Computes disproportionality metrics** (ROR with Bayesian shrinkage)
✅ **Detects temporal trends** across quarters
✅ **Enriches with PubMed literature** via weak supervision NER
✅ **Ranks signals** for clinical review

**Real Example from 2025Q2 Data:**
- Drug D7925 + Event E6128: ROR = 298.88 (CI: 218-409)
- This means the adverse event is reported ~299x more often with this drug vs. baseline
- Signal score: 130.53 (top-ranked finding requiring investigation)

**Why This Matters:**
- 🚀 **Reproducible**: Deterministic results, version-controlled
- 💻 **Privacy-first**: All processing happens locally
- ⚡ **Fast**: Full pipeline in ~20 minutes
- 🔓 **Open**: MIT licensed, no vendor lock-in

**Tech Stack:**
Rust | Polars | Axum | fastembed | Statistical pharmacovigilance methods

The code handles the entire workflow: from downloading FDA archives to serving an interactive web UI. Perfect for academic research, regulatory science, or building your own safety monitoring stack.

⚠️ **Important**: This is a hypothesis-generating tool only. All signals require clinical validation and specialist review. Not for medical decision-making.

Repo: [link to GitHub]

What pharmacovigilance challenges are you tackling? Would love to hear how teams are modernizing drug safety analytics.

#Pharmacovigilance #DrugSafety #RustLang #DataScience #RegulatoryScience #OpenSource #FDA #FAERS #MachineLearning

---

**Technical Deep Dive (Optional Comment):**

The statistical approach combines:
1. **Reporting Odds Ratio (ROR)**: Measures drug-event association strength
2. **Bayesian Empirical Shrinkage**: Reduces false positives from sparse data
3. **Temporal Trend Analysis**: Detects emerging safety patterns
4. **Literature Support Scoring**: Cross-validates with published evidence

Pipeline processes:
- 2x2 contingency tables for each drug-event-quarter combination
- Log-normal prior estimation across all signals
- Cosine similarity clustering for event deduplication
- Multi-factor ranking: statistical significance + literature + trend

All methods follow established pharmacovigilance best practices (FDA guidance, EMA guidelines).
