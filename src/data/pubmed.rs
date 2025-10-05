//! PubMed ingestion utilities leveraging E-utilities.

use std::{fs::File, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use quick_xml::de::from_str;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;
use urlencoding::encode;

use crate::config::Settings;

const EUTILS_BASE: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";

/// Normalised PubMed record persisted to JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubRecord {
    pub pmid: String,
    pub title: String,
    pub abstract_text: String,
    pub journal: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i32>,
}

pub async fn search_pubmed(drug: &str, max: usize, settings: &Settings) -> Result<Vec<String>> {
    if drug.trim().is_empty() {
        return Ok(vec![]);
    }
    let client = http_client(settings)?;
    let query = format!("{drug} adverse event");
    let term = encode(query.as_str());
    let url = format!(
        "{base}/esearch.fcgi?db=pubmed&retmode=json&term={term}&retmax={max}&tool={tool}&email={email}",
        base = EUTILS_BASE,
        term = term,
        max = max,
        tool = settings.pubmed_tool,
        email = settings.pubmed_email
    );
    let resp = client.get(url).send().await?;
    let payload: ESearchResponse = resp.json().await?;
    Ok(payload.esearchresult.idlist)
}

pub async fn fetch_pubmed(pmids: &[String], settings: &Settings) -> Result<Vec<PubRecord>> {
    if pmids.is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client(settings)?;
    let mut output = Vec::new();
    for chunk in pmids.chunks(200) {
        let ids = chunk.join(",");
        let url = format!(
            "{base}/efetch.fcgi?db=pubmed&rettype=abstract&retmode=xml&id={ids}&tool={tool}&email={email}",
            base = EUTILS_BASE,
            ids = ids,
            tool = settings.pubmed_tool,
            email = settings.pubmed_email
        );
        let xml = client.get(&url).send().await?.text().await?;
        let article_set: PubmedArticleSet = from_str(&xml).unwrap_or_default();
        for article in article_set.articles {
            if let Some(record) = article.into_record() {
                output.push(record);
            }
        }
    }
    Ok(output)
}

pub fn persist_records(drug: &str, records: &[PubRecord], settings: &Settings) -> Result<PathBuf> {
    let path = settings
        .join_data("raw/pubmed")
        .join(format!("{drug}.jsonl"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(&path).with_context(|| format!("create {path:?}"))?;
    for record in records {
        let line = serde_json::to_string(record)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    info!(path = %path.display(), count = records.len(), "saved pubmed records");
    Ok(path)
}

fn http_client(settings: &Settings) -> Result<Client> {
    Ok(Client::builder()
        .user_agent(format!("rwe-assistant/0.1 (+{})", settings.pubmed_email))
        .gzip(true)
        .brotli(true)
        .build()?)
}

#[derive(Debug, Deserialize)]
struct ESearchResponse {
    #[serde(default)]
    esearchresult: ESearchResult,
}

#[derive(Debug, Deserialize, Default)]
struct ESearchResult {
    #[serde(default, rename = "idlist")]
    idlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PubmedArticleSet {
    #[serde(rename = "PubmedArticle", default)]
    articles: Vec<PubmedArticle>,
}

#[derive(Debug, Deserialize)]
struct PubmedArticle {
    #[serde(rename = "MedlineCitation")]
    citation: MedlineCitation,
}

impl PubmedArticle {
    fn into_record(self) -> Option<PubRecord> {
        let pmid = self.citation.pmid.value;
        let article = self.citation.article;
        let title = article.title.value;
        let abstract_text = article
            .abstract_section
            .as_ref()
            .map(|abs| {
                abs.text
                    .iter()
                    .filter_map(|t| t.value.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let journal = article.journal.and_then(|j| j.title.map(|t| t.value));
        let authors = article
            .author_list
            .map(|list| {
                list.authors
                    .into_iter()
                    .filter_map(|a| a.formatted())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let year = self.citation.article_date.and_then(|d| d.year());

        Some(PubRecord {
            pmid,
            title,
            abstract_text,
            journal,
            authors,
            year,
        })
    }
}

#[derive(Debug, Deserialize)]
struct MedlineCitation {
    #[serde(rename = "PMID")]
    pmid: TextNode,
    #[serde(rename = "Article")]
    article: Article,
    #[serde(rename = "ArticleDate")]
    article_date: Option<ArticleDate>,
}

#[derive(Debug, Deserialize)]
struct Article {
    #[serde(rename = "ArticleTitle")]
    title: TextNode,
    #[serde(rename = "Abstract")]
    abstract_section: Option<Abstract>,
    #[serde(rename = "Journal")]
    journal: Option<Journal>,
    #[serde(rename = "AuthorList")]
    author_list: Option<AuthorList>,
}

#[derive(Debug, Deserialize)]
struct Abstract {
    #[serde(rename = "AbstractText", default)]
    text: Vec<AbstractText>,
}

#[derive(Debug, Deserialize)]
struct AbstractText {
    #[serde(rename = "$text")] // raw text content
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Journal {
    #[serde(rename = "Title")]
    title: Option<TextNode>,
}

#[derive(Debug, Deserialize)]
struct AuthorList {
    #[serde(rename = "Author", default)]
    authors: Vec<Author>,
}

#[derive(Debug, Deserialize)]
struct Author {
    #[serde(rename = "ForeName")]
    forename: Option<TextNode>,
    #[serde(rename = "LastName")]
    lastname: Option<TextNode>,
}

impl Author {
    fn formatted(self) -> Option<String> {
        match (
            self.forename.map(|n| n.value),
            self.lastname.map(|n| n.value),
        ) {
            (Some(first), Some(last)) => Some(format!("{first} {last}")),
            (None, Some(last)) => Some(last),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArticleDate {
    #[serde(rename = "Year")]
    year: Option<TextNode>,
}

impl ArticleDate {
    fn year(self) -> Option<i32> {
        self.year.and_then(|t| t.value.parse().ok())
    }
}

#[derive(Debug, Deserialize)]
struct TextNode {
    #[serde(rename = "$text")]
    value: String,
}

impl Default for PubmedArticleSet {
    fn default() -> Self {
        Self {
            articles: Vec::new(),
        }
    }
}
