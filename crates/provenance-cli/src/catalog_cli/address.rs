use provenance_store::operations::catalog::{self, Definition, HttpMethod};
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) struct Resolved {
    pub definition: &'static Definition,
    pub path: String,
    pub flags: Vec<String>,
    pub query: Option<&'static str>,
}

#[derive(Clone)]
enum Segment {
    Literal(&'static str),
    Parameter(&'static str),
}

struct Address {
    collection: &'static str,
    words: Vec<Segment>,
    definition: &'static Definition,
    query: Option<&'static str>,
}

pub(super) fn resolve(collection: &str, words: &[String]) -> anyhow::Result<Resolved> {
    let address_len = words
        .iter()
        .position(|word| word.starts_with("--"))
        .unwrap_or(words.len());
    let supplied = &words[..address_len];
    let candidates = addresses()
        .iter()
        .filter(|address| address.collection == collection)
        .filter(|address| address.words.len() == supplied.len())
        .collect::<Vec<_>>();
    let address = select_address(candidates, supplied).ok_or_else(|| {
        anyhow::anyhow!(
            "the catalog does not declare the {collection} command: {}",
            supplied.join(" ")
        )
    })?;
    let values = address_values(address, supplied).expect("selected address matches");
    Ok(Resolved {
        definition: address.definition,
        path: render_path(address.definition.path, &values)?,
        flags: words[address_len..].to_vec(),
        query: address.query,
    })
}

fn addresses() -> &'static [Address] {
    static ADDRESSES: OnceLock<Vec<Address>> = OnceLock::new();
    ADDRESSES.get_or_init(build)
}

fn build() -> Vec<Address> {
    let definitions = catalog::definitions();
    let mut addresses = Vec::new();
    for definition in definitions {
        let collection = definition.path.split('/').nth(1).expect("route collection");
        let route = route_segments(definition.path);
        let same_path_read = definitions
            .iter()
            .any(|other| other.path == definition.path && other.method == HttpMethod::Get);
        match definition.method {
            HttpMethod::Get if route.is_empty() => {
                push(
                    &mut addresses,
                    collection,
                    vec![literal("list")],
                    definition,
                    None,
                );
            }
            HttpMethod::Post if route.is_empty() => {
                push(
                    &mut addresses,
                    collection,
                    vec![literal("create")],
                    definition,
                    None,
                );
            }
            HttpMethod::Get => {
                push(&mut addresses, collection, route.clone(), definition, None);
                let mut suffixed = route.clone();
                suffixed.push(literal("get"));
                push(&mut addresses, collection, suffixed, definition, None);
                if route.len() == 1 && matches!(route[0], Segment::Parameter(_)) {
                    let mut prefixed = vec![literal("get")];
                    prefixed.extend(route.clone());
                    push(&mut addresses, collection, prefixed, definition, None);
                }
            }
            HttpMethod::Patch => {
                let mut suffixed = route.clone();
                suffixed.push(literal("update"));
                push(&mut addresses, collection, suffixed, definition, None);
                if route.len() == 1 && matches!(route[0], Segment::Parameter(_)) {
                    let mut prefixed = vec![literal("update")];
                    prefixed.extend(route.clone());
                    push(&mut addresses, collection, prefixed, definition, None);
                }
            }
            HttpMethod::Post => {
                let mut form = route.clone();
                if same_path_read {
                    form.push(literal("create"));
                }
                push(&mut addresses, collection, form, definition, None);
            }
        }
        for query in &definition.registration.queries {
            let mut form = route.clone();
            form.push(literal(query.name));
            push(
                &mut addresses,
                collection,
                form,
                definition,
                Some(query.name),
            );
        }
    }
    reject_ambiguous(&addresses);
    addresses
}

fn push(
    addresses: &mut Vec<Address>,
    collection: &'static str,
    words: Vec<Segment>,
    definition: &'static Definition,
    query: Option<&'static str>,
) {
    addresses.push(Address {
        collection,
        words,
        definition,
        query,
    });
}

fn reject_ambiguous(addresses: &[Address]) {
    let mut seen = BTreeMap::new();
    for address in addresses {
        let key = (
            address.collection,
            address
                .words
                .iter()
                .map(|segment| match segment {
                    Segment::Literal(word) => *word,
                    Segment::Parameter(_) => "{}",
                })
                .collect::<Vec<_>>(),
        );
        if let Some(previous) = seen.insert(key, address.definition.name) {
            panic!(
                "ambiguous CLI registrations: {previous} and {}",
                address.definition.name
            );
        }
    }
}

fn route_segments(path: &'static str) -> Vec<Segment> {
    path.split('/')
        .skip(2)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.strip_prefix('{')
                .and_then(|part| part.strip_suffix('}'))
                .map_or(Segment::Literal(part), Segment::Parameter)
        })
        .collect()
}

const fn literal(word: &'static str) -> Segment {
    Segment::Literal(word)
}

fn select_address<'a>(
    mut candidates: Vec<&'a Address>,
    supplied: &[String],
) -> Option<&'a Address> {
    for (index, actual) in supplied.iter().enumerate() {
        let literal = candidates.iter().any(
            |address| matches!(address.words[index], Segment::Literal(word) if word == actual),
        );
        candidates.retain(|address| match address.words[index] {
            Segment::Literal(word) => word == actual,
            Segment::Parameter(_) => !literal && !actual.is_empty(),
        });
    }
    (candidates.len() == 1).then(|| candidates[0])
}

fn address_values(
    address: &Address,
    supplied: &[String],
) -> Option<BTreeMap<&'static str, String>> {
    let mut values = BTreeMap::new();
    for (expected, actual) in address.words.iter().zip(supplied) {
        match expected {
            Segment::Literal(word) if *word != actual => return None,
            Segment::Literal(_) => {}
            Segment::Parameter(name) if actual.is_empty() => return None,
            Segment::Parameter(name) => {
                values.insert(*name, actual.clone());
            }
        }
    }
    Some(values)
}

fn render_path(pattern: &str, values: &BTreeMap<&'static str, String>) -> anyhow::Result<String> {
    let mut rendered = String::new();
    for part in pattern.split('/').filter(|part| !part.is_empty()) {
        rendered.push('/');
        if let Some(name) = part
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
        {
            rendered.push_str(
                values
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("missing registered path parameter {name}"))?,
            );
        } else {
            rendered.push_str(part);
        }
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "ambiguous CLI registrations")]
    fn construction_rejects_duplicate_address_grammars() {
        let definition = &catalog::definitions()[0];
        reject_ambiguous(&[
            Address {
                collection: "sources",
                words: vec![literal("list")],
                definition,
                query: None,
            },
            Address {
                collection: "sources",
                words: vec![literal("list")],
                definition,
                query: None,
            },
        ]);
    }
}
