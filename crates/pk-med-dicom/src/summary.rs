//! Studies and series, summarized from the headers of their files.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::header::Header;

/// One study, from the headers of its files. It leaves out the patient's
/// name, id, birth date and sex, but its UIDs, accession number and date
/// still identify one visit: it is medical data, for the person's own
/// terminal and records folder. Not a wire shape: a CLI that prints it
/// wraps it in its own schema-tagged DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySummary {
    pub study_uid: String,
    /// `YYYY-MM-DD`.
    pub date: Option<String>,
    pub description: Option<String>,
    pub accession_number: Option<String>,
    /// Each series' modality, sorted, once each.
    pub modalities: Vec<String>,
    /// By series number, then the order first seen.
    pub series: Vec<SeriesSummary>,
    /// Distinct instances (SOP Instance UIDs) across its series.
    pub instances: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesSummary {
    /// Series Instance UID; `None` groups the files that name none.
    pub uid: Option<String>,
    pub number: Option<i64>,
    pub modality: Option<String>,
    pub description: Option<String>,
    pub body_part: Option<String>,
    pub instances: usize,
}

impl StudySummary {
    /// One summary per study, in the order its first file appears. A file
    /// with no Study Instance UID is left out, as is a second file with an
    /// instance UID already counted.
    pub fn from_headers<'a>(headers: impl IntoIterator<Item = &'a Header>) -> Vec<StudySummary> {
        let mut studies: Vec<StudySummary> = Vec::new();
        let mut study_at: HashMap<String, usize> = HashMap::new();
        let mut series_at: HashMap<(usize, Option<String>), usize> = HashMap::new();
        let mut seen: HashSet<(usize, String)> = HashSet::new();
        for header in headers {
            let Some(study_uid) = header.study_uid().map(str::to_string) else {
                continue;
            };
            let s = *study_at.entry(study_uid.clone()).or_insert_with(|| {
                studies.push(StudySummary {
                    study_uid: study_uid.clone(),
                    date: header.study_date(),
                    description: header.study_description().map(str::to_string),
                    accession_number: header.accession().map(str::to_string),
                    modalities: Vec::new(),
                    series: Vec::new(),
                    instances: 0,
                });
                studies.len() - 1
            });
            let study = &mut studies[s];
            study.date = study.date.take().or_else(|| header.study_date());
            study.description = study
                .description
                .take()
                .or_else(|| header.study_description().map(str::to_string));
            study.accession_number = study
                .accession_number
                .take()
                .or_else(|| header.accession().map(str::to_string));
            if let Some(instance) = header.sop_instance_uid().map(str::to_string) {
                if !seen.insert((s, instance)) {
                    continue;
                }
            }
            study.instances += 1;
            let series_uid = header.series_uid().map(str::to_string);
            let i = *series_at.entry((s, series_uid.clone())).or_insert_with(|| {
                study.series.push(SeriesSummary {
                    uid: series_uid,
                    number: header.series_number(),
                    modality: header.modality().map(str::to_string),
                    description: header.series_description().map(str::to_string),
                    body_part: header.body_part().map(str::to_string),
                    instances: 0,
                });
                study.series.len() - 1
            });
            study.series[i].instances += 1;
        }
        for study in &mut studies {
            let modalities: BTreeSet<String> = study
                .series
                .iter()
                .filter_map(|s| s.modality.clone())
                .collect();
            study.modalities = modalities.into_iter().collect();
            // Stable: equal numbers keep the order first seen.
            study.series.sort_by_key(|s| s.number.unwrap_or(i64::MAX));
        }
        studies
    }
}
