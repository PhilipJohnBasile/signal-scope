-- DuckDB initialization for rwe-assistant exploratory queries
CREATE OR REPLACE VIEW v_faers_counts AS
SELECT
    drug_id,
    event_id,
    year_quarter,
    SUM(a) AS a,
    SUM(b) AS b,
    SUM(c) AS c,
    SUM(d) AS d
FROM read_parquet('data/clean/faers_norm.parquet')
GROUP BY 1,2,3;

CREATE OR REPLACE VIEW v_relations AS
SELECT *
FROM read_parquet('data/clean/relations.parquet');

CREATE OR REPLACE VIEW v_event_clusters AS
SELECT *
FROM read_parquet('data/clean/event_clusters.parquet');

CREATE OR REPLACE VIEW v_signals AS
SELECT *
FROM read_csv_auto('outputs/signals.csv');
