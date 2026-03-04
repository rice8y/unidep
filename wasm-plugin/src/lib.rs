use wasm_minimal_protocol::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
struct Token {
    id: String,
    form: String,
    lemma: String,
    upos: String,
    xpos: String,
    feats: String,
    head: String,
    deprel: String,
    deps: String,
    misc: String,
    is_multiword: bool,
    is_empty_node: bool,
}

#[derive(Serialize)]
struct Arc {
    start_idx: usize,
    end_idx: usize,
    dep_id: String,
    label: String,
    level: usize,
    is_head_left: bool,
    is_enhanced: bool,
}

#[derive(Serialize)]
struct Root {
    idx: usize,
    dep_id: String,
    label: String,
}

#[derive(Serialize)]
struct Sentence {
    sent_id: String,
    text: String,
    tokens: Vec<Token>,
    arcs: Vec<Arc>,
    roots: Vec<Root>,
}

initiate_protocol!();

#[wasm_func]
pub fn layout_unidep(input: &[u8]) -> Vec<u8> {
    let text = std::str::from_utf8(input).unwrap();
    let mut sentences: Vec<Sentence> = Vec::new();
    let mut current_tokens = Vec::new();
    let mut current_metadata = HashMap::new();

    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if !current_tokens.is_empty() {
                sentences.push(build_sentence(&mut current_metadata, &current_tokens));
                current_tokens.clear();
                current_metadata.clear();
            }
            continue;
        }
        if line.starts_with('#') {
            let comment = line[1..].trim();
            if let Some((k, v)) = comment.split_once('=') {
                current_metadata.insert(k.trim().to_string(), v.trim().to_string());
            }
            continue;
        }

        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 10 {
            let id = cols[0].to_string();
            current_tokens.push(Token {
                id: id.clone(),
                form: cols[1].to_string(),
                lemma: cols[2].to_string(),
                upos: cols[3].to_string(),
                xpos: cols[4].to_string(),
                feats: cols[5].to_string(),
                head: cols[6].to_string(),
                deprel: cols[7].to_string(),
                deps: cols[8].to_string(),
                misc: cols[9].to_string(),
                is_multiword: id.contains('-'),
                is_empty_node: id.contains('.'),
            });
        }
    }
    if !current_tokens.is_empty() {
        sentences.push(build_sentence(&mut current_metadata, &current_tokens));
    }
    serde_json::to_vec(&sentences).unwrap()
}

fn build_sentence(metadata: &mut HashMap<String, String>, tokens: &[Token]) -> Sentence {
    let mut id_to_idx = HashMap::new();
    let mut visual_tokens = Vec::new();

    for token in tokens {
        if !token.is_multiword {
            visual_tokens.push(token.clone());
        }
    }

    for (i, token) in visual_tokens.iter().enumerate() {
        id_to_idx.insert(token.id.clone(), i);
    }

    struct RawArc {
        start_idx: usize,
        end_idx: usize,
        dep_id: String,
        label: String,
        min_idx: usize,
        max_idx: usize,
        is_head_left: bool,
        is_enhanced: bool,
    }
    
    let mut raw_arcs = Vec::new();
    let mut roots = Vec::new();

for (end_idx, token) in visual_tokens.iter().enumerate() {
        if token.head == "0" {
            roots.push(Root { idx: end_idx, dep_id: token.id.clone(), label: token.deprel.clone() });
        }
        
        if token.deps != "_" {
            for dep in token.deps.split('|') {
                if let Some((head_id, rel)) = dep.split_once(':') {
                    if head_id == token.head && rel == token.deprel {
                        continue;
                    }

                    if head_id == "0" {
                        roots.push(Root { idx: end_idx, dep_id: token.id.clone(), label: rel.to_string() });
                    } else if let Some(&start_idx) = id_to_idx.get(head_id) {
                        let (min_idx, max_idx, is_head_left) = if start_idx < end_idx {
                            (start_idx, end_idx, true)
                        } else {
                            (end_idx, start_idx, false)
                        };
                        raw_arcs.push(RawArc {
                            start_idx: min_idx, end_idx: max_idx, dep_id: token.id.clone(),
                            label: rel.to_string(), min_idx, max_idx,
                            is_head_left, is_enhanced: true,
                        });
                    }
                }
            }
        }

        if token.head != "0" && token.head != "_" {
            if let Some(&start_idx) = id_to_idx.get(&token.head) {
                let (min_idx, max_idx, is_head_left) = if start_idx < end_idx {
                    (start_idx, end_idx, true)
                } else {
                    (end_idx, start_idx, false)
                };
                raw_arcs.push(RawArc {
                    start_idx: min_idx, end_idx: max_idx, dep_id: token.id.clone(),
                    label: token.deprel.clone(), min_idx, max_idx,
                    is_head_left, is_enhanced: false,
                });
            }
        }
    }

    raw_arcs.sort_by_key(|a| a.max_idx - a.min_idx);

    let mut arcs: Vec<Arc> = Vec::new();
    for raw in raw_arcs {
        let mut max_level = 0;
        for existing in &arcs {
            let existing_min = std::cmp::min(existing.start_idx, existing.end_idx);
            let existing_max = std::cmp::max(existing.start_idx, existing.end_idx);
            
            let is_overlap = std::cmp::max(raw.min_idx, existing_min) < std::cmp::min(raw.max_idx, existing_max);
            let is_identical_span = raw.min_idx == existing_min && raw.max_idx == existing_max;

            if is_overlap || is_identical_span {
                if existing.level > max_level {
                    max_level = existing.level;
                }
            }
        }
        
        arcs.push(Arc {
            start_idx: raw.start_idx, end_idx: raw.end_idx, dep_id: raw.dep_id,
            label: raw.label, level: max_level + 1,
            is_head_left: raw.is_head_left, is_enhanced: raw.is_enhanced,
        });
    }

    Sentence {
        sent_id: metadata.get("sent_id").cloned().unwrap_or_default(),
        text: metadata.get("text").cloned().unwrap_or_default(),
        tokens: visual_tokens, arcs, roots,
    }
}