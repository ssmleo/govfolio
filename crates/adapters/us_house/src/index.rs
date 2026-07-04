//! Discovery index (regime doc §2): the Clerk's `{YYYY}FD.zip` containing
//! `{YYYY}FD.xml` (UTF-8 with BOM, repeated `Member` elements). The XML is
//! authoritative; the TSV inside is a redundant rendering. Blank fields on
//! some `W` rows are tolerated — the `P` filter shields the adapter.

use std::io::Read as _;

use anyhow::Context as _;

/// One index `Member` row (only the fields discovery needs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexMember {
    pub(crate) doc_id: String,
    pub(crate) filing_type: String,
    pub(crate) year: String,
}

/// `https://…/financial-pdfs/{year}FD.zip` (regime doc §1).
pub(crate) fn index_zip_url(year: i32) -> String {
    format!("https://disclosures-clerk.house.gov/public_disc/financial-pdfs/{year}FD.zip")
}

/// `https://…/ptr-pdfs/{year}/{doc_id}.pdf` — P filings only (regime doc §2.3).
pub(crate) fn ptr_pdf_url(year: &str, doc_id: &str) -> String {
    format!("https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/{year}/{doc_id}.pdf")
}

/// Opens the index zip and parses the `*FD.xml` inside.
pub(crate) fn parse_index_zip(bytes: &[u8]) -> anyhow::Result<Vec<IndexMember>> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("opening index zip")?;
    let name = archive
        .file_names()
        .find(|name| name.ends_with("FD.xml"))
        .map(str::to_owned)
        .context("no *FD.xml member inside the index zip")?;
    let mut xml = String::new();
    archive
        .by_name(&name)
        .with_context(|| format!("opening {name} in index zip"))?
        .read_to_string(&mut xml)
        .with_context(|| format!("reading {name}"))?;
    parse_index_xml(&xml)
}

/// Parses the index XML (`FinancialDisclosure` root, repeated `Member`).
pub(crate) fn parse_index_xml(xml: &str) -> anyhow::Result<Vec<IndexMember>> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml.trim_start_matches('\u{feff}'));
    let mut members = Vec::new();
    let mut in_member = false;
    let mut field: Option<String> = None;
    let (mut doc_id, mut filing_type, mut year) = (String::new(), String::new(), String::new());
    loop {
        match reader.read_event().context("reading index XML")? {
            Event::Start(start) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                if name == "Member" {
                    in_member = true;
                    doc_id.clear();
                    filing_type.clear();
                    year.clear();
                } else if in_member {
                    field = Some(name);
                }
            }
            Event::Text(text) => {
                if let Some(name) = &field {
                    let value = String::from_utf8_lossy(text.as_ref());
                    match name.as_str() {
                        "DocID" => doc_id.push_str(value.trim()),
                        "FilingType" => filing_type.push_str(value.trim()),
                        "Year" => year.push_str(value.trim()),
                        _ => {}
                    }
                }
            }
            Event::End(end) => {
                if end.name().as_ref() == b"Member" {
                    in_member = false;
                    members.push(IndexMember {
                        doc_id: doc_id.clone(),
                        filing_type: filing_type.clone(),
                        year: year.clone(),
                    });
                } else {
                    field = None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(members)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn xml_parses_members_tolerating_bom_and_blank_w_rows() {
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
            <FinancialDisclosure>\
              <Member><Prefix>Hon.</Prefix><Last>Begich</Last><First>Nicholas</First>\
                <FilingType>P</FilingType><StateDst>AK00</StateDst><Year>2026</Year>\
                <FilingDate>6/12/2026</FilingDate><DocID>20020055</DocID></Member>\
              <Member><Last>Someone</Last><FilingType>W</FilingType><StateDst></StateDst>\
                <Year>2026</Year><FilingDate></FilingDate><DocID>8068</DocID></Member>\
            </FinancialDisclosure>";
        let members = parse_index_xml(xml).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(
            members[0],
            IndexMember {
                doc_id: "20020055".to_owned(),
                filing_type: "P".to_owned(),
                year: "2026".to_owned(),
            }
        );
        assert_eq!(members[1].filing_type, "W");
        assert_eq!(members[1].doc_id, "8068");
    }

    #[test]
    fn urls_follow_the_regime_doc_shapes() {
        assert_eq!(
            index_zip_url(2026),
            "https://disclosures-clerk.house.gov/public_disc/financial-pdfs/2026FD.zip"
        );
        assert_eq!(
            ptr_pdf_url("2026", "20020055"),
            "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20020055.pdf"
        );
    }
}
